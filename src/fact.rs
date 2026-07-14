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
/// Supports rest patterns (`..?name`) anywhere in the pattern, including
/// multiple rest patterns and rest patterns followed by non-rest arguments.
/// Rest patterns match the shortest number of elements that allows the
/// remainder of the pattern to match (shortest-match semantics).
pub fn match_pattern(pattern: &Pattern, fact: &Fact) -> Option<Bindings> {
    let mut bindings = Bindings::new();
    if try_match(pattern, fact, 0, 0, &mut bindings) {
        Some(bindings)
    } else {
        None
    }
}

/// Recursive helper: try to match `pattern[pat_idx..]` against `fact[fact_idx..]`,
/// accumulating bindings. Backtracks on rest variables to find the shortest match.
fn try_match(
    pattern: &Pattern,
    fact: &Fact,
    pat_idx: usize,
    fact_idx: usize,
    bindings: &mut Bindings,
) -> bool {
    // All pattern elements consumed — must also have consumed all fact elements.
    if pat_idx >= pattern.len() {
        return fact_idx >= fact.len();
    }

    let pat = &pattern[pat_idx];

    match pat {
        Pat::Atom(s) => {
            if fact_idx >= fact.len() || s != &fact[fact_idx] {
                return false;
            }
            try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings)
        }
        Pat::Var(name) => {
            if fact_idx >= fact.len() {
                return false;
            }
            let val = &fact[fact_idx];
            // Check consistency with existing bindings
            if let Some(idx) = bindings.iter().position(|(n, _)| n == name) {
                let (_, existing) = &bindings[idx];
                if existing.len() != 1 || existing[0] != *val {
                    return false;
                }
                try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings)
            } else {
                let saved_len = bindings.len();
                bindings.push((name.clone(), vec![val.clone()]));
                if try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings) {
                    return true;
                }
                // Backtrack: remove the binding we just added
                bindings.truncate(saved_len);
                false
            }
        }
        Pat::Rest(name) => {
            // Try consuming 0, 1, 2, ... remaining fact elements (shortest first).
            let remaining = fact.len() - fact_idx;
            for consume in 0..=remaining {
                let vals: Vec<String> = fact[fact_idx..fact_idx + consume].to_vec();

                // Check consistency with existing bindings
                if let Some(idx) = bindings.iter().position(|(n, _)| n == name) {
                    let (_, existing) = &bindings[idx];
                    if &existing != &&vals {
                        continue;
                    }
                } else {
                    let saved_len = bindings.len();
                    bindings.push((name.clone(), vals));
                    if try_match(pattern, fact, pat_idx + 1, fact_idx + consume, bindings) {
                        return true;
                    }
                    // Backtrack: remove the binding we just added
                    bindings.truncate(saved_len);
                    continue;
                }

                // Binding already existed and matched — just recurse
                if try_match(pattern, fact, pat_idx + 1, fact_idx + consume, bindings) {
                    return true;
                }
            }
            false
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pat(s: &str) -> Pattern {
        if let Some(paren) = s.find('(') {
            let pred = &s[..paren];
            let args_str = &s[paren + 1..s.len() - 1];
            let mut pattern = if pred.starts_with('?') {
                vec![Pat::Var(pred[1..].to_string())]
            } else if let Some(rest) = pred.strip_prefix("..?") {
                vec![Pat::Rest(rest.to_string())]
            } else {
                vec![Pat::Atom(pred.to_string())]
            };
            for arg in args_str.split(',') {
                let arg = arg.trim();
                if let Some(rest) = arg.strip_prefix("..?") {
                    pattern.push(Pat::Rest(rest.to_string()));
                } else if arg.starts_with('?') {
                    pattern.push(Pat::Var(arg[1..].to_string()));
                } else {
                    pattern.push(Pat::Atom(arg.to_string()));
                }
            }
            pattern
        } else if let Some(rest) = s.strip_prefix("..?") {
            vec![Pat::Rest(rest.to_string())]
        } else if s.starts_with('?') {
            vec![Pat::Var(s[1..].to_string())]
        } else {
            vec![Pat::Atom(s.to_string())]
        }
    }

    fn fact(s: &str) -> Fact {
        if let Some(paren) = s.find('(') {
            let pred = &s[..paren];
            let args_str = &s[paren + 1..s.len() - 1];
            let mut f = vec![pred.to_string()];
            for arg in args_str.split(',') {
                f.push(arg.trim().to_string());
            }
            f
        } else {
            vec![s.to_string()]
        }
    }

    /// Rest at end: matches remaining elements (same as before).
    #[test]
    fn rest_at_end() {
        let p = pat("pred(a, ..?rest)");
        let f = fact("pred(a, b, c)");
        let b = match_pattern(&p, &f).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["b", "c"]);
    }

    /// Rest at beginning: matches leading elements.
    #[test]
    fn rest_at_start() {
        let p = pat("pred(..?rest, z)");
        let f = fact("pred(x, y, z)");
        let b = match_pattern(&p, &f).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["x", "y"]);
    }

    /// Rest in middle: matches elements between fixed args.
    #[test]
    fn rest_in_middle() {
        let p = pat("pred(a, ..?rest, z)");
        let f = fact("pred(a, b, c, z)");
        let b = match_pattern(&p, &f).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["b", "c"]);
    }

    /// Two rest patterns: first gets shortest match, second gets the rest.
    #[test]
    fn two_rests() {
        let p = pat("pred(..?a, ..?b)");
        let f = fact("pred(x, y, z)");
        let b = match_pattern(&p, &f).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, Vec::<String>::new());
        assert_eq!(b.iter().find(|(n, _)| n == "b").unwrap().1, vec!["x", "y", "z"]);
    }

    /// Two rest patterns with fixed separator: shortest match for first.
    #[test]
    fn two_rests_with_separator() {
        let p = pat("pred(..?a, is, ..?b)");
        let f = fact("pred(big, red, ball, is, fun, toy)");
        let b = match_pattern(&p, &f).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, vec!["big", "red", "ball"]);
        assert_eq!(b.iter().find(|(n, _)| n == "b").unwrap().1, vec!["fun", "toy"]);
    }

    /// Rest matches zero elements.
    #[test]
    fn rest_matches_zero() {
        let p = pat("pred(..?rest, z)");
        let f = fact("pred(z)");
        let b = match_pattern(&p, &f).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, Vec::<String>::new());
    }

    /// Rest matches everything (only element).
    #[test]
    fn rest_matches_all() {
        let p = pat("pred(..?rest)");
        let f = fact("pred(x, y, z)");
        let b = match_pattern(&p, &f).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["x", "y", "z"]);
    }

    /// Same rest variable in two positions: must bind consistently.
    #[test]
    fn rest_consistent_binding() {
        let p = pat("pred(..?a, sep, ..?a)");
        let f = fact("pred(x, y, sep, x, y)");
        let b = match_pattern(&p, &f).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, vec!["x", "y"]);
    }

    /// Same rest variable in two positions: fails on inconsistency.
    #[test]
    fn rest_inconsistent_binding() {
        let p = pat("pred(..?a, sep, ..?a)");
        let f = fact("pred(x, y, sep, p, q)");
        assert!(match_pattern(&p, &f).is_none());
    }

    /// No rest: exact length required (unchanged behavior).
    #[test]
    fn no_rest_exact_length() {
        let p = pat("pred(a, b)");
        let f = fact("pred(a, b)");
        assert!(match_pattern(&p, &f).is_some());
    }

    /// No rest: wrong length fails.
    #[test]
    fn no_rest_wrong_length() {
        let p = pat("pred(a, b)");
        let f = fact("pred(a, b, c)");
        assert!(match_pattern(&p, &f).is_none());
    }

    /// Atom mismatch fails.
    #[test]
    fn atom_mismatch() {
        let p = pat("pred(a, ..?rest)");
        let f = fact("pred(x, b, c)");
        assert!(match_pattern(&p, &f).is_none());
    }

    /// Variable binding consistency across patterns.
    #[test]
    fn var_consistency() {
        let p = pat("pred(?x, ..?rest, ?x)");
        let f = fact("pred(hello, a, b, hello)");
        let b = match_pattern(&p, &f).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "x").unwrap().1, vec!["hello"]);
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["a", "b"]);
    }

    /// Variable binding inconsistency fails.
    #[test]
    fn var_inconsistency() {
        let p = pat("pred(?x, ..?rest, ?x)");
        let f = fact("pred(hello, a, b, world)");
        assert!(match_pattern(&p, &f).is_none());
    }
}
