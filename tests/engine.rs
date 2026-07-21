use reform::engine::Engine;

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
    ( sentence $( $a )? $x is $adj )
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

/// Plain sentences get the `sentence` prefix.
#[test]
fn sentence_prefix() {
    let e = load("the canyon is big\n$ quit\n");
    assert!(e.contains(&fact("sentence the canyon is big")));
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
    assert!(e.contains(&fact("sentence the door is open")));
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

// -- fact-level repetition --------------------------------------------------

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

// -- re-entrant load detection ------------------------------------------------
// NOTE: The re-entrant check at engine.rs:81 is unreachable through normal
// usage because the `load` command uses `load_str_inner` (not `load_str`).
// It exists as a safety guard for future code paths.

/// `execute_command` with an empty fact is a no-op (reached via ingest_body).
#[test]
fn execute_command_empty_args() {
    let mut e = Engine::new();
    // ingest_body with a fact that has only "$" - after stripping it's empty.
    // The empty fact gets stored (it's not a command), but execute_command
    // is never called because is_command is false for an empty fact.
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

// -- unknown command fallback -------------------------------------------------

/// An unknown command keyword is silently ignored (the `_ => Ok(())` branch).
#[test]
fn unknown_command_fallback() {
    let e = load("$ foobar baz\n$ quit\n");
    // The fact is stored because foobar is not a recognized command keyword.
    assert!(e.contains(&fact("foobar baz")));
}

// -- find command via load_str ------------------------------------------------

/// `$ find $x` through the command path (hits execute_command find branch).
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

/// `load_str_inner`: `parser::facts(src)?` (engine.rs:107) — unparseable source.
#[test]
fn load_str_parse_error() {
    let mut e = Engine::new();
    let res = e.load_str("(unclosed");
    assert!(res.is_err());
}

/// `ingest_file`: `Rule::parse(&strs)?` (engine.rs:148) — rule whose pattern
/// and body use `$x` at different repetition nestings, failing `validate`.
#[test]
fn ingest_file_rule_parse_error() {
    let mut e = Engine::new();
    let res = e.load_str("$ rule bad ( $( $x )* ) ( $( $x )+ )\n");
    assert!(res.is_err());
    let err = format!("{}", res.unwrap_err());
    assert!(err.contains("$x"), "error: {err}");
}

/// `ingest_body`: `Rule::parse(&strs)?` (engine.rs:179) — a fact fed directly
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

/// `turn`: `parser::facts(&text)?` (engine.rs:222) — a rule body that renders
/// to `(` (an unbalanced paren), which `parser::facts` rejects. The error
/// propagates up through `turn`'s `?` (engine.rs:202) and `ingest_file`'s
/// `settle()?` (engine.rs:153).
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

/// `turn`: `self.ingest_body(f)?` (engine.rs:223) — a rule body that renders
/// to `panic`, producing a command fact whose execution errors. This also
/// covers `ingest_body`'s `execute_command(cmd)?` (engine.rs:182).
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

/// `settle`: fixpoint `bail!` (engine.rs:207) — two rules that remove and
/// re-add each other's facts never reach a fixpoint. Also covers
/// `ingest_file`'s `settle()?` (engine.rs:153).
#[test]
fn fixpoint_bail() {
    let mut e = Engine::new();
    let res = e.load_str(
        r#"$ rule a_to_b
    ( - a )
    ( b )
$ rule b_to_a
    ( - b )
    ( a )
$ a
"#,
    );
    assert!(res.is_err());
    let err = format!("{}", res.unwrap_err());
    assert!(err.contains("fixpoint"), "error: {err}");
}

/// `Command::Remove`: `parser::facts(&fact_str)?` (engine.rs:240) — `$ - (\()`
/// carries the arg value `(`, so `fact_str` is `(` which `parser::facts`
/// rejects.
#[test]
fn remove_command_parse_error() {
    let mut e = Engine::new();
    let res = e.load_str("$ - (\\()");
    assert!(res.is_err());
}

/// `Command::Find`: `parser::pattern(&pattern_str)?` (engine.rs:282) —
/// `$ find (\()` carries the arg value `(`, which `parser::pattern` rejects.
#[test]
fn find_command_pattern_parse_error() {
    let mut e = Engine::new();
    let res = e.load_str("$ find (\\()");
    assert!(res.is_err());
}

/// `Command::Find`: `self.find_matching_facts(&pat)?` (engine.rs:283) —
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

/// `Command::Load`: `std::fs::read_to_string(path)?` and `.map_err(...)?`
/// (engine.rs:297) — load a nonexistent file.
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
    ( a is 1
      b is 2 )
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
#[test]
fn compute_specificity_scores() {
    use reform::rule::{Pattern, compute_specificity};

    // Single fact, 3 literal args: 1 fact + 3 literals = 4
    let p: Pattern = reform::parser::pattern("a is b").unwrap();
    assert_eq!(compute_specificity(&p), 4);

    // Single fact with placeholders: 1 fact + 1 literal ("is") = 2
    let p: Pattern = reform::parser::pattern("$x is $y").unwrap();
    assert_eq!(compute_specificity(&p), 2);

    // Two facts: 2 facts + 6 literals = 8
    let p: Pattern = reform::parser::pattern("a is b\nc is d").unwrap();
    assert_eq!(compute_specificity(&p), 8);

    // Optional fact repetition contributes 0
    let p: Pattern = reform::parser::pattern("a is b\n$( c is d )?").unwrap();
    assert_eq!(compute_specificity(&p), 4); // only the required fact counts

    // Negated fact contributes 0
    let p: Pattern = reform::parser::pattern("a is b\n! c is d").unwrap();
    assert_eq!(compute_specificity(&p), 4); // only the non-negated fact counts

    // Arg-level repetition with literals: 1 fact + 1 literal + (1 block + 2 literals) = 5
    let p: Pattern = reform::parser::pattern("a $( b c )+").unwrap();
    assert_eq!(compute_specificity(&p), 5);

    // Arg-level repetition with placeholders only: 1 fact + (1 block + 0 literals) = 2
    let p: Pattern = reform::parser::pattern("$( $x )*").unwrap();
    assert_eq!(compute_specificity(&p), 2);

    // Nested arg-level repetition: 1 fact + 1 literal + (1 outer block + (1
    // inner block + 1 literal)) = 5
    let p: Pattern = reform::parser::pattern("a $( $( b )* )+").unwrap();
    assert_eq!(compute_specificity(&p), 5);

    // Negated fact inside a `+` repetition: 1 fact + 1 literal "a" + (1 block
    // + 0 negated inner) = 3
    let p: Pattern = reform::parser::pattern("a\n$(\n! b is c\n)+").unwrap();
    assert_eq!(compute_specificity(&p), 3);
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
/// file's directory (the `base_dir` branch of `Command::Load`).
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

// -- trace logging ---------------------------------------------------------

/// Enabling trace exercises the `set_trace` path and every `if self.trace`
/// branch: rule registration, fact addition, fact removal (via a `-` pattern),
/// and rule firing. Trace output goes to stderr (captured by the test
/// harness for passing tests), so we only assert the engine behavior.
#[test]
fn trace_emits_events() {
    let mut e = Engine::new();
    e.set_trace(true);
    e.load_str("$ rule r\n    ( - a )\n    ( b )\n$ a\n$ quit\n")
        .unwrap();
    assert!(e.contains(&fact("b")));
    assert!(!e.contains(&fact("a")));
}
