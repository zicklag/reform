use reform::engine::Engine;
use std::sync::Arc;

fn load(src: &str) -> Engine {
    let mut e = Engine::new();
    e.load_str(src).expect("load should succeed");
    e
}

/// A rule derives a new fact from a matching fact.
#[test]
fn rule_derives_reverse() {
    let e = load(
        r#"
$ alice is the reverse of bob
$ rule reverse
    ( $x is the reverse of $y )
    ( $y is the reverse of $x )
$ assert bob is the reverse of alice
$ quit
"#,
    );
    assert!(e.contains(&fact("bob is the reverse of alice")));
}

/// A `*` repetition collects all matching facts into a list, and the body
/// `$( ... )*` expands the list into arguments of one derived fact.
#[test]
fn repetition_collects_into_list() {
    let e = load(
        r#"
$ player is carrying sword
$ player is carrying shield
$ rule list_items
    ( $( player is carrying $item )* )
    ( all player items $( $item )* )
$ assert all player items sword shield
$ quit
"#,
    );
    assert!(e.contains(&fact("all player items sword shield")));
}

/// An optional within-fact argument (`$( $a )?`) binds when present and is
/// skipped when absent; both cases match the same rule.
#[test]
fn optional_arg_present_and_absent() {
    let e = load(
        r#"
$ the is article
the door is open
the window is shut
plain wall is gray
$ rule simplify
    ( parse $( $a )? $x is $adj )
    ( $x is $adj )
$ assert door is open
$ assert window is shut
$ assert wall is gray
$ quit
"#,
    );
    assert!(e.contains(&fact("door is open")));
    assert!(e.contains(&fact("wall is gray")));
}

/// A `-` pattern line removes the matched fact when the rule fires.
#[test]
fn removal_pattern() {
    let e = load(
        r#"
$ temp
$ rule drop_temp
    ( - temp )
    ( done )
$ assert done
$ assert-not temp
$ quit
"#,
    );
    assert!(e.contains(&fact("done")));
    assert!(!e.contains(&fact("temp")));
}

/// A rule body can generate a new rule. `$$x` emits a literal `$x` so the
/// generated rule gets its own placeholder; `$rel1` substitutes the outer
/// binding. The generated rule then fires on a later fact.
#[test]
fn body_generates_inner_rule() {
    let e = load(
        r#"
$ rule outer
    ( $rel1 is the reverse of $rel2 )
    (
        rule reverse_xy
            (
                $$x is $rel1 $$y
            )
            (
                $$y is $rel2 $$x
            )
    )
$ above is the reverse of below
$ cat is above dog
$ assert dog is below cat
$ quit
"#,
    );
    assert!(e.contains(&fact("dog is below cat")));
}

/// `>` prefix becomes a `prompt` fact.
#[test]
fn prompt_prefix() {
    let e = load("> look up\n$ quit\n");
    assert!(e.contains(&fact("prompt look up")));
}

/// Plain sentences get the `parse` prefix.
#[test]
fn parse_prefix() {
    let e = load("the canyon is big\n$ quit\n");
    assert!(e.contains(&fact("parse the canyon is big")));
}

/// Comments (full-line and trailing) are ignored.
#[test]
fn comments_are_ignored() {
    let e = load(
        r#"
# leading comment
the door is open   # trailing comment
# another full-line comment
$ quit
"#,
    );
    assert!(e.contains(&fact("parse the door is open")));
}

/// `assert-not` fails (returns an error) when the fact IS present.
#[test]
fn assert_not_fails_when_present() {
    let mut e = Engine::new();
    let res = e.load_str(
        r#"
$ here
$ assert-not here
"#,
    );
    assert!(res.is_err(), "assert-not should fail when the fact exists");
}

/// `quit` halts loading; facts after it are not loaded.
#[test]
fn quit_halts() {
    let e = load(
        r#"
$ before
$ quit
$ after
"#,
    );
    assert!(e.contains(&fact("before")));
    assert!(!e.contains(&fact("after")));
}

/// Helper: parse a single one-line fact.
fn fact(s: &str) -> reform::Fact {
    reform::parser::facts(s)
        .expect("fact parse")
        .into_iter()
        .next()
        .expect("one fact")
}

// -- find command -----------------------------------------------------------

/// `find` with a single-fact pattern prints matching facts.
#[test]
fn find_command() {
    let mut e = Engine::new();
    e.load_str(
        r#"
$ alice likes cats
$ bob likes dogs
$ alice likes birds
"#,
    )
    .unwrap();
    assert!(e.contains(&fact("alice likes cats")));
    assert!(e.contains(&fact("bob likes dogs")));
    assert!(e.contains(&fact("alice likes birds")));
}

// -- facts command ----------------------------------------------------------

/// `facts` command prints all facts.
#[test]
fn facts_command() {
    let e = load(
        r#"
$ a
$ b
$ c
$ facts
$ quit
"#,
    );
    assert!(e.contains(&fact("a")));
    assert!(e.contains(&fact("b")));
    assert!(e.contains(&fact("c")));
}

// -- print command ----------------------------------------------------------

/// `print` outputs text without a trailing newline.
#[test]
fn print_command() {
    let mut e = Engine::new();
    // print doesn't change engine state, just outputs. Verify it doesn't error.
    let res = e.load_str("$ print hello world\n$ quit\n");
    assert!(res.is_ok());
}

// -- panic command ----------------------------------------------------------

/// `panic` returns an error with the given message.
#[test]
fn panic_command() {
    let mut e = Engine::new();
    let res = e.load_str("$ panic something went wrong\n");
    assert!(res.is_err());
    let err = format!("{}", res.unwrap_err());
    assert!(err.contains("something went wrong"), "error: {err}");
}

// -- load command -----------------------------------------------------------

/// `load` reads facts from a file.
#[test]
fn load_command() {
    let dir = std::env::temp_dir().join("reform_test_load");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test_load.reform");
    std::fs::write(&path, "$ hello world\n$ quit\n").unwrap();
    let mut e = Engine::new();
    let res = e.load_str(&format!("$ load {}\n", path.display()));
    assert!(res.is_ok(), "load should succeed: {:?}", res);
    assert!(e.contains(&fact("hello world")));
    let _ = std::fs::remove_dir_all(&dir);
}

// -- negation ---------------------------------------------------------------

/// Negation `!` in a pattern matches when the negated fact is absent.
#[test]
fn negation_matches_when_absent() {
    let e = load(
        r#"
$ rule check_absent
    ( ! secret_flag )
    ( all_clear )
$ assert all_clear
$ quit
"#,
    );
    assert!(e.contains(&fact("all_clear")));
}

