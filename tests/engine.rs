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