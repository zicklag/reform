use reform::engine::normal_form_fact;
use reform::parser::{body, facts};
use reform::rule::{BodyChunk, RepetitionKind};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse a single one-line fact.
fn fact(s: &str) -> reform::Fact {
    facts(s)
        .expect("fact parse")
        .into_iter()
        .next()
        .expect("one fact")
}

// ---------------------------------------------------------------------------
// Template arguments
// ---------------------------------------------------------------------------

/// A basic `[...]` template with literal text becomes a bracketed arg list.
#[test]
fn basic_template() {
    let f = fact("[hello world]");
    assert_eq!(f.len(), 3);
    assert_eq!(&f[0], "[");
    assert_eq!(&f[1], "hello world");
    assert_eq!(&f[2], "]");
}

/// A template with `{...}` curly-brace substitution splits the interior into
/// brace-delimited word-split args.
#[test]
fn template_with_curly_substitution() {
    let f = fact("[hello {name} world]");
    assert_eq!(f.len(), 7);
    assert_eq!(&f[0], "[");
    assert_eq!(&f[1], "hello ");
    assert_eq!(&f[2], "{");
    assert_eq!(&f[3], "name");
    assert_eq!(&f[4], "}");
    assert_eq!(&f[5], " world");
    assert_eq!(&f[6], "]");
}

/// Escaped braces `\{` and `\}` inside a template produce literal braces.
#[test]
fn escaped_braces_in_template() {
    let f = fact("[hello \\{name\\} world]");
    assert_eq!(f.len(), 3);
    assert_eq!(&f[0], "[");
    assert_eq!(&f[1], "hello {name} world");
    assert_eq!(&f[2], "]");
}

/// Escaped brackets `\[` and `\]` inside a template produce literal brackets.
#[test]
fn nested_balanced_brackets_in_template() {
    let f = fact("[hello \\[world\\] stuff]");
    assert_eq!(f.len(), 3);
    assert_eq!(&f[0], "[");
    assert_eq!(&f[1], "hello [world] stuff");
    assert_eq!(&f[2], "]");
}

// ---------------------------------------------------------------------------
// Multi-line facts
// ---------------------------------------------------------------------------

/// A fact can continue on indented lines; each indented line's args are
/// appended.
#[test]
fn multi_line_indentation_continuation() {
    let f = fact("hello\n  world\n  foo");
    assert_eq!(f.len(), 3);
    assert_eq!(&f[0], "hello");
    assert_eq!(&f[1], "world");
    assert_eq!(&f[2], "foo");
}

/// A blank line separates facts; two single-line facts with a blank between
/// them produce two facts.
#[test]
fn multi_line_blank_line_separator() {
    let parsed = facts("hello\n\nworld").expect("parse");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed[0].len(), 1);
    assert_eq!(&parsed[0][0], "hello");
    assert_eq!(parsed[1].len(), 1);
    assert_eq!(&parsed[1][0], "world");
}

/// A comment-only continuation line is skipped; the fact continues on the
/// next indented line.
#[test]
fn multi_line_comment_in_continuation() {
    let f = fact("hello\n  # comment\n  world");
    assert_eq!(f.len(), 2);
    assert_eq!(&f[0], "hello");
    assert_eq!(&f[1], "world");
}

// ---------------------------------------------------------------------------
// Comments
// ---------------------------------------------------------------------------

/// Comments in various positions are ignored.
#[test]
fn comments_various_positions() {
    // Full-line comment before a fact
    let parsed = facts("# comment\nhello").expect("parse");
    assert_eq!(parsed.len(), 1);
    assert_eq!(&parsed[0][0], "hello");

    // Comment between two facts
    let parsed = facts("hello\n# comment\nworld").expect("parse");
    assert_eq!(parsed.len(), 2);
    assert_eq!(&parsed[0][0], "hello");
    assert_eq!(&parsed[1][0], "world");

    // Trailing comment on each fact
    let parsed = facts("hello # first\nworld # second").expect("parse");
    assert_eq!(parsed.len(), 2);
    assert_eq!(&parsed[0][0], "hello");
    assert_eq!(&parsed[1][0], "world");
}

// ---------------------------------------------------------------------------
// Punctuation splitting
// ---------------------------------------------------------------------------

/// A trailing comma (followed by space/eol) splits into a separate arg.
#[test]
fn trailing_comma_splits() {
    let f = fact("hello ,");
    assert_eq!(f.len(), 2);
    assert_eq!(&f[0], "hello");
    assert_eq!(&f[1], ",");
}

/// A trailing period (followed by space/eol) splits into a separate arg.
#[test]
fn trailing_period_splits() {
    let f = fact("hello .");
    assert_eq!(f.len(), 2);
    assert_eq!(&f[0], "hello");
    assert_eq!(&f[1], ".");
}

