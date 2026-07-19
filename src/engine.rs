use crate::rule::Rule;
use crate::{parser, Arg, Fact};
use anyhow::{anyhow, bail, Result};

/// A parsed engine command extracted from a fact.
#[derive(Debug)]
enum Command<'a> {
    Remove(&'a [&'a str]),
    Println(&'a [&'a str]),
    Print(&'a [&'a str]),
    Quit,
    Panic(&'a [&'a str]),
    Assert(&'a [&'a str]),
    AssertNot(&'a [&'a str]),
    Find(&'a [&'a str]),
    Facts,
    Load(&'a [&'a str]),
}

/// Try to parse a fact as a command. Returns `None` if the fact is not a
/// recognized command keyword.
fn parse_command<'a>(args: &'a [&'a str]) -> Option<Command<'a>> {
    let first = *args.first()?;
    let rest = &args[1..];
    Some(match first {
        "-" => Command::Remove(rest),
        "println" => Command::Println(rest),
        "print" => Command::Print(rest),
        "quit" => Command::Quit,
        "panic" => Command::Panic(rest),
        "assert" => Command::Assert(rest),
        "assert-not" => Command::AssertNot(rest),
        "find" => Command::Find(rest),
        "facts" => Command::Facts,
        "load" => Command::Load(rest),
        _ => return None,
    })
}

/// The Reform rule engine: a fact store plus the registered rules that fire
/// against it each turn.
#[derive(Debug, Default)]
pub struct Engine {
    facts: Vec<Fact>,
    rules: Vec<Rule>,
    quit: bool,
    changed: bool,
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

    pub fn clear_quit(&mut self) {
        self.quit = false;
    }

    pub fn add_fact(&mut self, fact: Fact) -> bool {
        if self.facts.contains(&fact) {
            false
        } else {
            self.facts.push(fact);
            self.changed = true;
            true
        }
    }

    pub fn remove_fact(&mut self, fact: &Fact) -> bool {
        let before = self.facts.len();
        self.facts.retain(|f| f != fact);
        let removed = self.facts.len() != before;
        if removed {
            self.changed = true;
        }
        removed
    }

    pub fn add_rule(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    pub fn contains(&self, fact: &Fact) -> bool {
        self.facts.contains(fact)
    }

    // -- loading -----------------------------------------------------------

    pub fn load_str(&mut self, src: &str) -> Result<()> {
        let result = self.load_str_inner(src);
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

    pub fn ingest_file(&mut self, fact: Fact) -> Result<()> {
        let args: Vec<Arg> = fact.iter().cloned().collect();
        if args.is_empty() {
            return Ok(());
        }
        let stored = match &*args[0] {
            "$" => Fact(args[1..].to_vec()),
            ">" => Fact(
                std::iter::once(Arg::from("prompt"))
                    .chain(args[1..].iter().cloned())
                    .collect(),
            ),
            _ => Fact(
                std::iter::once(Arg::from("sentence"))
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
            _ => std::iter::once("sentence")
                .chain(args.iter().map(|a| &**a))
                .collect(),
        };
        let cmd = parse_command(&strs);
        if is_rule {
            self.add_rule(Rule::parse(&strs)?);
        }
        if cmd.is_none() {
            self.add_fact(stored);
        }
        self.settle()?;
        if let Some(cmd) = cmd {
            self.execute_command(cmd)?;
        }
        Ok(())
    }

    pub fn ingest_body(&mut self, fact: Fact) -> Result<()> {
        let args: Vec<Arg> = fact.iter().cloned().collect();
        if args.is_empty() {
            return Ok(());
        }
        let stripped = if &*args[0] == "$" {
            Fact(args[1..].to_vec())
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
        let cmd = parse_command(&strs);
        if is_rule {
            self.add_rule(Rule::parse(&strs)?);
        }
        if let Some(cmd) = cmd {
            self.execute_command(cmd)?;
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
        const MAX_TURNS: usize = 100_000;
        for _ in 0..MAX_TURNS {
            if self.quit {
                return Ok(());
            }
            self.changed = false;
            self.turn()?;
            if self.quit || !self.changed {
                return Ok(());
            }
        }
        bail!("engine did not reach a fixpoint within {MAX_TURNS} turns");
    }

    pub fn turn(&mut self) -> Result<()> {
        let snapshot = self.facts.clone();
        let rules = self.rules.clone();
        for rule in &rules {
            for bindings in rule.find_matches(&snapshot) {
                for rf in rule.removed_facts(&snapshot, &bindings) {
                    self.remove_fact(&rf);
                }
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

    fn execute_command(&mut self, cmd: Command) -> Result<()> {
        match cmd {
            Command::Remove(args) => {
                if !args.is_empty() {
                    let fact_str = args.join(" ");
                    let parsed = parser::facts(&fact_str)?;
                    for f in parsed {
                        self.remove_fact(&f);
                    }
                }
                Ok(())
            }
            Command::Println(args) => {
                println!("{}", args.join(" "));
                Ok(())
            }
            Command::Print(args) => {
                print!("{}", args.join(" "));
                Ok(())
            }
            Command::Quit => {
                self.quit = true;
                Ok(())
            }
            Command::Panic(args) => Err(anyhow!("panic: {}", args.join(" "))),
            Command::Assert(args) => {
                let target = Fact(args.iter().map(|s| Arg::from(*s)).collect());
                if self.contains(&target) {
                    Ok(())
                } else {
                    Err(anyhow!("assert failed: fact {:?} not in engine", target))
                }
            }
            Command::AssertNot(args) => {
                let target = Fact(args.iter().map(|s| Arg::from(*s)).collect());
                if !self.contains(&target) {
                    Ok(())
                } else {
                    Err(anyhow!("assert-not failed: fact {:?} is in engine", target))
                }
            }
            Command::Find(args) => {
                let pattern_str = if args.len() == 1 {
                    args[0].to_string()
                } else {
                    args.join(" ")
                };
                let pat = parser::pattern(&pattern_str)?;
                for f in self.find_matching_facts(&pat)? {
                    println!("{}", normal_form_fact(&f));
                }
                Ok(())
            }
            Command::Facts => {
                for f in &self.facts {
                    println!("{}", normal_form_fact(f));
                }
                Ok(())
            }
            Command::Load(args) => {
                let path = args.first().copied().unwrap_or("");
                let src = std::fs::read_to_string(path)
                    .map_err(|e| anyhow!("load {}: {e}", path))?;
                self.load_str_inner(&src)
            }
        }
    }

    /// Facts in the engine that match the given (single-fact-line) pattern.
    pub fn find_matching_facts(&self, pat: &crate::rule::Pattern) -> Result<Vec<Fact>> {
        if pat.len() != 1 {
            bail!("find only supports single-fact patterns");
        }
        let Some(crate::rule::PatternItem::Fact(pf)) = pat.first() else {
            bail!("find only supports single-fact patterns");
        };
        Ok(self.facts
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
