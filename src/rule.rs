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

    /// Find every distinct match for this rule in the given facts.
    /// Each match is returned once, deduplicated by the set of matched facts.
    pub fn find_all_matches(&self, facts: &[Fact]) -> Vec<(Bindings, Vec<Fact>)> {
        let mut results: Vec<(Bindings, Vec<Fact>)> = Vec::new();
        self.collect_matches(facts, 0, &Bindings::new(), &mut results);
        results.sort_by(|a, b| a.1.cmp(&b.1));
        results.dedup_by(|a, b| a.1 == b.1);
        results
    }

    fn collect_matches(
        &self,
        facts: &[Fact],
        pattern_idx: usize,
        bindings: &Bindings,
        out: &mut Vec<(Bindings, Vec<Fact>)>,
    ) {
        if pattern_idx >= self.matches.len() {
            if self.check_negations(facts, bindings) {
                out.push((bindings.clone(), Vec::new()));
            }
            return;
        }

        let pattern = &self.matches[pattern_idx];
        for fact in facts {
            if let Some(new_bindings) = match_pattern(pattern, fact, bindings) {
                let merged = match merge_bindings(bindings, &new_bindings) {
                    Some(m) => m,
                    None => continue,
                };
                let mut prev_len = out.len();
                self.collect_matches(facts, pattern_idx + 1, &merged, out);
                while out.len() > prev_len {
                    out[prev_len].1.insert(0, fact.clone());
                    prev_len += 1;
                }
            }
        }
    }

    fn find_match_from(
        &self,
        facts: &[Fact],
        pattern_idx: usize,
        bindings: &Bindings,
    ) -> Option<(Bindings, Vec<Fact>)> {
        if pattern_idx >= self.matches.len() {
            if self.check_negations(facts, bindings) {
                return Some((bindings.clone(), Vec::new()));
            }
            return None;
        }

        let pattern = &self.matches[pattern_idx];
        for fact in facts {
            if let Some(new_bindings) = match_pattern(pattern, fact, bindings) {
                let merged = match merge_bindings(bindings, &new_bindings) {
                    Some(m) => m,
                    None => continue,
                };
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
    pub(crate) fn check_negations(&self, facts: &[Fact], bindings: &Bindings) -> bool {
        for neg_pat in &self.not_matches {
            for fact in facts {
                if let Some(new_bindings) = match_pattern(neg_pat, fact, bindings) {
                    if merge_bindings(bindings, &new_bindings).is_some() {
                        return false;
                    }
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
/// An empty binding (from a skipped optional) can be overwritten by a non-empty one.
fn merge_bindings(a: &Bindings, b: &Bindings) -> Option<Bindings> {
    let mut result = a.clone();
    for (name, values) in b {
        if let Some((_, existing)) = result.iter_mut().find(|(n, _)| n == name) {
            if existing.is_empty() {
                *existing = values.clone();
            } else if values.is_empty() {
            } else if existing != values {
                return None;
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
    use crate::engine::Engine;

    fn pat(s: &str) -> Pattern {
        if let Some(p) = crate::fact::parse_pattern_from_str(s) {
            p
        } else {
            vec![crate::fact::Pat::Atom(s.to_string())]
        }
    }

    fn fact(s: &str) -> Fact {
        s.split_whitespace().map(|w| w.to_string()).collect()
    }

    /// Regression test: when a variable is bound by an earlier pattern,
    /// and a later pattern has multiple candidate facts, a conflicting
    /// binding from one candidate must not abort the entire search.
    #[test]
    fn find_match_skips_conflicting_binding() {
        let rule = Rule {
            name: "go_north".into(),
            matches: vec![
                pat("n"),
                pat("here $h"),
                pat("north_of $g $h"),
            ],
            not_matches: vec![],
            consumes: vec![0, 1],
            effects: vec![pat("here $g")],
        };

        let facts = vec![
            fact("n"),
            fact("here frontroom"),
            fact("north_of kitchen bedroom"),
            fact("north_of bedroom frontroom"),
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
                pat("a $x"),
                pat("b $x"),
            ],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![],
        };

        let facts = vec![
            fact("a hello"),
            fact("b hello"),
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
                pat("a $x"),
                pat("b $x"),
            ],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![],
        };

        let facts = vec![
            fact("a hello"),
            fact("b world"),
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
                pat("a $x"),
            ],
            not_matches: vec![
                pat("b $x"),
            ],
            consumes: vec![],
            effects: vec![],
        };

        let facts = vec![
            fact("a hello"),
            fact("b hello"),
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
                pat("a $x"),
            ],
            not_matches: vec![
                pat("b $x"),
            ],
            consumes: vec![],
            effects: vec![],
        };

        let facts = vec![
            fact("a hello"),
            fact("b world"),
        ];

        let result = rule.find_match(&facts);
        assert!(result.is_some());
    }

    /// Regression: a rule must fire once per matching fact.
    #[test]
    fn find_all_matches_one_per_fact() {
        let rule = Rule {
            name: "north_of_south".into(),
            matches: vec![pat("north_of $a $b")],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![pat("south_of $b $a")],
        };

        let facts = vec![
            fact("north_of kitchen bedroom"),
            fact("north_of frontroom kitchen"),
        ];

        let matches = rule.find_all_matches(&facts);
        assert_eq!(matches.len(), 2, "should find one match per north_of fact");

        let effects: Vec<String> = matches
            .iter()
            .map(|(bindings, _)| {
                let applied = rule.apply(bindings);
                applied
                    .iter()
                    .map(|f| f.join(","))
                    .collect::<Vec<_>>()
                    .join(";")
            })
            .collect();
        assert!(effects.iter().any(|e| e == "south_of,bedroom,kitchen"));
        assert!(effects.iter().any(|e| e == "south_of,kitchen,frontroom"));
    }

    // ===== Optional binding rule tests =====

    /// Rule with optional + validation: works when optional present.
    #[test]
    fn rule_optional_validation_present() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![
                pat("sentence $( $a1 )? $thing is $rel $( $prep )? $( $a2 )? $other"),
                pat("article $a1"),
                pat("article $a2"),
                pat("preposition $prep"),
            ],
            not_matches: vec![],
            consumes: vec![0],
            effects: vec![pat("rel $thing $rel $prep $other")],
        };
        let facts = vec![
            fact("sentence the cow is over from the moon"),
            fact("article the"),
            fact("preposition from"),
        ];
        let result = rule.find_match(&facts);
        assert!(result.is_some(), "should match when optional is present");
        let (bindings, _) = result.unwrap();
        assert_eq!(bindings.iter().find(|(n, _)| n == "prep").unwrap().1[0], "from");
    }

    /// Rule with optional + validation: works when optional absent.
    #[test]
    fn rule_optional_validation_absent() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![
                pat("sentence $( $a1 )? $thing is $rel $( $prep )? $( $a2 )? $other"),
                pat("article $a1"),
                pat("article $a2"),
                pat("preposition $prep"),
            ],
            not_matches: vec![],
            consumes: vec![0],
            effects: vec![pat("rel $thing $rel $other")],
        };
        let facts = vec![
            fact("sentence cow is over moon"),
            fact("article the"),
            fact("preposition from"),
        ];
        let result = rule.find_match(&facts);
        assert!(result.is_some(), "should match when optional is absent");
        let (bindings, _) = result.unwrap();
        assert!(bindings.iter().find(|(n, _)| n == "prep").unwrap().1.is_empty());
        assert!(bindings.iter().find(|(n, _)| n == "a1").unwrap().1.is_empty());
        assert!(bindings.iter().find(|(n, _)| n == "a2").unwrap().1.is_empty());
    }

    /// Rule with optional + validation: absent optional does NOT bind from unrelated facts.
    #[test]
    fn rule_optional_absent_does_not_leak() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![
                pat("sentence $( $a1 )? $thing is $rel $( $prep )? $( $a2 )? $other"),
                pat("article $a1"),
                pat("article $a2"),
                pat("preposition $prep"),
            ],
            not_matches: vec![],
            consumes: vec![0],
            effects: vec![pat("rel $thing $rel $prep $other")],
        };
        let facts = vec![
            fact("sentence cow is over moon"),
            fact("article the"),
            fact("preposition from"),
        ];
        let result = rule.find_match(&facts);
        assert!(result.is_some(), "should match even with unrelated preposition fact");
        let (bindings, _) = result.unwrap();
        assert!(bindings.iter().find(|(n, _)| n == "prep").unwrap().1.is_empty(),
            "skipped optional var must not be bound by unrelated facts");
    }
    // ===== Rest pattern tests =====

    /// Rule with rest pattern: `a ..?rest` matching `a b c` should bind rest to ["b", "c"].
    #[test]
    fn rule_with_rest_pattern() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![pat("a ..?rest")],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![],
        };
        let facts = vec![fact("a b c")];
        let result = rule.find_match(&facts);
        assert!(result.is_some());
        let (bindings, _) = result.unwrap();
        assert_eq!(
            bindings.iter().find(|(n, _)| n == "rest").unwrap().1,
            vec!["b", "c"]
        );
    }

    /// Rule with optional and rest: `a $( $x )? ..?rest` matching `a b c d`
    /// With skip-first semantics, optional is skipped and rest captures all.
    #[test]
    fn rule_with_optional_and_rest() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![pat("a $( $x )? ..?rest")],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![],
        };
        let facts = vec![fact("a b c d")];
        let result = rule.find_match(&facts);
        assert!(result.is_some());
        let (bindings, _) = result.unwrap();
        assert!(
            bindings.iter().find(|(n, _)| n == "x").unwrap().1.is_empty(),
            "optional skipped, x should be empty"
        );
        assert_eq!(
            bindings.iter().find(|(n, _)| n == "rest").unwrap().1,
            vec!["b", "c", "d"]
        );
    }

    /// Rule with optional skipped and rest: `a $( $x )? ..?rest` matching `a b c`
    /// should bind x to [], rest to ["b", "c"] (optional skipped, rest captures all).
    #[test]
    fn rule_with_optional_skipped_and_rest() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![pat("a $( $x )? ..?rest")],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![],
        };
        let facts = vec![fact("a b c")];
        let result = rule.find_match(&facts);
        assert!(result.is_some());
        let (bindings, _) = result.unwrap();
        assert!(
            bindings.iter().find(|(n, _)| n == "x").unwrap().1.is_empty(),
            "skipped optional should leave empty binding"
        );
        assert_eq!(
            bindings.iter().find(|(n, _)| n == "rest").unwrap().1,
            vec!["b", "c"]
        );
    }

    // ===== Engine integration tests =====

    /// Rule with consumes=[0] should remove the matched fact from the engine.
    #[test]
    fn rule_consumes_matched_facts() {
        let mut engine = Engine::new();
        engine.assert(vec!["rule".into(), "test".into(), "-a $x".into(), "result $x".into()]);
        engine.assert(fact("a hello"));
        engine.run_fixedpoint();
        assert!(
            !engine.facts().iter().any(|f| f == &fact("a hello")),
            "consumed fact should be removed"
        );
        assert!(
            engine.facts().iter().any(|f| f == &fact("result hello")),
            "effect fact should be present"
        );
    }

    /// If a consumed fact was already removed by a prior rule, the rule should not fire.
    #[test]
    fn rule_does_not_fire_when_consumed_fact_gone() {
        let mut engine = Engine::new();
        engine.assert(vec!["rule".into(), "first".into(), "-a $x".into(), "first_result $x".into()]);
        engine.assert(vec!["rule".into(), "second".into(), "-a $x".into(), "second_result $x".into()]);
        engine.assert(fact("a hello"));
        engine.run_fixedpoint();
        assert!(
            engine.facts().iter().any(|f| f == &fact("first_result hello")),
            "first rule should fire"
        );
        assert!(
            !engine.facts().iter().any(|f| f == &fact("second_result hello")),
            "second rule should not fire because its consumed fact is gone"
        );
    }

    /// Two facts matching the same pattern should cause two firings.
    #[test]
    fn rule_fires_once_per_matching_fact() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![pat("a $x")],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![pat("result $x")],
        };
        let facts = vec![fact("a hello"), fact("a world")];
        let matches = rule.find_all_matches(&facts);
        assert_eq!(matches.len(), 2, "should find one match per fact");
    }

    // ===== Negation tests =====

    /// Rule with two not_matches patterns should be blocked if either matches.
    #[test]
    fn rule_with_multiple_negations() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![pat("a $x")],
            not_matches: vec![pat("b $x"), pat("c $x")],
            consumes: vec![],
            effects: vec![],
        };
        // Both negations match — blocked
        let facts1 = vec![fact("a hello"), fact("b hello"), fact("c hello")];
        assert!(rule.find_match(&facts1).is_none());
        // Only first negation matches — blocked
        let facts2 = vec![fact("a hello"), fact("b hello")];
        assert!(rule.find_match(&facts2).is_none());
        // Only second negation matches — blocked
        let facts3 = vec![fact("a hello"), fact("c hello")];
        assert!(rule.find_match(&facts3).is_none());
        // Neither negation matches — fires
        let facts4 = vec![fact("a hello")];
        assert!(rule.find_match(&facts4).is_some());
    }

    /// Negation pattern with optional should still block correctly.
    #[test]
    fn rule_with_negation_and_optional() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![pat("a $( $x )? $y")],
            not_matches: vec![pat("b $y")],
            consumes: vec![],
            effects: vec![],
        };
        // Negation matches — blocked
        let facts1 = vec![fact("a hello world"), fact("b world")];
        assert!(rule.find_match(&facts1).is_none());
        // Negation doesn't match — fires
        let facts2 = vec![fact("a hello world")];
        assert!(rule.find_match(&facts2).is_some());
    }

    // ===== Apply / effects tests =====

    /// Rule with effects should produce the right output facts from bindings.
    #[test]
    fn rule_apply_produces_correct_facts() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![pat("a $x $y")],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![pat("result $y $x")],
        };
        let facts = vec![fact("a hello world")];
        let result = rule.find_match(&facts);
        assert!(result.is_some());
        let (bindings, _) = result.unwrap();
        let effects = rule.apply(&bindings);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0], fact("result world hello"));
    }

    // ===== Deduplication test =====

    /// find_all_matches should not return duplicate matches.
    #[test]
    fn rule_find_all_matches_deduplicates() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![pat("a $x")],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![],
        };
        // Two identical facts should produce only one unique match
        let facts = vec![fact("a hello"), fact("a hello")];
        let matches = rule.find_all_matches(&facts);
        assert_eq!(matches.len(), 1, "should deduplicate identical matches");
    }

    // ===== Edge-case rule configurations =====

    /// Rule with no effects and no consumes should still fire (side-effect free matching).
    #[test]
    fn rule_with_empty_effects() {
        let rule = Rule {
            name: "test".into(),
            matches: vec![pat("a $x")],
            not_matches: vec![],
            consumes: vec![],
            effects: vec![],
        };
        let facts = vec![fact("a hello")];
        let result = rule.find_match(&facts);
        assert!(result.is_some(), "rule with no effects and no consumes should still match");
    }

    /// Rule with consumes but no effects should remove facts.
    #[test]
    fn rule_with_only_consumes() {
        let mut engine = Engine::new();
        engine.assert(vec!["rule".into(), "test".into(), "-a $x".into(), "".into()]);
        engine.assert(fact("a hello"));
        engine.run_fixedpoint();
        assert!(
            !engine.facts().iter().any(|f| f == &fact("a hello")),
            "consumed fact should be removed even without effects"
        );
    }

    // ===== merge_bindings tests =====

    /// Merging bindings with different values for same key should return None.
    #[test]
    fn merge_bindings_conflict() {
        let a: Bindings = vec![("x".into(), vec!["hello".into()])];
        let b: Bindings = vec![("x".into(), vec!["world".into()])];
        assert!(merge_bindings(&a, &b).is_none());
    }

    /// Merging with empty binding should keep the non-empty one.
    #[test]
    fn merge_bindings_empty_overwrites() {
        let a: Bindings = vec![("x".into(), vec!["hello".into()])];
        let b: Bindings = vec![("x".into(), vec![])];
        let result = merge_bindings(&a, &b);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().iter().find(|(n, _)| n == "x").unwrap().1,
            vec!["hello"]
        );
    }

    /// Merging with same values should succeed.
    #[test]
    fn merge_bindings_same_values() {
        let a: Bindings = vec![("x".into(), vec!["hello".into()])];
        let b: Bindings = vec![("x".into(), vec!["hello".into()])];
        let result = merge_bindings(&a, &b);
        assert!(result.is_some());
        assert_eq!(
            result.unwrap().iter().find(|(n, _)| n == "x").unwrap().1,
            vec!["hello"]
        );
    }
}
