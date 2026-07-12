use std::io::{self, BufRead};

use crate::engine::Engine;
use crate::fact::{Fact, split_top_level};

/// Run a simple REPL that reads lines and processes them.
pub fn run_repl(engine: &mut Engine) -> anyhow::Result<()> {
    let stdin = io::stdin();
    println!("Reform Engine REPL");
    println!("Commands:");
    println!("  pred(arg1, arg2)    - assert a fact");
    println!("  rule:name:pat:eff   - add a rule (pat/eff comma-separated)");
    println!("  run                 - run fixed-point loop");
    println!("  facts               - dump all facts");
    println!("  rules               - dump all rules");
    println!("  load <file>         - load and execute a script file");
    println!("  checkpoint          - save state checkpoint");
    println!("  restore             - restore to last checkpoint");
    println!("  step                - run one iteration of fixed-point");
    println!("  quit                - exit");
    println!();

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
                } else if line.starts_with("rule:") {
                    handle_rule(engine, &line);
                } else if line.starts_with("assert") {
                    handle_assert(engine, &line);
                } else {
                    // Try to parse as a fact: pred(arg1, arg2)
                    if let Some(fact) = parse_fact(&line) {
                        engine.assert(fact);
                        println!("Fact asserted.");
                    } else {
                        println!("Unknown command. Try: pred(arg1, arg2) or rule:name:pat:eff");
                    }
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
        if line.starts_with("rule:") {
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
        } else if let Some(fact) = parse_fact(line) {
            engine.assert(fact);
        } else {
            println!("  [warning: could not parse line: {}]", line);
        }
    }
    Ok(())
}

/// Handle a rule: command by asserting a rule(...) fact.
fn handle_rule(engine: &mut Engine, line: &str) {
    // Format: rule:name:pat1,pat2:eff1,eff2:consume_idx1,consume_idx2
    let parts: Vec<&str> = line.splitn(5, ':').collect();
    if parts.len() < 4 {
        println!("Usage: rule:name:pattern1,pattern2:effect1,effect2");
        println!("   or: rule:name:pattern1,pattern2:effect1,effect2:0,1");
        return;
    }
    let name = parts[1].to_string();
    let match_str = parts[2].to_string();
    let effect_str = parts[3].to_string();
    let consume_str = if parts.len() > 4 && !parts[4].is_empty() {
        parts[4].to_string()
    } else {
        String::new()
    };

    // Build the rule(...) fact: rule(name, match_patterns, effect_patterns, consume_indices)
    // Match and effect patterns are stored as comma-separated strings.
    let mut fact = vec!["rule".to_string(), name, match_str, effect_str];
    if !consume_str.is_empty() {
        fact.push(consume_str);
    }

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

    if let Some(fact) = parse_fact(fact_str) {
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

/// Parse a fact string like "pred(arg1, arg2)" into a Fact.
fn parse_fact(s: &str) -> Option<Fact> {
    let s = s.trim();
    if let Some(paren) = s.find('(') {
        let pred = &s[..paren];
        let args_str = &s[paren + 1..s.len() - 1];
        let mut fact = vec![pred.to_string()];
        for arg in split_top_level(args_str) {
            let arg = arg.trim();
            if !arg.is_empty() {
                fact.push(arg.to_string());
            }
        }
        Some(fact)
    } else {
        Some(vec![s.to_string()])
    }
}
