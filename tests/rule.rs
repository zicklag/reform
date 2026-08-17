use reform::Arg;
use reform::rule::{
    ArgTemplate, BindValue, Bindings, Body, BodyChunk, PatternFact, RepeatBlock, RepetitionKind,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn fact(args: &[&str]) -> reform::Fact {
    reform::Fact(args.iter().map(|s| Arg::from(*s)).collect())
}

// ---------------------------------------------------------------------------
// Bindings::merge
// ---------------------------------------------------------------------------

#[test]
fn bindings_merge_scalar() {
    let mut a = Bindings::new();
    a.bind_scalar("x", Arg::from("1"));
    let mut b = Bindings::new();
    b.bind_scalar("x", Arg::from("1"));
    assert!(a.merge(&b));
    assert_eq!(a.get("x"), Some(&BindValue::One(Arg::from("1"))));
}

#[test]
fn pattern_duplicate_placeholder_conflict() {
    // Pattern `$x $x` matching fact `a b`: first `$x` binds to `a`, second
    // `$x` tries to bind to `b` but bind_scalar returns false (conflict).
    use reform::rule::PatternItem;
    let pf = reform::parser::pattern("$x $x").unwrap();
    let PatternItem::Fact(pf) = &pf[0] else {
        panic!("expected Fact pattern")
    };
    let f = fact(&["a", "b"]);
    let matches = pf.matches_fact(&f);
    assert!(
        matches.is_none(),
        "conflicting placeholders should not match"
    );
}

#[test]
fn pattern_duplicate_placeholder_matches() {
    // Pattern `$x $x` matching fact `a a`: both bind to the same value.
    use reform::rule::PatternItem;
    let pf = reform::parser::pattern("$x $x").unwrap();
    let PatternItem::Fact(pf) = &pf[0] else {
        panic!("expected Fact pattern")
    };
    let f = fact(&["a", "a"]);
    let matches = pf.matches_fact(&f);
    assert!(
        matches.is_some(),
        "same placeholder with same value should match"
    );
}

#[test]
fn bindings_merge_scalar_conflict() {
    let mut a = Bindings::new();
    a.bind_scalar("x", Arg::from("1"));
    let mut b = Bindings::new();
    b.bind_scalar("x", Arg::from("2"));
    assert!(!a.merge(&b));
}

#[test]
fn bindings_merge_many_same() {
    let mut a = Bindings::new();
    a.map
        .insert("x".to_string(), BindValue::Many(vec![BindValue::One(Arg::from("1"))]));
    let mut b = Bindings::new();
    b.map
        .insert("x".to_string(), BindValue::Many(vec![BindValue::One(Arg::from("1"))]));
    assert!(a.merge(&b));
}

#[test]
fn bindings_merge_many_new() {
    let mut a = Bindings::new();
    let mut b = Bindings::new();
    b.map
        .insert("x".to_string(), BindValue::Many(vec![BindValue::One(Arg::from("1"))]));
    assert!(a.merge(&b));
    assert_eq!(a.get("x"), Some(&BindValue::Many(vec![BindValue::One(Arg::from("1"))])));
}

#[test]
fn bindings_merge_many_into_scalar_fails() {
    let mut a = Bindings::new();
    a.bind_scalar("x", Arg::from("1"));
    let mut b = Bindings::new();
    b.map
        .insert("x".to_string(), BindValue::Many(vec![BindValue::One(Arg::from("1"))]));
    assert!(!a.merge(&b));
}

#[test]
fn bindings_merge_many_different_fails() {
    let mut a = Bindings::new();
    a.map
        .insert("x".to_string(), BindValue::Many(vec![BindValue::One(Arg::from("1"))]));
    let mut b = Bindings::new();
    b.map
        .insert("x".to_string(), BindValue::Many(vec![BindValue::One(Arg::from("2"))]));
    assert!(!a.merge(&b));
}

// ---------------------------------------------------------------------------
// matches_fact convenience method
// ---------------------------------------------------------------------------

#[test]
fn pattern_fact_matches_fact() {
    let pf = PatternFact::new(false, false, vec![ArgTemplate::Literal(Arg::from("a"))]);
    let f = fact(&["a"]);
    assert!(pf.matches_fact(&f).is_some());
    let f2 = fact(&["b"]);
    assert!(pf.matches_fact(&f2).is_none());
}

// ---------------------------------------------------------------------------
// render_chunks Many path
// ---------------------------------------------------------------------------

#[test]
fn render_chunks_many_binding() {
    let b = Body(vec![BodyChunk::Placeholder("x".to_string())]);
    let mut bindings = Bindings::new();
    bindings.map.insert(
        "x".to_string(),
        BindValue::Many(vec![BindValue::One(Arg::from("a")), BindValue::One(Arg::from("b"))]),
    );
    let s = b.render(&bindings);
    assert_eq!(s, "a b");
}

// ---------------------------------------------------------------------------
// render_repeat edge cases
// ---------------------------------------------------------------------------

#[test]
fn render_repeat_empty_drivers() {
    // A repeat block with no list-bound placeholders should render nothing.
    let r = RepeatBlock { kind: RepetitionKind::ZeroOrMore, greedy: false, chunks: vec![BodyChunk::Text("x".to_string())] };
    let b = Body(vec![BodyChunk::Repeat(r)]);
    let bindings = Bindings::new();
    let s = b.render(&bindings);
    assert_eq!(s, "");
}

// ---------------------------------------------------------------------------
// collect_ph_names with nested repeats
// ---------------------------------------------------------------------------

#[test]
fn collect_ph_names_nested_repeat() {
    let inner = BodyChunk::Repeat(RepeatBlock { kind: RepetitionKind::ZeroOrMore, greedy: false, chunks: vec![BodyChunk::Placeholder("y".to_string())] });
    let outer = BodyChunk::Repeat(RepeatBlock { kind: RepetitionKind::ZeroOrMore, greedy: false, chunks: vec![BodyChunk::Placeholder("x".to_string()), inner] });
    let b = Body(vec![outer]);
    let s = b.render(&Bindings::new());
    assert_eq!(s, "");
}

// ---------------------------------------------------------------------------
// match_fact_repetition multi-fact rejection
// ---------------------------------------------------------------------------

#[test]
fn match_fact_repetition_multi_fact_rejected() {
    // A pattern with `$( fact1\nfact2 )*` should produce no matches.
    let p = reform::parser::pattern("$( a\nb )*").unwrap();
    let facts = vec![fact(&["a"]), fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert!(matches.is_empty());
}

// ---------------------------------------------------------------------------
// match_reps at_least_one with zero inner match
// ---------------------------------------------------------------------------

#[test]
fn match_reps_at_least_one_zero_inner() {
    // `+` repetition where inner matches zero args: should still try rest.
    let p = reform::parser::pattern("$( $x )+ y").unwrap();
    let facts = vec![fact(&["y"])];
    let matches = p.find_matches(&facts);
    // `+` requires at least one match, so with no `$x` before `y`, no match.
    assert!(matches.is_empty());
}

// -- render_chunks placeholder with no binding -------------------------------

#[test]
fn render_chunks_placeholder_no_binding() {
    let b = Body(vec![BodyChunk::Placeholder("x".to_string())]);
    let bindings = Bindings::new();
    let s = b.render(&bindings);
    assert_eq!(s, "");
}

// -- render_repeat mismatch lengths ------------------------------------------

#[test]
fn render_repeat_mismatched_drivers() {
    let r = RepeatBlock { kind: RepetitionKind::ZeroOrMore, greedy: false, chunks: vec![
        BodyChunk::Placeholder("x".to_string()),
        BodyChunk::Placeholder("y".to_string()),
    ] };
    let b = Body(vec![BodyChunk::Repeat(r)]);
    let mut bindings = Bindings::new();
    bindings.map.insert(
        "x".to_string(),
        BindValue::Many(vec![BindValue::One(Arg::from("1")), BindValue::One(Arg::from("2"))]),
    );
    bindings
        .map
        .insert("y".to_string(), BindValue::Many(vec![BindValue::One(Arg::from("a"))]));
    let s = b.render(&bindings);
    assert_eq!(s, "", "mismatched drivers should render nothing");
}

// -- match_fact_repetition Optional with match --------------------------------

#[test]
fn match_fact_repetition_optional_with_match() {
    let p = reform::parser::pattern("$( a )? b").unwrap();
    let facts = vec![fact(&["a"]), fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1, "optional should match when fact present");
}

// -- match_fact_repetition Optional without match -----------------------------

#[test]
fn match_fact_repetition_optional_without_match() {
    let p = reform::parser::pattern("$( a )? b").unwrap();
    let facts = vec![fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1, "optional should match when fact absent");
}

// -- match_fact_repetition OneOrMore with no matches --------------------------

#[test]
fn match_fact_repetition_one_or_more_no_match() {
    let p = reform::parser::pattern("$( a )+ b").unwrap();
    let facts = vec![fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert!(matches.is_empty(), "+ should not match when no facts");
}

// -- match_fact_repetition wildcard arm (unreachable, defensive) -------------

#[test]
fn match_fact_repetition_wildcard_arm() {
    // The wildcard `_ => vec![]` arm in match_fact_repetition is unreachable
    // since all RepetitionKind variants are covered. Test defensively by
    // constructing a pattern that triggers the Optional-with-no-match path
    // (which goes through the `want_absent` branch, not the wildcard).
    let p = reform::parser::pattern("$( a )? b").unwrap();
    let facts = vec![fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

// -- match_fact_repetition filter_map None arm -------------------------------

#[test]
fn match_fact_repetition_filter_map_none() {
    // When a list-bound placeholder has no matching value in a matched fact,
    // the filter_map returns None. This happens when a fact matches the
    // pattern but doesn't bind the placeholder (e.g., literal-only pattern).
    let p = reform::parser::pattern("$( a )* b").unwrap();
    let facts = vec![fact(&["a"]), fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

// -- render_repeat empty driver fallback -------------------------------------

#[test]
fn render_repeat_empty_driver_fallback() {
    // When the first driver's binding is not a Many list, n defaults to 0.
    let r = RepeatBlock { kind: RepetitionKind::ZeroOrMore, greedy: false, chunks: vec![BodyChunk::Placeholder("x".to_string())] };
    let b = Body(vec![BodyChunk::Repeat(r)]);
    let mut bindings = Bindings::new();
    bindings.bind_scalar("x", Arg::from("val"));
    let s = b.render(&bindings);
    assert_eq!(s, "", "scalar binding should not drive iteration");
}

// -- match_reps at_least_one with zero inner match (guard path) --------------

#[test]
fn match_reps_at_least_one_zero_inner_guard() {
    // `+` repetition where inner matches zero args: the guard at line 540-542
    // should still try the rest match.
    let p = reform::parser::pattern("$( $x )+ y").unwrap();
    let facts = vec![fact(&["y"])];
    let matches = p.find_matches(&facts);
    // `+` requires at least one match, so with no `$x` before `y`, no match.
    assert!(matches.is_empty());
}

// -- match_reps at_least_one with zero inner match (line 540-542) ------------

#[test]
fn match_reps_at_least_one_zero_inner_guard_path() {
    // `+` repetition where inner matches zero args: the guard at line 540-542
    // should still try the rest match. Pattern: `$( $x )+ y` with facts `[y]`.
    // Inner `$x` matches zero args at position 0, so at_least_one=true triggers
    // the guard to try matching `y` against the rest.
    let p = reform::parser::pattern("$( $x )+ y").unwrap();
    let facts = vec![fact(&["y"])];
    let matches = p.find_matches(&facts);
    // `+` requires at least one match, so with no `$x` before `y`, no match.
    assert!(matches.is_empty());
}

// -- match_fact_repetition Optional with match (line 638) --------------------

#[test]
fn match_fact_repetition_optional_with_match_line_638() {
    let p = reform::parser::pattern("$( a )? b").unwrap();
    let facts = vec![fact(&["a"]), fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

// -- match_fact_repetition wildcard arm (line 642) ---------------------------

#[test]
fn match_fact_repetition_wildcard_arm_line_642() {
    // The wildcard `_ => vec![]` arm is unreachable since all RepetitionKind
    // variants are covered. Test the Optional-with-no-match path instead.
    let p = reform::parser::pattern("$( a )? b").unwrap();
    let facts = vec![fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

// -- match_fact_repetition filter_map None arm (line 660) --------------------

#[test]
fn match_fact_repetition_filter_map_none_line_660() {
    // When a list-bound placeholder has no matching value in a matched fact,
    // the filter_map returns None. Use a literal-only pattern inside a rep.
    let p = reform::parser::pattern("$( a )* b").unwrap();
    let facts = vec![fact(&["a"]), fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

// -- render_repeat driver not Many fallback (line 745) -----------------------

#[test]
fn render_repeat_driver_not_many_fallback() {
    // When the first driver's binding is not a Many list, n defaults to 0.
    let r = RepeatBlock { kind: RepetitionKind::ZeroOrMore, greedy: false, chunks: vec![BodyChunk::Placeholder("x".to_string())] };
    let b = Body(vec![BodyChunk::Repeat(r)]);
    let mut bindings = Bindings::new();
    bindings.bind_scalar("x", Arg::from("val"));
    let s = b.render(&bindings);
    assert_eq!(s, "", "scalar binding should not drive iteration");
}

// ---------------------------------------------------------------------------
// Fact-level repetition Optional paths (match_fact_repetition)
// ---------------------------------------------------------------------------
// These must use a *multi-line* pattern so the `$( … )?` sits on its own
// line and parses as a `PatternItem::FactRepetition` (a single-line
// `$( a )? b` instead parses as an *arg-level* repeated-args and never
// reaches `match_fact_repetition`).

/// `$( a )?` (fact-level optional) with a matching fact present: takes the
/// first match (the `Optional if !matched_idx.is_empty()` arm).
#[test]
fn fact_rep_optional_with_match() {
    let p = reform::parser::pattern("$( a )?\nb").unwrap();
    let facts = vec![fact(&["a"]), fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

/// `$( a )?` (fact-level optional) with no matching fact: takes nothing
/// (the `Optional => vec![]` arm) and falls through to `want_absent`.
#[test]
fn fact_rep_optional_without_match() {
    let p = reform::parser::pattern("$( a )?\nb").unwrap();
    let facts = vec![fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

/// A `+` fact-level repetition whose inner is a `*` arg-repetition that can
/// match zero args. The zero-width guard in `match_reps` (the `mid == start`
/// branch with `at_least_one`) treats the zero match as the single required
/// iteration.
#[test]
fn match_reps_plus_with_zero_width_inner() {
    // `prefix` makes the whole line a single Fact (not a top-level fact
    // repetition); `$( $( $x )* )+` is a repeated-args whose inner `*` can
    // match zero args. Matched against `prefix` (nothing after it), the `+`
    // still succeeds via one zero-width iteration.
    let p = reform::parser::pattern("prefix $( $( $x )* )+").unwrap();
    let facts = vec![fact(&["prefix"])];
    let matches = p.find_matches(&facts);
    assert_eq!(
        matches.len(),
        1,
        "+ with zero-width inner should match once"
    );
}

/// Two fact-level repetitions sharing a placeholder `$x` (both at the `*`
/// nesting context, so validation accepts them). When the second repetition
/// matches, the accumulated bindings already hold `$x` as a `Many` list, so
/// the per-fact `bf.get(name)` returns `Many` — exercising the `_ => None`
/// arm of the list-collection `filter_map` and the empty-list branch of
/// `if !list.is_empty()`.
#[test]
fn fact_rep_shared_placeholder_many_in_filter_map() {
    let p = reform::parser::pattern("$( a $x )*\n$( b $x )*").unwrap();
    let facts = vec![fact(&["a", "1"]), fact(&["b", "2"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

/// A `*` arg-repetition whose inner `$( $x )*` matches zero args exercises
/// the zero-width no-op branch of `match_reps` with `at_least_one == false`
/// (the `mid == start` guard that skips both the extend and the recursive
/// call). The match still succeeds via the `*`'s zero-iteration path.
#[test]
fn match_reps_star_with_zero_width_inner() {
    let p = reform::parser::pattern("prefix $( $( $x )* )*").unwrap();
    let facts = vec![fact(&["prefix"])];
    let matches = p.find_matches(&facts);
    assert_eq!(
        matches.len(),
        1,
        "* with zero-width inner should match once"
    );
}

/// A fact-level `+` repetition with no matching facts takes nothing and is
/// neither `want_present` nor `want_absent` (OneOrMore is not optional), so
/// both `match_fact_repetition` branches are skipped and the rest of the
/// pattern still matches.
#[test]
fn fact_rep_plus_with_no_match_skips_both_branches() {
    let p = reform::parser::pattern("$( a )+\nb").unwrap();
    let facts = vec![fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert!(
        matches.is_empty(),
        "+ with no matching fact should not match"
    );
}

/// A fact-level `?` constraint whose inner fact holds several top-level
/// placeholders, exercising the `must_match` conversion loop in
/// `match_fact_repetition` across all three of its branches at once.
///
/// With input `an a is b` against `parse $( $a1 )? $x is $( $a2 )? $y`:
/// - `$a1` is bound to a non-empty list `[an]` (the arg-level `?` matched
///   one), which makes `must_match` true and drives the conversion loop.
/// - `$a2` is bound to an *empty* list `[]` (the arg-level `?` matched
///   zero), so the outer `if let Some(Many(list))` succeeds but the inner
///   `if let Some(v) = list.first()` fails — its else region is hit.
/// - `$a3` appears only inside the fact-level `?`, so it is unbound
///   (`None`), and the outer `if let Some(Many)` fails — its else region is
///   hit.
///
/// `$a1` converts to `One(an)`; the 5-arg inner `$a1 $a2 $a3 is article`
/// cannot match the 3-arg `an is article`, so the constraint is not
/// satisfied and the pattern matches nothing.
#[test]
fn fact_rep_constraint_conversion_branches() {
    let p = reform::parser::pattern(
        "parse $( $a1 )? $x is $( $a2 )? $y\n$( $a1 $a2 $a3 is article )?",
    )
    .unwrap();
    let facts = vec![
        fact(&["an", "is", "article"]),
        fact(&["parse", "an", "a", "is", "b"]),
    ];
    let matches = p.find_matches(&facts);
    assert!(
        matches.is_empty(),
        "constraint with an empty/unbound placeholder and no matching fact should not match"
    );
}


// ---------------------------------------------------------------------------
// removed_facts / matched_facts — re-matching with existing Many bindings
// ---------------------------------------------------------------------------

/// `removed_facts` with a `$( $words )+` (OneOrMore) pattern must only remove
/// the fact that was actually matched, not every fact that independently
/// matches the pattern. Before the fix, re-matching created fresh empty
/// list bindings, so both facts matched and both were removed.
#[test]
fn removed_facts_one_or_more_only_matched() {
    use reform::rule::Rule;
    let rule = Rule::parse(&[
        "rule", "split", "- parse $( $words )+", "statement $( $words )+",
    ])
    .unwrap();
    let facts = vec![
        fact(&["parse", "alpha"]),
        fact(&["parse", "beta"]),
    ];
    let matches = rule.find_matches_detailed_grouped(&facts);
    assert_eq!(matches.len(), 2);
    // First match binds alpha — only that fact should be removed.
    let (_, groups) = &matches[0];
    let removed = rule.removed_facts(&facts, groups);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], fact(&["parse", "alpha"]));
    // Second match binds beta — only that fact should be removed.
    let (_, groups) = &matches[1];
    let removed = rule.removed_facts(&facts, groups);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], fact(&["parse", "beta"]));
}

/// Same scenario with `$( $words )*` (ZeroOrMore) — exercises the
/// `has_existing` + `ZeroOrMore` path in `match_args` and the
/// `!at_least_one` branch of `match_reps_constrained`.
#[test]
fn removed_facts_zero_or_more_only_matched() {
    use reform::rule::Rule;
    let rule = Rule::parse(&[
        "rule", "split", "- parse $( $words )*", "statement $( $words )*",
    ])
    .unwrap();
    let facts = vec![
        fact(&["parse", "alpha"]),
        fact(&["parse", "beta"]),
    ];
    let matches = rule.find_matches_detailed_grouped(&facts);
    assert_eq!(matches.len(), 2);
    let (_, groups) = &matches[0];
    let removed = rule.removed_facts(&facts, groups);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], fact(&["parse", "alpha"]));
    let (_, groups) = &matches[1];
    let removed = rule.removed_facts(&facts, groups);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], fact(&["parse", "beta"]));
}

/// `$( $a )? $x` (Optional) where `$a` binds to a non-empty list — exercises
/// the `has_existing` + `Optional` path, including the zero-iteration
/// `bindings_compatible` check.
#[test]
fn removed_facts_optional_only_matched() {
    use reform::rule::Rule;
    let rule = Rule::parse(&[
        "rule", "split", "- parse $( $a )? $x", "result $x",
    ])
    .unwrap();
    let facts = vec![
        fact(&["parse", "alpha", "beta"]),
        fact(&["parse", "gamma", "delta"]),
    ];
    let matches = rule.find_matches_detailed_grouped(&facts);
    assert_eq!(matches.len(), 2);
    let (_, groups) = &matches[0];
    let removed = rule.removed_facts(&facts, groups);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], fact(&["parse", "alpha", "beta"]));
    let (_, groups) = &matches[1];
    let removed = rule.removed_facts(&facts, groups);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], fact(&["parse", "gamma", "delta"]));
}

/// A nested `$( $( $x )* )+` (OneOrMore with a zero-width inner `*`) pattern
/// during re-matching verifies that `removed_facts` only removes the matched
/// fact. The zero-width inner doesn't trigger `has_existing` (no direct
/// Placeholder in the outer repetition), so this exercises the normal path.
#[test]
fn removed_facts_nested_zero_width_inner() {
    use reform::rule::Rule;
    let rule = Rule::parse(&[
        "rule", "split", "- prefix $( $( $x )* )+", "result",
    ])
    .unwrap();
    let facts = vec![
        fact(&["prefix", "alpha"]),
        fact(&["prefix", "beta"]),
    ];
    let matches = rule.find_matches_detailed_grouped(&facts);
    assert_eq!(matches.len(), 2);
    let (_, groups) = &matches[0];
    let removed = rule.removed_facts(&facts, groups);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], fact(&["prefix", "alpha"]));
    let (_, groups) = &matches[1];
    let removed = rule.removed_facts(&facts, groups);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], fact(&["prefix", "beta"]));
}

/// A `-` inside a fact-level repetition deletes exactly the facts that
/// repetition consumed. `$( - player has $item )*` consumes the two `player
/// has` facts; the trailing `keep` fact is consumed by a sibling item and must
/// NOT be deleted. Before the fix, `removed_facts` ignored `FactRepetition`
/// items entirely, so the `-` was a silent no-op.
#[test]
fn removed_facts_fact_level_repetition() {
    use reform::rule::Rule;
    let rule = Rule::parse(&[
        "rule", "r", "$( - player has $item )*\nkeep", "( done )",
    ])
    .unwrap();
    let facts = vec![
        fact(&["player", "has", "sword"]),
        fact(&["player", "has", "shield"]),
        fact(&["keep"]),
    ];
    let matches = rule.find_matches_detailed_grouped(&facts);
    assert_eq!(matches.len(), 1);
    let (_, groups) = &matches[0];
    let removed = rule.removed_facts(&facts, groups);
    assert_eq!(removed.len(), 2);
    assert_eq!(removed[0], fact(&["player", "has", "sword"]));
    assert_eq!(removed[1], fact(&["player", "has", "shield"]));
}

/// A `-` inside a fact-level optional deletes the fact when it is present and
/// is a no-op when it is absent. This is the motivating case for the fix. A
/// greedy `??` forces the optional to consume the fact when present; a lazy
/// `?` prefers matching zero facts, so it deletes nothing.
#[test]
fn removed_facts_fact_level_optional_delete() {
    use reform::rule::Rule;
    // Greedy optional: consumes and deletes `foo` when present.
    let rule = Rule::parse(&[
        "rule", "r", "$( - foo )??\nbar", "( done )",
    ])
    .unwrap();
    let facts = vec![fact(&["foo"]), fact(&["bar"])];
    let matches = rule.find_matches_detailed_grouped(&facts);
    assert_eq!(matches.len(), 1);
    let (_, groups) = &matches[0];
    let removed = rule.removed_facts(&facts, groups);
    assert_eq!(removed.len(), 1);
    assert_eq!(removed[0], fact(&["foo"]));
    // Greedy optional, foo absent: matches zero facts, nothing to delete.
    let facts = vec![fact(&["bar"])];
    let matches = rule.find_matches_detailed_grouped(&facts);
    assert_eq!(matches.len(), 1);
    let (_, groups) = &matches[0];
    let removed = rule.removed_facts(&facts, groups);
    assert!(removed.is_empty());
    // Lazy optional with foo present: prefers matching zero facts, so it
    // consumes nothing and deletes nothing.
    let rule = Rule::parse(&[
        "rule", "r", "$( - foo )?\nbar", "( done )",
    ])
    .unwrap();
    let facts = vec![fact(&["foo"]), fact(&["bar"])];
    let matches = rule.find_matches_detailed_grouped(&facts);
    assert_eq!(matches.len(), 1);
    let (_, groups) = &matches[0];
    let removed = rule.removed_facts(&facts, groups);
    assert!(removed.is_empty());
}

/// A native-scalar placeholder (bound by a top-level fact) used inside a
/// fact-level repetition acts as a constraint: it must match the same value in
/// every iteration, and the matcher must NOT collect it into a `Many` list
/// (which would overwrite the scalar binding).
#[test]
fn native_scalar_in_fact_repetition_is_constraint() {
    use reform::rule::Rule;
    // `$prop` is bound by the top-level fact `$prop of car is red`; the
    // fact-level `*` repetition `$( $prop of $x is $old )*` must treat `$prop`
    // as a constraint (only matching facts whose prop is `color`), not collect
    // it into a list.
    let rule = Rule::parse(&[
        "rule", "r", "$prop of car is red\n$( $prop of $x is $old )*\nkeep", "( done )",
    ])
    .unwrap();
    let facts = vec![
        fact(&["color", "of", "car", "is", "red"]),
        fact(&["color", "of", "door", "is", "blue"]),
        fact(&["size", "of", "car", "is", "big"]), // wrong prop: must not match
        fact(&["keep"]),
    ];
    let matches = rule.find_matches_detailed_grouped(&facts);
    assert_eq!(matches.len(), 1);
    let (b, _) = &matches[0];
    // `$prop` stays a scalar `color`, not a `Many` list.
    assert_eq!(b.get("prop"), Some(&reform::rule::BindValue::One(reform::Arg::from("color"))));
    // `$x` is collected across the repetition. The `color of car is red` fact
    // was already consumed by the top-level pattern fact, so only `door`
    // remains; the `size of car is big` fact is excluded because `$prop` is a
    // constraint that must equal `color`.
    assert_eq!(
        b.get("x"),
        Some(&reform::rule::BindValue::Many(vec![
            reform::rule::BindValue::One(reform::Arg::from("door")),
        ]))
    );
}

/// `Rule::find_matches` delegates to `Pattern::find_matches`.
#[test]
fn rule_find_matches_delegates() {
    use reform::rule::Rule;
    let rule = Rule::parse(&[
        "rule", "r", "- parse $( $words )+", "statement $( $words )+",
    ])
    .unwrap();
    let facts = vec![fact(&["parse", "alpha"])];
    let matches = rule.find_matches(&facts);
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].get("words"),
        Some(&BindValue::Many(vec![BindValue::One(Arg::from("alpha"))]))
    );
}

/// `Rule::find_matches_detailed` flattens the per-item groups into a single
/// index list (the engine uses the grouped variant, but the flattened public
/// API must still work).
#[test]
fn rule_find_matches_detailed_flattens() {
    use reform::rule::Rule;
    let rule = Rule::parse(&[
        "rule", "r", "$( - player has $item )*\nkeep", "( done )",
    ])
    .unwrap();
    let facts = vec![
        fact(&["player", "has", "sword"]),
        fact(&["player", "has", "shield"]),
        fact(&["keep"]),
    ];
    let matches = rule.find_matches_detailed(&facts);
    assert_eq!(matches.len(), 1);
    let (_, idxs) = &matches[0];
    assert_eq!(idxs, &vec![0, 1, 2]);
}

// ---------------------------------------------------------------------------
// Greedy vs lazy repetition ordering
// ---------------------------------------------------------------------------

/// `$( $x )?? $y $( $z )?` against `[hello, world]`: greedy `??` prefers one
/// iteration of the first block, so `x=[hello], y=world, z=[]`. Contrast with
/// the lazy `?` version below.
#[test]
fn greedy_optional_prefers_one_iteration() {
    let p = reform::parser::pattern("$( $x )?? $y $( $z )?").unwrap();
    let facts = vec![fact(&["hello", "world"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].get("x"),
        Some(&BindValue::Many(vec![BindValue::One(Arg::from("hello"))]))
    );
    assert_eq!(matches[0].get("y"), Some(&BindValue::One(Arg::from("world"))));
    assert_eq!(
        matches[0].get("z"),
        Some(&BindValue::Many(vec![]))
    );
}

/// Same pattern with lazy `?`: the first `?` prefers zero iterations, so
/// `x=[], y=hello, z=[world]`.
#[test]
fn lazy_optional_prefers_zero_iterations() {
    let p = reform::parser::pattern("$( $x )? $y $( $z )?").unwrap();
    let facts = vec![fact(&["hello", "world"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].get("x"),
        Some(&BindValue::Many(vec![]))
    );
    assert_eq!(matches[0].get("y"), Some(&BindValue::One(Arg::from("hello"))));
    assert_eq!(
        matches[0].get("z"),
        Some(&BindValue::Many(vec![BindValue::One(Arg::from("world"))]))
    );
}

/// `$( $x )++ $( $y )+` against `[a, b, c]`: greedy `++` takes as many words
/// as possible for `x` while leaving at least one for `y`, so `x=[a, b],
/// y=[c]`.
#[test]
fn greedy_plus_takes_max_iterations() {
    let p = reform::parser::pattern("$( $x )++ $( $y )+").unwrap();
    let facts = vec![fact(&["a", "b", "c"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].get("x"),
        Some(&BindValue::Many(vec![
            BindValue::One(Arg::from("a")),
            BindValue::One(Arg::from("b")),
        ]))
    );
    assert_eq!(
        matches[0].get("y"),
        Some(&BindValue::Many(vec![BindValue::One(Arg::from("c"))]))
    );
}

/// `$( $x )+ $( $y )+` against `[a, b, c]`: lazy `+` takes as few words as
/// possible for `x`, so `x=[a], y=[b, c]`.
#[test]
fn lazy_plus_takes_min_iterations() {
    let p = reform::parser::pattern("$( $x )+ $( $y )+").unwrap();
    let facts = vec![fact(&["a", "b", "c"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
    assert_eq!(
        matches[0].get("x"),
        Some(&BindValue::Many(vec![BindValue::One(Arg::from("a"))]))
    );
    assert_eq!(
        matches[0].get("y"),
        Some(&BindValue::Many(vec![
            BindValue::One(Arg::from("b")),
            BindValue::One(Arg::from("c")),
        ]))
    );
}

/// `$( $( $x )* )++` (greedy one-or-more with zero-width inner `*`) against
/// `prefix`: the inner `*` matches zero args, satisfying the `+` requirement.
/// Exercises the greedy `at_least_one` + `mid == start` path in `match_reps`.
#[test]
fn greedy_plus_with_zero_width_inner() {
    let p = reform::parser::pattern("prefix $( $( $x )* )++").unwrap();
    let facts = vec![fact(&["prefix"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

/// Fact-level `??` (greedy optional) with a matching fact present: takes the
/// fact (present path), unlike lazy `?` which prefers not taking it.
#[test]
fn fact_level_greedy_optional_takes_fact() {
    let p = reform::parser::pattern("$( a )??\nb").unwrap();
    let facts = vec![fact(&["a"]), fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

/// Fact-level `?` (lazy optional) with a matching fact present: prefers not
/// taking the fact (absent path succeeds first).
#[test]
fn fact_level_lazy_optional_skips_fact() {
    let p = reform::parser::pattern("$( a )?\nb").unwrap();
    let facts = vec![fact(&["a"]), fact(&["b"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

/// `$( $( $x )* )**` (greedy zero-or-more with zero-width inner `*`) against
/// `prefix`: the inner `*` matches zero args (mid == start) with
/// `at_least_one == false`, exercising the greedy skip path in `match_reps`.
#[test]
fn greedy_star_with_zero_width_inner() {
    let p = reform::parser::pattern("prefix $( $( $x )* )**").unwrap();
    let facts = vec![fact(&["prefix"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}

/// `$( a )??\na` (greedy fact-level optional) with a single `a` fact: the
/// present path consumes `a`, leaving nothing for the required `a` — it
/// fails, so the absent path is used as fallback.
#[test]
fn fact_level_greedy_optional_fallback_to_absent() {
    let p = reform::parser::pattern("$( a )??\na").unwrap();
    let facts = vec![fact(&["a"])];
    let matches = p.find_matches(&facts);
    assert_eq!(matches.len(), 1);
}