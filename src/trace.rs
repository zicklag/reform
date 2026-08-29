//! Structured engine tracing built on the [`tracing`] crate.
//!
//! The engine emits one event per state change and one span per causal scope,
//! all under targets prefixed `reform::`:
//!
//! | Target | Kind  | Fields |
//! |---|---|---|
//! | `reform::match`                  | event | `fact` — matched and kept |
//! | `reform::add` / `reform::remove` | event | `fact` (normal form) |
//! | `reform::rule`                   | event | `name`, `specificity` |
//! | `reform::fire`                   | span  | `rule` |
//! | `reform::cmd`                    | span  | `name` |
//! | `reform::file`                   | span  | `path` |
//!
//! A `fire` span is opened around every rule firing, so causality falls out
//! of span nesting: a fact event emitted inside `fire{rule=open}` was added
//! or removed by rule `open`, while an event at the top level came directly
//! from parsing or the API. Any `tracing` subscriber sees this structure;
//! [`TraceFormat`] renders it as a compact, indented trace:
//!
//! ```text
//! rule open (specificity 1)
//! + door locked                <- parsed from source
//! fire open                    <- the rule's pattern matched
//!   ✓ lock indicator lit       <- matched, left in place
//!   - door locked              <- removed by the firing
//!   + door unlocked            <- added by the rule body
//! ```
//!
//! Matched facts the firing consumes don't get a `✓` — they appear as `-`
//! removals inside the firing instead, so every fact in a firing's block is
//! either why it fired (removed matches, `✓` kept matches) or what it did
//! (`-`/`+`).
//!
//! The CLI installs `TraceFormat::stderr()` for `--trace` / `REFORM_TRACE`,
//! and the WASM bindings install a variant that renders into the virtual
//! terminal. Programs embedding reform can use any subscriber they like;
//! the engine only emits events.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tracing::field::{Field, Visit};
use tracing::level_filters::LevelFilter;
use tracing::span::{Attributes, Record};
use tracing::subscriber::Interest;
use tracing::{Event, Id, Metadata, Subscriber};

/// Prefix shared by all engine trace targets. The renderer matches on it so
/// engine events are rendered and everything else is ignored.
const TARGET: &str = "reform::";

/// The renderer's own bookkeeping: spans that have been created but not yet
/// dropped, so events and child spans can find their ancestry. Shared behind
/// a mutex so a [`TraceFormat`] is `Send + Sync` and can render from any thread.
#[derive(Default)]
struct Open {
    next_id: u64,
    spans: HashMap<Id, OpenSpan>,
}

struct OpenSpan {
    target: &'static str,
    fields: Vec<(&'static str, String)>,
}

impl OpenSpan {
    fn field(&self, name: &str) -> Option<&String> {
        self.fields
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v)
    }
}

// Per-thread stack of currently entered reform spans, mirroring the span
// context `tracing` maintains. Contextual events carry no resolvable parent
// id, so the renderer tracks `enter`/`exit` itself to know ancestry.
thread_local! {
    static STACK: RefCell<Vec<Id>> = const { RefCell::new(Vec::new()) };
}
struct FieldList {
    fields: Vec<(&'static str, String)>,
}

impl Default for FieldList {
    fn default() -> Self {
        Self { fields: Vec::new() }
    }
}

impl FieldList {
    fn get(&self, name: &str) -> Option<String> {
        self.fields
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, v)| v.clone())
    }
}

impl Visit for FieldList {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields.push((field.name(), format!("{value:?}")));
    }
}

/// Renders the engine's trace events as a compact causal tree.
///
/// Output shape (indentation = one level per enclosing `fire`/`cmd` span):
///
/// ```text
/// rule open-door (specificity 2)
/// + room hallway
/// fire open-door <- room hallway, door hallway locked
///   - door hallway locked
///   + door hallway open
/// ```
///
/// Lines arrive at the sink complete with their trailing newline, matching
/// the [`crate::engine::Output`] sink convention.
pub struct TraceFormat {
    /// Dynamic gate for [`Subscriber::enabled`]. Lets a host (the WASM demo)
    /// toggle tracing at runtime without replacing the global subscriber.
    on: AtomicBool,
    colors: bool,
    sink: Arc<dyn Fn(&str) + Send + Sync>,
    open: Mutex<Open>,
}

impl TraceFormat {
    /// A renderer that writes each line to process stderr with ANSI colors.
    pub fn stderr() -> Self {
        Self::with_sink(Arc::new(|s| eprint!("{s}")))
    }

    /// A renderer delivering each complete line to `sink`.
    pub fn with_sink(sink: Arc<dyn Fn(&str) + Send + Sync>) -> Self {
        Self {
            on: AtomicBool::new(true),
            colors: true,
            sink,
            open: Mutex::new(Open::default()),
        }
    }

    /// Disable ANSI colors (for pipes and tests that assert on exact text).
    pub fn plain(mut self) -> Self {
        self.colors = false;
        self
    }

    /// Turn rendering on or off at runtime.
    pub fn set_enabled(&self, on: bool) {
        self.on.store(on, Ordering::Relaxed);
    }

    fn accepted(&self, meta: &Metadata<'_>) -> bool {
        self.on.load(Ordering::Relaxed) && meta.target().starts_with(TARGET)
    }

