use crate::rule::Rule;
use crate::{parser, Arg, Fact};
use anyhow::{anyhow, bail, Result};

/// The Reform rule engine: a fact store plus the registered rules that fire
/// against it each turn.
#[derive(Debug, Default)]
pub struct Engine {
    facts: Vec<Fact>,
    rules: Vec<Rule>,
    quit: bool,
}

impl Engine {
    pub fn new() -> Self {
        Self::default()
    }

    /// All facts currently in the engine, in insertion order.
    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    /// All registered rules.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Insert a fact, ignoring duplicates. Returns whether it was newly added.
    pub fn add_fact(&mut self, fact: Fact) -> bool {
        if self.facts.contains(&fact) {
            false
        } else {
            self.facts.push(fact);
            true
        }
    }

    /// Remove a fact if present. Returns whether it was present.
    pub fn remove_fact(&mut self, fact: &Fact) -> bool {
        let before = self.facts.len();
        self.facts.retain(|f| f != fact);
        self.facts.len() != before
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Does any fact in the engine equal `fact`?
    pub fn contains(&self, fact: &Fact) -> bool {
        self.facts.contains(fact)
    }

    // -- loading -----------------------------------------------------------

    /// Parse and ingest reform source text (facts, rules, commands) from a
    /// file, applying the `$` / `>` / sentence prefix rules. After each fact
    /// the engine settles (rules fire to fixpoint), so a later command in the
    /// file — e.g. `assert` — sees the facts that earlier facts and rules
    /// produced.
    pub fn load_str(&mut self, src: &str) -> Result<()> {
        for fact in parser::facts(src)? {
            self.ingest_file(fact)?;
            if self.quit {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Ingest a fact parsed from a file: apply the `$` / `>` / sentence prefix
    /// rules, register any rule, settle rules to a fixpoint, then execute the
    /// fact as a command (if it is one). Settling first means commands like
    /// `assert` observe the state produced by the facts and rules loaded so far.
    pub fn ingest_file(&mut self, fact: Fact) -> Result<()> {
        let args: Vec<Arg> = fact.iter().cloned().collect();
        if args.is_empty() {
            return Ok(());
        }
        let stored = match &*args[0] {
            // `$` prefix: strip it, store verbatim (no `sentence` prefix).
            "$" => Fact(args[1..].to_vec()),
            // `>` prefix: becomes a `prompt` fact.
            ">" => Fact(
                std::iter::once(Arg::from("prompt"))
                    .chain(args[1..].iter().cloned())
                    .collect(),
            ),
            // Otherwise: a plain sentence, prefixed with `sentence`.
            _ => Fact(
                std::iter::once(Arg::from("sentence"))
                    .chain(args.iter().cloned())
                    .collect(),
            ),
        };
        let is_command = stored.first().map(is_command_keyword).unwrap_or(false);
        let is_remove = stored.first().map(|a| &**a == "-").unwrap_or(false);
        // Register a rule, and keep the fact (removal directives aren't kept).
        if stored.is_rule() {
            let strs: Vec<&str> = stored.iter().map(arg_str).collect();
            self.add_rule(Rule::parse(&strs)?);
        }
        if !is_remove {
            self.add_fact(stored.clone());
        }
        // Let rules react to the new fact before running the command.
        self.settle()?;
        if is_command {
            self.execute_command(&stored)?;
        }
        Ok(())
    }

    /// Ingest a fact produced by a rule body during a turn. Stored verbatim
    /// (no `sentence` prefix); inner `rule` facts get registered; commands fire
    /// immediately. Does NOT settle (we are already inside a turn).
    pub fn ingest_body(&mut self, fact: Fact) -> Result<()> {
        if fact.iter().count() == 0 {
            return Ok(());
        }
        if fact.is_rule() {
            let strs: Vec<&str> = fact.iter().map(arg_str).collect();
            self.add_rule(Rule::parse(&strs)?);
        }
        let is_command = fact.first().map(is_command_keyword).unwrap_or(false);
        let is_remove = fact.first().map(|a| &**a == "-").unwrap_or(false);
        if is_command && !self.contains(&fact) {
            self.execute_command(&fact)?;
        }
        if !is_remove {
            self.add_fact(fact);
        }
        Ok(())
    }

    // -- turns -------------------------------------------------------------

    /// Run [`turn`](Self::turn) repeatedly until the engine reaches a
    /// fixpoint (no fact changes) or `quit`.
    pub fn run(&mut self) -> Result<()> {
        self.settle()
    }

    fn settle(&mut self) -> Result<()> {
        const MAX_TURNS: usize = 100_000;
        for _ in 0..MAX_TURNS {
            if self.quit {
                return Ok(());
            }
            let before = self.facts.clone();
            self.turn()?;
            if self.quit || self.facts == before {
                return Ok(());
            }
        }
        bail!("engine did not reach a fixpoint within {MAX_TURNS} turns");
    }

    /// One pass: every rule fires against a snapshot of the facts taken at
    /// the start of the turn. New facts produced this turn are visible next
    /// turn; facts removed by `-` pattern lines are removed immediately.
    pub fn turn(&mut self) -> Result<()> {
        let snapshot = self.facts.clone();
        let rules = self.rules.clone();
        for rule in &rules {
            for bindings in rule.find_matches(&snapshot) {
                // Remove facts matched by `-` pattern lines.
                for rf in rule.removed_facts(&snapshot, &bindings) {
                    self.remove_fact(&rf);
                }
                // Render the body to reform text and ingest the results.
                let text = rule.body.render(&bindings);
                if text.trim().is_empty() {
                    continue;
                }
                for f in parser::facts(&text)? {
                    self.ingest_body(f)?;
                    if self.quit {
                        return Ok(());
                    }
                }
            }
        }
        Ok(())
    }

    // -- commands ----------------------------------------------------------

    fn execute_command(&mut self, fact: &Fact) -> Result<()> {
        let args: Vec<&str> = fact.iter().map(arg_str).collect();
        if args.is_empty() {
            return Ok(());
        }
        match args[0] {
            "-" => {
                // `- a b c` removes the fact (a b c).
                if args.len() > 1 {
                    self.remove_fact(&Fact(args[1..].iter().map(|s| Arg::from(*s)).collect()));
                }
                Ok(())
            }
            "println" => {
                println!("{}", args[1..].join(" "));
                Ok(())
            }
            "print" => {
                print!("{}", args[1..].join(" "));
                Ok(())
            }
            "quit" => {
                self.quit = true;
                Ok(())
            }
            "panic" => Err(anyhow!("panic: {}", args[1..].join(" "))),
            "assert" => {
                let target = Fact(args[1..].iter().map(|s| Arg::from(*s)).collect());
                if self.contains(&target) {
                    Ok(())
                } else {
                    Err(anyhow!("assert failed: fact {:?} not in engine", target))
                }
            }
            "assert-not" => {
                let target = Fact(args[1..].iter().map(|s| Arg::from(*s)).collect());
                if !self.contains(&target) {
                    Ok(())
                } else {
                    Err(anyhow!("assert-not failed: fact {:?} is in engine", target))
                }
            }
            "find" => {
                let pattern_str = if args.len() == 2 {
                    args[1].to_string()
                } else {
                    args[1..].join(" ")
                };
                let pat = parser::pattern(&pattern_str)?;
                for f in self.find_matching_facts(&pat) {
                    println!("{}", normal_form_fact(&f));
                }
                Ok(())
            }
            "load" => {
                let path = args.get(1).copied().unwrap_or("");
                let src = std::fs::read_to_string(path)
                    .map_err(|e| anyhow!("load {}: {e}", path))?;
                self.load_str(&src)
            }
            _ => Ok(()),
        }
    }

    /// Facts in the engine that match the given (single-fact-line) pattern.
    fn find_matching_facts(&self, pat: &crate::rule::Pattern) -> Vec<Fact> {
        let Some(crate::rule::PatternItem::Fact(pf)) = pat.first() else {
            // Multi-line patterns aren't supported by `find`; fall back to all.
            return self.facts.clone();
        };
        self.facts
            .iter()
            .filter(|f| pf.matches_fact(f).is_some())
            .cloned()
            .collect()
    }
}

fn arg_str(a: &Arg) -> &str {
    &**a
}

fn is_command_keyword(a: &Arg) -> bool {
    matches!(
        &**a,
        "-" | "println" | "print" | "quit" | "panic" | "assert" | "assert-not" | "find" | "load"
    )
}

/// Render a fact as a single normal-form line: args space-separated, each
/// wrapped in parens if it needs it.
fn normal_form_fact(f: &Fact) -> String {
    let parts: Vec<String> = f.iter().map(normal_form_arg).collect();
    parts.join(" ")
}

fn normal_form_arg(a: &Arg) -> String {
    let s: &str = &**a;
    if s.is_empty() {
        return "()".to_string();
    }
    let needs = s.chars().any(|c| c.is_whitespace() || c == '(' || c == ')')
        || s.ends_with(|c| matches!(c, ';' | '.' | ':' | '\''));
    if !needs {
        return s.to_string();
    }
    let mut out = String::from("(");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            ')' => out.push_str("\\)"),
            _ => out.push(c),
        }
    }
    out.push(')');
    out
}