/// A dot inside a word (not followed by space/eol) stays part of the word.
#[test]
fn domain_name_keeps_dot() {
    let f = fact("example.com");
    assert_eq!(f.len(), 1);
    assert_eq!(&f[0], "example.com");
}

// ---------------------------------------------------------------------------
// Body parsing
// ---------------------------------------------------------------------------
/// `$$` in a body produces a literal `$` text chunk.
#[test]
fn body_double_dollar_is_literal_dollar() {
    let b = body("$$");
    assert_eq!(b.len(), 1);
    assert!(matches!(&b.0[..], [BodyChunk::Text(t)] if t == "$"), "got {:?}", b);
}

/// A bare `$` in a body (not followed by a valid placeholder name) is
/// literal text.
#[test]
fn body_bare_dollar_is_literal() {
    let b = body("$");
    assert_eq!(b.len(), 1);
    assert!(matches!(&b.0[..], [BodyChunk::Text(t)] if t == "$"), "got {:?}", b);
}

/// An empty body produces no chunks.
#[test]
fn body_empty() {
    let b = body("");
    assert_eq!(b.len(), 0);
}


// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

/// A blank line produces no facts.
#[test]
fn empty_fact_blank_line() {
    let parsed = facts("\n").expect("parse");
    assert_eq!(parsed.len(), 0);
}

/// A fact consisting of only punctuation.
#[test]
fn fact_only_punctuation() {
    let f = fact(".");
    assert_eq!(f.len(), 1);
    assert_eq!(&f[0], ".");
}

/// An empty parenthesized arg `()` produces an empty string arg.
#[test]
fn empty_parenthesized_arg() {
    let f = fact("()");
    assert_eq!(f.len(), 1);
    assert_eq!(&f[0], "");
    // normal_form_fact should round-trip: empty arg -> `()`
    assert_eq!(normal_form_fact(&f), "()");
}

/// Escaped parens `\(` and `\)` inside a literal arg produce literal parens.
#[test]
fn escaped_parens_in_literal_arg() {
    let f = fact("(\\(hello\\))");
    assert_eq!(f.len(), 1);
    assert_eq!(&f[0], "(hello)");
}

/// Double-paren `((example))` — the outer parens delimit a literal arg, the
/// inner parens are balanced and become part of the arg value.
#[test]
fn double_paren_literal_parens() {
    let f = fact("((example))");
    assert_eq!(f.len(), 1);
    assert_eq!(&f[0], "(example)");
}

// -- escaped backslash in template -------------------------------------------

/// Escaped backslash `\\\\` in a template produces a literal backslash.
#[test]
fn escaped_backslash_in_template() {
    let f = fact("[a\\\\b]");
    // Template: [ a\\b ] -> args: [, a\b, ]
    assert_eq!(f.len(), 3);
    assert_eq!(&f[1], "a\\b");
}

// -- escaped backslash in literal arg ----------------------------------------

/// Escaped backslash `\\\\` in a literal arg produces a literal backslash.
#[test]
fn escaped_backslash_in_literal_arg() {
    let f = fact("(a\\\\b)");
    assert_eq!(f.len(), 1);
    assert_eq!(&f[0], "a\\b");
}

// -- body $$ and $ alone in repeat -------------------------------------------

/// `$$` inside a `$( ... )` repeat block produces a literal `$` text chunk.
#[test]
fn body_double_dollar_in_repeat() {
    use reform::parser::body;
    let b = body("$( $$x )*");
    assert_eq!(b.len(), 1);
    assert!(
        matches!(&b.0[..], [BodyChunk::Repeat(r)] if r.kind == RepetitionKind::ZeroOrMore
            && matches!(&r.chunks[..], [BodyChunk::Text(t)] if t == " $x ")),
        "got {:?}", b
    );
}

/// A bare `$` inside a `$( ... )` repeat block is literal text.
#[test]
fn body_bare_dollar_in_repeat() {
    use reform::parser::body;
    let b = body("$( $ )*");
    assert_eq!(b.len(), 1);
    assert!(
        matches!(&b.0[..], [BodyChunk::Repeat(r)] if r.kind == RepetitionKind::ZeroOrMore
            && matches!(&r.chunks[..], [BodyChunk::Text(t)] if t == " $ ")),
        "got {:?}", b
    );
}

// -- normal_form_arg backslash escape ----------------------------------------

/// `normal_form_arg` escapes backslashes in arguments that need wrapping.
#[test]
fn normal_form_arg_backslash_escape() {
    use reform::normal_form_arg;
    use reform::Arg;
    // Backslash in an arg that needs parens (has trailing period)
    assert_eq!(normal_form_arg(&Arg::from("a\\b.")), "(a\\\\b.)");
    // Backslash in an arg that doesn't need parens stays clean
    assert_eq!(normal_form_arg(&Arg::from("a\\b")), "a\\b");
}