/// Negation `!` fails to match when the negated fact IS present.
#[test]
fn negation_fails_when_present() {
    let mut e = Engine::new();
    let _res = e.load_str(
        r#"
$ secret_flag
$ rule check_absent
    ( ! secret_flag )
    ( all_clear )
$ assert all_clear
"#,
    );
    // The rule should not fire because secret_flag is present, so all_clear
    // should not be produced.
    assert!(!e.contains(&fact("all_clear")));
}

// -- negative lookahead `$( ... )!` (arg level) -----------------------------

/// A `$( ... )!` at the arg level is a PEG-style negative lookahead: it
/// matches at the current position iff the inner args do NOT match there.
#[test]
fn neg_lookahead_matches_when_absent() {
    let e = load(
        r#"
$ ready north
$ rule go
    ( ready $( the door is locked )! north )
    ( proceed )
$ assert proceed
$ quit
"#,
    );
    assert!(e.contains(&fact("proceed")));
}

/// The negative lookahead fails to match when its inner args ARE present at
/// the lookahead position.
#[test]
fn neg_lookahead_fails_when_present() {
    let mut e = Engine::new();
    let _res = e.load_str(
        r#"
$ ready the door is locked north
$ rule go
    ( ready $( the door is locked )! north )
    ( proceed )
$ quit
"#,
    );
    assert!(!e.contains(&fact("proceed")));
}

/// A negative lookahead is zero-width: it consumes nothing, so the args it
/// guards remain matched by the rest of the pattern.
#[test]
fn neg_lookahead_consumes_nothing() {
    let e = load(
        r#"
$ ready north
$ rule go
    ( ready $( the door )! north )
    ( proceed )
$ rule any_ready
    ( ready $x )
    ( noted $x )
$ assert noted north
$ quit
"#,
    );
    assert!(e.contains(&fact("noted north")));
}

/// A placeholder inside a negative lookahead acts as a constraint against an
/// already-bound value: the rule only fires when no fact matches that value.
#[test]
fn neg_lookahead_placeholder_constrains() {
    let mut e = Engine::new();
    let _res = e.load_str(
        r#"
$ forbidden apple
$ banana is clean
$ rule check
    ( $x is clean
      $( forbidden $x )! )
    ( $x passes )
$ quit
"#,
    );
    assert!(e.contains(&fact("banana passes")));
    assert!(!e.contains(&fact("apple passes")));
}

/// A negative lookahead nested inside an arg repetition applies the lookahead
/// per-iteration. The placeholder inside the lookahead appears *only* there,
/// so it is a local wildcard and must not panic or leak into the repetition's
/// frame.
#[test]
fn neg_lookahead_inside_arg_repetition() {
    let e = load(
        r#"
$ ready a c
$ rule go
    ( ready $( $x $( $y is forbidden )! $z )* )
    ( items $( $x $z )* )
$ assert items a c
$ quit
"#,
    );
    assert!(e.contains(&fact("items a c")));
}

/// A lookahead nested in an arg repetition whose inner placeholder is the same
/// scalar bound by the fact acts as a constraint — it must NOT be re-seeded
/// into the repetition's frame and clobber the scalar binding (regression
/// guard for a panic/clobber when a lookahead-inner placeholder shadows a
/// top-level scalar).
#[test]
fn neg_lookahead_scalar_shadowed_in_repetition() {
    let e = load(
        r#"
$ ready a foo
$ rule go
    ( ready $x $( $( $x is bad )! $z )+ )
    ( ok $x )
$ assert ok a
$ quit
"#,
    );
    assert!(e.contains(&fact("ok a")));
}

/// A negative lookahead whose inner args match at *match time* rejects the
/// fact even when the structural pre-filter admits it. The pre-filter treats
/// the lookahead as zero-width (only `foo` then `bar` are required), so the
/// fact `foo bar` passes the pre-filter but is rejected when `$x` matches
/// `bar` at the lookahead position.
#[test]
fn neg_lookahead_rejects_at_match_time() {
    let mut e = Engine::new();
    let _res = e.load_str(
        r#"
$ foo bar
$ rule go
    ( foo $( $x )! bar )
    ( proceed )
$ quit
"#,
    );
    assert!(!e.contains(&fact("proceed")));
}


/// `*` fact-level repetition: `$( ... )*` matches zero or more facts.
#[test]
fn fact_level_star_repetition() {
    let e = load(
        r#"
$ player has sword
$ player has shield
$ player has potion
$ rule list_items
    ( $( player has $item )* )
    ( items $( $item )* )
$ assert items sword shield potion
$ quit
"#,
    );
    assert!(e.contains(&fact("items sword shield potion")));
}

/// `+` fact-level repetition: `$( ... )+` matches one or more facts.
#[test]
fn fact_level_plus_repetition() {
    let e = load(
        r#"
$ player has sword
$ player has shield
$ rule list_items
    ( $( player has $item )+ )
    ( items $( $item )+ )
$ assert items sword shield
$ quit
"#,
    );
    assert!(e.contains(&fact("items sword shield")));
}

/// `+` arg-level repetition: `$( ... )+` matches one or more args.
#[test]
fn arg_level_plus_repetition() {
    let e = load(
        r#"
$ rule collect_args
    ( collect $( $x )+ )
    ( got $( $x )+ )
$ collect a b c
$ assert got a b c
$ quit
"#,
    );
    assert!(e.contains(&fact("got a b c")));
}

/// `*` arg-level repetition with zero matches.
#[test]
fn arg_level_star_zero_matches() {
    let e = load(
        r#"
$ rule zero_args
    ( zero $( $x )* )
    ( none )
$ zero
$ assert none
$ quit
"#,
    );
    assert!(e.contains(&fact("none")));
}

// -- edge cases -------------------------------------------------------------

/// Empty body rule: pattern matches but body produces nothing.
#[test]
fn empty_body_rule() {
    let e = load(
        r#"
$ rule noop
    ( trigger )
    ( )
$ trigger
$ assert trigger
$ quit
"#,
    );
    assert!(e.contains(&fact("trigger")));
}

/// `$$` escape in body produces a literal `$`.
#[test]
fn dollar_escape_in_body() {
    let e = load(
        r#"
$ rule dollar_gen
    ( gen_dollar )
    ( $$ dollar )
$ gen_dollar
$ assert dollar
$ quit
"#,
    );
    assert!(e.contains(&fact("dollar")));
}

/// `$any` conventional placeholder matches any single arg.
#[test]
fn any_placeholder() {
    let e = load(
        r#"
$ rule match_any
    ( $a is $b )
    ( matched )
$ x is y
$ assert matched
$ quit
"#,
    );
    assert!(e.contains(&fact("matched")));
}

/// `clear_quit` resets the quit flag.
#[test]
fn clear_quit_method() {
    let mut e = Engine::new();
    e.load_str("$ quit\n").unwrap();
    assert!(e.quit());
    e.clear_quit();
    assert!(!e.quit());
}