    fn paint(&self, ansi: &str, text: &str) -> String {
        if self.colors {
            format!("\x1b[{ansi}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }

    fn out(&self, line: String) {
        (self.sink)(&format!("{line}\n"));
    }

    /// Depth (number of enclosing `fire`/`cmd` spans) and, when there is no
    /// `fire` above, the innermost `cmd` cause — rendered as a `(via <name>)`
    /// suffix so command-driven mutations name their command.
    fn context(&self, ids: &[Id]) -> (usize, Option<String>) {
        let open = self.open.lock().unwrap();
        let mut depth = 0;
        let mut fire = false;
        let mut cmd = None;
        for id in ids {
            let Some(span) = open.spans.get(id) else {
                continue;
            };
            match span.target {
                s if s.starts_with("reform::fire") => {
                    fire = true;
                    depth += 1;
                }
                s if s.starts_with("reform::cmd") => {
                    cmd = span.field("name").cloned();
                    depth += 1;
                }
                _ => {}
            }
        }
        let via = if fire { None } else { cmd };
        (depth, via)
    }

    /// Render the one-line summary printed when a `fire` span opens.
    fn fire_line(&self, fields: &FieldList) -> String {
        let rule = fields.get("rule").unwrap_or_default();
        format!("fire {}", self.paint("35", &rule))
    }

    /// Render the one-line summary printed when a `file` span opens.
    fn file_line(&self, fields: &FieldList) -> String {
        let path = fields.get("path").unwrap_or_default();
        format!("{} {path}", self.paint("36", "file"))
    }

    /// Render a fact mutation event.
    fn event_line(&self, event: &Event<'_>, fields: &FieldList, depth: usize, via: &Option<String>) -> Option<String> {
        let indent = "  ".repeat(depth);
        let via = via
            .as_ref()
            .map(|n| format!(" {}", self.paint("2", &format!("(via {n})"))))
            .unwrap_or_default();
        let line = match event.metadata().target() {
            t if t.starts_with("reform::add") => {
                format!("{indent}{} {}{via}", self.paint("32", "+"), fields.get("fact")?)
            }
            t if t.starts_with("reform::remove") => {
                format!("{indent}{} {}{via}", self.paint("31", "-"), fields.get("fact")?)
            }
            t if t.starts_with("reform::match") => {
                // Matched and left in place; removals show as `-` instead.
                format!("{indent}{}", self.paint("2", &format!("✓ {}", fields.get("fact")?)))
            }
            t if t.starts_with("reform::rule") => {
                let name = fields.get("name")?;
                let specificity = fields.get("specificity")?;
                format!(
                    "{indent}{}",
                    self.paint("2", &format!("rule {name} (specificity {specificity})"))
                )
            }
            _ => format!("{indent}{}", fields.get("message")?),
        };
        Some(line)
    }
}

impl Subscriber for TraceFormat {
    fn enabled(&self, meta: &Metadata<'_>) -> bool {
        self.accepted(meta)
    }

    fn new_span(&self, attrs: &Attributes<'_>) -> Id {
        let target = attrs.metadata().target();
        let mut fields = FieldList::default();
        attrs.record(&mut fields);
        let id = {
            let mut open = self.open.lock().unwrap();
            open.next_id += 1;
            Id::from_u64(open.next_id)
        };
        // Snapshot the ancestor stack before this span is entered; the line
        // goes out at open time so it precedes this firing's effects.
        let stack = STACK.with(|s| s.borrow().clone());
        let (depth, _) = self.context(&stack);
        let line = match target {
            t if t.starts_with("reform::fire") => Some(self.fire_line(&fields)),
            t if t.starts_with("reform::file") => Some(self.file_line(&fields)),
            _ => None,
        };
        if let Some(line) = line {
            self.out(format!("{}{}", "  ".repeat(depth), line));
        }
        self.open.lock().unwrap().spans.insert(
            id.clone(),
            OpenSpan {
                target,
                fields: fields.fields,
            },
        );
        id
    }

    /// Span fields can also be recorded after creation; update the stored
    /// node so later events still see the current values.
    fn record(&self, id: &Id, values: &Record<'_>) {
        let mut values_fields = FieldList::default();
        values.record(&mut values_fields);
        self.open.lock().unwrap().spans.get_mut(id).map(|span| {
            span.fields.extend(std::mem::take(&mut values_fields.fields));
        });
    }

    fn record_follows_from(&self, _id: &Id, _follows: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut fields = FieldList::default();
        event.record(&mut fields);
        let stack = STACK.with(|s| s.borrow().clone());
        let (depth, via) = self.context(&stack);
        if let Some(line) = self.event_line(event, &fields, depth, &via) {
            self.out(line);
        }
    }

    fn enter(&self, id: &Id) {
        STACK.with(|s| s.borrow_mut().push(id.clone()));
    }

    fn exit(&self, id: &Id) {
        STACK.with(|s| {
            let mut stack = s.borrow_mut();
            match stack.iter().rposition(|i| i == id) {
                Some(pos) if pos == stack.len() - 1 => {
                    stack.pop();
                }
                Some(pos) => {
                    stack.remove(pos);
                }
                None => {}
            }
        });
    }

    fn try_close(&self, id: Id) -> bool {
        self.open.lock().unwrap().spans.remove(&id);
        true
    }

    /// Interesting in reform callsites only, but `sometimes` (not `always`)
    /// so `enabled` is consulted per event and the WASM wrapper can toggle
    /// tracing at runtime.
    fn register_callsite(&self, meta: &'static Metadata<'static>) -> Interest {
        if meta.target().starts_with(TARGET) {
            Interest::sometimes()
        } else {
            Interest::never()
        }
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        if self.on.load(Ordering::Relaxed) {
            Some(LevelFilter::TRACE)
        } else {
            Some(LevelFilter::OFF)
        }
    }
}