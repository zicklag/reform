use std::io::{self, BufRead, Write};

use crate::engine::Engine;
use crate::fact::{format_fact, parse_pattern_from_str, split_patterns};
use crate::parser::{Stmt, parse_stmt};
use crate::rule::Rule;

/// Options controlling REPL behavior.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReplOptions {
    /// Print the command list on startup.
    pub show_help: bool,
    /// Prepend `> ` to input lines as prompt(...) facts.
    pub prompt_mode: bool,
    /// Print auto-run firing counts.
    pub verbose: bool,
    /// Allow $-prefixed commands when in prompt mode.
    pub allow_commands: bool,
}

/// Run the REPL with the given options.
pub fn run_repl(engine: &mut Engine, options: ReplOptions) -> anyhow::Result<()> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    println!("Reform Engine REPL");
    if options.show_help {
        println!("Commands:");
        println!("  (pred, arg1, arg2)  - assert a fact");
        println!("  -(pred, arg1, arg2)  - retract a fact");
        println!("  facts               - dump all facts");
        println!("  find pattern        - find facts matching pattern");
        println!("  load <file>         - load and execute a script file");
        println!("  assert (fact, ...)  - crash if fact doesn't exist");
        println!("  assert not (fact)   - crash if fact exists");
        println!("  quit                - exit");
        println!();
    }

    if options.prompt_mode {
        print!("> ");
        stdout.flush()?;
    }

    for line in stdin.lock().lines() {
        let line = line?;
        let line = line.trim().to_string();

        if line.is_empty() {
            if options.prompt_mode {
                print!("> ");
                stdout.flush()?;
            }
            continue;
        }

        // Determine the input to parse
        let input = if options.prompt_mode && options.allow_commands {
            // Lines starting with $, (, or - are commands; everything else is a prompt
            if line.starts_with('$') || line.starts_with('(') || line.starts_with('-') {
                line.clone()
            } else {
                format!("> {}", line)
            }
        } else if options.prompt_mode {
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
                let should_auto_run = matches!(&stmt, Stmt::Fact(_) | Stmt::DeleteFact(_));
                if !exec_stmt(engine, stmt) {
                    break;
                }
                if should_auto_run {
                    let firings = engine.run_fixedpoint();
                    if options.verbose && firings > 0 {
                        println!("  [auto-run: {} firings]", firings);
                    }
                }
            }
        }

        if options.prompt_mode {
            print!("> ");
            stdout.flush()?;
        }
    }

    Ok(())
}



/// Load and execute a script file.
/// `base_dir` is the directory to resolve relative `load` paths against.
pub fn load_script(engine: &mut Engine, path: &str) -> anyhow::Result<()> {
    let base_dir = std::path::Path::new(path)
        .parent()
        .unwrap_or(std::path::Path::new("."))
        .to_path_buf();
    load_script_from(engine, path, &base_dir)
}

fn load_script_from(
    engine: &mut Engine,
    path: &str,
    base_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let content = std::fs::read_to_string(path)?;
    let stmts = crate::parser::parse_file(&content)
        .map_err(|e| anyhow::anyhow!("Parse error in '{}': {}", path, e))?;
    for stmt in stmts {
        let should_auto_run = matches!(&stmt, Stmt::Fact(_) | Stmt::DeleteFact(_));
        exec_stmt_from(engine, stmt, base_dir);
        if should_auto_run {
            engine.run_fixedpoint();
        }
    }
    Ok(())
}

fn exec_stmt(engine: &mut Engine, stmt: Stmt) -> bool {
    exec_stmt_from(engine, stmt, std::path::Path::new("."))
}

/// Execute a parsed statement with a base directory for resolving relative `load` paths.
fn exec_stmt_from(engine: &mut Engine, stmt: Stmt, base_dir: &std::path::Path) -> bool {
    match stmt {
        Stmt::Quit => return false,
        Stmt::Facts => {
            println!("  [facts after run:]");
            print!("{}", engine.dump_facts());
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
        Stmt::Fact(fact) => {
            engine.assert(fact);
        }
        Stmt::DeleteFact(fact) => {
            engine.retract(&fact);
        }
        Stmt::Assert(fact) => {
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
        Stmt::Panic(args) => {
            eprintln!("{}", args.join(" "));
            std::process::exit(1);
        }
        Stmt::Println(args) => {
            println!("{}", args.join(" "));
        }
        Stmt::Print(args) => {
            print!("{}", args.join(" "));
            let _ = io::stdout().flush();
        }
    }
    true
}
