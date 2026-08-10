//! Tests for the Glushkov NFA construction in `reform::regex`.
//!
//! Each test builds a pattern's args as `&[ArgTemplate]`, runs `Nfa::from_tree`,
//! and checks the resulting `symbols` / `follows` / `first` / `last` / `nullable`.
//! Follow sets and `first`/`last` are compared as sorted `Vec<u8>` so the tests
//! don't depend on insertion order.

use std::collections::BTreeSet;

use reform::regex::{Nfa, RepetitionKind};
use reform::rule::{ArgTemplate, RepeatedArgs};
use reform::Arg;

/// Shorthand for a literal-arg item.
fn lit(s: &str) -> ArgTemplate {
    ArgTemplate::Literal(Arg::from(s))
}

/// Shorthand for a placeholder item.
fn ph(s: &str) -> ArgTemplate {
    ArgTemplate::Placeholder(s.to_string())
}

/// Wrap `items` in a repetition of the given kind.
fn rep(kind: RepetitionKind, items: Vec<ArgTemplate>) -> ArgTemplate {
    ArgTemplate::RepeatedArgs(RepeatedArgs::new(kind, false, items))
}

/// Sorted, deduped view of a position set, for order-independent comparison.
fn set(xs: &[u8]) -> Vec<u8> {
    BTreeSet::from_iter(xs.iter().copied()).into_iter().collect()
}

/// Sorted follower set for position `p`.
fn follow(nfa: &Nfa, p: usize) -> Vec<u8> {
    set(&nfa.follows[p])
}

#[test]
fn single_symbol() {
    let nfa = Nfa::from_tree(&[lit("a")]);
    assert_eq!(nfa.symbols.len(), 1);
    assert_eq!(follow(&nfa, 0), Vec::<u8>::new());
    assert_eq!(set(&nfa.qualities.first), vec![0]);
    assert_eq!(set(&nfa.qualities.last), vec![0]);
    assert!(!nfa.qualities.nullable);
}

#[test]
fn concatenation_two_symbols() {
    let nfa = Nfa::from_tree(&[lit("a"), lit("b")]);
    assert_eq!(follow(&nfa, 0), vec![1]);
    assert_eq!(follow(&nfa, 1), Vec::<u8>::new());
    assert_eq!(set(&nfa.qualities.first), vec![0]);
    assert_eq!(set(&nfa.qualities.last), vec![1]);
    assert!(!nfa.qualities.nullable);
}

#[test]
fn optional_is_nullable_no_loopback() {
    let nfa = Nfa::from_tree(&[rep(RepetitionKind::Optional, vec![lit("a")])]);
    assert_eq!(follow(&nfa, 0), Vec::<u8>::new());
    assert_eq!(set(&nfa.qualities.first), vec![0]);
    assert_eq!(set(&nfa.qualities.last), vec![0]);
    assert!(nfa.qualities.nullable);
}

#[test]
fn zero_or_more_loops_back_and_is_nullable() {
    let nfa = Nfa::from_tree(&[rep(RepetitionKind::ZeroOrMore, vec![lit("a")])]);
    assert_eq!(follow(&nfa, 0), vec![0]);
    assert_eq!(set(&nfa.qualities.first), vec![0]);
    assert_eq!(set(&nfa.qualities.last), vec![0]);
    assert!(nfa.qualities.nullable);
}

#[test]
fn one_or_more_loops_back_but_not_nullable() {
    let nfa = Nfa::from_tree(&[rep(RepetitionKind::OneOrMore, vec![lit("a")])]);
    assert_eq!(follow(&nfa, 0), vec![0]);
    assert_eq!(set(&nfa.qualities.first), vec![0]);
    assert_eq!(set(&nfa.qualities.last), vec![0]);
    assert!(!nfa.qualities.nullable);
}

