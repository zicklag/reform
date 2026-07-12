/// A pattern atom: either a literal string or a variable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pat {
    /// A literal string that must match exactly.
    Atom(String),
    /// A variable that matches anything and binds the value.
    Var(String),
    /// A rest variable that matches zero or more remaining arguments.
    /// Binds a `Vec<String>` of all remaining values.
    Rest(String),
}

/// A pattern is a list of pattern atoms.
/// The first atom is the predicate name.
pub type Pattern = Vec<Pat>;

/// A ground fact: a list of strings.
/// The first string is the predicate name.
pub type Fact = Vec<String>;

/// A binding from variable names to values.
pub type Bindings = Vec<(String, Vec<String>)>;

/// Split a comma-separated string into individual patterns, respecting
/// parentheses and quotes. This is used to parse match/effect pattern lists
/// stored in rule(...) facts.
pub fn split_patterns(s: &str) -> Vec<&str> {
    let mut result = Vec::new();
    let mut depth = 0;
    let mut in_quotes = false;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            '(' if !in_quotes => depth += 1,
            ')' if !in_quotes => depth -= 1,
            ',' if depth == 0 && !in_quotes => {
                result.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    result.push(&s[start..]);
    result
}

/// Check if a pattern matches a fact, returning bindings if it does.
pub fn match_pattern(pattern: &Pattern, fact: &Fact) -> Option<Bindings> {
    // If the pattern has a Rest, it can match any number of remaining fact elements.
    // Otherwise, lengths must match exactly.
    let has_rest = pattern.iter().any(|p| matches!(p, Pat::Rest(_)));
    if !has_rest && pattern.len() != fact.len() {
        return None;
    }
    if has_rest && pattern.len() > fact.len() {
        return None;
    }

    let mut bindings: Bindings = Vec::new();
    let mut fact_idx = 0;
    for pat in pattern.iter() {
        match pat {
            Pat::Atom(s) => {
                if fact_idx >= fact.len() || s != &fact[fact_idx] {
                    return None;
                }
                fact_idx += 1;
            }
            Pat::Var(name) => {
                if fact_idx >= fact.len() {
                    return None;
                }
                let val = &fact[fact_idx];
                // Check consistency with existing bindings
                if let Some((_, existing)) = bindings.iter().find(|(n, _)| n == name) {
                    if existing.len() != 1 || &existing[0] != val {
                        return None;
                    }
                } else {
                    bindings.push((name.clone(), vec![val.clone()]));
                }
                fact_idx += 1;
            }
            Pat::Rest(name) => {
                // Consume all remaining fact elements
                let remaining: Vec<String> = fact[fact_idx..].to_vec();
                // Check consistency with existing bindings
                if let Some((_, existing)) = bindings.iter().find(|(n, _)| n == name) {
                    if existing != &remaining {
                        return None;
                    }
                } else {
                    bindings.push((name.clone(), remaining));
                }
                fact_idx = fact.len(); // consumed everything
            }
        }
    }
    // If we didn't consume all fact elements and there's no rest, fail
    if fact_idx < fact.len() && !has_rest {
        return None;
    }
    Some(bindings)
}

/// Substitute variables in a pattern using bindings, producing a fact.
/// If a variable is not bound, it stays as a variable (for partial patterns).
pub fn substitute(pattern: &Pattern, bindings: &Bindings) -> Fact {
    let mut result = Vec::new();
    for pat in pattern.iter() {
        match pat {
            Pat::Atom(s) => result.push(s.clone()),
            Pat::Var(name) => {
                let val = bindings
                    .iter()
                    .find(|(n, _)| n == name)
                    .map(|(_, v)| v[0].clone())
                    .unwrap_or_else(|| format!("?{name}"));
                result.push(val);
            }
            Pat::Rest(name) => {
                if let Some((_, vals)) = bindings.iter().find(|(n, _)| n == name) {
                    result.extend(vals.clone());
                }
            }
        }
    }
    result
}

/// Format a fact for display.
pub fn format_fact(fact: &Fact) -> String {
    if fact.len() == 1 {
        fact[0].clone()
    } else {
        format!("{}({})", fact[0], fact[1..].join(", "))
    }
}

/// Format a pattern for display.
pub fn format_pattern(pattern: &Pattern) -> String {
    if pattern.is_empty() {
        return String::new();
    }
    let pred = match &pattern[0] {
        Pat::Atom(s) => s.clone(),
        Pat::Var(s) => format!("?{s}"),
        Pat::Rest(s) => format!("..?{s}"),
    };
    if pattern.len() == 1 {
        return pred;
    }
    let args: Vec<String> = pattern[1..]
        .iter()
        .map(|pat| match pat {
            Pat::Atom(s) => s.clone(),
            Pat::Var(s) => format!("?{s}"),
            Pat::Rest(s) => format!("..?{s}"),
        })
        .collect();
    format!("{}({})", pred, args.join(", "))
}

peg::parser! {
    grammar pattern_parser() for str {
        /// Parse a single pattern: "pred" or "pred(arg1, arg2, ...)"
        pub rule pattern() -> Pattern
            = p:predicate() { p }

        /// Parse a predicate with optional args: "pred" or "pred(arg1, arg2, ...)"
        rule predicate() -> Pattern
            = name:ident() ws() "(" ws() args:arg_list() ws() ")" {
                let mut p = vec![Pat::Atom(name)];
                p.extend(args);
                p
            }
            / name:ident() { vec![Pat::Atom(name)] }

        /// Parse a comma-separated list of arguments
        rule arg_list() -> Vec<Pat>
            = a:arg() ** (ws() "," ws()) { a }

        /// Parse a single argument: atom, variable, or rest variable
        rule arg() -> Pat
            = rest_var() / quoted_string() / variable() / atom()

        /// Parse a rest variable: "..?name"
        rule rest_var() -> Pat
            = "..?" n:ident() { Pat::Rest(n.to_string()) }

        /// Parse a quoted string: '"hello, world"'
        rule quoted_string() -> Pat
            = "\"" s:$([^'"']*) "\"" { Pat::Atom(s.to_string()) }

        /// Parse a variable: "?name"
        rule variable() -> Pat
            = "?" n:ident() { Pat::Var(n.to_string()) }

        /// Parse an atom: a bare identifier or number
        rule atom() -> Pat
            = s:$(letter() (letter() / digit() / "_" / "-")*) { Pat::Atom(s.to_string()) }
            / n:$("-"? digit()+ ("." digit()+)?) { Pat::Atom(n.to_string()) }

        /// Parse an identifier (starts with letter or underscore)
        rule ident() -> String
            = s:$(letter() (letter() / digit() / "_" / "-")*) { s.to_string() }

        /// Parse a letter
        rule letter() -> char
            = ['a'..='z' | 'A'..='Z' | '_']

        /// Parse a digit
        rule digit() -> char
            = ['0'..='9']

        /// Optional whitespace
        rule ws()
            = quiet!{ [' ' | '\t']* }
    }
}

/// Parse a pattern string like "pred(arg1, ?var)" into a Pattern.
pub fn parse_pattern_from_str(s: &str) -> Option<Pattern> {
    pattern_parser::pattern(s.trim()).ok()
}

/// Parse a fact string like "pred(arg1, arg2)" into a Fact.
pub fn parse_fact_from_str(s: &str) -> Option<Fact> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Try to parse as a structured fact: pred(arg1, arg2)
    if let Some(paren) = s.find('(') {
        // Check it ends with ')'
        if !s.ends_with(')') {
            return None;
        }
        let pred = &s[..paren];
        let args_str = &s[paren + 1..s.len() - 1];
        let mut fact = vec![pred.to_string()];
        // Split args respecting quotes
        let mut depth = 0;
        let mut in_quotes = false;
        let mut start = 0;
        for (i, c) in args_str.char_indices() {
            match c {
                '"' => in_quotes = !in_quotes,
                '(' if !in_quotes => depth += 1,
                ')' if !in_quotes => depth -= 1,
                ',' if depth == 0 && !in_quotes => {
                    let arg = args_str[start..i].trim();
                    if !arg.is_empty() {
                        fact.push(strip_quotes(arg));
                    }
                    start = i + 1;
                }
                _ => {}
            }
        }
        let last = args_str[start..].trim();
        if !last.is_empty() {
            fact.push(strip_quotes(last));
        }
        Some(fact)
    } else {
        // Bare word: "n" or "hello"
        Some(vec![s.to_string()])
    }
}

/// Strip surrounding quotes from a string if present.
fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}
