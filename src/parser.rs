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
            = _ s:stmt() _ { s }

        /// Parse a statement
        rule stmt() -> Stmt =
            // Delete a fact
            f:del_fact() { Stmt::DeleteFact(f) } /
            // Add a fact
            f:fact() { Stmt::Fact(f) } /
            // Run a command
            cmd_stmt() /
            // Parse a prompt fact
            f:prompt() { Stmt::Fact(f) } /
            // Parse a sentence fact
            f:sentence() { Stmt::Fact(f) }

        /// Parse a sentence
        rule sentence() -> Fact = words:word() ++ _ {
            let mut f = vec!["sentence".to_owned()];
            f.extend(words);
            f
        }

        /// A word in a sentence is anything not whitespace separated by whitespace
        rule word() -> String = s:$((![' ' | '\t' | '\n' | '\r'] [_])+) { s.to_owned() }
        rule prompt() -> Fact = ">" _ words:word() ++ _ {
            let mut f = vec!["prompt".to_owned()];
            f.extend(words);
            f
        }

        /// Fact deletion
        rule del_fact() -> Fact = "-" f:fact() { f }

        /// Fact
        rule fact() -> Fact =
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
                    // Any char except parens, newlines
                    !['(' | ')' | '\n' | '\r'] [_]
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
            "facts" __ { Stmt::Facts } /
            "quit" __ { Stmt::Quit }

        /// Single line whitespace
        rule _() = [' ' | '\t' ]*

        /// Multi-line whitespace, including comments
        rule __() = ( [' ' | '\t' ] / newline() / line_comment() )*

        /// Line comment
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
    fn parse_lang() {
        file_parser::file(LANG_REF).unwrap();
    }
}
