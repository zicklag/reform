use crate::fact::{Bindings, Fact, Pattern, match_pattern, substitute};

/// A rule in the engine.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Name of the rule (for debugging).
    pub name: String,
    /// Patterns to match in the fact base (body).
    /// All must match for the rule to fire.
    pub matches: Vec<Pattern>,
    /// Patterns that must NOT match for the rule to fire (negation).
    pub not_matches: Vec<Pattern>,
    /// Which of the matched facts to consume (remove).
    /// Indexes into the `matches` list.
    pub consumes: Vec<usize>,
    /// Patterns for new facts to assert (effect).
    pub effects: Vec<Pattern>,
}

impl Rule {
    /// Try to find a complete match for this rule in the given facts.
    /// Returns the bindings and the matched facts if all patterns match.
    pub fn find_match(&self, facts: &[Fact]) -> Option<(Bindings, Vec<Fact>)> {
        self.find_match_from(facts, 0, &Bindings::new())
    }

    fn find_match_from(
        &self,
        facts: &[Fact],
        pattern_idx: usize,
        bindings: &Bindings,
    ) -> Option<(Bindings, Vec<Fact>)> {
        if pattern_idx >= self.matches.len() {
            // All positive patterns matched. Check negations.
            if self.check_negations(facts, bindings) {
                return Some((bindings.clone(), Vec::new()));
            }
            return None;
        }

        let pattern = &self.matches[pattern_idx];
        for fact in facts {
            if let Some(new_bindings) = match_pattern(pattern, fact) {
                // Merge bindings
                let merged = merge_bindings(bindings, &new_bindings)?;
                // Recurse to match remaining patterns
                if let Some((final_bindings, mut matched)) =
                    self.find_match_from(facts, pattern_idx + 1, &merged)
                {
                    matched.insert(0, fact.clone());
                    return Some((final_bindings, matched));
                }
            }
        }
        None
    }

    /// Check that none of the not_matches patterns match with the given bindings.
    fn check_negations(&self, facts: &[Fact], bindings: &Bindings) -> bool {
        for neg_pat in &self.not_matches {
            // Substitute bindings into the negation pattern
            let concrete = substitute(neg_pat, bindings);
            // Check if any fact matches the concrete pattern
            for fact in facts {
                if &concrete == fact {
                    return false; // A negation matched — rule is blocked
                }
            }
        }
        true
    }

    /// Apply the rule: produce effect facts from the bindings.
    pub fn apply(&self, bindings: &Bindings) -> Vec<Fact> {
        self.effects
            .iter()
            .map(|p| substitute(p, bindings))
            .collect()
    }

    /// Get the indices of matched facts to consume.
    pub fn consumed_indices(&self) -> &[usize] {
        &self.consumes
    }
}

/// Merge two bindings sets. Returns None if they conflict.
fn merge_bindings(a: &Bindings, b: &Bindings) -> Option<Bindings> {
    let mut result = a.clone();
    for (name, values) in b {
        if let Some((_, existing)) = result.iter_mut().find(|(n, _)| n == name) {
            if existing != values {
                return None; // conflicting bindings
            }
        } else {
            result.push((name.clone(), values.clone()));
        }
    }
    Some(result)
}
