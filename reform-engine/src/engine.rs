use crate::fact::{Bindings, Fact, format_fact, parse_pattern_from_str, split_top_level};
use crate::rule::Rule;

/// A snapshot of the engine state for checkpoint/restore.
#[derive(Debug, Clone)]
pub struct Checkpoint {
    facts: Vec<Fact>,
}

/// The engine: a fact base + rule set + fixed-point loop.
#[derive(Debug, Clone)]
pub struct Engine {
    /// All current facts.
    facts: Vec<Fact>,
    /// Rules derived from rule(...) facts (rebuilt each iteration).
    rules: Vec<Rule>,
    /// Checkpoints for LSP-style incremental editing.
    checkpoints: Vec<Checkpoint>,
}

impl Engine {
    /// Create a new empty engine.
    pub fn new() -> Self {
        Engine {
            facts: Vec::new(),
            rules: Vec::new(),
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

    /// Get all current facts.
    pub fn facts(&self) -> &[Fact] {
        &self.facts
    }

    /// Get all current rules.
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Save a checkpoint of the current state.
    pub fn save_checkpoint(&mut self) {
        self.checkpoints.push(Checkpoint {
            facts: self.facts.clone(),
        });
    }

    /// Restore to the last checkpoint.
    pub fn restore_checkpoint(&mut self) -> bool {
        if let Some(cp) = self.checkpoints.pop() {
            self.facts = cp.facts;
            self.rules.clear();
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
            self.rules.clear();
            true
        } else {
            false
        }
    }

    /// Get the number of checkpoints.
    pub fn checkpoint_count(&self) -> usize {
        self.checkpoints.len()
    }

    /// Rebuild rules from rule(...) facts in the fact base.
    fn rebuild_rules(&mut self) {
        self.rules.clear();

        for fact in self.facts.iter() {
            if fact.len() < 3 || fact[0] != "rule" {
                continue;
            }
            let name = fact[1].clone();

            // Parse patterns from the remaining args.
            // Format: rule(name, match_patterns, effect_patterns, consume_indices)
            // match_patterns and effect_patterns are comma-separated strings.
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

            // First pattern arg is the comma-separated match patterns,
            // second is the comma-separated effect patterns.
            let match_strs: Vec<&str> = if let Some(s) = pattern_args.first() {
                split_top_level(s)
            } else {
                continue;
            };
            let effect_strs: Vec<&str> = if pattern_args.len() > 1 {
                split_top_level(pattern_args[1])
            } else {
                continue;
            };

            // Parse match and effect patterns independently from their comma-separated strings.
            let mut matches = Vec::new();
            let mut not_matches = Vec::new();
            for m in match_strs {
                let m = m.trim();
                if let Some(rest) = m.strip_prefix('!') {
                    if let Some(mp) = parse_pattern_from_str(rest) {
                        not_matches.push(mp);
                    }
                } else if let Some(mp) = parse_pattern_from_str(m) {
                    matches.push(mp);
                }
            }

            let mut effects = Vec::new();
            for e in effect_strs {
                let e = e.trim();
                if !e.is_empty() {
                    if let Some(ep) = parse_pattern_from_str(e) {
                        effects.push(ep);
                    }
                }
            }

            if !matches.is_empty() && !effects.is_empty() {
                self.rules.push(Rule {
                    name,
                    matches,
                    not_matches,
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
            // Phase 1: Rebuild rules from rule(...) facts
            self.rebuild_rules();

            let mut fired = false;

            // Collect all matches first (to avoid borrow issues)
            let all_rules: Vec<Rule> = self.rules.clone();

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
        if self.rules.is_empty() {
            println!("  (no rules)");
            return;
        }
        for rule in &self.rules {
            println!("  {}: {} patterns, {} effects, {} consumed",
                rule.name, rule.matches.len(), rule.effects.len(), rule.consumes.len());
        }
    }
}
