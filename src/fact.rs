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
    let mut in_single = false;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '\'' if !in_quotes => in_single = !in_single,
            '"' if !in_single => in_quotes = !in_quotes,
            '(' if !in_quotes && !in_single => depth += 1,
            ')' if !in_quotes && !in_single => depth -= 1,
            ',' if depth == 0 && !in_quotes && !in_single => {
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
/// Also substitutes `?var` patterns inside Atom strings (for rule-in-rule effects).
pub fn substitute(pattern: &Pattern, bindings: &Bindings) -> Fact {
    let mut result = Vec::new();
    for pat in pattern.iter() {
        match pat {
            Pat::Atom(s) => {
                // Substitute ?var references inside the atom string too
                let mut s = s.clone();
                for (name, vals) in bindings {
                    if vals.len() == 1 {
                        s = s.replace(&format!("?{name}"), &vals[0]);
                    }
                }
                result.push(s);
            }
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

/// Format a fact for display as a tuple: `(pred, arg1, arg2)`.
/// Arguments containing top-level commas are wrapped in parentheses.
pub fn format_fact(fact: &Fact) -> String {
    if fact.is_empty() {
        return String::new();
    }
    let args: Vec<String> = fact[1..]
        .iter()
        .map(|a| {
            if has_top_level_comma(a) {
                format!("({})", a)
            } else {
                a.clone()
            }
        })
        .collect();
    if args.is_empty() {
        format!("({})", fact[0])
    } else {
        format!("({}, {})", fact[0], args.join(", "))
    }
}

/// Check if a string contains a comma at depth 0 (not inside parentheses or quotes).
fn has_top_level_comma(s: &str) -> bool {
    let mut depth = 0;
    let mut in_quotes = false;
    let mut in_single = false;
    for c in s.chars() {
        match c {
            '\'' if !in_quotes => in_single = !in_single,
            '"' if !in_single => in_quotes = !in_quotes,
            '(' if !in_quotes && !in_single => depth += 1,
            ')' if !in_quotes && !in_single => depth -= 1,
            ',' if depth == 0 && !in_quotes && !in_single => return true,
            _ => {}
        }
    }
    false
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
/// Parse a pattern string like "(pred, arg1, ?var)" into a Pattern.
/// Delegates to the parser module's fact parser.
pub fn parse_pattern_from_str(s: &str) -> Option<Pattern> {
    crate::parser::parse_pattern(s)
}
