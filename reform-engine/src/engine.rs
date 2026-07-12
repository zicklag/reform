use crate::fact::{Bindings, Fact, format_fact, parse_pattern_from_str};
use crate::rule::Rule;

/// A snapshot of the engine state for checkpoint/restore.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    facts: Vec<Fact>,
    rules: Vec<Rule>,
}

/// The engine: a fact base + rule set + fixed-point loop.
#[derive(Debug, Clone)]
pub struct Engine {
    /// All current facts.
    facts: Vec<Fact>,
    /// Built-in rules (added via add_rule, persist forever).
    builtin_rules: Vec<Rule>,
    /// Rules derived from rule(...) facts (rebuilt each iteration).
    fact_rules: Vec<Rule>,
    /// Checkpoints for LSP-style incremental editing.
    checkpoints: Vec<Checkpoint>,
}

impl Engine {
    /// Create a new empty engine.
    pub fn new() -> Self {
        Engine {
            facts: Vec::new(),
            builtin_rules: Vec::new(),
            fact_rules: Vec::new(),
            checkpoints: Vec::new(),
        }
    }

    /// Assert a fact into the fact base.
    pub fn assert(&mut self, fact: Fact) {
        if !self.facts.contains(&fact) {
            self.facts.push(fact);
        }
    }

    /// Retract (remove) a fact from the fact base.
    pub fn retract(&mut self, fact: &Fact) {
        self.facts.retain(|f| f != fact);
    }

    /// Add a built-in rule to the engine.
    pub fn add_rule(&mut self, rule: Rule) {
        self.builtin_rules.push(rule);
    }

    /// Get all current facts.
    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    /// Get all current rules (built-in + fact-derived).
    pub fn rules(&self) -> Vec<&Rule> {
        self.builtin_rules
            .iter()
            .chain(self.fact_rules.iter())
            .collect()
    }

    /// Save a checkpoint of the current state.
    pub fn save_checkpoint(&mut self) {
        self.checkpoints.push(Checkpoint {
            facts: self.facts.clone(),
            rules: self.builtin_rules.clone(),
        });
    }

    /// Restore to the last checkpoint.
    pub fn restore_checkpoint(&mut self) -> bool {
        if let Some(cp) = self.checkpoints.pop() {
            self.facts = cp.facts;
            self.builtin_rules = cp.rules;
            self.fact_rules.clear();
            true
        } else {
            false
        }
    }

    /// Restore to a specific checkpoint index (0 = first).
    pub fn restore_to(&mut self, index: usize) -> bool {
        if index < self.checkpoints.len() {
            let cp = self.checkpoints[index].clone();
            self.checkpoints.truncate(index);
            self.facts = cp.facts;
            self.builtin_rules = cp.rules;
            self.fact_rules.clear();
            true
        } else {
            false
        }
    }

    /// Get the number of checkpoints.
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// Rebuild fact_rules from rule(...) facts in the fact base.
    fn rebuild_fact_rules(&mut self) {
        self.fact_rules.clear();

        for fact in self.facts.iter() {
            if fact.len() < 3 || fact[0] != "rule" {
                continue;
            }
            let name = fact[1].clone();

            // Parse patterns from the remaining args
            let mut consume_indices: Vec<usize> = Vec::new();
            let mut pattern_args: Vec<&str> = Vec::new();

            for arg in &fact[2..] {
                if arg.contains(',') && arg.chars().all(|c| c.is_ascii_digit() || c == ',') {
                    consume_indices = arg.split(',')
                        .filter_map(|s| s.trim().parse().ok())
                        .collect();
                } else {
                    pattern_args.push(arg);
                }
            }

            // Pair up match/effect patterns
            let mut matches = Vec::new();
            let mut effects = Vec::new();
            let mut i = 0;
            while i + 1 < pattern_args.len() {
                if let (Some(mp), Some(ep)) = (
                    parse_pattern_from_str(pattern_args[i]),
                    parse_pattern_from_str(pattern_args[i + 1]),
                ) {
                    matches.push(mp);
                    effects.push(ep);
                }
                i += 2;
            }

            if !matches.is_empty() && !effects.is_empty() {
                if consume_indices.is_empty() {
                    consume_indices = (0..matches.len()).collect();
                }
                self.fact_rules.push(Rule {
                    name,
                    matches,
                    not_matches: Vec::new(),
                    consumes: consume_indices,
                    effects,
                });
            }
        }
    }

    /// Run the fixed-point loop until no more rules match.
    /// Returns the number of rule firings.
    pub fn run_fixedpoint(&mut self) -> usize {
        let mut total_firings = 0;
        loop {
            // Phase 1: Rebuild fact-derived rules from rule(...) facts
            self.rebuild_fact_rules();

            let mut fired = false;

            // Collect all matches first (to avoid borrow issues)
            let all_rules: Vec<Rule> = self.builtin_rules
                .iter()
                .chain(self.fact_rules.iter())
                .cloned()
                .collect();

            let matches: Vec<(usize, Bindings, Vec<Fact>)> = all_rules
                .iter()
                .enumerate()
                .filter_map(|(idx, rule)| {
                    rule.find_match(&self.facts)
                        .map(|(bindings, matched)| (idx, bindings, matched))
                })
                .collect();

            for (rule_idx, bindings, matched_facts) in &matches {
                let rule = &all_rules[*rule_idx];

                // Check if this rule will actually change anything
                let mut changed = false;

                // Check if any consumed facts exist
                for &ci in rule.consumed_indices() {
                    if ci < matched_facts.len() {
                        if self.facts.contains(&matched_facts[ci]) {
                            changed = true;
                        }
                    }
                }

                // Check if any effect facts are new
                let new_facts = rule.apply(bindings);
                for fact in &new_facts {
                    if !self.facts.contains(fact) {
                        changed = true;
                    }
                }

                if !changed {
                    continue;
                }

                // Consume matched facts
                for &ci in rule.consumed_indices() {
                    if ci < matched_facts.len() {
                        self.retract(&matched_facts[ci]);
                    }
                }

                // Produce effect facts
                for fact in new_facts {
                    self.assert(fact);
                }

                fired = true;
                total_firings += 1;
            }

            if !fired {
                break;
            }
        }
        total_firings
    }

    /// Print all current facts.
    pub fn dump_facts(&self) {
        if self.facts.is_empty() {
            println!("  (no facts)");
            return;
        }
        for fact in &self.facts {
            println!("  {}", format_fact(fact));
        }
    }

    /// Print all current rules.
    pub fn dump_rules(&self) {
        let all: Vec<&Rule> = self.rules();
        if all.is_empty() {
            println!("  (no rules)");
            return;
        }
        for rule in &all {
            let source = if self.builtin_rules.iter().any(|r| r.name == rule.name) {
                "builtin"
            } else {
                "fact"
            };
            println!("  {} [{}]: {} patterns, {} effects, {} consumed",
                rule.name, source, rule.matches.len(), rule.effects.len(), rule.consumes.len());
        }
    }
}
