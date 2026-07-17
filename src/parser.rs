type Fact = Vec<String>;

/// A parsed statement from a script file or REPL input.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Assert a fatct
    Fact(Fact),
    /// Retract a fact
    DeleteFact(Fact),
    /// Crash if the given fact does not exist
    Assert(Fact),
    /// Crash if the given fact exists
    AssertNot(Fact),
    /// Load a reform file
    Load(String),
    /// Find facts matching a pattern
    Find(String),
    /// Print out all fcts
    Facts,
    /// Quit
    Quit,
}

/// Expand a template string `[...]` into multiple words.
///
/// Text between `{...}` blocks becomes `(text)` — a single parenthesized word.
/// `{...}` blocks themselves become `{`, words..., `}` — the braces as separate
/// punctuation words with the inner content split by whitespace.
///
/// Example: `[It is very much {if locked}locked{otherwise}open{end if}]`
/// expands to: `(It is very much )`, `{`, `if`, `locked`, `}`, `(locked)`,
/// `{`, `otherwise`, `}`, `(open)`, `{`, `end`, `if`, `}`
fn expand_template(s: &str) -> Vec<String> {
    let mut result = Vec::new();
    let mut remaining = s;

    while !remaining.is_empty() {
        if let Some(open_idx) = remaining.find('{') {
            // Check if there's a matching closing brace
            if let Some(close_idx) = remaining[open_idx..].find('}') {
                // Text before the brace — wrap in parens as a single word
                if open_idx > 0 {
                    result.push(format!("({})", &remaining[..open_idx]));
                }
                let inner = &remaining[open_idx + 1..open_idx + close_idx];
                // Emit {, inner words, }
                result.push("{".to_string());
                for word in inner.split_whitespace() {
                    if !word.is_empty() {
                        result.push(word.to_string());
                    }
                }
                result.push("}".to_string());
                remaining = &remaining[open_idx + close_idx + 1..];
            } else {
                // No closing brace — treat the whole remaining text as literal
                result.push(format!("({})", remaining));
                remaining = "";
            }
        } else {
            // No more braces — rest is literal text
            result.push(format!("({})", remaining));
            remaining = "";
        }
    }

    result
}

peg::parser! {
    grammar file_parser() for str {
        /// Parse a reform file
        pub rule file() -> Vec<Stmt> =
            // A list of statements separated by whitespace and surrounded by whitespace
            __ stmt:statement() ** __ __ { stmt }

        /// Parse a single statement from a line.
        pub rule statement() -> Stmt
            = _ s:stmt() { s }

        /// Parse a statement
        rule stmt() -> Stmt =
            // Delete a fact
            f:del_fact() (newline() / ![_]) { Stmt::DeleteFact(f) } /
            // Add a fact
            f:fact() (newline() / ![_]) { Stmt::Fact(f) } /
            // Run a command
            cmd:cmd_stmt() (newline() / ![_]) { cmd } /
            // Parse a prompt fact
            f:prompt() (newline() / ![_]) { Stmt::Fact(f) } /
            // Parse a sentence fact
            f:sentence() (newline() / ![_]) { Stmt::Fact(f) }

        /// Parse a sentence, with indented continuation lines joined as one.
        rule sentence() -> Fact =
            words:word() ++ _ continuation:(newline() indent() words:word() ++ _ { words })* {
                let mut f = vec!["sentence".to_owned()];
                for w in words { f.extend(w); }
                for cont in continuation {
                    for w in cont { f.extend(w); }
                }
                f
            }

        /// A word in a sentence. Returns a Vec because template strings `[...]`
        /// can expand into multiple words.
        ///
        /// - `(...)` — balanced paren group, single word (content without parens)
        /// - `[...]` — template string, expands into multiple words
        /// - `{` / `}` / `:` / `;` / `.` — punctuation, single word
        /// - anything else — non-whitespace non-punctuation, single word
        rule word() -> Vec<String> =
            // A template string — expands into multiple words
            "[" s:$((!['[' | ']'] [_])*) "]" { expand_template(s) } /
            // A balanced paren group — return content without outer parens
            "(" s:balanced_content() ")" { vec![s.to_owned()] } /
            // A punctuation character
            s:$([':' | ';' | '.' | '{' | '}']) { vec![s.to_owned()] } /
            // Any non-whitespace, non-punctuation chars
            s:$((![' ' | '\t' | '\n' | '\r' | ':' | ';' | '.' | '{' | '}'] [_])+) { vec![s.to_owned()] }

        rule prompt() -> Fact = ">" _ words:word() ++ _ {
            let mut f = vec!["prompt".to_owned()];
            for w in words { f.extend(w); }
            f
        }

        /// Fact deletion
        rule del_fact() -> Fact = "-" f:fact() { f }

        /// Fact
        pub rule fact() -> Fact =
            "(" words:fact_arg() ** "," ","? __ ")"
            { words }

        /// Parse a fact argument, allowing parenthesis to wrap around
        /// the arg to group the special characters inside.
        rule fact_arg() -> String =
            // Match on an arg surrounded in parens: capture balanced content
            __ "(" s:balanced_content() ")" __ { s.into() } /

            // Not an open paren
            !"("
            __
            s:$(
                (
                    // A nested balanced group
                    "(" balanced_content() ")" /
                    // Any char except newlines, commas, or right parens
                    !['\n' | '\r' | ',' | ')'] [_]
                )+
            )
            __
            { s.into() }

        /// Match balanced parentheses content (no top-level commas).
        /// Returns the raw text between the outer parens.
        rule balanced_content() -> &'input str =
            s:$(
                (
                    // A nested balanced group
                    "(" balanced_content() ")" /
                    // Any char except parens
                    !['(' | ')'] [_]
                )*
            ) { s }


        /// A command statement
        rule cmd_stmt() -> Stmt = "$" _ c:cmd() { c }

        /// A particular command
        rule cmd() -> Stmt =
            "assert" _ f:fact() _ { Stmt::Assert(f) } /
            "assert not" _ f:fact() _ { Stmt::AssertNot(f) } /
            "load" _ file:$((!(newline()) [_])+) _ { Stmt::Load(file.to_owned()) } /
            "find" pattern:$((!newline() [_])+) _ { Stmt::Find(pattern.to_owned()) } /
            "facts" _ { Stmt::Facts } /
            "quit" _ { Stmt::Quit }

        /// Single line whitespace
        rule _() = [' ' | '\t' ]*

        /// Multi-line whitespace, including comments
        rule __() = ( [' ' | '\t' ] / newline() / line_comment() )*

        /// Line comment

        /// Indentation: one or more leading spaces or tabs (start of line).
        rule indent() = [' ' | '\t']+
        rule line_comment() = "#" (!newline() [_])*

        /// Newline
        rule newline() = ['\n' | '\r']
    }
}

