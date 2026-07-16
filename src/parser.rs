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
                f.extend(words);
                for cont in continuation {
                    f.extend(cont);
                }
                f
            }

        /// A word in a sentence is anything not whitespace separated by whitespace.
        /// Balanced parentheses are treated as a single word.
        /// Punctuation characters (:;.) are split into their own words.
        /// A word in a sentence is anything not whitespace separated by whitespace.
        /// Balanced parentheses are treated as a single word, with internal
        /// newlines replaced by spaces.
        /// Punctuation characters (:;.) are split into their own words.
        rule word() -> String =
            // A balanced paren group — return content without outer parens
            "(" s:balanced_content() ")" { s.to_owned() } /
            // A punctuation character
            s:$([':' | ';' | '.']) { s.to_owned() } /
            // Any non-whitespace, non-punctuation chars
            s:$((![' ' | '\t' | '\n' | '\r' | ':' | ';' | '.'] [_])+) { s.to_owned() }
        rule prompt() -> Fact = ">" _ words:word() ++ _ {
            let mut f = vec!["prompt".to_owned()];
            f.extend(words);
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
