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
        .insert("x".to_string(), BindValue::Many(vec![Arg::from("1")]));
    let mut b = Bindings::new();
    b.map
        .insert("x".to_string(), BindValue::Many(vec![Arg::from("1")]));
    assert!(a.merge(&b));
}

#[test]
fn bindings_merge_many_new() {
    let mut a = Bindings::new();
    let mut b = Bindings::new();
    b.map
        .insert("x".to_string(), BindValue::Many(vec![Arg::from("1")]));
    assert!(a.merge(&b));
    assert_eq!(a.get("x"), Some(&BindValue::Many(vec![Arg::from("1")])));
}

#[test]
fn bindings_merge_many_into_scalar_fails() {
    let mut a = Bindings::new();
    a.bind_scalar("x", Arg::from("1"));
    let mut b = Bindings::new();
    b.map
        .insert("x".to_string(), BindValue::Many(vec![Arg::from("1")]));
    assert!(!a.merge(&b));
}

#[test]
fn bindings_merge_many_different_fails() {
    let mut a = Bindings::new();
    a.map
        .insert("x".to_string(), BindValue::Many(vec![Arg::from("1")]));
    let mut b = Bindings::new();
    b.map
        .insert("x".to_string(), BindValue::Many(vec![Arg::from("2")]));
    assert!(!a.merge(&b));
}

// ---------------------------------------------------------------------------
// matches_fact convenience method
// ---------------------------------------------------------------------------

#[test]
fn pattern_fact_matches_fact() {
    let pf = PatternFact {
        removed: false,
        negated: false,
        args: vec![ArgTemplate::Literal(Arg::from("a"))],
    };
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
        BindValue::Many(vec![Arg::from("a"), Arg::from("b")]),
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
    let r = RepeatBlock {
        kind: RepetitionKind::ZeroOrMore,
        chunks: vec![BodyChunk::Text("x".to_string())],
    };
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
    let inner = BodyChunk::Repeat(RepeatBlock {
        kind: RepetitionKind::ZeroOrMore,
        chunks: vec![BodyChunk::Placeholder("y".to_string())],
    });
    let outer = BodyChunk::Repeat(RepeatBlock {
        kind: RepetitionKind::ZeroOrMore,
        chunks: vec![BodyChunk::Placeholder("x".to_string()), inner],
    });
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
    let r = RepeatBlock {
        kind: RepetitionKind::ZeroOrMore,
        chunks: vec![
            BodyChunk::Placeholder("x".to_string()),
            BodyChunk::Placeholder("y".to_string()),
        ],
    };
    let b = Body(vec![BodyChunk::Repeat(r)]);
    let mut bindings = Bindings::new();
    bindings.map.insert(
        "x".to_string(),
        BindValue::Many(vec![Arg::from("1"), Arg::from("2")]),
    );
    bindings
        .map
        .insert("y".to_string(), BindValue::Many(vec![Arg::from("a")]));
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
    let r = RepeatBlock {
        kind: RepetitionKind::ZeroOrMore,
        chunks: vec![BodyChunk::Placeholder("x".to_string())],
    };
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
    let r = RepeatBlock {
        kind: RepetitionKind::ZeroOrMore,
        chunks: vec![BodyChunk::Placeholder("x".to_string())],
    };
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
    assert_eq!(matches.len(), 1, "+ with zero-width inner should match once");
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
    assert_eq!(matches.len(), 1, "* with zero-width inner should match once");
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
    assert!(matches.is_empty(), "+ with no matching fact should not match");
}
