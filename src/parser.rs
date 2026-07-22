use crate::Arg;
use crate::rule::{
    ArgTemplate, Body, BodyChunk, Pattern, PatternFact, PatternFactRepetition, PatternItem,
    RepeatBlock, RepeatedArgs, RepetitionKind,
};

pub use reform_parser::{facts, pattern, pattern_fact};

/// Parse a rule body template. The body grammar is *infallible*: every input
/// string parses (any character that isn't part of a `$…` placeholder or
/// `$( … )` repetition is consumed as opaque literal text, including stray
/// `(`, `)`, and `$`). We expose the infallible signature so callers don't
/// carry a `Result` for a condition that can never occur.
pub fn body(src: &str) -> Body {
    reform_parser::body(src).expect("body parser is infallible")
}

peg::parser! {
    grammar reform_parser() for str {
        use peg::RuleResult;
        use crate::Fact;

        // A file is a sequence of facts, separated by blank or comment-only
        // lines.
        pub rule facts() -> Vec<Fact> =
            sep()*
            facts:(f:fact() sep()* { f })*
            (" " / "\t" / "\n")*
            { facts }

        // A fact is a list of parsed arguments
        rule fact() -> Fact =
            // Consume the current indentation level as the "base" indent.
            base:measure_indent()
            // Parse the first line (a trailing comment is allowed).
            first_line:line_args() comment()? eol()
            // The fact may continue on following, more-indented lines.
            rest:(continued_line(base))*
            {
                let mut v = first_line;
                for r in rest { v.extend(r); }
                Fact(v)
            }

        // A separator between facts: a blank line or a comment-only line.
        rule sep() = comment_line() / blank_line()
        // A comment-only line (at any indentation).
        rule comment_line() = (" " / "\t")* comment() eol()
        // A comment runs from `#` to the end of the line (newline not consumed).
        rule comment() = "#" (!eol() [_])*

        // Parse the arguments from a single line. A trailing comment is
        // allowed (consumed here so callers don't have to).
        rule line_args() -> Vec<Arg> =
            args:(batch:line_arg_batch() " "* { batch })+
            " "*
            comment()?
            { args.into_iter().flat_map(|x| x.into_iter()).collect() }
        rule line_arg_batch() -> Vec<Arg> =
            template_args() /
            arg:literal_arg() { vec![arg] } /
            arg:plain_word() { vec![arg] }

        // Parse a template arg, which is a special arg type delimited by square
        // brackets that will expand to multiple args
        rule template_args() -> Vec<Arg> =
            // A template starts with a square bracket
            "["
                // Inside are one or more chunks
                chunks:(
                    // There could be strech of literal string
                    arg:template_string_arg() { vec![arg] } /
                    // Or it could be a curly-brace delimited section of word-split
                    // args.
                    template_curly_args()
                )+
            // And ends with a square bracket
            "]"
            {
                let mut args = Vec::new();
                args.push("[".into());
                args.extend(chunks.into_iter().flatten());
                args.push("]".into());
                args
            }

        // Parse a batch of arguments inside of a curly-brace delimited section in a
        // template string.
        //
        // TODO: this doesn't match on all the kinds of valid syntax. For example parenthesis
        // literal strings should work inside curlies like they do outside.
        rule template_curly_args() -> Vec<Arg> =
            "{"
                matched:(
                    arg:plain_word()
                ) ** " "
            "}"
            {
                let mut args = Vec::new();
                args.push("{".into());
                args.extend(matched);
                args.push("}".into());
                args
            }

        // Parse a contiguous literal string in a template literal
        rule template_string_arg() -> Arg =
            substrs:(
                // Escaped braces
                "\\{" { "{".to_string() } /
                "\\}" { "}".to_string() } /
                "\\[" { "[".to_string() } /
                "\\]" { "]".to_string() } /
                // Escaped backslash
                "\\\\" { "\\".to_string() } /
                // Anything that isn't a curly brace or square bracket
                not_curlies() not_squares() char:[_] { char.into() }
            )+
            { substrs.join("").as_str().into() }


        // A literal arg is an arg with it's contents wrapped in parenthesis to make
        // it taken literally all in the same arg.
        rule literal_arg() -> Arg =
            "(" arg:literal_arg_inner() ")" { arg.as_str().into() }

        // Parses the inner content of a literal argument.
        // TODO(perf): it'd be nicest if this didn't allocate. Maybe we can return
        // an iterator.
        rule literal_arg_inner() -> String =
            substrs:(
                // Escaped parenthesis
                "\\(" { "(".into() } /
                "\\)" { ")".into() } /
                // Escaped backlash
                "\\\\" { "\\".to_string() } /
                // A balanced set of parenthes contianing more inner content
                "(" inner:literal_arg_inner() ")" { format!("({inner})") }  /
                // Anything that is not a parenthesis
                not_parens() char:[_] { char.into() }
            )* { substrs.join("") }

        // A plain word is one that does not need to be wrapped in parenthesis
        // and that parses to a single argument.
        rule plain_word() -> Arg =
            word:$(
                // One or more characters. The word stops at brackets, a
                // comment (`#`), a space / end of line, or punctuation that
                // is followed by a space / end of line.
                (
                    not_brackets()
                    !("#")
                    !( punctuation()? (" " / eol()) )
                    [_]
                )+
            ) { word.into() } /
            // A single item of punctuation is also allowed
            p:punctuation() { p.into() }

        // Matches normal sentence punctuation.
        rule punctuation() -> &'input str = $( ";" / "." / "'" / ":" )

        // Helpers for negative lookahead
        rule not_brackets() = not_curlies() not_squares() not_parens()
        rule not_curlies() = !("{" / "}")
        rule not_squares() = !("[" / "]")
        rule not_parens() = !( "(" / ")" )

        // Parse a line that is continuing a previous fact indented at the provided
        // `base` level.
        rule continued_line(base: usize) -> Vec<Arg> =
            // A blank or comment-only line is a valid continuation that adds
            // no arguments.
            (blank_line() / (greater_indent_than(base) comment() eol())) { vec![] } /
            // Otherwise match only if indented more than `base`, then parse
            // the line's args and the end of the line.
            greater_indent_than(base) args:line_args() eol()
            { args }

        // End of line: a newline, or end of input.
        rule eol() = "\n" / ![_]

        // A whitespace-only line.
        rule blank_line() = ("\t" / " ")* "\n"
        // Match on all of the leading spaces only if there are more than the
        // given `base` indentation.
        rule greater_indent_than(base: usize) = #{|input, pos| {
            let b = input.as_bytes();
            let mut p = pos;
            while p < b.len() && b[p] == b' ' { p += 1; }
            if p - pos > base { RuleResult::Matched(p, ()) }
            else              { RuleResult::Failed }
        }}

        // Match on all the leading spaces at the current position and return
        // the indent level.
        rule measure_indent() -> usize = #{|input, pos| {
            let b = input.as_bytes();
            let mut p = pos;
            while p < b.len() && b[p] == b' ' { p += 1; }
            RuleResult::Matched(p, p - pos)
        }}

        // -----------------------------------------------------------------------
        // Rule pattern / body parsing
        // -----------------------------------------------------------------------

        // Parse a rule pattern from its literal string content.
        pub rule pattern() -> Pattern =
            ws() items:(pattern_item())* ws() { Pattern(items) }

        rule pattern_item() -> PatternItem =
            fact_repetition:pattern_fact_repetition() { PatternItem::FactRepetition(fact_repetition) } /
            fact:pattern_fact() { PatternItem::Fact(fact) }

        rule pattern_fact_repetition() -> PatternFactRepetition =
            ws() "$("
                ws() facts:(pattern_fact())*
            ws() ")"
            kind:repetition_kind()
            (" " / "\t")* eol()
            { PatternFactRepetition { kind, facts } }

        pub rule pattern_fact() -> PatternFact =
            " "* "-" args:arg_templates() fact_end() { PatternFact::new(true, false, args) } /
            " "* "!" args:arg_templates() fact_end() { PatternFact::new(false, true, args) } /
            " "* args:arg_templates() fact_end() { PatternFact::new(false, false, args) }

        // Parse a rule body as a substitution template. The body is a flat
        // sequence of chunks: literal text, `$name` placeholders (substituted
        // from the pattern's bindings at fire time), and `$( ... )?/+/*`
        // repetition blocks (aligned with the pattern's repetitions). A
        // literal `$` in the output is written `$$`. The generated text is
        // later fed to `facts()` to produce real facts, so anything that
        // isn't a `$`-form placeholder or repetition is opaque literal text —
        // including parens, newlines, and the contents of generated (inner)
        // rules. Inner rules that need their own `$x`/`$( ... )` write them
        // as `$$x`/`$$( ... )`.
        pub rule body() -> Body =
            chunks:body_chunk()* { Body(merge_text(chunks)) }

        // A chunk at the top level of a body. A bare `)` is ordinary text
        // here: it only closes a `$( ... )` block when we are inside one.
        rule body_chunk() -> BodyChunk =
            "$$" { BodyChunk::Text("$".to_string()) } /
            rep:body_repeat() { BodyChunk::Repeat(rep) } /
            ph:placeholder() { BodyChunk::Placeholder(ph) } /
            "$" { BodyChunk::Text("$".to_string()) } /
            text:$((!"$" [_])+) { BodyChunk::Text(text.to_string()) }

        // A chunk inside a `$( ... )` repetition. Here a bare `)` closes the
        // repetition, so it is not consumed as text.
        rule body_chunk_in_repeat() -> BodyChunk =
            "$$" { BodyChunk::Text("$".to_string()) } /
            rep:body_repeat() { BodyChunk::Repeat(rep) } /
            ph:placeholder() { BodyChunk::Placeholder(ph) } /
            "$" { BodyChunk::Text("$".to_string()) } /
            text:$((!")" !"$" [_])+) { BodyChunk::Text(text.to_string()) }

        rule body_repeat() -> RepeatBlock =
            "$("
                chunks:body_chunk_in_repeat()*
            ")"
            kind:repetition_kind()
            { RepeatBlock { kind, chunks: merge_text(chunks) } }



        rule repetition_kind() -> RepetitionKind =
            "?" { RepetitionKind::Optional } /
            "+" { RepetitionKind::OneOrMore } /
            "*" { RepetitionKind::ZeroOrMore }
        // Parse a sequence of arg templates on a single line. Requires at least
        // one arg template; spaces between (and around) args are skipped.
        rule arg_templates() -> Vec<ArgTemplate> =
            " "* args:(arg:arg_template() " "* { arg })+ " "* { args }

        rule arg_template() -> ArgTemplate =
            repeated:arg_repetition() { ArgTemplate::RepeatedArgs(repeated) } /
            placeholder:placeholder() { ArgTemplate::Placeholder(placeholder) } /
            literal:literal_arg() { ArgTemplate::Literal(literal) } /
            literal:literal_word() { ArgTemplate::Literal(literal) }

        rule arg_repetition() -> RepeatedArgs =
            "$("
                args:arg_templates()
            ")"
            kind:repetition_kind()
            {
                let top_ph = crate::rule::top_placeholders(&args);
                RepeatedArgs { kind, args, top_ph }
            }

        rule placeholder() -> String =
            "$" name:$((!(" " / "\n" / "\t" / "#" / "$" / "(" / ")" / "?" / "+" / "*" / "." / "," / ";" / ":" / "'" / "!") [_])+)
            { name.to_string() }
        rule literal_word() -> Arg =
            word:$((!(" " / "\n" / "\t" / "#" / "$" / "(" / ")" / "?" / "+" / "*" / "!") [_])+)
            { word.into() }

        // End of a fact: a newline/EOF, or a closing `)` (lookahead, not consumed)
        // for facts that live inside a single-line `$( ... )?` block.
        rule fact_end() = eol() / &(")")

        // Whitespace (spaces, tabs, newlines) skipped around pattern/body items.
        rule ws() = (" " / "\t" / "\n")*
    }
}

/// Merge adjacent [`BodyChunk::Text`] chunks into a single `Text` chunk so the
/// body tree stays compact (e.g. a `$$` escape followed by a run of literal
/// text becomes one `Text`).
fn merge_text(chunks: Vec<BodyChunk>) -> Vec<BodyChunk> {
    let mut merged: Vec<BodyChunk> = Vec::new();
    for chunk in chunks {
        if let BodyChunk::Text(t) = &chunk
            && let Some(BodyChunk::Text(prev)) = merged.last_mut()
        {
            prev.push_str(t);
            continue;
        }
        merged.push(chunk);
    }
    merged
}