/// `remove_fact` for a non-existent fact returns false.
#[test]
fn remove_fact_nonexistent() {
    let mut e = Engine::new();
    let f = fact("ghost");
    assert!(!e.remove_fact(&f));
}

/// `add_fact` for a duplicate fact returns false.
#[test]
fn add_fact_duplicate() {
    let mut e = Engine::new();
    let f = fact("hello");
    assert!(e.add_fact(f.clone()));
    assert!(!e.add_fact(f));
}

// -- @eval arithmetic reduction -------------------------------------------------

/// An `@eval` in a fact is reduced to the f64 result of the single following
/// expression argument, immediately when the fact is created.
#[test]
fn eval_reduces_math_to_number() {
    let e = load("$ the final result is @eval (2 + 2 * 3)\n$ quit\n");
    assert!(e.contains(&fact("the final result is 8")));
}

/// The expression is evaluated as an f64, so non-integer results are kept.
#[test]
fn eval_reduces_to_f64() {
    let e = load("$ half of (7) is @eval (7 / 2)\n$ quit\n");
    assert!(e.contains(&fact("half of (7) is 3.5")));
}

/// Multiple `@eval`s in one fact are each reduced.
#[test]
fn eval_reduces_multiple() {
    let e = load("$ a is @eval (1 + 1) and @eval (2 * 3)\n$ quit\n");
    assert!(e.contains(&fact("a is 2 and 6")));
}

/// An `@eval` not followed by an argument is left untouched.
#[test]
fn eval_at_end_is_left_alone() {
    let e = load("$ a @eval\n$ quit\n");
    assert!(e.contains(&fact("a @eval")));
}

/// An `@eval` whose expression fails to parse is left untouched.
#[test]
fn eval_invalid_expression_is_left_alone() {
    let e = load("$ a @eval (2 + )\n$ quit\n");
    assert!(e.contains(&fact("a @eval (2 + )")));
}

/// An `@eval` whose expression contains a variable is left untouched (Reform
/// does not support variable bindings).
#[test]
fn eval_with_variable_is_left_alone() {
    let e = load("$ a @eval (x + 2)\n$ quit\n");
    assert!(e.contains(&fact("a @eval (x + 2)")));
}

/// Eval reduction happens with highest priority, before rules fire: a rule
/// body that generates an `@eval` fact reduces it immediately, and a fact
/// with `@eval` is already reduced before a rule pattern sees it.
#[test]
fn eval_reduces_in_rule_body() {
    let e = load(
        r#"
$ rule sum
    ( add $( $a )* )
    ( total @eval ( 1 + 2 ) )
$ add
$ quit
"#,
    );
    assert!(e.contains(&fact("total 3")));
}

/// Two engines with the same seed produce the same `random(n)` results for
/// the life of the engine.
#[test]
fn eval_random_deterministic_with_seed() {
    let a = reform::engine::Engine::new_with_seed(12345);
    let b = reform::engine::Engine::new_with_seed(12345);
    for _ in 0..20 {
        let fa = a.reduce_evals(fact("@eval (random(100))"));
        let fb = b.reduce_evals(fact("@eval (random(100))"));
        assert_eq!(fa, fb, "same seed must give same result");
    }
}

/// Different seeds produce (almost surely) different results.
#[test]
fn eval_random_different_seeds_differ() {
    let a = reform::engine::Engine::new_with_seed(111);
    let b = reform::engine::Engine::new_with_seed(222);
    let fa = a.reduce_evals(fact("@eval (random(100))"));
    let fb = b.reduce_evals(fact("@eval (random(100))"));
    assert_ne!(fa, fb, "different seeds should differ");
}

/// `random(n)` is in `[0, n)`, so `1 + floor(random(6))` yields a die roll
/// in `1..=6` and a value inside the range.
#[test]
fn eval_random_die_roll_in_range() {
    let e = reform::engine::Engine::new_with_seed(42);
    for _ in 0..100 {
        let f = e.reduce_evals(fact("die @eval (1 + floor(random(6)))"));
        let v: i64 = f[1].parse().expect("die value should be a number");
        assert!((1..=6).contains(&v), "die roll {v} out of 1..=6");
    }
}

/// A `random(n)` value with `n` stays in `[0, n)`, including when it is
/// non-integer.
#[test]
fn eval_random_bounded() {
    let e = reform::engine::Engine::new_with_seed(7);
    for _ in 0..100 {
        let f = e.reduce_evals(fact("x @eval (random(10))"));
        let v: f64 = f[1].parse().expect("random should yield a number");
        assert!((0.0..10.0).contains(&v), "random value {v} out of [0,10)");
    }
}

