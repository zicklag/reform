//! Rendering tests for `reform::trace::TraceFormat` and the engine's trace
//! event structure: facts attributed to their rule / command / file cause,
//! the compact output shape, and the runtime enable/disable gate.

use reform::engine::Engine;
use std::sync::{Arc, Mutex};

/// Run a program through a fresh engine with the plain-text trace renderer
/// installed, returning the captured trace output.
fn traced(program: &str) -> String {
    let buf = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&buf);
    let fmt = reform::trace::TraceFormat::with_sink(Arc::new(move |s| {
        sink.lock().unwrap().push_str(s);
    }))
    .plain();
    tracing::subscriber::with_default(fmt, || {
        Engine::new().load_str(program).expect("load should succeed");
    });
    buf.lock().unwrap().clone()
}

/// Facts loaded directly from source render flush-left with no origin
/// suffix — their cause is the absence of indentation.
#[test]
fn source_facts_render_flush_left() {
    let out = traced("$ a\n$ b\n$ quit\n");
    assert_eq!(out, "+ a\n+ b\n");
}

/// A rule firing prints `fire <rule>` followed by the firing's effects,
/// indented beneath it: matches consumed by the pattern render as removals
/// (implicitly matched), body additions render as adds.
#[test]
fn rule_firing_groups_its_effects() {
    let out = traced(
        "$ rule open
    ( - door locked )
    ( door unlocked )
$ door locked
$ quit
",
    );
    let rule_at = out.find("rule open (specificity ").expect("rule event");
    let src_at = out.find("+ door locked\n").expect("source add");
    let fire_at = out.find("fire open\n").expect("fire line");
    let remove_at = out.find("  - door locked\n").expect("indented remove");
    let add_at = out.find("  + door unlocked\n").expect("indented add");
    assert!(rule_at < src_at && src_at < fire_at && fire_at < remove_at && remove_at < add_at);
    // The consumed match produces no ✓ — the removal line carries it.
    assert!(!out.contains("✓"), "trace: {out}");
}

/// Pattern facts that match but are not consumed render as `✓` lines inside
/// the firing; they are why it fired but stayed in the fact store.
#[test]
fn kept_matches_render_with_a_checkmark() {
    let out = traced(
        "$ rule check
    (
      item present
      shelf holds item
    )
    ( item checked )
$ item present
$ shelf holds item
$ quit
",
    );
    assert!(out.contains("  ✓ item present\n"), "trace: {out}");
    assert!(out.contains("  ✓ shelf holds item\n"), "trace: {out}");
    assert!(out.contains("  + item checked\n"), "trace: {out}");
    assert!(out.contains("fire check\n"), "trace: {out}");
}

/// The computed specificity appears on rule registration lines.
#[test]
fn rule_registration_shows_specificity() {
    let out = traced("$ rule r\n    ( a )\n    ( b )\n$ quit\n");
    assert!(out.contains("rule r (specificity "), "trace: {out}");
    // The rule fact itself is not echoed as an add — registration is shown
    // once, by the `rule` line.
    assert!(!out.contains("+ rule r"), "trace: {out}");
}

/// Facts removed by a command (`$ -`) are indented and name the command.
#[test]
fn command_removals_name_the_command() {
    let out = traced("$ a\n$ - a\n$ quit\n");
    assert!(out.contains("  - a (via -)\n"), "trace: {out}");
}

/// `$ load` renders a `file` line for the loaded path, and its adds are
/// attributed via the command, not the file's parse context.
#[test]
fn loaded_files_get_a_file_line() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "reform-trace-test-{}-{}.rf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, "inner fact\n").unwrap();
    let program = format!("$ load {}\n$ quit\n", path.display());
    let out = traced(&program);
    std::fs::remove_file(&path).ok();
    assert!(out.contains(&format!("file {}\n", path.display())), "trace: {out}");
    assert!(out.contains("  + parse inner fact (via load)\n"), "trace: {out}");
}

/// Parsing a file renders a `file` line for `load_file` too.
#[test]
fn load_file_gets_a_file_line() {
    let dir = std::env::temp_dir();
    let path = dir.join(format!(
        "reform-trace-loadfile-{}-{}.rf",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, "$ a\n").unwrap();
    let buf = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&buf);
    let fmt = reform::trace::TraceFormat::with_sink(Arc::new(move |s| {
        sink.lock().unwrap().push_str(s);
    }))
    .plain();
    tracing::subscriber::with_default(fmt, || {
        Engine::new().load_file(&path).expect("load file");
    });
    std::fs::remove_file(&path).ok();
    let out = buf.lock().unwrap().clone();
    assert!(out.contains(&format!("file {}\n", path.display())), "trace: {out}");
    assert!(out.contains("+ a\n"), "trace: {out}");
}

