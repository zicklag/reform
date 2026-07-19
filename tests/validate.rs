use reform::rule::Rule;

fn parse_rule(src: &str) -> Result<Rule, String> {
    let facts = reform::parser::facts(src).expect("facts parse");
    let f = &facts[0];
    let args: Vec<&str> = f.iter().map(|a| &**a).collect();
    // strip a leading `$`
    let rule_args = if args[0] == "$" { &args[1..] } else { &args[..] };
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
    assert!(
        err.contains("inconsistent nesting"),
        "got: {err}"
    );
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