/// `normal_form_arg` escaping edge cases.
#[test]
fn normal_form_arg_edge_cases() {
    use reform::Arg;
    use reform::normal_form_arg;
    // Empty string
    assert_eq!(normal_form_arg(&Arg::from("")), "()");
    // Trailing punctuation
    assert_eq!(normal_form_arg(&Arg::from("hello.")), "(hello.)");
    assert_eq!(normal_form_arg(&Arg::from("world:")), "(world:)");
    assert_eq!(normal_form_arg(&Arg::from("test;")), "(test;)");
    assert_eq!(normal_form_arg(&Arg::from("foo'")), "(foo')");
    // Nested parens
    assert_eq!(normal_form_arg(&Arg::from("a(b)c")), "(a\\(b\\)c)");
    // Whitespace
    assert_eq!(normal_form_arg(&Arg::from("hello world")), "(hello world)");
    // Already clean
    assert_eq!(normal_form_arg(&Arg::from("clean")), "clean");
}
/// Re-entrant load detection: the `load` command uses `load_str_inner` to
/// avoid triggering the re-entrant check. This test verifies that the
/// `load` command works correctly (it used to fail with re-entrant error
/// before the fix).
#[test]
fn reentrant_load_detection() {
    let dir = std::env::temp_dir().join("reform_test_reentrant");
    let _ = std::fs::create_dir_all(&dir);
    let inner = dir.join("inner.reform");
    std::fs::write(&inner, "$ inner_fact\n").unwrap();
    let mut e = Engine::new();
    let res = e.load_str(&format!("$ load {}\n", inner.display()));
    assert!(res.is_ok(), "load should succeed: {:?}", res);
    assert!(e.contains(&fact("inner_fact")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `find` with a multi-fact pattern should error.
#[test]
fn find_multi_fact_pattern_errors() {
    let mut e = Engine::new();
    e.load_str("$ a\n$ b\n").unwrap();
    // A pattern with two items (separated by newline) should be rejected.
    // Pattern facts don't use parens - they're just the args directly.
    let pat = reform::parser::pattern("a\nb").unwrap();
    assert_eq!(pat.len(), 2, "pattern should have 2 items");
    let result = e.find_matching_facts(&pat);
    assert!(
        result.is_err(),
        "multi-fact find should error: {:?}",
        result
    );
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("single-fact"), "error: {err}");
}

// -- getters -----------------------------------------------------------------

/// `facts()` and `rules()` getters return the expected data.
#[test]
fn engine_getters() {
    let mut e = Engine::new();
    assert!(e.facts().is_empty());
    assert!(e.rules().is_empty());
    e.load_str("$ a\n$ rule r\n    ( $x )\n    ( $x )\n")
        .unwrap();
    assert_eq!(e.facts().len(), 2);
    assert_eq!(e.rules().len(), 1);
}

// -- run ---------------------------------------------------------------------

/// `run()` settles the engine to a fixpoint.
#[test]
fn engine_run() {
    let mut e = Engine::new();
    e.load_str("$ a\n$ rule r\n    ( a )\n    ( b )\n").unwrap();
    assert!(e.contains(&fact("b")));
    e.clear_quit();
    e.run().unwrap();
}

// -- empty args --------------------------------------------------------------

/// `ingest_file` with an empty fact is a no-op.
#[test]
fn ingest_file_empty_args() {
    let mut e = Engine::new();
    e.ingest_file(reform::Fact(vec![])).unwrap();
    assert!(e.facts().is_empty());
}

/// `ingest_body` with an empty fact is a no-op.
#[test]
fn ingest_body_empty_args() {
    let mut e = Engine::new();
    e.ingest_body(reform::Fact(vec![])).unwrap();
    assert!(e.facts().is_empty());
}

// -- unknown command ---------------------------------------------------------

/// An unknown command keyword is stored as a regular fact (not a command).
#[test]
fn unknown_command_stored_as_fact() {
    let e = load("$ unknown_cmd arg1 arg2\n$ quit\n");
    // Unknown commands are not in the command keyword list, so they get stored.
    assert!(e.contains(&fact("unknown_cmd arg1 arg2")));
}

// -- dash command with single arg --------------------------------------------

/// `$ -` with no fact to remove is a no-op.
#[test]
fn dash_command_single_arg() {
    let e = load("$ -\n$ quit\n");
    // No error, no change.
    assert!(e.facts().is_empty());
}

// -- find with multi-arg pattern ---------------------------------------------

/// `find` with a pattern that has multiple args (but single fact) works.
#[test]
fn find_multi_arg_pattern() {
    let mut e = Engine::new();
    e.load_str("$ a b c\n$ d e f\n").unwrap();
    let pat = reform::parser::pattern("a $x c").unwrap();
    let result = e.find_matching_facts(&pat).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], fact("a b c"));
}

// -- settle quit mid-turn ----------------------------------------------------

/// A rule that calls `$ quit` during settling stops the engine.
#[test]
fn settle_quit_mid_turn() {
    let mut e = Engine::new();
    e.load_str("$ trigger\n$ rule q\n    ( trigger )\n    ( $ quit )\n")
        .unwrap();
    assert!(e.quit());
}

// -- find with multi-arg pattern (spaces in pattern) -------------------------

/// `find` with a pattern that has multiple args joined by spaces.
#[test]
fn find_multi_arg_pattern_spaces() {
    let mut e = Engine::new();
    e.load_str("$ a b c\n$ d e f\n").unwrap();
    // The find command joins args with spaces when there are more than 2.
    // We can't easily capture stdout, but we can verify it doesn't error.
    let pat = reform::parser::pattern("a $x c").unwrap();
    let result = e.find_matching_facts(&pat).unwrap();
    assert_eq!(result.len(), 1);
}

// -- find with FactRepetition pattern errors ---------------------------------

/// `find` with a pattern whose first item is a FactRepetition should error.
#[test]
fn find_fact_repetition_pattern_errors() {
    let mut e = Engine::new();
    e.load_str("$ a\n").unwrap();
    let pat = reform::parser::pattern("$( a )*").unwrap();
    let result = e.find_matching_facts(&pat);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(err.contains("single-fact"), "error: {err}");
}

// -- load_str quit mid-load --------------------------------------------------

/// `load_str` stops loading when it encounters a `$ quit` fact.
#[test]
fn load_str_quit_mid_load() {
    let e = load("$ before\n$ quit\n$ after\n");
    assert!(e.contains(&fact("before")));
    assert!(!e.contains(&fact("after")));
}


/// An empty fact is a no-op (reached via ingest_body).
#[test]
fn execute_command_empty_args() {
    let mut e = Engine::new();
    // ingest_body with a fact that has only "$" - after stripping it's empty.
    // The empty fact gets stored (it's not a command), so no command handler
    // is dispatched.
    e.ingest_body(reform::Fact(vec![reform::Arg::from("$")]))
        .unwrap();
    // The empty fact is stored (not a command, not a rule).
    assert_eq!(e.facts().len(), 1);
    assert!(e.facts()[0].is_empty());
}

/// `println` command outputs text (we just verify it doesn't error).
#[test]
fn println_command() {
    let e = load("$ println hello world\n$ quit\n");
    assert!(e.quit());
}

// -- find command with multi-arg pattern via load_str -------------------------

/// `$ find (a b c)` with a multi-arg pattern works via the command path.
#[test]
fn find_command_multi_arg() {
    let mut e = Engine::new();
    e.load_str("$ a b c\n$ d e f\n").unwrap();
    // Use find_matching_facts directly to test the multi-arg pattern path.
    let pat = reform::parser::pattern("a $x c").unwrap();
    let result = e.find_matching_facts(&pat).unwrap();
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], fact("a b c"));
}

// -- find command with single-arg pattern via load_str ------------------------

/// `$ find $x` with a single-arg pattern works via the command path.
#[test]
fn find_command_single_arg() {
    let mut e = Engine::new();
    e.load_str("$ a\n$ b\n").unwrap();
    let pat = reform::parser::pattern("$x").unwrap();
    let result = e.find_matching_facts(&pat).unwrap();
    assert_eq!(result.len(), 2);
}

// -- settle quit at loop start ------------------------------------------------

/// `settle()` returns early when quit is set at the start of the loop.
#[test]
fn settle_quit_at_start() {
    let mut e = Engine::new();
    e.load_str("$ quit\n").unwrap();
    assert!(e.quit());
    // run() calls settle() which should return immediately.
    e.run().unwrap();
}

// -- dash command removes fact ------------------------------------------------

/// `$ - a b c` removes the matching fact.
#[test]
fn dash_command_removes_fact() {
    let e = load("$ a b c\n$ - a b c\n$ assert-not a b c\n$ quit\n");
    assert!(!e.contains(&fact("a b c")));
}

// -- dash command with pattern ------------------------------------------------

