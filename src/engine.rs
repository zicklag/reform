use crate::fact::{Bindings, Fact, format_fact, parse_pattern_from_str};
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

/// Split a rule pattern/body string into individual pattern lines,
/// joining lines that are inside `$( ... )` blocks (which span multiple lines).
fn split_pattern_lines(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut current = String::new();
    let mut depth: u32 = 0;
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            // Always drop empty/comment lines; they are never part of a pattern.
            continue;
        }
        // Track $( ... ) depth across lines
        for c in trimmed.chars() {
            if c == '(' { depth += 1; }
            else if c == ')' { depth = depth.saturating_sub(1); }
        }
        if depth > 0 {
            // Inside a $( block — accumulate
            if !current.is_empty() { current.push(' '); }
            current.push_str(trimmed);
        } else {
            // Not inside a block — this is a complete pattern line
            if !current.is_empty() {
                if !current.is_empty() { current.push(' '); }
                current.push_str(trimmed);
                result.push(current.clone());
                current.clear();
            } else {
                result.push(trimmed.to_string());
            }
        }
    }
    if !current.is_empty() {
        result.push(current);
    }
    result
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
    /// `print(...)` facts are handled as built-in output and not stored.
    pub fn assert(&mut self, fact: Fact) {
        if fact.len() >= 2 && fact[0] == "print" {
            println!("{}", fact[1..].concat());
            return;
        }
        if fact.len() >= 2 && fact[0] == "println" {
            println!("{}", fact[1..].concat());
            return;
        }
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


    /// Rebuild rules from rule facts in the fact base.
    ///
    /// Rule facts have the format: [rule, name, pattern_content, body_content]
    /// where pattern_content and body_content are raw strings from paren groups.
    /// Patterns are separated by lines, but `$(` ... `)` blocks span multiple lines
    /// and must be joined into a single pattern.
    /// `-` prefix on a pattern line means consume it.
    /// `!` prefix on a pattern line means negation.
    pub fn rebuild_rules(&mut self) {
        self.rules.clear();

        for fact in self.facts.iter() {
            if fact.len() < 3 || fact[0] != "rule" {
                continue;
            }
            let name = fact[1].clone();

            let match_str = &fact[2];
            let effect_str = &fact[3];

            // Split match patterns by line, joining lines inside $( ... ) blocks
            let match_lines = split_pattern_lines(match_str);
            let effect_lines = split_pattern_lines(effect_str);

            // Parse match patterns, tracking consume and negation
            let mut matches = Vec::new();
            let mut not_matches = Vec::new();
            let mut consume_indices: Vec<usize> = Vec::new();

            for (i, m) in match_lines.iter().enumerate() {
                if let Some(rest) = m.strip_prefix('-') {
                    let rest = rest.trim();
                    if let Some(mp) = parse_pattern_from_str(rest) {
                        matches.push(mp);
                        consume_indices.push(i);
                    }
                } else if let Some(rest) = m.strip_prefix('!') {
                    let rest = rest.trim();
                    if let Some(mp) = parse_pattern_from_str(rest) {
                        not_matches.push(mp);
                    }
                } else if let Some(mp) = parse_pattern_from_str(m) {
                    matches.push(mp);
                }
            }

            let mut effects = Vec::new();
            for e in effect_lines {
                if !e.is_empty() {
                    if let Some(ep) = parse_pattern_from_str(&e) {
                        effects.push(ep);
                    }
                }
            }

            if !matches.is_empty() && (!effects.is_empty() || !consume_indices.is_empty()) {
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
                .flat_map(|(idx, rule)| {
                    rule.find_all_matches(&self.facts)
                        .into_iter()
                        .map(move |(bindings, matched)| (idx, bindings, matched))
                })
                .collect();

            for (rule_idx, bindings, matched_facts) in &matches {
                let rule = &all_rules[*rule_idx];

                // Skip if any consumed fact is already gone (consumed by an earlier rule)
                let mut all_consumed_exist = true;
                for &ci in rule.consumed_indices() {
                    if ci < matched_facts.len() {
                        if !self.facts.contains(&matched_facts[ci]) {
                            all_consumed_exist = false;
                        }
                    }
                }
                if !all_consumed_exist {
                    continue;
                }
                // Re-check negations at apply time (facts may have changed since collection)
                if !rule.check_negations(&self.facts, bindings) {
                    continue;
                }
                // Check if this rule will actually change anything.
                // The fact base changes only if a consumed fact is not re-produced
                // or if an effect fact is not already present.
                let new_facts = rule.apply(bindings);
                let mut changed = false;

                for &ci in rule.consumed_indices() {
                    if ci < matched_facts.len() {
                        let consumed = &matched_facts[ci];
                        if self.facts.contains(consumed) && !new_facts.contains(consumed) {
                            changed = true;
                        }
                    }
                }

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

    /// Return a formatted string of all current facts.
    pub fn dump_facts(&self) -> String {
        let mut s = String::new();
        if self.facts.is_empty() {
            s.push_str("  (no facts)
");
            return s;
        }
        for fact in &self.facts {
            s.push_str(&format!("  {}
", format_fact(fact)));
        }
        s
    }

    /// Return a formatted string of all current rules.
    pub fn dump_rules(&self) -> String {
        let mut s = String::new();
        if self.rules.is_empty() {
            s.push_str("  (no rules)
");
            return s;
        }
        for rule in &self.rules {
            s.push_str(&format!("  {}: {} patterns, {} effects, {} consumed
",
                rule.name, rule.matches.len(), rule.effects.len(), rule.consumes.len()));
        }
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_assert_adds_fact() {
        let mut eng = Engine::new();
        let f = vec!["hello".to_string(), "world".to_string()];
        eng.assert(f.clone());
        assert_eq!(eng.facts().len(), 1);
        assert_eq!(eng.facts()[0], f);
    }

    #[test]
    fn engine_assert_deduplicates() {
        let mut eng = Engine::new();
        let f = vec!["hello".to_string(), "world".to_string()];
        eng.assert(f.clone());
        eng.assert(f.clone());
        assert_eq!(eng.facts().len(), 1);
    }

    #[test]
    fn engine_retract_removes_fact() {
        let mut eng = Engine::new();
        let f = vec!["hello".to_string(), "world".to_string()];
        eng.assert(f.clone());
        eng.retract(&f);
        assert!(eng.facts().is_empty());
    }

    #[test]
    fn engine_retract_nonexistent() {
        let mut eng = Engine::new();
        let f = vec!["hello".to_string(), "world".to_string()];
        eng.retract(&f);
        assert!(eng.facts().is_empty());
    }

    #[test]
    fn engine_print_fact_not_stored() {
        let mut eng = Engine::new();
        let f = vec!["print".to_string(), "hello".to_string()];
        eng.assert(f);
        assert!(eng.facts().is_empty());
    }

    #[test]
    fn engine_save_and_restore_checkpoint() {
        let mut eng = Engine::new();
        eng.save_checkpoint();
        let f = vec!["hello".to_string(), "world".to_string()];
        eng.assert(f);
        assert_eq!(eng.facts().len(), 1);
        let restored = eng.restore_checkpoint();
        assert!(restored);
        assert!(eng.facts().is_empty());
    }

    #[test]
    fn engine_restore_to_index() {
        let mut eng = Engine::new();
        eng.save_checkpoint();
        let f1 = vec!["first".to_string()];
        eng.assert(f1);
        eng.save_checkpoint();
        let f2 = vec!["second".to_string()];
        eng.assert(f2);
        assert_eq!(eng.facts().len(), 2);
        let restored = eng.restore_to(0);
        assert!(restored);
        assert!(eng.facts().is_empty());
    }

    #[test]
    fn engine_rebuild_rules_from_facts() {
        let mut eng = Engine::new();
        let rule_fact = vec![
            "rule".to_string(),
            "test".to_string(),
            "hello $x".to_string(),
            "hi $x".to_string(),
        ];
        eng.assert(rule_fact);
        eng.rebuild_rules();
        assert_eq!(eng.rules().len(), 1);
        assert_eq!(eng.rules()[0].name, "test");
    }

    #[test]
    fn engine_rebuild_rules_ignores_non_rule() {
        let mut eng = Engine::new();
        let f = vec!["hello".to_string(), "world".to_string()];
        eng.assert(f);
        eng.rebuild_rules();
        assert!(eng.rules().is_empty());
    }

    #[test]
    fn engine_run_fixedpoint_no_rules() {
        let mut eng = Engine::new();
        let firings = eng.run_fixedpoint();
        assert_eq!(firings, 0);
    }

    #[test]
    fn engine_run_fixedpoint_simple_rule() {
        let mut eng = Engine::new();
        let rule_fact = vec![
            "rule".to_string(),
            "test".to_string(),
            "- hello $x".to_string(),
            "hi $x".to_string(),
        ];
        eng.assert(rule_fact);
        let fact = vec!["hello".to_string(), "world".to_string()];
        eng.assert(fact);
        let firings = eng.run_fixedpoint();
        assert_eq!(firings, 1);
        assert!(!eng.facts().iter().any(|f| f == &vec!["hello".to_string(), "world".to_string()]));
        assert!(eng.facts().iter().any(|f| f == &vec!["hi".to_string(), "world".to_string()]));
    }

    #[test]
    fn engine_run_fixedpoint_chain() {
        let mut eng = Engine::new();
        let rule1 = vec![
            "rule".to_string(),
            "r1".to_string(),
            "- a $x".to_string(),
            "b $x".to_string(),
        ];
        eng.assert(rule1);
        let rule2 = vec![
            "rule".to_string(),
            "r2".to_string(),
            "- b $x".to_string(),
            "c $x".to_string(),
        ];
        eng.assert(rule2);
        let fact = vec!["a".to_string(), "1".to_string()];
        eng.assert(fact);
        let firings = eng.run_fixedpoint();
        assert_eq!(firings, 2);
        assert!(!eng.facts().iter().any(|f| f == &vec!["a".to_string(), "1".to_string()]));
        assert!(!eng.facts().iter().any(|f| f == &vec!["b".to_string(), "1".to_string()]));
        assert!(eng.facts().iter().any(|f| f == &vec!["c".to_string(), "1".to_string()]));
    }

    #[test]
    fn engine_run_fixedpoint_stops_when_stable() {
        let mut eng = Engine::new();
        let rule_fact = vec![
            "rule".to_string(),
            "test".to_string(),
            "a $x".to_string(),
            "a $x".to_string(),
        ];
        eng.assert(rule_fact);
        let fact = vec!["a".to_string(), "1".to_string()];
        eng.assert(fact);
        let firings = eng.run_fixedpoint();
        assert_eq!(firings, 0);
    }

    #[test]
    fn engine_dump_facts_does_not_panic() {
        let mut eng = Engine::new();
        let _ = eng.dump_facts();
        let f = vec!["hello".to_string(), "world".to_string()];
        eng.assert(f);
        let _ = eng.dump_facts();
    }

    #[test]
    fn engine_dump_rules_does_not_panic() {
        let mut eng = Engine::new();
        let _ = eng.dump_rules();
        let rule_fact = vec![
            "rule".to_string(),
            "test".to_string(),
            "hello $x".to_string(),
            "hi $x".to_string(),
        ];
        eng.assert(rule_fact);
        eng.rebuild_rules();
        let _ = eng.dump_rules();
    }

    #[test]
    fn engine_checkpoint_count() {
        let mut eng = Engine::new();
        assert_eq!(eng.checkpoint_count(), 0);
        eng.save_checkpoint();
        assert_eq!(eng.checkpoint_count(), 1);
        eng.save_checkpoint();
        assert_eq!(eng.checkpoint_count(), 2);
    }

    #[test]
    fn engine_restore_to_invalid_index() {
        let mut eng = Engine::new();
        let result = eng.restore_to(0);
        assert!(!result);
    }

    #[test]
    fn engine_restore_checkpoint_no_checkpoints() {
        let mut eng = Engine::new();
        let result = eng.restore_checkpoint();
        assert!(!result);
    }
}