#[test]
fn repeated_sequence_loops_end_to_start() {
    // (a b)* : follow a -> b, follow b -> a (loop back).
    let nfa = Nfa::from_tree(&[rep(
        RepetitionKind::ZeroOrMore,
        vec![lit("a"), lit("b")],
    )]);
    assert_eq!(follow(&nfa, 0), vec![1]);
    assert_eq!(follow(&nfa, 1), vec![0]);
    assert_eq!(set(&nfa.qualities.first), vec![0]);
    assert_eq!(set(&nfa.qualities.last), vec![1]);
    assert!(nfa.qualities.nullable);
}

#[test]
fn sequential_nullable_repetitions_accumulate_first_and_last() {
    // (a?)(b?) : the case that motivated the follower-accumulation design.
    // Both symbols can be the first and last matched position because each
    // repetition can vanish, and `a` is followed by `b`.
    let nfa = Nfa::from_tree(&[
        rep(RepetitionKind::Optional, vec![lit("a")]),
        rep(RepetitionKind::Optional, vec![lit("b")]),
    ]);
    assert_eq!(follow(&nfa, 0), vec![1]);
    assert_eq!(follow(&nfa, 1), Vec::<u8>::new());
    assert_eq!(set(&nfa.qualities.first), vec![0, 1]);
    assert_eq!(set(&nfa.qualities.last), vec![0, 1]);
    assert!(nfa.qualities.nullable);
}

#[test]
fn nullable_prefix_inside_repeated_body() {
    // (a? b)* : `a` is optional inside a repeated body, so the loop-back from
    // `b` must reach both `a` and `b` (since `a` can be skipped on re-entry),
    // and `first` must include both `a` and `b`.
    let nfa = Nfa::from_tree(&[rep(
        RepetitionKind::ZeroOrMore,
        vec![rep(RepetitionKind::Optional, vec![lit("a")]), lit("b")],
    )]);
    assert_eq!(follow(&nfa, 0), vec![1]); // a -> b
    assert_eq!(follow(&nfa, 1), vec![0, 1]); // b loops back to a and b
    assert_eq!(set(&nfa.qualities.first), vec![0, 1]);
    assert_eq!(set(&nfa.qualities.last), vec![1]);
    assert!(nfa.qualities.nullable);
}

#[test]
fn nested_nullable_repetition_no_duplicate_followers() {
    // ((a)*)* : the outer loop-back would re-add `a` to its own followers; the
    // dedup in `extend_unique` keeps follow(a) == {a}, not {a, a}.
    let nfa = Nfa::from_tree(&[rep(
        RepetitionKind::ZeroOrMore,
        vec![rep(RepetitionKind::ZeroOrMore, vec![lit("a")])],
    )]);
    assert_eq!(follow(&nfa, 0), vec![0]);
    assert_eq!(set(&nfa.qualities.first), vec![0]);
    assert_eq!(set(&nfa.qualities.last), vec![0]);
    assert!(nfa.qualities.nullable);
}

#[test]
fn placeholders_are_positions_too() {
    // A mix of literals and placeholders: every symbol gets its own position.
    let nfa = Nfa::from_tree(&[ph("x"), lit("is"), rep(RepetitionKind::OneOrMore, vec![ph("y")])]);
    // positions: 0 = $x, 1 = is, 2 = $y
    assert_eq!(follow(&nfa, 0), vec![1]); // $x -> is
    assert_eq!(follow(&nfa, 1), vec![2]); // is -> $y
    assert_eq!(follow(&nfa, 2), vec![2]); // $y loops to itself
    assert_eq!(set(&nfa.qualities.first), vec![0]);
    assert_eq!(set(&nfa.qualities.last), vec![2]);
    assert!(!nfa.qualities.nullable);
}

#[test]
fn empty_tree_matches_empty() {
    let nfa = Nfa::from_tree(&[] as &[ArgTemplate]);
    assert_eq!(nfa.symbols.len(), 0);
    assert!(nfa.qualities.first.is_empty());
    assert!(nfa.qualities.last.is_empty());
    assert!(nfa.qualities.nullable);
}