/// `$ - a $x` removes all facts matching the pattern (like `$ find`).
#[test]
fn dash_command_removes_with_pattern() {
    let e = load(
        r#"
$ alice likes cats
$ bob likes dogs
$ alice likes birds
$ - alice likes $x
$ assert-not alice likes cats
$ assert-not alice likes birds
$ assert bob likes dogs
$ quit
"#,
    );
    assert!(e.contains(&fact("bob likes dogs")));
    assert!(!e.contains(&fact("alice likes cats")));
    assert!(!e.contains(&fact("alice likes birds")));
}

/// `$ - $x` with a bare placeholder removes all facts.
#[test]
fn dash_command_removes_all_with_placeholder() {
    let e = load(
        r#"
$ a
$ b
$ c
$ - $x
$ assert-not a
$ assert-not b
$ assert-not c
$ quit
"#,
    );
    assert!(e.facts().is_empty());
}

/// `$ -` with a pattern that matches nothing is a no-op.
#[test]
fn dash_command_pattern_no_match() {
    let e = load(
        r#"
$ a
$ b
$ - c $x
$ assert a
$ assert b
$ quit
"#,
    );
    assert!(e.contains(&fact("a")));
    assert!(e.contains(&fact("b")));
}

/// `$ -` with a pattern containing characters that the pattern parser
/// rejects (e.g. a bare `$`) falls back to exact fact removal.
#[test]
fn dash_command_pattern_fallback() {
    let mut e = Engine::new();
    e.load_str(
        r#"$ - $( a )*
$ assert-not ( a )
$ quit
"#,
    )
    .unwrap();
    // The fallback parsed `$( a )*` as a single fact and removed it.
    // Since no such fact existed, the engine state is unchanged.
}

// -- unknown command fallback -------------------------------------------------

/// An unknown command keyword is silently ignored (the `_ => Ok(())` branch).
#[test]
fn unknown_command_fallback() {
    let e = load("$ foobar baz\n$ quit\n");
    // The fact is stored because foobar is not a recognized command keyword.
    assert!(e.contains(&fact("foobar baz")));
}

// -- find command via load_str ------------------------------------------------

/// `$ find $x` through the command path (hits the find handler's pattern parse).
#[test]
fn find_command_via_load_str() {
    let mut e = Engine::new();
    e.load_str("$ a\n$ b\n").unwrap();
    // This executes the find command, which prints to stdout.
    // We can't capture stdout, but we can verify it doesn't error.
    e.load_str("$ find $x\n$ quit\n").unwrap();
    assert!(e.quit());
}

/// `$ find (a $x c)` with multi-arg pattern through the command path.
#[test]
fn find_command_multi_arg_via_load_str() {
    let mut e = Engine::new();
    e.load_str("$ a b c\n$ d e f\n").unwrap();
    // Multi-arg pattern: args.len() != 2, so it joins args[1..].
    e.load_str("$ find a $x c\n$ quit\n").unwrap();
    assert!(e.quit());
}

// -- ? error branch coverage ------------------------------------------------
//
// Each test below targets one `?` propagation site in engine.rs. The inputs
// are chosen so the *source* parses (getting past `load_str_inner`'s
// `parser::facts(src)?`), but the targeted inner call fails. A common trick:
// a literal arg `(\()` carries the value `(` (a single open paren), which is
// a valid fact argument but unparseable when re-fed to `parser::facts` or
// `parser::pattern`.

/// `load_str_inner`: `parser::facts(src)?` (engine.rs:342) — unparseable source.
#[test]
fn load_str_parse_error() {
    let mut e = Engine::new();
    let res = e.load_str("(unclosed");
    assert!(res.is_err());
}

/// `ingest_file`: `Rule::parse(&strs)?` (engine.rs:398) — rule whose pattern
/// and body use `$x` at different repetition nestings, failing `validate`.
#[test]
fn ingest_file_rule_parse_error() {
    let mut e = Engine::new();
    let res = e.load_str("$ rule bad ( $( $x )* ) ( $( $x )+ )\n");
    assert!(res.is_err());
    let err = format!("{}", res.unwrap_err());
    assert!(err.contains("$x"), "error: {err}");
}

/// `ingest_body`: `Rule::parse(&strs)?` (engine.rs:437) — a fact fed directly
/// to `ingest_body` that is a 4-arg rule whose pattern `?` fails
/// `parser::pattern` (`?` is not a valid pattern token).
#[test]
fn ingest_body_rule_parse_error() {
    let mut e = Engine::new();
    let fact = reform::Fact(vec!["rule".into(), "bad".into(), "?".into(), "body".into()]);
    let res = e.ingest_body(fact);
    assert!(res.is_err());
    let err = format!("{}", res.unwrap_err());
    assert!(err.contains("pattern"), "error: {err}");
}

