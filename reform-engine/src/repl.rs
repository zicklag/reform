use std::io::{self, BufRead};

use crate::engine::Engine;
use crate::fact::format_fact;
use crate::parser::{parse_stmt, Stmt};

/// Run a simple REPL that reads lines and processes them.
/// `show_help` controls whether the command list is printed on startup.
pub fn run_repl(engine: &mut Engine, prompt_mode: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, true, prompt_mode)
}

/// Run the REPL without printing the command list.
pub fn run_repl_quiet(engine: &mut Engine, prompt_mode: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, false, prompt_mode)
}

fn run_repl_with_help(engine: &mut Engine, show_help: bool, prompt_mode: bool) -> anyhow::Result<()> {
    let stdin = io::stdin();
    println!("Reform Engine REPL");
    if show_help {
        println!("Commands:");
        println!("  pred(arg1, arg2)    - assert a fact");
        println!("  -pred(arg1, arg2)   - retract a fact");
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

        match parse_stmt(&line) {
            None => {
                println!("Parse error. Try: pred(arg1, arg2) or rule name: pat -> eff");
            }
            Some(stmt) => {
                if !exec_stmt(engine, stmt, prompt_mode) {
                    break;
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
        if let Some(stmt) = parse_stmt(line) {
            exec_stmt(engine, stmt, false);
        }
    }
    Ok(())
}

/// Execute a parsed statement. Returns false if the program should quit.
fn exec_stmt(engine: &mut Engine, stmt: Stmt, prompt_mode: bool) -> bool {
    match stmt {
        Stmt::Quit => return false,
        Stmt::Run => {
            let firings = engine.run_fixedpoint();
            println!("  [run: {} firings]", firings);
        }
        Stmt::Facts => {
            println!("  [facts after run:]");
            engine.dump_facts();
        }
        Stmt::Rules => {
            engine.rebuild_rules();
            println!("Rules ({}):", engine.rules().len());
            engine.dump_rules();
        }
        Stmt::Checkpoint => {
            engine.save_checkpoint();
            println!("  [checkpoint saved]");
        }
        Stmt::Restore => {
            engine.restore_checkpoint();
            println!("  [restored]");
        }
        Stmt::Load(path) => {
            if let Err(e) = load_script(engine, &path) {
                println!("Error loading '{}': {}", path, e);
            } else {
                println!("Loaded '{}'.", path);
            }
        }
        Stmt::Assert(fact) => {
            engine.assert(fact);
        }
        Stmt::Retract(fact) => {
            engine.retract(&fact);
        }
        Stmt::AssertExists(fact) => {
            let fact_str = format_fact(&fact);
            if !engine.facts().contains(&fact) {
                eprintln!("Assertion failed: {} should exist", fact_str);
                std::process::exit(1);
            }
        }
        Stmt::AssertNot(fact) => {
            let fact_str = format_fact(&fact);
            if engine.facts().contains(&fact) {
                eprintln!("Assertion failed: {} should not exist", fact_str);
                std::process::exit(1);
            }
        }
        Stmt::Sentence(words) => {
            let pred = if prompt_mode { "prompt" } else { "sentence" };
            let mut fact = vec![pred.to_string()];
            fact.extend(words);
            engine.assert(fact);
        }
        Stmt::Rule { name, matches, effects } => {
            let fact = vec!["rule".to_string(), name, matches, effects];
            engine.assert(fact);
            println!("Rule added.");
        }
    }
    true
}

