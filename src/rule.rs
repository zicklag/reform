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
                let merged = match merge_bindings(bindings, &new_bindings) {
                    Some(m) => m,
                    None => continue,
                };
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fact::Pat;

    fn pat(s: &str) -> Pattern {
        if let Some(paren) = s.find('(') {
            let pred = &s[..paren];
            let args_str = &s[paren + 1..s.len() - 1];
            let mut pattern = if pred.starts_with('?') {
                vec![Pat::Var(pred[1..].to_string())]
            } else {
                vec![Pat::Atom(pred.to_string())]
            };
            for arg in args_str.split(',') {
                let arg = arg.trim();
                if arg.starts_with('?') {
                    pattern.push(Pat::Var(arg[1..].to_string()));
                } else {
                    pattern.push(Pat::Atom(arg.to_string()));
                }
            }
            pattern
        } else {
            vec![Pat::Atom(s.to_string())]
        }
    }

    fn fact(s: &str) -> Fact {
        if let Some(paren) = s.find('(') {
            let pred = &s[..paren];
            let args_str = &s[paren + 1..s.len() - 1];
            let mut f = vec![pred.to_string()];
            for arg in args_str.split(',') {
                f.push(arg.trim().to_string());
            }
            f
        } else {
            vec![s.to_string()]
        }
    }

    /// Regression test: when a variable is bound by an earlier pattern,
    /// and a later pattern has multiple candidate facts, a conflicting
    /// binding from one candidate must not abort the entire search —
    /// the next candidate should be tried.
    #[test]
    fn find_match_skips_conflicting_binding() {
        // go_north rule: n, here(?h), north_of(?g, ?h) -> here(?g)
        let rule = Rule {
            name: "go_north".into(),
            matches: vec![
                pat("n"),
                pat("here(?h)"),
                pat("north_of(?g, ?h)"),
            ],
            not_matches: vec![],
            consumes: vec![0, 1],
            effects: vec![pat("here(?g)")],
        };

        let facts = vec![
            fact("n"),
            fact("here(frontroom)"),
            // north_of(kitchen, bedroom) has h=bedroom — conflicts with h=frontroom
            fact("north_of(kitchen, bedroom)"),
            // north_of(bedroom, frontroom) has h=frontroom — matches!
            fact("north_of(bedroom, frontroom)"),
        ];

        let result = rule.find_match(&facts);
        assert!(result.is_some(), "should find a match despite conflicting candidate");
        let (bindings, matched) = result.unwrap();
        assert_eq!(bindings.iter().find(|(n, _)| n == "h").unwrap().1[0], "frontroom");
        assert_eq!(bindings.iter().find(|(n, _)| n == "g").unwrap().1[0], "bedroom");
        assert_eq!(matched.len(), 3);
    }

    /// Basic match: all patterns match with no conflicts.
    #[test]
    fn find_match_basic() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![
                pat("a(?x)"),
                pat("b(?x)"),
            ],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![],
        };

        let facts = vec![
            fact("a(hello)"),
            fact("b(hello)"),
        ];

        let result = rule.find_match(&facts);
        assert!(result.is_some());
    }

    /// No match when no fact satisfies a pattern.
    #[test]
    fn find_match_no_match() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![
                pat("a(?x)"),
                pat("b(?x)"),
            ],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![],
        };

        let facts = vec![
            fact("a(hello)"),
            fact("b(world)"),
        ];

        let result = rule.find_match(&facts);
        assert!(result.is_none());
    }

    /// Negation blocks a match.
    #[test]
    fn find_match_negation_blocks() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![
                pat("a(?x)"),
            ],
            not_matches: vec![
                pat("b(?x)"),
            ],
            consumes: vec![],
            effects: vec![],
        };

        let facts = vec![
            fact("a(hello)"),
            fact("b(hello)"),
        ];

        let result = rule.find_match(&facts);
        assert!(result.is_none());
    }

    /// Negation does not block when the negated pattern doesn't match.
    #[test]
    fn find_match_negation_allows() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![
                pat("a(?x)"),
            ],
            not_matches: vec![
                pat("b(?x)"),
            ],
            consumes: vec![],
            effects: vec![],
        };

        let facts = vec![
            fact("a(hello)"),
            fact("b(world)"),
        ];

        let result = rule.find_match(&facts);
        assert!(result.is_some());
    }
}
