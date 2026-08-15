//! WASM bindings for the Reform rule engine.
//!
//! Exposes a [`ReformEngine`] that wraps [`reform::engine::Engine`] and routes
//! engine output (stdout-style commands like `println`/`find`/`facts`, plus
//! trace events) to JS callbacks so a virtual terminal can render it. The
//! `input` method mirrors the CLI REPL: a `$`-prefixed line is a direct fact
//! when `allow_direct` is on, otherwise it is treated as a player prompt.

use reform::engine::{Engine, Output};
use wasm_bindgen::prelude::*;

/// A Reform rule engine exposed to JavaScript.
#[wasm_bindgen]
pub struct ReformEngine {
    engine: Engine,
    /// Whether `$`-prefixed lines typed at the terminal are inserted directly
    /// as facts (CLI `-s` / `--safe` disables this). When off, they are treated
    /// as player prompts.
    allow_direct: bool,
}

#[wasm_bindgen]
impl ReformEngine {
    /// Create a new engine with default output (routed to the JS console).
    #[wasm_bindgen(constructor)]
    pub fn new() -> ReformEngine {
        let engine = Engine::new();
        let mut this = ReformEngine {
            engine,
            allow_direct: false,
        };
        // Default sinks go to the JS console so output is visible even before
        // the caller installs custom callbacks.
        this.set_output(console_log(), console_error());
        this
    }

    /// Enable or disable trace logging. Trace events are emitted to the
    /// stderr sink (default: `console.error`).
    pub fn set_trace(&mut self, on: bool) {
        self.engine.set_trace(on);
    }

    /// Whether `$`-prefixed terminal lines are inserted directly as facts
    /// (CLI `-s` / `--safe` disables this). When false they are treated as
    /// player prompts.
    pub fn set_allow_direct(&mut self, on: bool) {
        self.allow_direct = on;
    }

    /// Route engine stdout and stderr output to the given JS callbacks. Each
    /// callback receives the exact characters to emit (callers append their
    /// own newline).
    pub fn set_output(&mut self, stdout: js_sys::Function, stderr: js_sys::Function) {
        self.engine.set_output(Output {
            stdout: Arc::new(move |s| {
                let _ = stdout.call1(&JsValue::UNDEFINED, &JsValue::from_str(s));
            }),
            stderr: Arc::new(move |s| {
                let _ = stderr.call1(&JsValue::UNDEFINED, &JsValue::from_str(s));
            }),
        });
    }

    /// Load a reform source string (facts, rules, `$` commands) into the
    /// engine. Returns an error message on parse/load failure.
    pub fn load(&mut self, src: &str) -> Result<(), JsValue> {
        self.engine
            .load_str(src)
            .map_err(|e| JsValue::from_str(&format!("{e:?}")))
    }

    /// Feed one line of terminal input. A `$`-prefixed line is a direct fact
    /// when `allow_direct` is on; otherwise it is a player prompt. Returns an
    /// error message on failure.
    pub fn input(&mut self, line: &str) -> Result<(), JsValue> {
        if line.trim().is_empty() {
            // Ignore blank lines.
            return Ok(());
        }
        if self.allow_direct && line.starts_with('$') {
            // A direct `$` fact / command, submitted immediately.
            return self.engine
                .load_str(line)
                .map_err(|e| JsValue::from_str(&format!("{e:?}")));
        }
        // A prompt: player input, processed immediately.
        self.engine
            .load_str(&format!("> {line}\n"))
            .map_err(|e| JsValue::from_str(&format!("{e:?}")))
    }

    /// Whether the engine has quit (e.g. a `$ quit` command ran).
    pub fn quit(&self) -> bool {
        self.engine.quit()
    }

    /// The current facts as a JS array of strings in normal form.
    pub fn facts(&self) -> js_sys::Array {
        let arr = js_sys::Array::new();
        for f in self.engine.facts() {
            arr.push(&JsValue::from_str(&reform::engine::normal_form_fact(f)));
        }
        arr
    }
}

impl Default for ReformEngine {
    fn default() -> Self {
        Self::new()
    }
}

use std::sync::Arc;

/// A JS callback that logs to `console.log`.
fn console_log() -> js_sys::Function {
    js_sys::Function::new_with_args(
        "s",
        "console.log(s);",
    )
}

/// A JS callback that logs to `console.error`.
fn console_error() -> js_sys::Function {
    js_sys::Function::new_with_args(
        "s",
        "console.error(s);",
    )
}