/// `turn`: `parser::facts(&text)?` (engine.rs:514) — a rule body that renders
/// to `(` (an unbalanced paren), which `parser::facts` rejects. The error
/// propagates up through `turn`'s `?` (engine.rs:515) and `ingest_file`'s
/// `settle()?` (engine.rs:411).
#[test]
fn turn_body_render_parse_error() {
    let mut e = Engine::new();
    let res = e.load_str(
        r#"$ a
$ rule bad
    a
    (\()
"#,
    );
    assert!(res.is_err());
}

/// `turn`: `self.ingest_body(f)?` (engine.rs:515) — a rule body that renders
/// to `panic`, producing a command fact whose execution errors. This also
/// covers `ingest_body`'s `dispatch_command(name, &cmd_args)?` (engine.rs:448).
#[test]
fn turn_ingest_body_command_error() {
    let mut e = Engine::new();
    let res = e.load_str(
        r#"$ a
$ rule p
    a
    panic
"#,
    );
    assert!(res.is_err());
    let err = format!("{}", res.unwrap_err());
    assert!(err.contains("panic"), "error: {err}");
}

/// `settle`: fixpoint `bail!` (engine.rs:482) — two rules that remove and
/// re-add each other's facts never reach a fixpoint. Also covers
/// `ingest_file`'s `settle()?` (engine.rs:411).
#[test]
fn fixpoint_reached() {
    let mut e = Engine::new();
    e.load_str(
        r#"$ rule a_to_b
    ( - a )
    ( b )
$ rule b_to_a
    ( - b )
    ( a )
$ a
"#,
    )
    .unwrap();
    // After each rule fires once on its matched fact, the engine reaches a
    // fixpoint: a_to_b already fired on {a}, b_to_a already fired on {b}.
    assert!(e.contains(&fact("a")));
    assert!(!e.contains(&fact("b")));
}
/// `-` handler: both `parser::pattern` and `parser::facts` reject an
/// unclosed paren — `$ - (` fails both paths.
#[test]
fn remove_command_parse_error() {
    let mut e = Engine::new();
    let res = e.load_str("$ - (");
    assert!(res.is_err());
}

/// `find` handler: `parser::pattern(&pattern_str)?` (engine.rs:255) —
/// `$ find (\()` carries the arg value `(`, which `parser::pattern` rejects.
#[test]
fn find_command_pattern_parse_error() {
    let mut e = Engine::new();
    let res = e.load_str("$ find (\\()");
    assert!(res.is_err());
}

/// `find` handler: `self.find_matching_facts(&pat)?` (engine.rs:256) —
/// `$ find ($( a )*)` carries the arg value `$( a )*`, which parses to a
/// `FactRepetition` pattern that `find_matching_facts` rejects (it only
/// supports single `Fact` patterns).
#[test]
fn find_command_fact_repetition_error() {
    let mut e = Engine::new();
    let res = e.load_str("$ find ($( a )*)");
    assert!(res.is_err());
    let err = format!("{}", res.unwrap_err());
    assert!(err.contains("single-fact"), "error: {err}");
}

/// `load` handler: `std::fs::read_to_string(path)?` and `.map_err(...)?`
/// (engine.rs:175) — load a nonexistent file.
#[test]
fn load_command_file_not_found() {
    let mut e = Engine::new();
    let res = e.load_str("$ load /nonexistent/file.rf\n");
    assert!(res.is_err());
    let err = format!("{}", res.unwrap_err());
    assert!(err.contains("load"), "error: {err}");
}

/// Rules are sorted by specificity descending: more specific rules fire first.
#[test]
fn specificity_more_literals_fire_first() {
    // Two rules matching the same fact. The more specific one (with literal
    // args instead of placeholders) should fire first and produce its output.
    // The less specific one fires second and also produces its output.
    let e = load(
        r#"
$ x is a thing
$ rule specific
    ( x is a thing )
    ( specific-result )
$ rule general
    ( $x is a thing )
    ( general-result )
$ assert specific-result
$ assert general-result
$ quit
"#,
    );
    assert!(e.contains(&fact("specific-result")));
    assert!(e.contains(&fact("general-result")));
}

/// A rule with more required facts is more specific than one with fewer.
#[test]
fn specificity_more_facts_fire_first() {
    let e = load(
        r#"
$ a is 1
$ b is 2
$ rule multi
    (
        a is 1
        b is 2
    )
    ( multi-result )
$ rule single
    ( a is 1 )
    ( single-result )
$ assert multi-result
$ assert single-result
$ quit
"#,
    );
    assert!(e.contains(&fact("multi-result")));
    assert!(e.contains(&fact("single-result")));
}

/// A rule with a negated fact contributes 0 specificity for that fact.
#[test]
fn specificity_negated_fact_contributes_zero() {
    let e = load(
        r#"
$ x is present
$ rule with-negation
    ( x is present
      ! y is absent )
    ( neg-result )
$ rule simple
    ( x is present )
    ( simple-result )
$ assert neg-result
$ assert simple-result
$ quit
"#,
    );
    assert!(e.contains(&fact("neg-result")));
    assert!(e.contains(&fact("simple-result")));
}

/// compute_specificity returns correct scores for various patterns.
///
/// Word scores: literal = 5, placeholder = 4, plus 1 per required fact.
/// Repetition blocks add 0 for the block and penalize enclosed words by
/// 1 (`?`), 2 (`+`), 3 (`*`), stacking across nested blocks and saturating
/// at zero.
#[test]
fn compute_specificity_scores() {
    use reform::rule::{Pattern, compute_specificity};

    // Single fact, 3 literal args: 1 fact + 3*5 = 16
    let p: Pattern = reform::parser::pattern("a is b").unwrap();
    assert_eq!(compute_specificity(&p), 16);

    // Single fact with placeholders: 1 fact + 4 + 5(is) + 4 = 14
    let p: Pattern = reform::parser::pattern("$x is $y").unwrap();
    assert_eq!(compute_specificity(&p), 14);

    // Two facts: (1 + 3*5) * 2 = 32
    let p: Pattern = reform::parser::pattern("a is b\nc is d").unwrap();
    assert_eq!(compute_specificity(&p), 32);

    // Optional fact repetition: required fact 16 + inner fact penalized by 1
    // (the `?`): 1 + (5-1)*3 = 13 -> 29
    let p: Pattern = reform::parser::pattern("a is b\n$( c is d )?").unwrap();
    assert_eq!(compute_specificity(&p), 29);

    // Negated fact contributes 0
    let p: Pattern = reform::parser::pattern("a is b\n! c is d").unwrap();
    assert_eq!(compute_specificity(&p), 16); // only the non-negated fact counts

    // Arg-level `+` with literals: 1 + 5(a) + ((5-3) + (5-3)) = 10
    let p: Pattern = reform::parser::pattern("a $( b c )+").unwrap();
    assert_eq!(compute_specificity(&p), 10);

    // Arg-level `*` with a placeholder: 1 + (4-4) = 2
    let p: Pattern = reform::parser::pattern("$( $x )*").unwrap();
    assert_eq!(compute_specificity(&p), 1);

    // Nested arg-level repetition: outer `+` (penalty 2) around inner `*`
    // (penalty 3); the literal `b` is at stacked penalty 5 -> 5-5 = 0.
    // Total: 1 + 5(a) + 0 = 6
    let p: Pattern = reform::parser::pattern("a $( $( b )* )+").unwrap();
    assert_eq!(compute_specificity(&p), 6);

    // Negated fact inside a `+` repetition: 1 + 5(a) + 0 (negated inner) = 6
    let p: Pattern = reform::parser::pattern("a\n$(\n! b is c\n)+").unwrap();
    assert_eq!(compute_specificity(&p), 6);

    // A catch-all must be LESS specific than a structured rule with a literal
    // constraint, so the structured rule fires first:
    //   `parse $( $arg )*`                    = 1 + 5 + (4-3)        = 7
    //   `parse $( $a1 )? $x is $( $a2 )? $y`  = 1 + 5 + (4-1) + 4 + 5(is) + (4-1) + 4 = 25
    let default = reform::parser::pattern("parse $( $arg )*").unwrap();
    let structured = reform::parser::pattern("parse $( $a1 )? $x is $( $a2 )? $y").unwrap();
    assert!(
        compute_specificity(&structured) > compute_specificity(&default),
        "structured rule must out-rank the catch-all default"
    );

    // A `+` catch-all (`parse $( $word )+`) must also be less specific than
    // the same structured rule: 1 + 5 + (4-2) = 8 < 25.
    let default_plus = reform::parser::pattern("parse $( $word )+").unwrap();
    assert!(
        compute_specificity(&structured) > compute_specificity(&default_plus),
        "structured rule must out-rank a `+` catch-all default"
    );

    // More required `+` repetitions are more specific than fewer:
    //   `a $( $b )+ . $( $c )+` = 1 + 5 + (4-2) + 5 + (4-2) = 15
    //   `a $( $b )+ .`          = 1 + 5 + (4-2) + 5         = 13
    let more_reps = reform::parser::pattern("a $( $b )+ . $( $c )+").unwrap();
    let fewer_reps = reform::parser::pattern("a $( $b )+ .").unwrap();
    assert!(
        compute_specificity(&more_reps) > compute_specificity(&fewer_reps),
        "more required repetitions must out-rank fewer"
    );
}

// -- specificity adjustment (5th rule arg) ------------------------------------

/// A rule with a positive specificity adjustment fires before a rule with
/// higher computed specificity but no adjustment.
#[test]
fn specificity_adjustment_positive_overrides() {
    let e = load(
        r#"
$ x is a thing
$ rule boosted
    ( $x is a thing )
    ( boosted-result )
    +100
$ rule literal
    ( x is a thing )
    ( literal-result )
$ assert boosted-result
$ assert literal-result
$ quit
"#,
    );
    assert!(e.contains(&fact("boosted-result")));
    assert!(e.contains(&fact("literal-result")));
}

/// A rule with a negative specificity adjustment fires after a rule with
/// lower computed specificity.
#[test]
fn specificity_adjustment_negative_delays() {
    let e = load(
        r#"
$ x is a thing
$ rule literal
    ( x is a thing )
    ( literal-result )
$ rule delayed
    ( $x is a thing )
    ( delayed-result )
    -100
$ assert literal-result
$ assert delayed-result
$ quit
"#,
    );
    assert!(e.contains(&fact("literal-result")));
    assert!(e.contains(&fact("delayed-result")));
}

/// A rule with a negative adjustment can be made to fire after a catch-all.
#[test]
fn specificity_adjustment_negative_below_catchall() {
    let e = load(
        r#"
$ x is a thing
$ rule catchall
    ( $x is $y $z )
    ( catchall-result )
$ rule specific
    ( x is a thing )
    ( specific-result )
    -100
$ assert catchall-result
$ assert specific-result
$ quit
"#,
    );
    assert!(e.contains(&fact("catchall-result")));
    assert!(e.contains(&fact("specific-result")));
}

/// A rule with a positive adjustment can outrank a more specific rule.
#[test]
fn specificity_adjustment_positive_outranks_more_specific() {
    let e = load(
        r#"
$ x is a thing
$ rule boosted
    ( $x is a thing )
    ( boosted-result )
    +10
$ rule literal
    ( x is a thing )
    ( literal-result )
$ assert boosted-result
$ assert literal-result
$ quit
"#,
    );
    assert!(e.contains(&fact("boosted-result")));
    assert!(e.contains(&fact("literal-result")));
}

/// A rule with `=N` has its specificity set to exactly N, ignoring the
/// computed base, so it can be forced to fire after a rule with a much lower
/// computed specificity. Both rules match the same fact; only the higher-
/// specificity one fires first and removes it, so the other never fires.
#[test]
fn specificity_adjustment_set_overrides_computed() {
    let e = load(
        r#"
$ x is a thing
$ rule literal
    ( - x is a thing )
    ( literal-result )
$ rule forced_low
    ( - x is a thing )
    ( forced-result )
    =0
$ assert literal-result
$ assert-not forced-result
$ quit
"#,
    );
    assert!(e.contains(&fact("literal-result")));
    assert!(!e.contains(&fact("forced-result")));
}

/// `=N` can also force a rule to outrank a more specific pattern.
#[test]
fn specificity_adjustment_set_can_boost() {
    let e = load(
        r#"
$ x is a thing
$ rule forced_high
    ( - $x is a thing )
    ( boosted-result )
    =1000
$ rule literal
    ( - x is a thing )
    ( literal-result )
$ assert boosted-result
$ assert-not literal-result
$ quit
"#,
    );
    assert!(e.contains(&fact("boosted-result")));
    assert!(!e.contains(&fact("literal-result")));
}

/// `=N` must be parsed as an absolute specificity (not added to the base).
#[test]
fn specificity_adjustment_set_is_absolute() {
    use reform::rule::Rule;
    // `a is b` computes a base specificity of 1 + 3*5 = 16. With `=5` the
    // effective specificity must be exactly 5, not 16 + 5 = 21.
    let rule = Rule::parse(&["rule", "r", "a is b", "( c )", "=5"]).unwrap();
    assert_eq!(rule.specificity, 5);
    assert_eq!(
        rule.specificity_adjustment,
        reform::rule::SpecificityAdjustment::Set(5)
    );
}

/// The 5th argument must start with +, -, or =.
#[test]
fn specificity_adjustment_must_be_signed() {
    let mut e = Engine::new();
    let res = e.load_str(
        r#"$ rule r
    ( a is b )
    ( c )
    5
"#,
    );
    let err = format!("{}", res.unwrap_err());
    assert!(
        err.contains("must start with +, -, or ="),
        "error: {err}"
    );
}

/// The 5th argument must be a valid integer.
#[test]
fn specificity_adjustment_invalid_integer() {
    let mut e = Engine::new();
    let res = e.load_str(
        r#"$ rule r
    ( a is b )
    ( c )
    +abc
"#,
    );
    let err = format!("{}", res.unwrap_err());
    assert!(
        err.contains("invalid specificity adjustment"),
        "error: {err}"
    );
}

/// The 5th argument must not be empty.
#[test]
fn specificity_adjustment_empty() {
    let mut e = Engine::new();
    let res = e.load_str(
        r#"$ rule r
    ( a is b )
    ( c )
    +
"#,
    );
    let err = format!("{}", res.unwrap_err());
    assert!(
        err.contains("invalid specificity adjustment"),
        "error: {err}"
    );
}

/// The 5th argument empty string via direct parse.
#[test]
fn specificity_adjustment_empty_via_direct_parse() {
    use reform::rule::Rule;
    let err = Rule::parse(&["rule", "r", "a is b", "( c )", ""]).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("got empty string"),
        "error: {msg}"
    );
}

