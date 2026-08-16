use crate::rule::Rule;
use crate::{Arg, Fact, parser};
use anyhow::{Result, anyhow, bail};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// A command handler: a closure that receives the engine and the command's
/// argument list (the first element of the original fact, i.e. the command
/// name, is NOT included).
///
/// Handlers are `Fn` (not `FnMut`) so they can be safely re-entered: a
/// handler that calls back into `dispatch_command` for the same name will
/// find itself still in the map and can be cloned again.
pub type CommandHandler = Arc<dyn Fn(&mut Engine, &[Arg]) -> Result<()>>;

/// A destination for engine text output. Each sink receives exactly the
/// characters to emit — callers add their own newline — so a sink can
/// distinguish `println` (line + `\n`) from `print` (no newline).
///
/// The default routes to the process stdout/stderr. WASM replaces these with
/// callbacks into JS so commands like `println`, `find`, and `facts`, plus
/// trace events, can be rendered into a virtual terminal.
#[derive(Clone)]
pub struct Output {
    pub stdout: Arc<dyn Fn(&str)>,
    pub stderr: Arc<dyn Fn(&str)>,
}

impl Default for Output {
    fn default() -> Self {
        Self {
            stdout: Arc::new(|s| print!("{s}")),
            stderr: Arc::new(|s| eprint!("{s}")),
        }
    }
}

