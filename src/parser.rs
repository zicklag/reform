use crate::rule::{ArgTemplate, Body, BodyChunk, Pattern, PatternFact, PatternFactRepetition, PatternItem, RepeatedArgs, RepeatBlock, RepetitionKind};
use crate::Arg;

pub use reform_parser::{facts, pattern, body};

peg::parser! {
    grammar reform_parser() for str {
        use peg::RuleResult;
        use crate::Fact;

        // A file is a sequence of facts.
        pub rule facts() -> Vec<Fact> =
            // We may start with any number of blank lines
            blank_line()*

            // Followed by a sequence of one or more facts
            facts:(fact()*)

            // And following with any number of blank lines
            blank_line()*

            // Return the parsed facts
            { facts }

        // A fact is a list of parsed arguments
        rule fact() -> Fact =
            // First we consume the current indentation level and record it
            // as the "base" implementation level of the fact.
            base:measure_indent()
            // then we can parse the first line
            first_line:line_args() eol()
            // The fact may or may not continue on the following lines.
            // The continued_line rule will only match on lines that are indented
            // under the base indentation level.
            rest:(continued_line(base))*
            {
                let mut v = first_line;
                for r in rest { v.extend(r); }
                Fact(v)
            }

        // Parse the arguments from a single line
        rule line_args() -> Vec<Arg> =
            // Collect batches of args separated by spaces.
            // They are batches instead of just "args" because template
            // args expand to multiple args
            args:(batch:line_arg_batch() " "* { batch } )+

            // Flatten ths list of batches into a list of args
            { args.into_iter().flat_map(|x| x.into_iter()).collect() }

        // Any valid line argument
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
            { substrs.join("").into() }


        // A literal arg is an arg with it's contents wrapped in parenthesis to make
        // it taken literally all in the same arg.
        rule literal_arg() -> Arg =
            "(" arg:literal_arg_inner() ")" { arg.into() }

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
                // The word must be one or more characters
                (
                    // Don't match any kind of brackets in plain words
                    not_brackets()
                    // And the word will stop at either a space or end of line,
                    // or punctuation followed by a space, without including
                    // the punctuation
                    !( punctuation()? (" " / eol()) )
                    // Everything else is matched by the word
                    [_]
                )+
            ) { dbg!(word.into()) } /
            // A single item of punctuation is also allowed
            p:punctuation(){ p.into() }

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
            // A blank line is a valid continued line, it just doesn't add any args
            blank_line() { vec![] } /
            // Otherwise we match only if this line is prefixed by more indentation than
            // the `base` and we parse the line's args followed by the end of the line.
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
            eol()
            { PatternFactRepetition { kind, facts } }

        rule pattern_fact() -> PatternFact =
            " "* "-" args:arg_templates() fact_end() { PatternFact { removed: true, args } } /
            " "* args:arg_templates() fact_end() { PatternFact { removed: false, args } }

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
            literal:literal_word() { ArgTemplate::Literal(literal) }

        rule arg_repetition() -> RepeatedArgs =
            "$("
                args:arg_templates()
            ")"
            kind:repetition_kind()
            { RepeatedArgs { kind, args } }

        rule placeholder() -> String =
            "$" name:$((!(" " / "\n" / "\t" / "#" / "$" / "(" / ")" / "?" / "+" / "*") [_])+)
            { name.to_string() }
        rule literal_word() -> Arg =
            word:$((!(" " / "\n" / "\t" / "#" / "$" / "(" / ")" / "?" / "+" / "*") [_])+)
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
        if let BodyChunk::Text(t) = &chunk {
            if let Some(BodyChunk::Text(prev)) = merged.last_mut() {
                prev.push_str(t);
                continue;
            }
        }
        merged.push(chunk);
    }
    merged
}