pub fn parse_file(s: &str) -> Result<Vec<Stmt>, peg::error::ParseError<peg::str::LineCol>> {
    file_parser::file(s)
}

/// Parse a single statement from a line of input.
pub fn parse_stmt(input: &str) -> Option<Stmt> {
    let input = input.trim();
    if input.is_empty() || input.starts_with('#') || input.starts_with("//") {
        return None;
    }
    file_parser::statement(input).ok()
}

#[cfg(test)]
mod test {
    use super::*;

    const LANG_REF: &str = include_str!("../demo/lang.rf");

    #[test]
    fn sentence_with_indented_continuation() {
        let input = "say (hello\n  world\n  foo)\n";
        let stmts = parse_file(input).unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::Fact(f) = &stmts[0] {
            assert_eq!(f[0], "sentence");
            // Newlines inside balanced parens are preserved literally
            assert_eq!(f[1..], ["say", "hello\n  world\n  foo"]);
        } else {
            panic!("expected a fact, got {:?}", stmts[0]);
        }
    }

    #[test]
    fn fact_not_affected_by_indentation() {
        let input = "(rule, test,\n  ( -(sentence, ?x) ),\n  ( (?x) )\n)\n";
        let stmts = parse_file(input).unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::Fact(f) = &stmts[0] {
            assert_eq!(f[0], "rule");
            assert_eq!(f[1], "test");
        } else {
            panic!("expected a fact, got {:?}", stmts[0]);
        }
    }

    #[test]
    fn sentence_indented_after_blank_line() {
        let input = "first\n\n  indented\n";
        let stmts = parse_file(input).unwrap();
        assert_eq!(stmts.len(), 2);
        if let Stmt::Fact(f) = &stmts[0] {
            assert_eq!(f[0], "sentence");
            assert_eq!(f[1..], ["first"]);
        } else {
            panic!("expected a fact, got {:?}", stmts[0]);
        }
        if let Stmt::Fact(f) = &stmts[1] {
            assert_eq!(f[0], "sentence");
            assert_eq!(f[1..], ["indented"]);
        } else {
            panic!("expected a fact, got {:?}", stmts[1]);
        }
    }

    #[test]
    fn sentence_indented_after_comment() {
        let input = "first\n# comment\n  indented\n";
        let stmts = parse_file(input).unwrap();
        assert_eq!(stmts.len(), 2);
        if let Stmt::Fact(f) = &stmts[0] {
            assert_eq!(f[0], "sentence");
            assert_eq!(f[1..], ["first"]);
        } else {
            panic!("expected a fact, got {:?}", stmts[0]);
        }
        if let Stmt::Fact(f) = &stmts[1] {
            assert_eq!(f[0], "sentence");
            assert_eq!(f[1..], ["indented"]);
        } else {
            panic!("expected a fact, got {:?}", stmts[1]);
        }
    }

