use crate::Arg;
use crate::rule::{
    ArgTemplate, Body, BodyChunk, Pattern, PatternFact, PatternFactRepetition, PatternItem,
    RepeatBlock, RepeatedArgs,
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
        use crate::regex::RepetitionKind;
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
            base:take_indent()
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
            backtick_template() /
            args:template_curly_args() { args } /
            arg:literal_arg() { vec![arg] } /
            arg:plain_word() { vec![arg] }

        // A template string delimited by backticks. Single backticks (`` `…` ``)
        // hold a template string; triple backticks (`` ```…``` ``) open a fenced
        // block whose interior is dedented to the fence's indentation and whose
        // leading/trailing newlines are ignored. Both expand to the same `` ` ``
        // marker args plus interior chunks (literal text runs and `{…}` curly-
        // brace substitution sections).
        rule backtick_template() -> Vec<Arg> =
            f:fenced_template() { f } /
            s:single_backtick_template() { s }

        // A single-backtick template string. The interior is taken literally (it
        // may span newlines) with `{…}` curly-brace substitutions and `` \` `` /
        // `\{` / `\}` / `\\` escapes, until the closing backtick.
        rule single_backtick_template() -> Vec<Arg> =
            "`" chunks:single_bt_interior() "`"
            {
                let mut args = vec!["`".into()];
                args.extend(chunks);
                args.push("`".into());
                args
            }

        rule single_bt_interior() -> Vec<Arg> =
            cs:(
                t:bt_text() { TmplChunk::Text(t) }
                / c:template_curly_args() { TmplChunk::Curly(c) }
            )*
            { merge_tmpl(cs) }

        // A literal text run inside a single-backtick template: any character
        // that is not a curly brace or backtick (newlines included), with escapes.
        rule bt_text() -> Arg =
            substrs:(
                "\\`" { "`".to_string() } /
                "\\{" { "{".to_string() } /
                "\\}" { "}".to_string() } /
                "\\\\" { "\\".to_string() } /
                not_curlies() not_backtick() c:[_] { c.to_string() }
            )+
            { substrs.join("").as_str().into() }

        // A triple-backtick fenced block. The column of the opening fence
        // determines how many leading spaces are stripped from each interior
        // line, so the block can be indented under its containing fact and the
        // content still comes out flush-left. The leading newline (right after
        // the opening fence) and the trailing newline (right before the closing
        // fence) are ignored.
        rule fenced_template() -> Vec<Arg> =
            col:get_column() "```"
            "\n"?
            first:(fence_line_content(col - 1))?
            rest:(fence_line_sep(col - 1))*
            "\n"?
            fence_close()
            {
                let mut chunks = Vec::new();
                if let Some(f) = first { chunks.extend(f); }
                for r in rest { chunks.extend(r); }
                let mut args = vec!["`".into()];
                args.extend(merge_tmpl(chunks));
                args.push("`".into());
                args
            }

        // One line of a fenced block's interior (the first line: no leading
        // newline). Stops if the line is the closing fence.
        rule fence_line_content(strip: usize) -> Vec<TmplChunk> =
            !fence_close()
            strip_line_indent(strip)
            chunks:tmpl_line_chunks()
            { chunks }

        // A newline ending the previous line plus the next interior line. The
        // newline becomes a `\n` text chunk so multi-line content stays one
        // continuous literal run after merging.
        rule fence_line_sep(strip: usize) -> Vec<TmplChunk> =
            "\n"
            !fence_close()
            strip_line_indent(strip)
            chunks:tmpl_line_chunks()
            {
                let mut v = vec![TmplChunk::Text("\n".into())];
                v.extend(chunks);
                v
            }

        // The chunks of a single fenced interior line: zero or more literal
        // text runs and `{…}` curly-brace substitution sections.
        rule tmpl_line_chunks() -> Vec<TmplChunk> =
            cs:(
                t:fence_text_line() { TmplChunk::Text(t) }
                / c:template_curly_args() { TmplChunk::Curly(c) }
            )*
            { cs }

        // A literal text run on a single fenced line: any character that is not a
        // curly brace or newline, stopping at an un-escaped `` ``` `` (which
        // closes the fence wherever it appears). Single/double backticks are
        // literal; escape `` ``` `` as `` \``` `` to include it veritably.
        rule fence_text_line() -> Arg =
            substrs:(
                "\\{" { "{".to_string() } /
                "\\}" { "}".to_string() } /
                "\\```" { "```".to_string() } /
                "\\\\" { "\\".to_string() } /
                not_curlies() !("\n") !("```") c:[_] { c.to_string() }
            )+
            { substrs.join("").as_str().into() }

        // The closing fence: optional horizontal whitespace then `` ``` ``.
        // Unlike the old rule, the line need not end here — content after the
        // closing fence is parsed as regular arguments by the caller.
        rule fence_close() =
            (" " / "\t")* "```"

        // Strip up to `strip` leading spaces from the current line.
        rule strip_line_indent(strip: usize) = #{|input, pos| {
            let b = input.as_bytes();
            let mut p = pos;
            let mut n = 0;
            while p < b.len() && b[p] == b' ' && n < strip { p += 1; n += 1; }
            RuleResult::Matched(p, ())
        }}

        // Parse a batch of arguments inside of a curly-brace delimited section in a
        // template string.
        //
        // TODO: this doesn't match on all the kinds of valid syntax. For example parenthesis
        // literal strings should work inside curlies like they do outside.
        rule template_curly_args() -> Vec<Arg> =
            "{"
                " "*
                matched:(
                    arg:plain_word()
                ) ** " "
                " "*
            "}"
            {
                let mut args = Vec::new();
                args.push("{".into());
                args.extend(matched);
                args.push("}".into());
                args
            }


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
        rule punctuation() -> &'input str = $( "," / ";" / "." / "'" / ":" )

        // Helpers for negative lookahead
        rule not_brackets() = not_curlies() not_backtick() not_parens()
        rule not_curlies() = !("{" / "}")
        rule not_backtick() = !("`")
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
        rule take_indent() -> usize = #{|input, pos| {
            let b = input.as_bytes();
            let mut p = pos;
            while p < b.len() && b[p] == b' ' { p += 1; }
            RuleResult::Matched(p, p - pos)
        }}

        // Get the current column in the file without consuming anything.
        rule get_column() -> usize = #{|input, pos| {
            let previous_newline_or_start = input[..pos].rfind('\n').unwrap_or(0);
            let col = input[previous_newline_or_start..pos].chars().count() + 1;
            RuleResult::Matched(pos, col)
        }}

        // -----------------------------------------------------------------------
        // Rule pattern / body parsing
        // -----------------------------------------------------------------------

        // Parse a rule pattern from its literal string content. Indentation is
        // significant, mirroring file-level fact parsing: a pattern fact may
        // span multiple lines, and a line indented more than the fact's first
        // line continues that fact (appending its args). A `$( ... )` reached
        // as a continuation arg is an arg-level repetition; a `$( ... )` at a
        // fact's base indent (a sibling item) is a fact-level repetition.
        pub rule pattern() -> Pattern =
            sep()*
            items:(item:pattern_item() sep()* { item })*
            ws()
            { Pattern(items) }

        rule pattern_item() -> PatternItem =
            fact_repetition:pattern_fact_repetition() { PatternItem::FactRepetition(fact_repetition) } /
            fact:pattern_fact() { PatternItem::Fact(fact) }

        rule pattern_fact_repetition() -> PatternFactRepetition =
            (" " / "\t")* "$("
                (" " / "\t")* comment()? eol()?
                sep()*
                facts:(f:pattern_fact() sep()* { f })*
            ws() ")"
            marker:repetition_marker()
            (" " / "\t")* comment()? eol()
            { let (kind, greedy) = marker; PatternFactRepetition { kind, greedy, facts } }

        pub rule pattern_fact() -> PatternFact =
            base:take_indent() p:pattern_fact_prefix() first:arg_templates() comment()? fact_end()
            rest:(pattern_continued_line(base))*
            {
                let mut args = first;
                for r in rest { args.extend(r); }
                let (removed, negated) = p;
                PatternFact::new(removed, negated, args)
            }

        // The `-` (remove) / `!` (negate) / none prefix of a pattern fact.
        rule pattern_fact_prefix() -> (bool, bool) =
            "-" { (true, false) } /
            "!" { (false, true) } /
            "" { (false, false) }

        // A continuation line of a pattern fact: indented more than `base`, it
        // appends its args to the fact. A blank line, or a comment-only line at
        // greater indent, is a no-op continuation (mirrors `continued_line`).
        rule pattern_continued_line(base: usize) -> Vec<ArgTemplate> =
            (blank_line() / (greater_indent_than(base) comment() eol())) { vec![] } /
            greater_indent_than(base) args:arg_templates() comment()? fact_end() { args }

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

        // A chunk inside a `$( ... )` repetition. Here a `)` closes the
        // repetition only when it is followed by a repetition marker
        // (`)`, `)+`, `)?`, `)*`, …); otherwise it is literal text, so a
        // parenthesized arg like `( )` stays inside the block.
        rule body_chunk_in_repeat() -> BodyChunk =
            "$$" { BodyChunk::Text("$".to_string()) } /
            rep:body_repeat() { BodyChunk::Repeat(rep) } /
            ph:placeholder() { BodyChunk::Placeholder(ph) } /
            "$" { BodyChunk::Text("$".to_string()) } /
            text:$((
                !")" !"$" [_] /
                ")" !repetition_marker()
            )+) { BodyChunk::Text(text.to_string()) }

        rule body_repeat() -> RepeatBlock =
            "$("
                chunks:body_chunk_in_repeat()*
            ")"
            marker:repetition_marker()
            { let (kind, greedy) = marker; RepeatBlock { kind, greedy, chunks: merge_text(chunks) } }



        rule repetition_marker() -> (RepetitionKind, bool) =
            "??" { (RepetitionKind::Optional, true) } /
            "++" { (RepetitionKind::OneOrMore, true) } /
            "**" { (RepetitionKind::ZeroOrMore, true) } /
            "?" { (RepetitionKind::Optional, false) } /
            "+" { (RepetitionKind::OneOrMore, false) } /
            "*" { (RepetitionKind::ZeroOrMore, false) }
        // Parse a sequence of arg templates on a single line. Requires at least
        // one arg template; spaces between (and around) args are skipped.
        rule arg_templates() -> Vec<ArgTemplate> =
            " "* args:(arg:arg_template() " "* { arg })+ " "* { args }

        rule arg_template() -> ArgTemplate =
            repeated:arg_repetition() { ArgTemplate::RepeatedArgs(repeated) } /
            placeholder:placeholder() { ArgTemplate::Placeholder(placeholder) } /
            literal:literal_arg() { ArgTemplate::Literal(literal) } /
            literal:literal_word() { ArgTemplate::Literal(literal) }

        // Parse a sequence of arg templates that may span multiple lines,
        // tolerating whitespace (including newlines and indentation) between
        // and around the args. Used for the interior of an arg-level
        // `$( ... )` repetition, so a block that wraps to several lines under
        // a continuation indent parses as a single repetition of arguments
        // within the fact — not as a sibling fact-level repetition (which
        // would require each inner item to be its own fact).
        rule arg_templates_multi() -> Vec<ArgTemplate> =
            ws() args:(arg:arg_template() ws() { arg })+ ws() { args }

        rule arg_repetition() -> RepeatedArgs =
            "$("
                args:arg_templates_multi()
            ")"
            marker:repetition_marker()
            {
                let (kind, greedy) = marker;
                RepeatedArgs::new(kind, greedy, args)
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

/// A typed chunk of a template string's interior, used only during parsing so
/// that adjacent literal text runs (e.g. across the line breaks of a fenced
/// block) can be merged into a single argument before flattening to [`Arg`]s.
/// Curly-brace substitution sections break the runs so their inner word-split
/// arguments stay separate.
enum TmplChunk {
    Text(Arg),
    Curly(Vec<Arg>),
}

/// Flatten typed template chunks into arguments, merging consecutive `Text`
/// runs into one argument (so a fenced block's multi-line literal text becomes a
/// single arg, just like a single-backtick template's char run would).
fn merge_tmpl(chunks: Vec<TmplChunk>) -> Vec<Arg> {
    let mut out: Vec<Arg> = Vec::new();
    let mut text = String::new();
    for ch in chunks {
        match ch {
            TmplChunk::Text(t) => text.push_str(&t),
            TmplChunk::Curly(args) => {
                if !text.is_empty() {
                    out.push(text.as_str().into());
                    text.clear();
                }
                out.extend(args);
            }
        }
    }
    if !text.is_empty() {
        out.push(text.as_str().into());
    }
    out
}