/// The renderer can be muted at runtime (used by the WASM toggle).
#[test]
fn disabling_the_renderer_mutes_it() {
    let buf = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&buf);
    let fmt = reform::trace::TraceFormat::with_sink(Arc::new(move |s| {
        sink.lock().unwrap().push_str(s);
    }))
    .plain();
    fmt.set_enabled(false);
    tracing::subscriber::with_default(fmt, || {
        Engine::new().load_str("$ a\n").unwrap();
    });
    assert_eq!(buf.lock().unwrap().as_str(), "");
}

/// Renderers use ANSI colors unless marked plain.
#[test]
fn default_rendering_is_colored() {
    let buf = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&buf);
    let plain = reform::trace::TraceFormat::with_sink(Arc::new(move |line| {
        sink.lock().unwrap().push_str(line);
    }))
    .plain();
    tracing::subscriber::with_default(plain, || {
        Engine::new().load_str("$ a\n").unwrap();
    });
    assert_eq!(buf.lock().unwrap().as_str(), "+ a\n");

    // Colored renderer: `+` is wrapped in green.
    buf.lock().unwrap().clear();
    let sink2 = Arc::clone(&buf);
    let colored = reform::trace::TraceFormat::with_sink(Arc::new(move |s| {
        sink2.lock().unwrap().push_str(s);
    }));
    tracing::subscriber::with_default(colored, || {
        Engine::new().load_str("$ a\n").unwrap();
    });
    let out = buf.lock().unwrap().clone();
    assert!(out.contains("\x1b[32m+\x1b[0m a\n"), "trace: {out:?}");
}

/// A mixed firing shows kept matches as `✓` and consumed matches as the
/// indented removal, under one fire line.
#[test]
fn firing_shows_kept_and_consumed_matches() {
    let out = traced(
        "$ rule go
    (
      exit player $room
      - at player $room
    )
    (
      walk player $room
    )
$ at player hall
$ exit player hall
$ quit
",
    );
    let fire_at = out.find("fire go\n").expect("fire line");
    let kept_at = out.find("  ✓ exit player hall\n").expect("kept match");
    let remove_at = out.find("  - at player hall\n").expect("removed match");
    let add_at = out.find("  + walk player hall\n").expect("body add");
    assert!(fire_at < kept_at && kept_at < remove_at && remove_at < add_at, "trace: {out}");
    assert!(!out.contains("✓ at player hall"), "trace: {out}");
}

/// Direct subscriber-level contract: field recording after span creation,
/// span ordering links, unknown targets, foreign callsites, and synthetic
/// span bookkeeping the engine never produces.
#[test]
fn renderer_contract_edge_cases() {
    // The stderr constructor routes through the process's own stderr.
    tracing::subscriber::with_default(reform::trace::TraceFormat::stderr(), || {
        Engine::new().load_str("$ stderr-constructor-smoke\n").unwrap();
    });
    let buf = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&buf);
    let fmt = Arc::new(
        reform::trace::TraceFormat::with_sink(Arc::new(move |s| {
            sink.lock().unwrap().push_str(s);
        }))
        .plain(),
    );

    use tracing::Subscriber as _;

    // A bogus ancestor id on the stack: events under it still render at
    // depth 0 (the missing entry is skipped when walking ancestry); exotic
    // enter/exit orders and unknown ids are tolerated.
    let bogus = tracing::Id::from_u64(999);
    tracing::subscriber::with_default(Arc::clone(&fmt), || {
        fmt.enter(&bogus);
        // Unknown reform targets render their message.
        tracing::trace!(target: "reform::other", "custom event");
        // Known targets without their fields render nothing.
        tracing::trace!(target: "reform::add", "no fact");
        tracing::trace!(target: "reform::remove", "no fact");
        tracing::trace!(target: "reform::match", "no fact");
        tracing::trace!(target: "reform::rule", name = %("r"));
        tracing::trace!(target: "reform::rule", specificity = 3);
        tracing::trace!(target: "reform::other", fact = %("x")); // no message
        // Foreign callsites are not ours — the renderer ignores them.
        tracing::trace!(target: "someone-else", "foreign");
        // Fields recorded after span creation update the stored node, and
        // span-to-span links are accepted.
        let span = tracing::trace_span!(target: "reform::fire", "fire", rule = %("late"));
        span.record("rule", "updated");
        let other = tracing::trace_span!(target: "reform::file", "file", path = %("x"));
        span.follows_from(&other);
        fmt.exit(&bogus);

        // Mid-stack and unknown-id exits are tolerated.
        let a = tracing::Id::from_u64(1);
        let b = tracing::Id::from_u64(2);
        fmt.enter(&a);
        fmt.enter(&b);
        fmt.exit(&a);
        fmt.exit(&b);
        fmt.exit(&tracing::Id::from_u64(555));
    });

    let out = buf.lock().unwrap().clone();
    assert!(out.contains("custom event\n"), "trace: {out:?}");
    assert!(out.contains("fire late\n"), "trace: {out:?}");
    assert!(!out.contains("no fact field"), "trace: {out:?}");
    assert!(!out.contains("foreign"), "trace: {out:?}");
}