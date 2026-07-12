use std::io::{self, BufRead};

use crate::engine::Engine;
use crate::fact::parse_fact_from_str;

/// Run a simple REPL that reads lines and processes them.
/// `show_help` controls whether the command list is printed on startup.
pub fn run_repl(engine: &mut Engine) -> anyhow::Result<()> {
    run_repl_with_help(engine, true)
}

/// Run the REPL without printing the command list.
pub fn run_repl_quiet(engine: &mut Engine) -> anyhow::Result<()> {
    run_repl_with_help(engine, false)
}

fn run_repl_with_help(engine: &mut Engine, show_help: bool) -> anyhow::Result<()> {
    let stdin = io::stdin();
    println!("Reform Engine REPL");
    if show_help {
        println!("Commands:");
        println!("  pred(arg1, arg2)    - assert a fact");
        println!("  rule name: pat -> eff - add a rule");
        println!("  run                 - run fixed-point loop");
        println!("  facts               - dump all facts");
        println!("  rules               - dump all rules");
        println!("  load <file>         - load and execute a script file");
        println!("  checkpoint          - save state checkpoint");
        println!("  restore             - restore to last checkpoint");
        println!("  step                - run one iteration of fixed-point");
        println!("  assert pred(arg1,..) - crash if fact doesn't exist");
        println!("  assert not pred(..)  - crash if fact exists");
        println!("  quit                - exit");
        println!();
    }

    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim().to_string();

        if line.is_empty() {
            continue;
        }

        match line.as_str() {
            "quit" | "exit" => break,
            "run" => {
                let firings = engine.run_fixedpoint();
                println!("Fixed point reached. {} rule firings.", firings);
            }
            "step" => {
                let firings = engine.run_fixedpoint();
                println!("Stepped. {} rule firings.", firings);
            }
            "facts" => {
                println!("Facts ({}):", engine.facts().len());
                engine.dump_facts();
            }
            "rules" => {
                engine.rebuild_rules();
                println!("Rules ({}):", engine.rules().len());
                engine.dump_rules();
            }
            "checkpoint" => {
                engine.save_checkpoint();
                println!("Checkpoint saved ({} total).", engine.checkpoint_count());
            }
            "restore" => {
                if engine.restore_checkpoint() {
                    println!("Restored to previous checkpoint.");
                } else {
                    println!("No checkpoint to restore.");
                }
            }
            "clear" => {
                println!("Clearing not supported in REPL. Create a new session.");
            }
            _ => {
                if line.starts_with("load ") {
                    let path = line[5..].trim();
                    if let Err(e) = load_script(engine, path) {
                        println!("Error loading '{}': {}", path, e);
                    } else {
                        println!("Loaded '{}'.", path);
                    }
                } else if line.starts_with("rule ") {
                    handle_rule(engine, &line);
                } else if line.starts_with("assert") {
                    handle_assert(engine, &line);
                } else if let Some(fact) = parse_fact_from_str(&line) {
                    engine.assert(fact);
                    println!("Fact asserted.");
                } else {
                    println!("Unknown command. Try: pred(arg1, arg2) or rule name: pat -> eff");
                }
            }
        }
    }

    Ok(())
}

/// Load and execute a script file.
pub fn load_script(engine: &mut Engine, path: &str) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)?;
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if line.starts_with("rule ") {
            handle_rule(engine, line);
        } else if line == "run" {
            let firings = engine.run_fixedpoint();
            println!("  [run: {} firings]", firings);
        } else if line == "facts" {
            println!("  [facts after run:]");
            engine.dump_facts();
        } else if line == "checkpoint" {
            engine.save_checkpoint();
            println!("  [checkpoint saved]");
        } else if line == "restore" {
            engine.restore_checkpoint();
            println!("  [restored]");
        } else if line.starts_with("assert") {
            handle_assert(engine, line);
        } else if let Some(fact) = parse_fact_from_str(line) {
            engine.assert(fact);
        } else {
            println!("  [warning: could not parse line: {}]", line);
        }
    }
    Ok(())
}

/// Handle a rule command by asserting a rule(...) fact.
///
/// Syntax:
///   rule name: match1, match2 -> effect1, effect2
///   rule name: -match1, match2 -> effect1
///
/// The `-` prefix on a match means it will be consumed.
/// The `!` prefix on a match means it must NOT exist (negation).
fn handle_rule(engine: &mut Engine, line: &str) {
    // Format: rule name: match_patterns -> effect_patterns
    let rest = line["rule".len()..].trim();

    // Split on "->" to separate matches from effects
    let arrow_pos = rest.find("->");
    let (left, right) = if let Some(pos) = arrow_pos {
        (rest[..pos].trim(), rest[pos + 2..].trim())
    } else {
        println!("Usage: rule name: match1, match2 -> effect1, effect2");
        return;
    };

    // Split left side on first ":" to get name and match patterns
    let colon_pos = left.find(':');
    let (name, match_str) = if let Some(pos) = colon_pos {
        (left[..pos].trim().to_string(), left[pos + 1..].trim())
    } else {
        println!("Usage: rule name: match1, match2 -> effect1, effect2");
        return;
    };

    if name.is_empty() || match_str.is_empty() || right.is_empty() {
        println!("Usage: rule name: match1, match2 -> effect1, effect2");
        return;
    }

    // Build the rule(...) fact: rule(name, match_patterns, effect_patterns)
    // Match and effect patterns are stored as comma-separated strings.
    let fact = vec!["rule".to_string(), name, match_str.to_string(), right.to_string()];

    engine.assert(fact);
    println!("Rule added.");
}

/// Handle an assert command.
/// Format: assert pred(arg1, arg2)  — crashes if fact does not exist
///         assert not pred(arg1, arg2) — crashes if fact exists
fn handle_assert(engine: &mut Engine, line: &str) {
    let rest = line["assert".len()..].trim();
    let (negated, fact_str) = if let Some(r) = rest.strip_prefix("not ") {
        (true, r.trim())
    } else {
        (false, rest)
    };

    if let Some(fact) = parse_fact_from_str(fact_str) {
        let exists = engine.facts().contains(&fact);
        if negated {
            if exists {
                eprintln!("Assertion failed: {} should not exist", fact_str);
                std::process::exit(1);
            }
        } else {
            if !exists {
                eprintln!("Assertion failed: {} should exist", fact_str);
                std::process::exit(1);
            }
        }
    } else {
        eprintln!("Could not parse assertion: {}", line);
        std::process::exit(1);
    }
}
