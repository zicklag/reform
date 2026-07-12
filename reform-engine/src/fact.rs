/// A pattern atom: either a literal string or a variable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pat {
    /// A literal string that must match exactly.
    Atom(String),
    /// A variable that matches anything and binds the value.
    Var(String),
}

/// A pattern is a list of pattern atoms.
/// The first atom is the predicate name.
pub type Pattern = Vec<Pat>;

/// A ground fact: a list of strings.
/// The first string is the predicate name.
pub type Fact = Vec<String>;

/// A binding from variable names to values.
pub type Bindings = Vec<(String, Vec<String>)>;

/// Check if a pattern matches a fact, returning bindings if it does.
pub fn match_pattern(pattern: &Pattern, fact: &Fact) -> Option<Bindings> {
    if pattern.len() != fact.len() {
        return None;
    }
    let mut bindings: Bindings = Vec::new();
    for (pat, val) in pattern.iter().zip(fact.iter()) {
        match pat {
            Pat::Atom(s) => {
                if s != val {
                    return None;
                }
            }
            Pat::Var(name) => {
                // Check consistency with existing bindings
                if let Some((_, existing)) = bindings.iter().find(|(n, _)| n == name) {
                    // existing is &Vec<String>, val is &String
                    if existing.len() != 1 || &existing[0] != val {
                        return None;
                    }
                } else {
                    bindings.push((name.clone(), vec![val.clone()]));
                }
            }
        }
    }
    Some(bindings)
}

/// Substitute variables in a pattern using bindings, producing a fact.
/// If a variable is not bound, it stays as a variable (for partial patterns).
pub fn substitute(pattern: &Pattern, bindings: &Bindings) -> Fact {
    pattern
        .iter()
        .map(|pat| match pat {
            Pat::Atom(s) => s.clone(),
            Pat::Var(name) => bindings
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, v)| v[0].clone())
                .unwrap_or_else(|| format!("?{name}")),
        })
        .collect()
}

/// Format a fact for display.
pub fn format_fact(fact: &Fact) -> String {
    if fact.is_empty() {
        return "()".to_string();
    }
    let pred = &fact[0];
    if fact.len() == 1 {
        return pred.clone();
    }
    let args: Vec<&str> = fact[1..].iter().map(|s| s.as_str()).collect();
    format!("{}({})", pred, args.join(", "))
}

/// Format a pattern for display.
pub fn format_pattern(pattern: &Pattern) -> String {
    if pattern.is_empty() {
        return "()".to_string();
    }
    let pred = match &pattern[0] {
        Pat::Atom(s) => s.clone(),
        Pat::Var(v) => format!("?{v}"),
    };
    if pattern.len() == 1 {
        return pred;
    }
    let args: Vec<String> = pattern[1..]
        .iter()
        .map(|p| match p {
            Pat::Atom(s) => s.clone(),
            Pat::Var(v) => format!("?{v}"),
        })
        .collect();
    format!("{}({})", pred, args.join(", "))
}

/// Parse a pattern string like "pred(arg1, ?var)" into a Pattern.
/// This is a simple parser used by the engine to convert rule facts.
pub fn parse_pattern_from_str(s: &str) -> Option<Pattern> {
    let s = s.trim();
    if let Some(paren) = s.find('(') {
        let pred = &s[..paren];
        let args_str = &s[paren + 1..s.len() - 1];
        let mut pattern = if pred.starts_with('?') {
            vec![Pat::Var(pred[1..].to_string())]
        } else {
            vec![Pat::Atom(pred.to_string())]
        };
        for arg in args_str.split(',') {
            let arg = arg.trim();
            if arg.is_empty() {
                continue;
            }
            if arg.starts_with('?') {
                pattern.push(Pat::Var(arg[1..].to_string()));
            } else {
                pattern.push(Pat::Atom(arg.to_string()));
            }
        }
        Some(pattern)
    } else if s.starts_with('?') {
        Some(vec![Pat::Var(s[1..].to_string())])
    } else {
        Some(vec![Pat::Atom(s.to_string())])
    }
}