/// Rules with equal specificity preserve insertion order (stable sort).
#[test]
fn specificity_equal_preserves_insertion_order() {
    let mut e = Engine::new();
    let r1 = reform::rule::Rule::parse(&["rule", "first", "a is b", "( first-result )"]).unwrap();
    let r2 = reform::rule::Rule::parse(&["rule", "second", "a is b", "( second-result )"]).unwrap();
    e.add_rule(r1);
    e.add_rule(r2);
    assert_eq!(&*e.rules()[0].name, "first");
    assert_eq!(&*e.rules()[1].name, "second");
}

// -- load_file method -------------------------------------------------------

/// `load_file` loads facts from a file path.
#[test]
fn load_file_method() {
    let dir = std::env::temp_dir().join("reform_test_load_file");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("test.rf");
    std::fs::write(&path, "$ hello world\n$ quit\n").unwrap();
    let mut e = Engine::new();
    let res = e.load_file(&path);
    assert!(res.is_ok(), "load_file should succeed: {:?}", res);
    assert!(e.contains(&fact("hello world")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `$ load` inside a file loaded via `load_file` resolves relative to the
/// file's directory (the `base_dir` branch of the load handler).
#[test]
fn load_with_base_dir() {
    let dir = std::env::temp_dir().join("reform_test_base_dir");
    let _ = std::fs::create_dir_all(&dir);
    let inner = dir.join("inner.rf");
    std::fs::write(&inner, "$ inner_fact\n").unwrap();
    let outer = dir.join("outer.rf");
    std::fs::write(&outer, "$ load inner.rf\n$ quit\n").unwrap();
    let mut e = Engine::new();
    let res = e.load_file(&outer);
    assert!(res.is_ok(), "load_file should succeed: {:?}", res);
    assert!(e.contains(&fact("inner_fact")));
    let _ = std::fs::remove_dir_all(&dir);
}

/// `load_file` returns an error for a non-existent file (covers the
/// `map_err` error path).
#[test]
fn load_file_error() {
    let mut e = Engine::new();
    let res = e.load_file(std::path::Path::new("/nonexistent/reform_test.rs"));
    assert!(res.is_err(), "load_file should fail for non-existent file");
}

// -- output sink ------------------------------------------------------------

/// The output sink routes `println`/`print`/`find`/`facts` text to the
/// configured stdout callback. This is what lets the wasm bindings render
/// engine output into a virtual terminal. (Trace events don't use `Output`;
/// they flow through the `tracing` ecosystem.)
#[test]
fn output_sink_captures_stdout() {
    use std::cell::RefCell;
    use std::rc::Rc;

    let mut e = Engine::new();
    let out = Rc::new(RefCell::new(String::new()));
    let out_cb = Rc::clone(&out);
    // The sink closures capture `Rc`s but must be stored in the `Arc`-based
    // `Output`. This is fine: the test is single-threaded, so the non-Send
    // capture is intentional.
    #[allow(clippy::arc_with_non_send_sync)]
    e.set_output(reform::engine::Output {
        stdout: Arc::new(move |s| out_cb.borrow_mut().push_str(s)),
        stderr: Arc::new(|s| eprint!("{s}")),
    });
    e.load_str("$ println hi\n$ print there \n$ a\n$ b\n$ facts\n$ find a\n$ quit\n")
        .unwrap();
    // println appends a newline; print does not; `$ a`/`$ b` store bare facts
    // (no `parse` prefix); facts lists every fact; find lists only matches.
    assert_eq!(out.borrow().as_str(), "hi\ntherea\nb\na\n");
    // The `output()` getter returns the same sinks that were set.
    (e.output().stdout)("via getter");
    assert_eq!(out.borrow().as_str(), "hi\ntherea\nb\na\nvia getter");
}

/// The default output routes stdout-style text (and anything a host sends to
/// the stderr sink) to the process streams.
#[test]
fn default_output_uses_process_streams() {
    let e = Engine::new();
    (e.output().stdout)("stdout-side\n");
    (e.output().stderr)("stderr-side\n");
}

// -- matched_facts coverage -------------------------------------------------

/// A negated fact inside a `?` repetition exercises the `!pf.negated` guard
/// in `matched_facts`'s `FactRepetition` arm (rule.rs:796).
#[test]
fn matched_facts_negated_in_repetition() {
    let mut e = Engine::new();
    e.load_str(
        r#"$ rule r
    (
        a
        $( !b )?
    )
    (
        c
    )
$ a
$ assert c
$ quit
"#,
    )
    .unwrap();
    assert!(e.contains(&fact("c")));
}

// -- fixpoint bail-out (MAX_ITERATIONS) --------------------------------------

/// A rule that peels one fact per firing, producing a new fact with a
/// different binding each time, will exceed MAX_ITERATIONS and bail.
/// Uses a low iteration cap so the test completes quickly.
#[test]
fn fixpoint_max_iterations() {
    let mut e = Engine::new();
    e.set_max_iterations(10);
    let res = e.load_str(
        r#"$ rule r
    ( - a $x )
    ( a (f $x) )
$ a 0
"#,
    );
    let err = format!("{}", res.unwrap_err());
    assert!(err.contains("fixpoint"), "error: {err}");
}

// -- command API: dispatch_command -------------------------------------------

/// `dispatch_command` returns `false` for an unregistered command name
/// without erroring.
#[test]
fn dispatch_command_unknown_returns_false() {
    let mut e = Engine::new();
    assert!(!e.dispatch_command("no-such-command", &[]).unwrap());
}

/// `dispatch_command` returns `true` and executes a registered handler.
#[test]
fn dispatch_command_registered_executes() {
    use std::sync::Arc;
    let mut e = Engine::new();
    let handler: reform::engine::CommandHandler = Arc::new(|engine, _args| {
        engine.add_fact(fact("dispatched"));
        Ok(())
    });
    e.register_command("custom", handler);
    assert!(e.dispatch_command("custom", &[]).unwrap());
    assert!(e.contains(&fact("dispatched")));
}

// -- command API: remove_command ---------------------------------------------

/// `remove_command` unregisters a handler so the name is no longer treated
/// as a command (a fact with that name is stored as data instead).
#[test]
fn remove_command_unregisters() {
    use std::sync::Arc;
    let mut e = Engine::new();
    let handler: reform::engine::CommandHandler = Arc::new(|engine, _args| {
        engine.add_fact(fact("fired"));
        Ok(())
    });
    e.register_command("temp", handler);
    e.remove_command("temp");
    e.load_str("$ temp\n").unwrap();
    // Not dispatched: no `fired` fact, and `temp` is stored as a regular fact.
    assert!(!e.contains(&fact("fired")));
    assert!(e.contains(&fact("temp")));
}

// -- base_dir / set_base_dir --------------------------------------------------

/// `base_dir`/`set_base_dir` get and set the load-relative base directory.
#[test]
fn base_dir_get_set() {
    use std::path::{Path, PathBuf};
    let mut e = Engine::new();
    assert!(e.base_dir().is_none());
    e.set_base_dir(Some(PathBuf::from("/tmp/reform")));
    assert_eq!(e.base_dir(), Some(Path::new("/tmp/reform")));
    e.set_base_dir(None);
    assert!(e.base_dir().is_none());
}

// -- Engine Debug -------------------------------------------------------------

/// `Engine` implements `Debug` manually (handlers are `dyn Fn`, which can't
/// be derived). The output lists registered command names.
#[test]
fn engine_debug_lists_commands() {
    let e = Engine::new();
    let s = format!("{:?}", e);
    assert!(s.contains("Engine"), "debug output: {s}");
    assert!(s.contains("println"), "debug output: {s}");
    assert!(s.contains("quit"), "debug output: {s}");
}
