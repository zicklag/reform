use crate::fact::Fact;

/// A parsed statement from a script file or REPL input.
#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    /// Assert a fact: `pred(arg1, arg2)`
    Assert(Fact),
    /// Retract a fact: `-pred(arg1, arg2)`
    Retract(Fact),
    /// Assert that a fact exists (crash if not): `assert pred(...)`
    AssertExists(Fact),
    /// Assert that a fact does NOT exist (crash if it does): `assert not pred(...)`
    AssertNot(Fact),
    /// Add a rule: `rule name: match1, match2 -> effect1, effect2`
    Rule {
        name: String,
        /// Comma-separated match patterns (with optional `-`/`!` prefixes)
        matches: String,
        /// Comma-separated effect patterns
        effects: String,
    },
    /// Run the fixed-point loop
    Run,
    /// Dump all facts
    Facts,
    /// Dump all rules
    Rules,
    /// Save a checkpoint
    Checkpoint,
    /// Restore to last checkpoint
    Restore,
    /// Load a script file
    Load(String),
    /// Quit
    Quit,
}

peg::parser! {
    grammar file_parser() for str {
        // ===== Top-level =====

        /// Parse a single statement from a line.
        pub rule statement() -> Stmt
            = _ s:stmt() _ { s }

        rule stmt() -> Stmt
            = rule_stmt()
            / assert_stmt()
            / retract_stmt()
            / command_stmt()
            / fact_stmt()
            / sentence_stmt()

        // ===== Commands =====

        rule command_stmt() -> Stmt
            = "run" !ident_char() { Stmt::Run }
            / "facts" !ident_char() { Stmt::Facts }
            / "rules" !ident_char() { Stmt::Rules }
            / "checkpoint" !ident_char() { Stmt::Checkpoint }
            / "restore" !ident_char() { Stmt::Restore }
            / "quit" !ident_char() { Stmt::Quit }
            / "exit" !ident_char() { Stmt::Quit }
            / "load" _ path:$((!newline() [_])+) { Stmt::Load(path.trim().to_string()) }

        // ===== Assert =====

        rule assert_stmt() -> Stmt
            = "assert" _ "not" _ f:fact() { Stmt::AssertNot(f) }
            / "assert" _ f:fact() { Stmt::AssertExists(f) }

        // ===== Retract =====

        rule retract_stmt() -> Stmt
            = "-" f:fact() { Stmt::Retract(f) }

        // ===== Rule =====

        rule rule_stmt() -> Stmt
            = "rule" _ name:ident() _ ":" _ matches:pattern_list() _ "->" _ effects:pattern_list() {
                Stmt::Rule {
                    name,
                    matches: matches.join(", "),
                    effects: effects.join(", "),
                }
            }

        // ===== Fact =====

        rule fact_stmt() -> Stmt
            = f:fact() { Stmt::Assert(f) }

        /// Parse a fact: `pred` or `pred(arg1, arg2, ...)`
        /// Bare ident only matches if it's the whole line (nothing else follows).
        rule fact() -> Fact
            = name:ident() _ "(" _ args:arg_list() _ ")" {
                let mut f = vec![name];
                f.extend(args);
                f
            }
            / name:ident() !(_ ident_char()) { vec![name] }

        /// Parse a comma-separated list of arguments (strings, no variables)
        rule arg_list() -> Vec<String>
            = a:arg_value() ** (_ "," _) { a }

        /// Parse a single argument value (no variables in facts)
        rule arg_value() -> String
            = quoted_string()
            / s:$(ident_char()+ / number()) { s.to_string() }

        // ===== Sentence fallback =====

        /// Fallback: any unrecognized line becomes a sentence fact.
        /// Each word is a separate argument.
        rule sentence_stmt() -> Stmt
            = words:sentence_words() {
                let mut fact = vec!["sentence".to_string()];
                fact.extend(words);
                Stmt::Assert(fact)
            }

        rule sentence_words() -> Vec<String>
            = w:$(ident_char()+ / number()) ++ _ { w.iter().map(|s| s.to_string()).collect() }

        // ===== Pattern list (for rules) =====

        /// Parse a comma-separated list of match/effect patterns.
        /// Each pattern may have `-` or `!` prefix.
        rule pattern_list() -> Vec<String>
            = p:pattern_entry() ** (_ "," _) { p }

        /// Parse a single pattern entry with optional prefix.
        rule pattern_entry() -> String
            = prefix:prefix()? p:pattern_str() {
                match prefix {
                    Some(pfx) => format!("{}{}", pfx, p),
                    None => p,
                }
            }

        rule prefix() -> &'input str
            = "-" { "-" }
            / "!" { "!" }

        /// Parse a single pattern string: `pred` or `pred(arg1, ?var, ..?rest)`
        /// or `?pred` or `?pred(arg1, ?var)`
        rule pattern_str() -> String
            = name:pattern_pred() _ "(" _ args:pattern_arg_list() _ ")" {
                format!("{}({})", name, args.join(", "))
            }
            / name:pattern_pred() { name }

        /// Parse a pattern predicate: `?name` or `name`
        rule pattern_pred() -> String
            = "?" n:ident() { format!("?{}", n) }
            / n:ident() { n }

        /// Parse a comma-separated list of pattern arguments
        rule pattern_arg_list() -> Vec<String>
            = a:pattern_arg() ** (_ "," _) { a }

        /// Parse a single pattern argument
        rule pattern_arg() -> String
            = rest_var()
            / quoted_string()
            / variable()
            / s:$(ident_char()+ / number()) { s.to_string() }

        rule rest_var() -> String
            = "..?" n:ident() { format!("..?{}", n) }

        rule variable() -> String
            = "?" n:ident() { format!("?{}", n) }

        rule quoted_string() -> String
            = "\"" s:$([^'"']*) "\"" { s.to_string() }

        // ===== Primitives =====

        rule ident() -> String
            = s:$(letter() (ident_char())*) { s.to_string() }

        rule number() -> &'input str
            = $("-"? digit()+ ("." digit()+)?)

        rule letter() -> char
            = ['a'..='z' | 'A'..='Z' | '_']

        rule ident_char() -> char
            = letter() / digit() / ['-']

        rule digit() -> char
            = ['0'..='9']

        /// Whitespace (including newlines for multi-line rules)
        rule _()
            = quiet!{ [' ' | '\t' | '\n' | '\r']* }

        rule newline()
            = ['\n' | '\r']
    }
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
mod tests {
    use super::*;

    #[test]
    fn parse_fact_bare() {
        assert_eq!(parse_stmt("n"), Some(Stmt::Assert(vec!["n".into()])));
    }

    #[test]
    fn parse_fact_with_args() {
        assert_eq!(
            parse_stmt("room(kitchen)"),
            Some(Stmt::Assert(vec!["room".into(), "kitchen".into()]))
        );
    }

    #[test]
    fn parse_retract() {
        assert_eq!(
            parse_stmt("-here(frontroom)"),
            Some(Stmt::Retract(vec!["here".into(), "frontroom".into()]))
        );
    }

    #[test]
    fn parse_rule() {
        let result = parse_stmt("rule go_north: -n, -here(?h), north_of(?g, ?h) -> here(?g)");
        assert!(result.is_some());
        match result.unwrap() {
            Stmt::Rule { name, matches, effects } => {
                assert_eq!(name, "go_north");
                assert_eq!(matches, "-n, -here(?h), north_of(?g, ?h)");
                assert_eq!(effects, "here(?g)");
            }
            other => panic!("expected Rule, got {:?}", other),
        }
    }

    #[test]
    fn parse_assert_exists() {
        assert_eq!(
            parse_stmt("assert here(frontroom)"),
            Some(Stmt::AssertExists(vec!["here".into(), "frontroom".into()]))
        );
    }

    #[test]
    fn parse_assert_not() {
        assert_eq!(
            parse_stmt("assert not here(kitchen)"),
            Some(Stmt::AssertNot(vec!["here".into(), "kitchen".into()]))
        );
    }

    #[test]
    fn parse_run() {
        assert_eq!(parse_stmt("run"), Some(Stmt::Run));
    }

    #[test]
    fn parse_facts() {
        assert_eq!(parse_stmt("facts"), Some(Stmt::Facts));
    }

    #[test]
    fn parse_quit() {
        assert_eq!(parse_stmt("quit"), Some(Stmt::Quit));
    }

    #[test]
    fn parse_load() {
        assert_eq!(
            parse_stmt("load demo/game.txt"),
            Some(Stmt::Load("demo/game.txt".into()))
        );
    }

    #[test]
    fn parse_comment() {
        assert_eq!(parse_stmt("# this is a comment"), None);
        assert_eq!(parse_stmt("// this is a comment"), None);
    }

    #[test]
    fn parse_empty() {
        assert_eq!(parse_stmt(""), None);
        assert_eq!(parse_stmt("   "), None);
    }

    #[test]
    fn parse_rule_with_negation() {
        let result = parse_stmt("rule infer: ?rel(?x, ?y), !instance(?x, ?domain) -> instance(?x, ?domain)");
        assert!(result.is_some());
        match result.unwrap() {
            Stmt::Rule { name, matches, .. } => {
                assert_eq!(name, "infer");
                assert!(matches.contains("!instance(?x, ?domain)"));
            }
            other => panic!("expected Rule, got {:?}", other),
        }
    }

    #[test]
    fn parse_rule_multi_effect() {
        let result = parse_stmt("rule test: a(?x), b(?y) -> c(?x), d(?y)");
        assert!(result.is_some());
        match result.unwrap() {
            Stmt::Rule { name, matches, effects } => {
                assert_eq!(name, "test");
                assert_eq!(matches, "a(?x), b(?y)");
                assert_eq!(effects, "c(?x), d(?y)");
            }
            other => panic!("expected Rule, got {:?}", other),
        }
    }

    #[test]
    fn parse_rule_variable_predicate() {
        let result = parse_stmt("rule infer: ?rel(?x, ?y), !instance(?x, ?domain) -> instance(?x, ?domain)");
        assert!(result.is_some());
    }

    #[test]
    fn parse_sentence() {
        let result = parse_stmt("go north");
        assert!(result.is_some());
        match result.unwrap() {
            Stmt::Assert(fact) => {
                assert_eq!(fact, vec!["sentence", "go", "north"]);
            }
            other => panic!("expected Assert(sentence), got {:?}", other),
        }
    }

    #[test]
    fn parse_sentence_multi_word() {
        let result = parse_stmt("take the apple");
        assert!(result.is_some());
        match result.unwrap() {
            Stmt::Assert(fact) => {
                assert_eq!(fact, vec!["sentence", "take", "the", "apple"]);
            }
            other => panic!("expected Assert(sentence), got {:?}", other),
        }
    }
}
