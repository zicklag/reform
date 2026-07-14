use std::io::{self, BufRead, Write};

use crate::engine::Engine;
use crate::fact::{format_fact, split_patterns, parse_pattern_from_str};
use crate::rule::Rule;
use crate::parser::{parse_stmt, Stmt};

/// Run the REPL with full control over all options.
pub fn run_repl_full(engine: &mut Engine, show_help: bool, prompt_mode: bool, verbose: bool, allow_commands: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, show_help, prompt_mode, verbose, allow_commands)
}

/// Run a simple REPL that reads lines and processes them.
/// `show_help` controls whether the command list is printed on startup.
pub fn run_repl(engine: &mut Engine, prompt_mode: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, true, prompt_mode, false, false)
}

/// Run the REPL without printing the command list.
pub fn run_repl_quiet(engine: &mut Engine, prompt_mode: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, false, prompt_mode, false, false)
}

/// Run a simple REPL that reads lines and processes them, with verbose output.
pub fn run_repl_verbose(engine: &mut Engine, prompt_mode: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, true, prompt_mode, true, false)
}

/// Run the REPL without printing the command list, with verbose output.
pub fn run_repl_quiet_verbose(engine: &mut Engine, prompt_mode: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, false, prompt_mode, true, false)
}

/// Run a simple REPL that reads lines and processes them, with allow_commands.
pub fn run_repl_allow_commands(engine: &mut Engine, prompt_mode: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, true, prompt_mode, false, true)
}

/// Run the REPL without printing the command list, with allow_commands.
pub fn run_repl_quiet_allow_commands(engine: &mut Engine, prompt_mode: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, false, prompt_mode, false, true)
}

/// Run a simple REPL that reads lines and processes them, with verbose and allow_commands.
pub fn run_repl_verbose_allow_commands(engine: &mut Engine, prompt_mode: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, true, prompt_mode, true, true)
}

/// Run the REPL without printing the command list, with verbose and allow_commands.
pub fn run_repl_quiet_verbose_allow_commands(engine: &mut Engine, prompt_mode: bool) -> anyhow::Result<()> {
    run_repl_with_help(engine, false, prompt_mode, true, true)
}

fn run_repl_with_help(engine: &mut Engine, show_help: bool, prompt_mode: bool, verbose: bool, allow_commands: bool) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    println!("Reform Engine REPL");
    if show_help {
        println!("Commands:");
        println!("  pred(arg1, arg2)    - assert a fact");
        println!("  -pred(arg1, arg2)   - retract a fact");
        println!("  rule name: pat -> eff - add a rule");
        println!("  run                 - run fixed-point loop");
        println!("  facts               - dump all facts");
        println!("  rules               - dump all rules");
        println!("  find pat(?x, ?y)    - find facts matching pattern");
        println!("  load <file>         - load and execute a script file");
        println!("  checkpoint          - save state checkpoint");
        println!("  restore             - restore to last checkpoint");
        println!("  step                - run one iteration of fixed-point");
        println!("  assert pred(arg1,..) - crash if fact doesn't exist");
        println!("  assert not pred(..)  - crash if fact exists");
        println!("  quit                - exit");
        println!();
    }

    if prompt_mode {
        print!("> ");
        stdout.flush()?;
    }

    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim().to_string();

        if line.is_empty() {
            if prompt_mode {
                print!("> ");
                stdout.flush()?;
            }
            continue;
        }

        // Determine the input to parse
        let input = if prompt_mode && allow_commands {
            // Try parsing the raw line first — if it's a recognized statement, use it as-is
            match parse_stmt(&line) {
                Some(Stmt::Sentence(_)) | None => {
                    // Not a command — treat as prompt
                    format!("> {}", line)
                }
                Some(_) => {
                    // Recognized command — use raw
                    line.clone()
                }
            }
        } else if prompt_mode {
            // Always prepend "> " so plain input becomes a prompt fact
            format!("> {}", line)
        } else {
            line.clone()
        };

        match parse_stmt(&input) {
            None => {
                println!("Parse error. Try: pred(arg1, arg2) or rule name: pat -> eff");
            }
            Some(stmt) => {
                let should_auto_run = matches!(&stmt,
                    Stmt::Assert(_) | Stmt::Retract(_) | Stmt::Sentence(_) | Stmt::Prompt(_) | Stmt::Rule { .. }
                );
                if !exec_stmt(engine, stmt) {
                    break;
                }
                if should_auto_run {
                    let firings = engine.run_fixedpoint();
                    if verbose && firings > 0 {
                        println!("  [auto-run: {} firings]", firings);
                    }
                }
            }
        }

        if prompt_mode {
            print!("> ");
            stdout.flush()?;
        }
    }

    Ok(())
}

