use reform::rule::Rule;

fn parse_rule(src: &str) -> Result<Rule, String> {
    let facts = reform::parser::facts(src).expect("facts parse");
    let f = &facts[0];
    let args: Vec<&str> = f.iter().map(|a| &**a).collect();
    // strip a leading `$`
    let rule_args = if args[0] == "$" {
        &args[1..]
    } else {
        &args[..]
    };
    Rule::parse(rule_args).map_err(|e| format!("{e}"))
}

/// A placeholder used at two different nesting depths in the pattern is
/// rejected.
#[test]
fn inconsistent_pattern_nesting_rejected() {
    let src = r#"
$ rule r
    (
        a $x b
        $( c $x )*
    )
    ( d )
"#;
    let err = parse_rule(src).expect_err("should reject");
    assert!(err.contains("inconsistent nesting"), "got: {err}");
}

/// A placeholder used in the body at a different nesting than in the pattern
/// is rejected.
#[test]
fn body_pattern_nesting_mismatch_rejected() {
    let src = r#"
$ rule r
    ( $( a $x )* )
    ( b $x )
"#;
    let err = parse_rule(src).expect_err("should reject");
    assert!(err.contains("different nesting"), "got: {err}");
}

/// A body placeholder not declared in the pattern is rejected.
#[test]
fn undeclared_body_placeholder_rejected() {
    let src = r#"
$ rule r
    ( a $x )
    ( b $y )
"#;
    let err = parse_rule(src).expect_err("should reject");
    assert!(err.contains("not declared in pattern"), "got: {err}");
}

/// A consistent rule (same nesting in pattern and body, including `$$`
/// escapes for an inner rule's own placeholders) parses cleanly.
#[test]
fn consistent_inner_rule_accepted() {
    let src = r#"
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
"#;
    parse_rule(src).expect("should parse");
}

/// A list-bound placeholder in the pattern, used inside a matching `$( ... )*`
/// in the body, parses cleanly.
#[test]
fn list_placeholder_in_body_repetition_accepted() {
    let src = r#"
$ rule r
    ( $( player is carrying $item )* )
    ( all player items $( $item )* )
"#;
    parse_rule(src).expect("should parse");
}

/// A rule with no name (only `rule` keyword, no name argument) is rejected.
#[test]
fn rule_with_no_name_rejected() {
    let src = r#"
$ rule
    ( a )
    ( b )
"#;
    let err = parse_rule(src).expect_err("should reject");
    assert!(err.contains("exactly 4 arguments"), "got: {err}");
}

/// A rule with extra arguments beyond the required 4 is rejected.
#[test]
fn rule_with_wrong_number_of_args_rejected() {
    let src = r#"
$ rule r extra
    ( a )
    ( b )
"#;
    let err = parse_rule(src).expect_err("should reject");
    assert!(err.contains("exactly 4 arguments"), "got: {err}");
}

/// An empty pattern (no pattern items) is valid.
#[test]
fn empty_pattern_accepted() {
    let src = r#"
$ rule r
    ( )
    ( b )
"#;
    parse_rule(src).expect("empty pattern should be valid");
}

/// An empty body (no body chunks) is valid.
#[test]
fn empty_body_accepted() {
    let src = r#"
$ rule r
    ( a )
    ( )
"#;
    parse_rule(src).expect("empty body should be valid");
}

/// A pattern fact with both `-` (removal) and `!` (negation) markers.
/// The parser doesn't have a combined `-!` production, so `-!a` fails to
/// parse. However `!-a` is valid: `!` is the negation marker and `-a` is
/// a literal argument.
#[test]
fn pattern_with_both_removal_and_negation_mixed() {
    let src = r#"
$ rule r
    ( -!a )
    ( b )
"#;
    let err = parse_rule(src).expect_err("should reject");
    assert!(err.contains("failed to parse rule pattern"), "got: {err}");

    let src = r#"
$ rule r
    ( !-a )
    ( b )
"#;
    parse_rule(src).expect("!-a should parse as negation of literal `-a`");
}

/// Placeholder names with special characters (like `@`) are valid.
#[test]
fn placeholder_with_special_chars_accepted() {
    let src = r#"
$ rule r
    ( a $x@y )
    ( b $x@y )
"#;
    parse_rule(src).expect("placeholder with special chars should be valid");
}

/// A pattern consisting only of negated (`!`) facts is valid.
#[test]
fn pattern_only_negation_accepted() {
    let src = r#"
$ rule r
    ( !a )
    ( b )
"#;
    parse_rule(src).expect("negation-only pattern should be valid");
}

/// A pattern consisting only of removal (`-`) facts is valid.
#[test]
fn pattern_only_removal_accepted() {
    let src = r#"
$ rule r
    ( -a )
    ( b )
"#;
    parse_rule(src).expect("removal-only pattern should be valid");
}

/// A body placeholder referencing a pattern placeholder at the same nesting
/// depth (another variant of the consistent-placeholder test).
#[test]
fn body_placeholder_matches_pattern_accepted() {
    let src = r#"
$ rule r
    ( a $x b )
    ( c $x d )
"#;
    parse_rule(src).expect("should parse");
}

/// Nested repetitions with consistent placeholder nesting are valid.
#[test]
fn nested_repetitions_consistent_accepted() {
    let src = r#"
$ rule r
    ( $( a $( b $x )* )* )
    ( $( $( $x )* )* )
"#;
    parse_rule(src).expect("nested consistent repetitions should be valid");
}

/// Nested repetitions with inconsistent placeholder nesting are rejected.
#[test]
fn nested_repetitions_inconsistent_rejected() {
    let src = r#"
$ rule r
    ( $( a $( b $x )* )* )
    ( $( $x )* )
"#;
    let err = parse_rule(src).expect_err("should reject");
    assert!(err.contains("different nesting"), "got: {err}");
}

/// A placeholder used at two different nesting depths *within the body* is
/// rejected (the body's own `collect_body` consistency check, independent of
/// the pattern).
#[test]
fn body_internal_inconsistent_nesting_rejected() {
    let src = r#"
$ rule r
    ( a )
    ( $( $x )* $( $x )+ )
"#;
    let err = parse_rule(src).expect_err("should reject");
    assert!(err.contains("inconsistent nesting"), "got: {err}");
    assert!(err.contains("body"), "got: {err}");
}

/// A placeholder used at two different nesting depths *within a single
/// repeated arg list* of a pattern fact is rejected (the nested
/// `collect_arg` consistency check, exercised through the `Fact` arm of
/// `collect_pattern`).
#[test]
fn pattern_repeated_arg_inconsistent_nesting_rejected() {
    // The leading `prefix` makes this a `Fact` (not a top-level `$(` fact
    // repetition); the `$( $x $( $x )* )+` is a repeated-args whose inner
    // placeholder `$x` appears at two different nesting depths.
    let src = r#"
$ rule r
    ( prefix $( $x $( $x )* )+ )
    ( a )
"#;
    let err = parse_rule(src).expect_err("should reject");
    assert!(err.contains("inconsistent nesting"), "got: {err}");
    assert!(err.contains("pattern"), "got: {err}");
}