    #[test]
    fn expand_template_simple() {
        let result = expand_template("It is very much {if locked}locked{otherwise}open{end if}");
        assert_eq!(result, vec![
            "(It is very much )",
            "{",
            "if",
            "locked",
            "}",
            "(locked)",
            "{",
            "otherwise",
            "}",
            "(open)",
            "{",
            "end",
            "if",
            "}",
        ]);
    }

    #[test]
    fn expand_template_no_braces() {
        let result = expand_template("plain text only");
        assert_eq!(result, vec!["(plain text only)"]);
    }

    #[test]
    fn expand_template_empty() {
        let result = expand_template("");
        assert!(result.is_empty());
    }

    #[test]
    fn expand_template_leading_braces() {
        let result = expand_template("{if x}then{end if}");
        assert_eq!(result, vec![
            "{",
            "if",
            "x",
            "}",
            "(then)",
            "{",
            "end",
            "if",
            "}",
        ]);
    }

    #[test]
    fn expand_template_trailing_text() {
        let result = expand_template("{if x}then{end if}after");
        assert_eq!(result, vec![
            "{",
            "if",
            "x",
            "}",
            "(then)",
            "{",
            "end",
            "if",
            "}",
            "(after)",
        ]);
    }

    #[test]
    fn expand_template_unclosed_brace() {
        let result = expand_template("text {unclosed");
        assert_eq!(result, vec!["(text {unclosed)"]);
    }

    #[test]
    fn sentence_with_template_string() {
        let input = "The description is [It is very much {if locked}locked{otherwise}open{end if}].\n";
        let stmts = parse_file(input).unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::Fact(f) = &stmts[0] {
            assert_eq!(f[0], "sentence");
            assert_eq!(f[1..], [
                "The", "description", "is",
                "(It is very much )",
                "{", "if", "locked", "}",
                "(locked)",
                "{", "otherwise", "}",
                "(open)",
                "{", "end", "if", "}",
                ".",
            ]);
        } else {
            panic!("expected a fact, got {:?}", stmts[0]);
        }
    }

    #[test]
    fn sentence_with_template_no_braces() {
        let input = "The description is [plain text].\n";
        let stmts = parse_file(input).unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::Fact(f) = &stmts[0] {
            assert_eq!(f[0], "sentence");
            assert_eq!(f[1..], ["The", "description", "is", "(plain text)", "."]);
        } else {
            panic!("expected a fact, got {:?}", stmts[0]);
        }
    }

    #[test]
    fn sentence_with_template_and_parens() {
        let input = "The description is [It is (very) much {if x}special{end if}].\n";
        let stmts = parse_file(input).unwrap();
        assert_eq!(stmts.len(), 1);
        if let Stmt::Fact(f) = &stmts[0] {
            assert_eq!(f[0], "sentence");
            assert_eq!(f[1..], [
                "The", "description", "is",
                "(It is (very) much )",
                "{", "if", "x", "}",
                "(special)",
                "{", "end", "if", "}",
                ".",
            ]);
        } else {
            panic!("expected a fact, got {:?}", stmts[0]);
        }
    }
}

/// Parse a pattern tuple like `(pred, arg1, ?var)` into a Pattern.
/// Reuses the parser's fact() rule to parse the tuple, then converts
/// each string to a Pat by checking for `?`/`..?` prefixes.
pub fn parse_pattern(s: &str) -> Option<crate::fact::Pattern> {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') {
        return None;
    }
    let fact = file_parser::fact(s).ok()?;
    Some(
        fact.into_iter()
            .map(|arg| {
                // Check for optional bracket syntax: [?var] or [literal]
                if let Some(inner) = arg.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                    if let Some(var) = inner.strip_prefix('?') {
                        crate::fact::Pat::OptionalVar(var.to_owned())
                    } else {
                        crate::fact::Pat::OptionalAtom(inner.to_owned())
                    }
                } else if let Some(rest) = arg.strip_prefix("..?") {
                    crate::fact::Pat::Rest(rest.to_owned())
                } else if let Some(var) = arg.strip_prefix('?') {
                    crate::fact::Pat::Var(var.to_owned())
                } else {
                    crate::fact::Pat::Atom(arg)
                }
            })
            .collect(),
    )
}

/// Parse a fact tuple string like `"(pred, arg1, arg2)"` into its elements.
/// Returns None if the string is not a valid fact tuple.
pub fn parse_fact_str(s: &str) -> Option<Vec<String>> {
    file_parser::fact(s).ok()
}