/// Load and execute a script file.
/// `base_dir` is the directory to resolve relative `load` paths against.
pub fn load_script(engine: &mut Engine, path: &str) -> anyhow::Result<()> {
    let base_dir = std::path::Path::new(path).parent().unwrap_or(std::path::Path::new(".")).to_path_buf();
    load_script_from(engine, path, &base_dir)
}

fn load_script_from(engine: &mut Engine, path: &str, base_dir: &std::path::Path) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)?;
    for line in content.lines() {
        let line = line.trim();
        if let Some(stmt) = parse_stmt(line) {
            let should_auto_run = matches!(&stmt,
                Stmt::Assert(_) | Stmt::Retract(_) | Stmt::Sentence(_) | Stmt::Prompt(_) | Stmt::Rule { .. }
            );
            exec_stmt_from(engine, stmt, base_dir);
            if should_auto_run {
                engine.run_fixedpoint();
            }
        }
    }
    Ok(())
}

/// Execute a parsed statement. Returns false if the program should quit.
/// Uses cwd (`.`) as base directory for `load` commands.
fn exec_stmt(engine: &mut Engine, stmt: Stmt) -> bool {
    exec_stmt_from(engine, stmt, std::path::Path::new("."))
}

/// Execute a parsed statement with a base directory for resolving relative `load` paths.
fn exec_stmt_from(engine: &mut Engine, stmt: Stmt, base_dir: &std::path::Path) -> bool {
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
            let resolved = if std::path::Path::new(&path).is_relative() {
                base_dir.join(&path)
            } else {
                std::path::PathBuf::from(&path)
            };
            let new_base = resolved.parent().unwrap_or(std::path::Path::new("."));
            if let Err(e) = load_script_from(engine, &resolved.to_string_lossy(), new_base) {
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
            let mut fact = vec!["sentence".to_string()];
            fact.extend(words);
            engine.assert(fact);
        }
        Stmt::Prompt(words) => {
            let mut fact = vec!["prompt".to_string()];
            fact.extend(words);
            engine.assert(fact);
        }
        Stmt::Rule { name, matches, effects } => {
            let fact = vec!["rule".to_string(), name, matches, effects];
            engine.assert(fact);
        }
        Stmt::Find(pat) => {
            let match_strs = split_patterns(&pat);
            let mut matches = Vec::new();
            let mut not_matches = Vec::new();
            for m in match_strs {
                let m = m.trim();
                if let Some(rest) = m.strip_prefix('!') {
                    if let Some(p) = parse_pattern_from_str(rest) {
                        not_matches.push(p);
                    }
                } else if let Some(p) = parse_pattern_from_str(m) {
                    matches.push(p);
                }
            }
            if matches.is_empty() {
                println!("  [could not parse pattern: {}]", pat);
                return true;
            }
            let rule = Rule {
                name: "find".into(),
                matches,
                not_matches,
                consumes: Vec::new(),
                effects: Vec::new(),
            };
            let all_matches = rule.find_all_matches(engine.facts());
            if all_matches.is_empty() {
                println!("  (no matches)");
            } else {
                // Collect and print every matched fact, deduplicated.
                let mut seen: Vec<crate::fact::Fact> = Vec::new();
                for (_bindings, matched_facts) in &all_matches {
                    for fact in matched_facts {
                        if !seen.contains(fact) {
                            seen.push(fact.clone());
                            println!("  {}", format_fact(fact));
                        }
                    }
                }
            }
        }
    }
    true
}
