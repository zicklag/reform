//! Tests for `NfaPrefilter`, the bit-parallel Glushkov simulation.
//!
//! Each test builds a `RegexTree` by hand, lifts it to an `Nfa`, then to an
//! `NfaPrefilter`, and checks `matches` against hand-built `Fact`s. The
//! simulation is an exact acceptor for the Glushkov automaton, so for patterns
//! made of literals and *distinct* placeholders these expectations are exact.
//! (A repeated binding placeholder like `$x $x` would be over-approximated,
//! since the automaton forgets that the two occurrences must bind equally —
//! that soundness slack is the whole point of it being a *pre*-filter.)

use reform::regex::{Nfa, NfaPrefilter, NfaSymbol, RegexItem, RegexTree, RepetitionKind};
use reform::{Arg, Fact};

fn lit(s: &str) -> RegexItem {
    RegexItem::Symbol(NfaSymbol::Literal(Arg::from(s)))
}

fn ph(s: &str) -> RegexItem {
    RegexItem::Symbol(NfaSymbol::Placeholder(reform::Str::from(s)))
}

fn rep(kind: RepetitionKind, items: Vec<RegexItem>) -> RegexItem {
    RegexItem::Repetition { kind, items }
}

fn tree(items: Vec<RegexItem>) -> RegexTree {
    RegexTree(items)
}

fn prefilter(items: Vec<RegexItem>) -> NfaPrefilter {
    NfaPrefilter::from(Nfa::from_tree(tree(items)))
}

fn fact(args: &[&str]) -> Fact {
    Fact(args.iter().copied().map(Arg::from).collect())
}

#[test]
fn single_literal() {
    let p = prefilter(vec![lit("a")]);
    assert!(p.matches(&fact(&["a"])));
    assert!(!p.matches(&fact(&["b"])));
    assert!(!p.matches(&Fact(vec![])));
    assert!(!p.matches(&fact(&["a", "a"])));
}

#[test]
fn literal_concatenation() {
    let p = prefilter(vec![lit("a"), lit("b")]);
    assert!(p.matches(&fact(&["a", "b"])));
    assert!(!p.matches(&fact(&["a"])));
    assert!(!p.matches(&fact(&["b"])));
    assert!(!p.matches(&fact(&["b", "a"])));
    assert!(!p.matches(&fact(&["a", "b", "c"])));
}

#[test]
fn zero_or_more_literal() {
    let p = prefilter(vec![rep(RepetitionKind::ZeroOrMore, vec![lit("a")])]);
    assert!(p.matches(&Fact(vec![])));
    assert!(p.matches(&fact(&["a"])));
    assert!(p.matches(&fact(&["a", "a", "a"])));
    assert!(!p.matches(&fact(&["b"])));
    assert!(!p.matches(&fact(&["a", "b"])));
}

#[test]
fn one_or_more_literal() {
    let p = prefilter(vec![rep(RepetitionKind::OneOrMore, vec![lit("a")])]);
    assert!(!p.matches(&Fact(vec![])));
    assert!(p.matches(&fact(&["a"])));
    assert!(p.matches(&fact(&["a", "a"])));
    assert!(!p.matches(&fact(&["b"])));
}

#[test]
fn zero_or_more_sequence() {
    let p = prefilter(vec![rep(
        RepetitionKind::ZeroOrMore,
        vec![lit("a"), lit("b")],
    )]);
    assert!(p.matches(&Fact(vec![])));
    assert!(p.matches(&fact(&["a", "b"])));
    assert!(p.matches(&fact(&["a", "b", "a", "b"])));
    assert!(!p.matches(&fact(&["a"])));
    assert!(!p.matches(&fact(&["b"])));
    assert!(!p.matches(&fact(&["a", "b", "a"])));
}

#[test]
fn sequential_optional_repetitions() {
    // (a?)(b?) : the motivating case for the follower-accumulation design.
    let p = prefilter(vec![
        rep(RepetitionKind::Optional, vec![lit("a")]),
        rep(RepetitionKind::Optional, vec![lit("b")]),
    ]);
    assert!(p.matches(&Fact(vec![])));
    assert!(p.matches(&fact(&["a"])));
    assert!(p.matches(&fact(&["b"])));
    assert!(p.matches(&fact(&["a", "b"])));
    assert!(!p.matches(&fact(&["a", "a"])));
    assert!(!p.matches(&fact(&["b", "b"])));
    assert!(!p.matches(&fact(&["a", "b", "a"])));
}

#[test]
fn placeholder_star_then_literal() {
    // $( $x )* . : any number of args followed by a literal `.`.
    let p = prefilter(vec![rep(RepetitionKind::ZeroOrMore, vec![ph("x")]), lit(".")]);
    assert!(p.matches(&fact(&["."])));
    assert!(p.matches(&fact(&["a", "."])));
    assert!(p.matches(&fact(&["a", "b", "c", "."])));
    assert!(!p.matches(&Fact(vec![])));
    assert!(!p.matches(&fact(&["a"])));
    assert!(!p.matches(&fact(&["a", "b"])));
}

#[test]
fn two_distinct_placeholders() {
    let p = prefilter(vec![ph("x"), ph("y")]);
    assert!(p.matches(&fact(&["a", "b"])));
    assert!(!p.matches(&fact(&["a"])));
    assert!(!p.matches(&fact(&["a", "b", "c"])));
    assert!(!p.matches(&Fact(vec![])));
}

#[test]
fn optional_placeholder_then_literal() {
    // ( $x? ) a : zero-or-one any arg, then a literal `a`.
    let p = prefilter(vec![rep(RepetitionKind::Optional, vec![ph("x")]), lit("a")]);
    assert!(p.matches(&fact(&["a"]))); // x skipped
    assert!(p.matches(&fact(&["b", "a"]))); // x = b
    assert!(p.matches(&fact(&["a", "a"]))); // x = a
    assert!(!p.matches(&fact(&["a", "a", "a"]))); // too long
    assert!(!p.matches(&fact(&["b"]))); // no trailing a
}

#[test]
fn optional_then_zero_or_more_then_required() {
    // ( a? ) ( b )* c
    let p = prefilter(vec![
        rep(RepetitionKind::Optional, vec![lit("a")]),
        rep(RepetitionKind::ZeroOrMore, vec![lit("b")]),
        lit("c"),
    ]);
    assert!(p.matches(&fact(&["c"])));
    assert!(p.matches(&fact(&["a", "c"])));
    assert!(p.matches(&fact(&["b", "c"])));
    assert!(p.matches(&fact(&["a", "b", "c"])));
    assert!(p.matches(&fact(&["a", "b", "b", "c"])));
    assert!(!p.matches(&fact(&["a", "b"]))); // no c
    assert!(!p.matches(&fact(&["c", "c"]))); // too long
}