/// The Reform rule engine: a fact store plus the registered rules that fire
/// against it each turn.
pub struct Engine {
    facts: Vec<Fact>,
    rules: Vec<Rule>,
    quit: bool,
    changed: bool,
    /// Directory that `$ load` relative paths resolve against.
    /// `None` means resolve against the process current working directory.
    base_dir: Option<PathBuf>,
    /// When true, emit trace events to stderr: `+`/`-` for facts added or
    /// removed, `rule` for rules registered, and `fire <name>` when a rule
    /// matches and fires. Enabled via `set_trace(true)` (CLI `--trace` or
    /// `REFORM_TRACE=1`).
    trace: bool,
    /// Tracks which (rule, matched-fact-set) pairs have already fired in the
    /// current `turn()` call, to prevent re-firing on the same facts.
    fired: Vec<Vec<std::collections::HashSet<Fact>>>,
    /// Maximum iterations per `turn()` call before bailing with a fixpoint
    /// error. Exposed for testing; the default (100_000) is a safety net.
    max_iterations: usize,
    /// Custom command handlers, keyed by command name (e.g. `"println"`,
    /// `"load"`, `"-"`). All commands are registered handlers — there are no
    /// special-cased built-ins.
    commands: HashMap<String, CommandHandler>,
    /// Where stdout-style output (print/println/find/facts) and trace events
    /// go. Defaults to the process stdout/stderr; WASM swaps in JS callbacks.
    output: Output,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("facts", &self.facts)
            .field("rules", &self.rules)
            .field("quit", &self.quit)
            .field("changed", &self.changed)
            .field("base_dir", &self.base_dir)
            .field("trace", &self.trace)
            .field("fired", &self.fired)
            .field("max_iterations", &self.max_iterations)
            .field("commands", &self.commands.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for Engine {
    fn default() -> Self {
        let mut engine = Self {
            facts: Vec::new(),
            rules: Vec::new(),
            quit: false,
            changed: false,
            base_dir: None,
            trace: false,
            fired: Vec::new(),
            max_iterations: 100_000,
            commands: HashMap::new(),
            output: Output::default(),
        };
        engine.register_default_commands();
        engine
    }
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn quit(&self) -> bool {
        self.quit
    }

    pub fn set_trace(&mut self, on: bool) {
        self.trace = on;
    }

    /// Replace the stdout/stderr sinks. Callbacks receive the exact characters
    /// to emit (callers append their own newline). Defaults to the process
    /// stdout/stderr.
    pub fn set_output(&mut self, output: Output) {
        self.output = output;
    }

    /// The current output sinks.
    pub fn output(&self) -> &Output {
        &self.output
    }

    /// Set the maximum iterations per `turn()` call. Lower values are useful
    /// for testing the fixpoint bail-out without waiting for 100k iterations.
    pub fn set_max_iterations(&mut self, n: usize) {
        self.max_iterations = n;
    }

    pub fn clear_quit(&mut self) {
        self.quit = false;
    }

    /// The current base directory for `$ load` relative path resolution.
    /// `None` means resolve against the process current working directory.
    pub fn base_dir(&self) -> Option<&Path> {
        self.base_dir.as_deref()
    }

    /// Set the base directory for `$ load` relative path resolution.
    pub fn set_base_dir(&mut self, dir: Option<PathBuf>) {
        self.base_dir = dir;
    }

    /// Register a custom command handler. The handler receives the engine and
    /// the command's argument list (everything after the command name). If a
    /// handler for `name` already exists it is replaced.
    pub fn register_command(&mut self, name: &str, handler: CommandHandler) {
        self.commands.insert(name.to_string(), handler);
    }

    /// Remove a previously registered command handler.
    pub fn remove_command(&mut self, name: &str) {
        self.commands.remove(name);
    }

    fn register_default_commands(&mut self) {
        // - (remove)
        self.register_command(
            "-",
            Arc::new(|engine, args| {
                if args.is_empty() {
                    // `$ -` with no args removes nothing (no-op).
                    return Ok(());
                }
                let pattern_str = args
                    .iter()
                    .map(crate::normal_form_arg)
                    .collect::<Vec<_>>()
                    .join(" ");
                if let Ok(pf) = parser::pattern_fact(&pattern_str) {
                    let matching: Vec<Fact> = engine
                        .facts
                        .iter()
                        .filter(|f| pf.matches_fact(f).is_some())
                        .cloned()
                        .collect();
                    for f in matching {
                        engine.remove_fact(&f);
                    }
                } else {
                    let facts = parser::facts(&pattern_str)
                        .expect("fact parser succeeds on input from the fact parser");
                    for f in facts {
                        engine.remove_fact(&f);
                    }
                }
                Ok(())
            }),
        );
        // load
        self.register_command(
            "load",
            Arc::new(|engine, args| {
                let raw = args.first().map(|a| &**a).unwrap_or("");
                let path = match &engine.base_dir {
                    Some(dir) => dir.join(raw),
                    None => PathBuf::from(raw),
                };
                let src = std::fs::read_to_string(&path)
                    .map_err(|e| anyhow!("load {}: {e}", path.display()))?;
                let prev = engine.base_dir.take();
                engine.base_dir = path.parent().map(|p| p.to_path_buf());
                // Safe to use load_str_inner here: since handlers are Fn (not
                // FnMut), dispatch_command clones the Arc without removing it
                // from the map, so nested `$ load` directives can re-enter
                // dispatch_command and find the handler still present.
                let result = engine.load_str_inner(&src);
                engine.base_dir = prev;
                result
            }),
        );
        // println
        self.register_command(
            "println",
            Arc::new(|engine, args| {
                let s: String = args.iter().map(|a| &**a).collect();
                let line = format!("{s}\n");
                (engine.output.stdout)(&line);
                Ok(())
            }),
        );
        // print
        self.register_command(
            "print",
            Arc::new(|engine, args| {
                let s: String = args.iter().map(|a| &**a).collect();
                (engine.output.stdout)(&s);
                Ok(())
            }),
        );
        // quit
        self.register_command(
            "quit",
            Arc::new(|engine, _args| {
                engine.quit = true;
                Ok(())
            }),
        );
        // panic
        self.register_command(
            "panic",
            Arc::new(|_engine, args| {
                let s: String = args.iter().map(|a| &**a).collect::<Vec<_>>().join(" ");
                Err(anyhow!("panic: {s}"))
            }),
        );
        // assert
        self.register_command(
            "assert",
            Arc::new(|engine, args| {
                let target = Fact(args.into());
                if engine.contains(&target) {
                    Ok(())
                } else {
                    Err(anyhow!("assert failed: fact {:?} not in engine", target))
                }
            }),
        );
        // assert-not
        self.register_command(
            "assert-not",
            Arc::new(|engine, args| {
                let target = Fact(args.into());
                if !engine.contains(&target) {
                    Ok(())
                } else {
                    Err(anyhow!("assert-not failed: fact {:?} is in engine", target))
                }
            }),
        );
        // find
        self.register_command(
            "find",
            Arc::new(|engine, args| {
                let pattern_str = if args.len() == 1 {
                    args[0].to_string()
                } else {
                    args.iter().map(|a| &**a).collect::<Vec<_>>().join(" ")
                };
                let pat = parser::pattern(&pattern_str)?;
                for f in engine.find_matching_facts(&pat)? {
                    let line = normal_form_fact(&f);
                    (engine.output.stdout)(&format!("{line}\n"));
                }
                Ok(())
            }),
        );
        // facts
        self.register_command(
            "facts",
            Arc::new(|engine, _args| {
                for f in &engine.facts {
                    let line = normal_form_fact(f);
                    (engine.output.stdout)(&format!("{line}\n"));
                }
                Ok(())
            }),
        );
    }

    pub fn add_fact(&mut self, fact: Fact) -> bool {
        let fact = self.reduce_evals(fact);
        if self.facts.contains(&fact) {
            false
        } else {
            if self.trace {
                (self.output.stderr)(&format!("\x1b[2m[trace] + {}\x1b[0m\n", normal_form_fact(&fact)));
            }
            self.facts.push(fact);
            self.changed = true;
            true
        }
    }

    /// Reduce `@eval` arguments in a fact before it is stored, substituting
    /// the result of evaluating the single following argument as an f64
    /// arithmetic expression (via `meval`). This happens with the highest
    /// priority — immediately when a fact is created, before rules ever see
    /// it — so math is reduced as soon as it appears.
    ///
    /// An `@eval` only interprets the single argument that directly follows
    /// it. Any `@eval` that isn't followed by an argument, whose expression
    /// fails to parse, contains variables (we don't support variable
    /// bindings), or fails to evaluate, is left untouched and the fact
    /// proceeds unchanged.
    pub fn reduce_evals(&self, fact: Fact) -> Fact {
        let mut out: Vec<Arg> = Vec::with_capacity(fact.len());
        let mut i = 0;
        while i < fact.len() {
            if &*fact[i] == "@eval"
                && let Some(expr) = fact.get(i + 1)
                && let Ok(value) = meval::eval_str(&**expr)
            {
                let s = format!("{value}");
                out.push(Arg::from(s.as_str()));
                i += 2;
                continue;
            }
            out.push(fact[i]);
            i += 1;
        }
        Fact(out)
    }

    pub fn remove_fact(&mut self, fact: &Fact) -> bool {
        let before = self.facts.len();
        self.facts.retain(|f| f != fact);
        let removed = self.facts.len() != before;
        if removed {
            if self.trace {
                (self.output.stderr)(&format!("\x1b[2m[trace] - {}\x1b[0m\n", normal_form_fact(fact)));
            }
            self.changed = true;
            // If the removed fact is a rule fact, also remove the rule.
            if fact.is_rule() {
                let name = &fact[1];
                self.rules.retain(|r| r.name != *name);
            }
        }
        removed
    }

    pub fn add_rule(&mut self, rule: Rule) {
        if self.trace {
            (self.output.stderr)(&format!(
                "\x1b[2m[trace] rule {} (specificity {})\x1b[0m\n",
                rule.name, rule.specificity
            ));
        }
        self.rules.push(rule);
        // Sort by specificity descending so more specific rules fire first.
        // When specificity is equal, insertion order is preserved (stable sort).
        self.rules.sort_by_key(|b| std::cmp::Reverse(b.specificity));
    }

    pub fn contains(&self, fact: &Fact) -> bool {
        self.facts.contains(fact)
    }

    // -- loading -----------------------------------------------------------

    pub fn load_str(&mut self, src: &str) -> Result<()> {
        self.load_str_inner(src)
    }

    /// Load facts from a file, setting `base_dir` to the file's parent
    /// directory so that `$ load` directives inside the file resolve
    /// relative to the file's location.
    pub fn load_file(&mut self, path: &Path) -> Result<()> {
        let src =
            std::fs::read_to_string(path).map_err(|e| anyhow!("load {}: {e}", path.display()))?;
        let prev = self.base_dir.take();
        self.base_dir = path.parent().map(|p| p.to_path_buf());
        let result = self.load_str_inner(&src);
        self.base_dir = prev;
        result
    }

    fn load_str_inner(&mut self, src: &str) -> Result<()> {
        for fact in parser::facts(src)? {
            self.ingest_file(fact)?;
            if self.quit {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Dispatch a command by name. Returns `true` if a handler was found and
    /// executed, `false` if no handler is registered for this name.
    ///
    /// Since handlers are `Fn` (not `FnMut`), we clone the `Arc` without
    /// removing it from the map. This means a handler can safely re-enter
    /// `dispatch_command` for the same name — the handler is still present
    /// and can be cloned again.
    pub fn dispatch_command(&mut self, name: &str, args: &[Arg]) -> Result<bool> {
        let handler = match self.commands.get(name) {
            Some(h) => h.clone(),
            None => return Ok(false),
        };
        (handler)(self, args)?;
        Ok(true)
    }

    pub fn ingest_file(&mut self, fact: Fact) -> Result<()> {
        let args: Vec<Arg> = fact.iter().cloned().collect();
        if args.is_empty() {
            return Ok(());
        }
        let stored = match &*args[0] {
            "$" => Fact(args[1..].into()),
            ">" => Fact(
                std::iter::once(Arg::from("prompt"))
                    .chain(args[1..].iter().cloned())
                    .collect(),
            ),
            _ => Fact(
                std::iter::once(Arg::from("parse"))
                    .chain(args.iter().cloned())
                    .collect(),
            ),
        };
        let is_rule = stored.is_rule();
        // Build strs from args (not stored) to avoid a borrow conflict when
        // moving stored into add_fact below.
        let strs: Vec<&str> = match &*args[0] {
            "$" => args[1..].iter().map(|a| &**a).collect(),
            ">" => std::iter::once("prompt")
                .chain(args[1..].iter().map(|a| &**a))
                .collect(),
            _ => std::iter::once("parse")
                .chain(args.iter().map(|a| &**a))
                .collect(),
        };
        if is_rule {
            self.add_rule(Rule::parse(&strs)?);
        }
        // Commands aren't stored as facts — execute them after settle so
        // rules fire first (e.g. `assert` needs rules to have run). Non-
        // commands are stored, then settle fires rules on the new fact. Both
        // paths settle exactly once, matching the original structure.
        let cmd_name = match strs.first() {
            Some(&name) if self.commands.contains_key(name) => Some(name),
            _ => None,
        };
        if cmd_name.is_none() {
            self.add_fact(stored);
        }
        self.settle()?;
        if let Some(name) = cmd_name {
            let cmd_args: Vec<Arg> = strs[1..].iter().map(|s| Arg::from(*s)).collect();
            self.dispatch_command(name, &cmd_args)?;
        }
        Ok(())
    }

    pub fn ingest_body(&mut self, fact: Fact) -> Result<()> {
        let args: Vec<Arg> = fact.iter().cloned().collect();
        if args.is_empty() {
            return Ok(());
        }
        let stripped = if &*args[0] == "$" {
            Fact(args[1..].into())
        } else {
            fact
        };
        let is_rule = stripped.is_rule();
        // Build strs from args (not stripped) to avoid a borrow conflict.
        let strs: Vec<&str> = if &*args[0] == "$" {
            args[1..].iter().map(|a| &**a).collect()
        } else {
            args.iter().map(|a| &**a).collect()
        };
        if is_rule {
            self.add_rule(Rule::parse(&strs)?);
        }
        // Check if this is a registered command. If so, don't store the fact
        // as data — execute the command immediately (no settle needed since
        // we're already inside a turn).
        let cmd_name = match strs.first() {
            Some(&name) if self.commands.contains_key(name) => Some(name),
            _ => None,
        };
        if let Some(name) = cmd_name {
            let cmd_args: Vec<Arg> = strs[1..].iter().map(|s| Arg::from(*s)).collect();
            self.dispatch_command(name, &cmd_args)?;
        } else {
            self.add_fact(stripped);
        }
        Ok(())
    }

    // -- turns -------------------------------------------------------------

    pub fn run(&mut self) -> Result<()> {
        self.settle()
    }

    fn settle(&mut self) -> Result<()> {
        // `turn()` loops internally until no rule changes the facts, so a
        // single call reaches the fixpoint. Infinite recursion (a rule whose
        // output re-matches itself forever) is bounded by `turn()`'s
        // iteration cap.
        if self.quit {
            return Ok(());
        }
        self.turn()
    }

    pub fn turn(&mut self) -> Result<()> {
        let rules = self.rules.clone();
        let mut any_changed = false;
        // Reset fired tracking for this turn.
        self.fired = vec![Vec::new(); rules.len()];
        let mut i = 0;
        let mut iterations = 0;
        while i < rules.len() {
            iterations += 1;
            if iterations > self.max_iterations {
                bail!(
                    "engine did not reach a fixpoint within {} iterations",
                    self.max_iterations
                );
            }
            let rule = &rules[i];
            // Snapshot facts per-rule so that removals by a more specific rule
            // prevent less specific rules from matching the same facts.
            let snapshot = self.facts.clone();
            self.changed = false;
            for (bindings, _matched_indices) in rule.find_matches_detailed(&snapshot) {
                // Check if this rule has already fired on this exact set of
                // matched facts. If so, skip to prevent re-firing on the same
                // facts (which causes infinite loops when a rule doesn't
                // remove its matched facts).
                let matched = rule.matched_facts(&snapshot, &bindings);
                let matched_set: std::collections::HashSet<Fact> = matched.into_iter().collect();
                if self.fired[i].contains(&matched_set) {
                    continue;
                }
                self.fired[i].push(matched_set);
                for rf in rule.removed_facts(&snapshot, &bindings) {
                    self.remove_fact(&rf);
                }
                let text = rule.body.render(&bindings);
                if self.trace {
                    let rendered = text.trim_end();
                    (self.output.stderr)(&format!("\x1b[2m[trace] fire {} -> {}\x1b[0m\n", rule.name, rendered));
                }
                if text.trim().is_empty() {
                    continue;
                }
                for f in parser::facts(&text)? {
                    self.ingest_body(f)?;
                    if self.quit {
                        self.changed = any_changed;
                        return Ok(());
                    }
                }
            }
            if self.changed {
                any_changed = true;
                // Restart from the most-specific rule so higher-specificity
                // rules get first dibs on the changed facts. A rule is NOT
                // marked fired: it may fire again on its own output, which is
                // what makes recursive rules (e.g. peeling one item per firing)
                // work within a single turn.
                i = 0;
            } else {
                i += 1;
            }
        }
        self.changed = any_changed;
        Ok(())
    }

    /// Facts in the engine that match the given (single-fact-line) pattern.
    pub fn find_matching_facts(&self, pat: &crate::rule::Pattern) -> Result<Vec<Fact>> {
        if pat.len() != 1 {
            bail!("find only supports single-fact patterns");
        }
        let Some(crate::rule::PatternItem::Fact(pf)) = pat.first() else {
            bail!("find only supports single-fact patterns");
        };
        Ok(self
            .facts
            .iter()
            .filter(|f| pf.matches_fact(f).is_some())
            .cloned()
            .collect())
    }
}

/// Render a fact as a single normal-form line: args space-separated, each
/// wrapped in parens if it needs it.
pub fn normal_form_fact(f: &Fact) -> String {
    let parts: Vec<String> = f.iter().map(crate::normal_form_arg).collect();
    parts.join(" ")
}
