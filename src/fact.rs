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
    /// An optional literal — matches if present, skips if absent.
    OptionalAtom(String),
    /// An optional variable — binds if present, leaves unbound if absent.
    OptionalVar(String),
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
///
/// `outer` carries bindings from previously matched patterns in the same rule.
/// A variable with an empty binding in `outer` (skipped optional from an earlier
/// pattern) acts as a wildcard — matches any single element without adding a binding.
pub fn match_pattern(pattern: &Pattern, fact: &Fact, outer: &Bindings) -> Option<Bindings> {
    let mut bindings = Bindings::new();
    if try_match(pattern, fact, 0, 0, &mut bindings, outer) {
        Some(bindings)
    } else {
        None
    }
}

/// Recursive helper: try to match `pattern[pat_idx..]` against `fact[fact_idx..]`,
/// accumulating bindings. Backtracks on rest variables to find the shortest match.
/// `outer` carries bindings from previously matched patterns (read-only).
fn try_match(
    pattern: &Pattern,
    fact: &Fact,
    pat_idx: usize,
    fact_idx: usize,
    bindings: &mut Bindings,
    outer: &Bindings,
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
            try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer)
        }
        Pat::Var(name) => {
            // First, consult outer bindings (from previously matched patterns).
            // If outer has an empty binding, the var was skipped upstream — act as wildcard.
            if let Some((_, existing)) = outer.iter().find(|(n, _)| n == name) {
                if existing.is_empty() {
                    // Wildcard: consume one fact element, add no binding
                    if fact_idx >= fact.len() {
                        return false;
                    }
                    return try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer);
                }
                // Outer has a non-empty binding — require consistency
                if fact_idx >= fact.len() || &fact[fact_idx] != &existing[0] {
                    return false;
                }
                return try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer);
            }
            // Check consistency with local (intra-pattern) bindings
            if let Some(idx) = bindings.iter().position(|(n, _)| n == name) {
                let (_, existing) = &bindings[idx];
                if existing.is_empty() {
                    // Intra-pattern skipped optional — LOCKED, cannot match
                    return false;
                }
                if fact_idx >= fact.len() || existing.len() != 1 || existing[0] != fact[fact_idx] {
                    return false;
                }
                try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer)
            } else {
                if fact_idx >= fact.len() {
                    return false;
                }
                let saved_len = bindings.len();
                bindings.push((name.clone(), vec![fact[fact_idx].clone()]));
                if try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer) {
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

                // Check consistency with existing bindings (local first, then outer)
                if let Some(idx) = bindings.iter().position(|(n, _)| n == name) {
                    let (_, existing) = &bindings[idx];
                    if &existing != &&vals {
                        continue;
                    }
                } else if let Some((_, existing)) = outer.iter().find(|(n, _)| n == name) {
                    if existing.is_empty() {
                        // Outer empty — wildcard: consume zero
                        if try_match(pattern, fact, pat_idx + 1, fact_idx, bindings, outer) {
                            return true;
                        }
                        continue;
                    }
                    if &existing != &&vals {
                        continue;
                    }
                } else {
                    let saved_len = bindings.len();
                    bindings.push((name.clone(), vals));
                    if try_match(pattern, fact, pat_idx + 1, fact_idx + consume, bindings, outer) {
                        return true;
                    }
                    // Backtrack: remove the binding we just added
                    bindings.truncate(saved_len);
                    continue;
                }

                // Binding already existed and matched — just recurse
                if try_match(pattern, fact, pat_idx + 1, fact_idx + consume, bindings, outer) {
                    return true;
                }
            }
            false
        }
        Pat::OptionalAtom(s) => {
            // If the next fact element matches, consume it.
            if fact_idx < fact.len() && s == &fact[fact_idx] {
                try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer)
            } else {
                // Otherwise skip — optional.
                try_match(pattern, fact, pat_idx + 1, fact_idx, bindings, outer)
            }
        }
        Pat::OptionalVar(name) => {
            // Check outer first — if already skipped upstream, can't bind now
            if let Some((_, existing)) = outer.iter().find(|(n, _)| n == name) {
                if existing.is_empty() {
                    return try_match(pattern, fact, pat_idx + 1, fact_idx, bindings, outer);
                }
            }
            if fact_idx < fact.len() {
                let val = &fact[fact_idx];
                // Check consistency with existing local bindings
                if let Some(idx) = bindings.iter().position(|(n, _)| n == name) {
                    let (_, existing) = &bindings[idx];
                    if existing.is_empty() {
                        // Was previously skipped — can't bind now
                        return try_match(pattern, fact, pat_idx + 1, fact_idx, bindings, outer);
                    }
                    if existing.len() == 1 && existing[0] == *val {
                        // Consistent binding — consume and advance
                        if try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer) {
                            return true;
                        }
                    }
                    // Binding exists but doesn't match — try skipping
                    return try_match(pattern, fact, pat_idx + 1, fact_idx, bindings, outer);
                }
                // No existing binding — try skipping first (shortest match)
                let saved_len = bindings.len();
                bindings.push((name.clone(), vec![]));
                if try_match(pattern, fact, pat_idx + 1, fact_idx, bindings, outer) {
                    return true;
                }
                // Backtrack: remove the skip binding
                bindings.truncate(saved_len);
                // Try binding and advancing instead
                bindings.push((name.clone(), vec![val.clone()]));
                if try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer) {
                    return true;
                }
                bindings.truncate(saved_len);
                false
            } else {
                // Fact exhausted — skip (optional), mark as skipped
                let saved_len = bindings.len();
                bindings.push((name.clone(), vec![]));
                if try_match(pattern, fact, pat_idx + 1, fact_idx, bindings, outer) {
                    return true;
                }
                bindings.truncate(saved_len);
                false
            }
        }
    }
}

/// Substitute variables in a pattern using bindings, producing a fact.
/// If a variable is not bound, it stays as a variable (for partial patterns).
/// Also substitutes `?var` patterns inside Atom strings (for rule-in-rule effects).
/// If an Atom string looks like a pattern tuple `(...)`, it is recursively parsed,
/// substituted, and formatted back — so optional vars inside are properly handled
/// (bound → emit value, unbound → skip entirely, no brackets leaking).
pub fn substitute(pattern: &Pattern, bindings: &Bindings) -> Fact {
    let mut result = Vec::new();
    for pat in pattern.iter() {
        match pat {
            Pat::Atom(s) => {
                // Do naive string replacement.
                // Handle optional bracket syntax: [?var] and [literal].
                // Bound optional var → emit value (no brackets).
                // Unbound optional var → keep as [?var] (for rule-generating rules).
                // Optional literal → emit literal (no brackets).
                let mut s = s.clone();
                // First, handle [?var] patterns — replace bound ones with value
                for (name, vals) in bindings {
                    if vals.len() == 1 {
                        let bracketed = format!("[?{name}]");
                        s = s.replace(&bracketed, &vals[0]);
                    }
                }
                // Handle [literal] — remove brackets (but not [?var] — keep those)
                let mut i = 0;
                while i < s.len() {
                    if s.as_bytes().get(i) == Some(&b'[') {
                        // Check if this is [?var] — skip those
                        if s[i..].starts_with("[?") {
                            i += 1;
                            continue;
                        }
                        if let Some(end) = s[i..].find(']') {
                            let inner = s[i + 1..i + end].to_string();
                            s.drain(i..i + end + 1);
                            s.insert_str(i, &inner);
                            i += inner.len();
                            continue;
                        }
                    }
                    i += 1;
                }
                // Handle ..?var (rest/splat) replacement in Atom strings
                // MUST come before ?var replacement to avoid ?var matching inside ..?var
                for (name, vals) in bindings {
                    let rest_pat = format!("..?{name}");
                    if s.contains(&rest_pat) {
                        if vals.is_empty() {
                            // Remove ..?var and any preceding comma+space
                            let with_comma = format!(", {}", rest_pat);
                            s = s.replace(&with_comma, "");
                            s = s.replace(&rest_pat, "");
                        } else {
                            let replacement: Vec<String> = vals
                                .iter()
                                .map(|v| {
                                    if v.contains(' ') || v.contains(',') || v.contains('(')
                                        || v.contains(')')
                                    {
                                        format!("({})", v)
                                    } else {
                                        v.clone()
                                    }
                                })
                                .collect();
                            s = s.replace(&rest_pat, &replacement.join(", "));
                        }
                    }
                }
                // Then do normal ?var replacement
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
            Pat::OptionalAtom(s) => {
                // Always emit the literal (same as Atom in effects)
                result.push(s.clone());
            }
            Pat::OptionalVar(name) => {
                // Emit value if bound, skip if unbound
                if let Some((_, vals)) = bindings.iter().find(|(n, _)| n == name) {
                    if !vals.is_empty() {
                        result.push(vals[0].clone());
                    }
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
        Pat::OptionalAtom(s) => format!("[{s}]"),
        Pat::OptionalVar(s) => format!("[?{s}]"),
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
            Pat::OptionalAtom(s) => format!("[{s}]"),
            Pat::OptionalVar(s) => format!("[?{s}]"),
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
                if let Some(inner) = arg.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
                    if let Some(var) = inner.strip_prefix('?') {
                        pattern.push(Pat::OptionalVar(var.to_string()));
                    } else {
                        pattern.push(Pat::OptionalAtom(inner.to_string()));
                    }
                } else if let Some(rest) = arg.strip_prefix("..?") {
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
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["b", "c"]);
    }

    /// Rest at beginning: matches leading elements.
    #[test]
    fn rest_at_start() {
        let p = pat("pred(..?rest, z)");
        let f = fact("pred(x, y, z)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["x", "y"]);
    }

    /// Rest in middle: matches elements between fixed args.
    #[test]
    fn rest_in_middle() {
        let p = pat("pred(a, ..?rest, z)");
        let f = fact("pred(a, b, c, z)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["b", "c"]);
    }

    /// Two rest patterns: first gets shortest match, second gets the rest.
    #[test]
    fn two_rests() {
        let p = pat("pred(..?a, ..?b)");
        let f = fact("pred(x, y, z)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, Vec::<String>::new());
        assert_eq!(b.iter().find(|(n, _)| n == "b").unwrap().1, vec!["x", "y", "z"]);
    }

    /// Two rest patterns with fixed separator: shortest match for first.
    #[test]
    fn two_rests_with_separator() {
        let p = pat("pred(..?a, is, ..?b)");
        let f = fact("pred(big, red, ball, is, fun, toy)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, vec!["big", "red", "ball"]);
        assert_eq!(b.iter().find(|(n, _)| n == "b").unwrap().1, vec!["fun", "toy"]);
    }

    /// Rest matches zero elements.
    #[test]
    fn rest_matches_zero() {
        let p = pat("pred(..?rest, z)");
        let f = fact("pred(z)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, Vec::<String>::new());
    }

    /// Rest matches everything (only element).
    #[test]
    fn rest_matches_all() {
        let p = pat("pred(..?rest)");
        let f = fact("pred(x, y, z)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["x", "y", "z"]);
    }

    /// Same rest variable in two positions: must bind consistently.
    #[test]
    fn rest_consistent_binding() {
        let p = pat("pred(..?a, sep, ..?a)");
        let f = fact("pred(x, y, sep, x, y)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, vec!["x", "y"]);
    }

    /// Same rest variable in two positions: fails on inconsistency.
    #[test]
    fn rest_inconsistent_binding() {
        let p = pat("pred(..?a, sep, ..?a)");
        let f = fact("pred(x, y, sep, p, q)");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }

    /// No rest: exact length required (unchanged behavior).
    #[test]
    fn no_rest_exact_length() {
        let p = pat("pred(a, b)");
        let f = fact("pred(a, b)");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }

    /// No rest: wrong length fails.
    #[test]
    fn no_rest_wrong_length() {
        let p = pat("pred(a, b)");
        let f = fact("pred(a, b, c)");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }

    /// Atom mismatch fails.
    #[test]
    fn atom_mismatch() {
        let p = pat("pred(a, ..?rest)");
        let f = fact("pred(x, b, c)");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }

    /// Variable binding consistency across patterns.
    #[test]
    fn var_consistency() {
        let p = pat("pred(?x, ..?rest, ?x)");
        let f = fact("pred(hello, a, b, hello)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "x").unwrap().1, vec!["hello"]);
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["a", "b"]);
    }

    /// Variable binding inconsistency fails.
    #[test]
    fn var_inconsistency() {
        let p = pat("pred(?x, ..?rest, ?x)");
        let f = fact("pred(hello, a, b, world)");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }

    // ===== Optional pattern tests =====

    /// Optional atom present: matches and consumes.
    #[test]
    fn optional_atom_present() {
        let p = pat("pred(a, [b])");
        let f = fact("pred(a, b)");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }

    /// Optional atom absent: skips and matches.
    #[test]
    fn optional_atom_absent() {
        let p = pat("pred(a, [b])");
        let f = fact("pred(a)");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }

    /// Optional atom absent but fact has extra: fails (extra unmatched).
    #[test]
    fn optional_atom_absent_extra_fails() {
        let p = pat("pred(a, [b])");
        let f = fact("pred(a, c)");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }

    /// Optional variable present: binds and consumes.
    #[test]
    fn optional_var_present() {
        let p = pat("pred(a, [?x])");
        let f = fact("pred(a, hello)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "x").unwrap().1, vec!["hello"]);
    }

    /// Optional variable absent: skipped, binding exists but is empty.
    #[test]
    fn optional_var_absent() {
        let p = pat("pred(a, [?x])");
        let f = fact("pred(a)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        let (_, vals) = b.iter().find(|(n, _)| n == "x").unwrap();
        assert!(vals.is_empty());
    }

    /// Multiple optionals: all present.
    #[test]
    fn multiple_optionals_present() {
        let p = pat("pred([?a], prefix, and, [?b])");
        let f = fact("pred(x, prefix, and, y)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, vec!["x"]);
        assert_eq!(b.iter().find(|(n, _)| n == "b").unwrap().1, vec!["y"]);
    }
    /// Multiple optionals: some absent — skipped vars have empty bindings.
    #[test]
    fn multiple_optionals_some_absent() {
        let p = pat("pred([?a], prefix, and, [?b])");
        let f = fact("pred(prefix, and)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        let (_, vals_a) = b.iter().find(|(n, _)| n == "a").unwrap();
        assert!(vals_a.is_empty());
        let (_, vals_b) = b.iter().find(|(n, _)| n == "b").unwrap();
        assert!(vals_b.is_empty());
    }

    /// Optional var with consistent binding across patterns.
    #[test]
    fn optional_var_consistent() {
        let p = pat("pred([?x], ?x)");
        let f = fact("pred(hello, hello)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "x").unwrap().1, vec!["hello"]);
    }

    /// Optional var with inconsistent binding: optional skips, non-optional binds.
    #[test]
    fn optional_var_inconsistent_skips() {
        let p = pat("pred([?x], ?x)");
        let f = fact("pred(hello, world)");
        // Optional ?x can't bind to "hello" because ?x later must match "world"
        // Skipping leaves "hello" and "world" for one Var pattern — fails (extra element)
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }
    /// Optional var skips, non-optional cannot bind — skipped locks the var.
    #[test]
    fn optional_var_skip_then_bind() {
        let p = pat("pred([?x], ?x)");
        let f = fact("pred(hello)");
        // Optional skips, locking ?x. ?x can't bind to "hello" — fails.
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }

    /// Optional atom with rest: rest captures remaining.
    #[test]
    fn optional_atom_with_rest() {
        let p = pat("pred(a, [b], ..?rest)");
        let f = fact("pred(a, b, c, d)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["c", "d"]);
    }

    /// Optional atom absent with rest: rest captures everything after required.
    #[test]
    fn optional_atom_absent_with_rest() {
        let p = pat("pred(a, [b], ..?rest)");
        let f = fact("pred(a, c, d)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["c", "d"]);
    }

    /// Substitute: optional var bound emits value.
    #[test]
    fn substitute_optional_var_bound() {
        let p = vec![Pat::OptionalVar("x".to_string())];
        let b = vec![("x".to_string(), vec!["hello".to_string()])];
        let f = substitute(&p, &b);
        assert_eq!(f, vec!["hello"]);
    }

    /// Substitute: optional var unbound emits nothing.
    #[test]
    fn substitute_optional_var_unbound() {
        let p = vec![Pat::OptionalVar("x".to_string())];
        let b = vec![];
        let f = substitute(&p, &b);
        assert!(f.is_empty());
    }

    /// Substitute: optional atom always emits.
    #[test]
    fn substitute_optional_atom() {
        let p = vec![Pat::OptionalAtom(".".to_string())];
        let b = vec![];
        let f = substitute(&p, &b);
        assert_eq!(f, vec!["."]);
    }

    /// Format pattern: optional atom.
    #[test]
    fn format_optional_atom() {
        let p = vec![Pat::Atom("pred".to_string()), Pat::OptionalAtom("x".to_string())];
        assert_eq!(format_pattern(&p), "pred([x])");
    }

    /// Format pattern: optional var.
    #[test]
    fn format_optional_var() {
        let p = vec![Pat::Atom("pred".to_string()), Pat::OptionalVar("x".to_string())];
        assert_eq!(format_pattern(&p), "pred([?x])");
    }
    // ===== Optional binding lock tests =====

    /// Optional var skipped produces empty binding.
    #[test]
    fn optional_var_skipped_empty_binding() {
        let p = pat("pred([?x])");
        let f = fact("pred");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        let (_, vals) = b.iter().find(|(n, _)| n == "x").unwrap();
        assert!(vals.is_empty(), "skipped optional var should have empty binding");
    }

    /// Substitute: bound optional var in inner pattern string emits value without brackets.
    #[test]
    fn substitute_bound_optional_in_atom() {
        let p = vec![Pat::Atom("(?thing, is, ?rel, [?prep], ?other)".to_string())];
        let b = vec![
            ("thing".to_string(), vec!["cow".to_string()]),
            ("rel".to_string(), vec!["over".to_string()]),
            ("prep".to_string(), vec!["from".to_string()]),
            ("other".to_string(), vec!["moon".to_string()]),
        ];
        let f = substitute(&p, &b);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0], "(cow, is, over, from, moon)");
    }

    /// Substitute: unbound optional var in inner pattern string keeps [?var] slot.
    #[test]
    fn substitute_unbound_optional_in_atom() {
        let p = vec![Pat::Atom("(?thing, is, ?rel, [?prep], ?other)".to_string())];
        let b = vec![
            ("thing".to_string(), vec!["cow".to_string()]),
            ("rel".to_string(), vec!["over".to_string()]),
            ("other".to_string(), vec!["moon".to_string()]),
        ];
        let f = substitute(&p, &b);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0], "(cow, is, over, [?prep], moon)");
    }

    /// Optional var should try skipping if binding leads to outer-pattern failure.
    /// Pattern: (sentence, [?a1], ?thing, is, ?rel, [?prep], [?a2], ?other)
    /// Fact: (sentence, The, cow, is, over, the, moon)
    /// ?prep should NOT bind to "the" — it should skip so outer (?prep, is, preposition) can match.
    #[test]
    fn optional_var_should_try_skip_when_binding_wrong() {
        let p = pat("sentence([?a1], ?thing, is, ?rel, [?prep], [?a2], ?other)");
        let f = fact("sentence(The, cow, is, over, the, moon)");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        // ?prep should be empty (skipped), not bound to "the"
        let (_, prep_vals) = b.iter().find(|(n, _)| n == "prep").unwrap();
        assert!(prep_vals.is_empty(), "?prep should be skipped, not bound to 'the'");
        // ?a1 should be "The"
        let (_, a1_vals) = b.iter().find(|(n, _)| n == "a1").unwrap();
        assert_eq!(a1_vals, &[String::from("The")]);
        // ?a2 should be "the"
        let (_, a2_vals) = b.iter().find(|(n, _)| n == "a2").unwrap();
        assert_eq!(a2_vals, &[String::from("the")]);
        // ?thing should be "cow"
        let (_, thing_vals) = b.iter().find(|(n, _)| n == "thing").unwrap();
        assert_eq!(thing_vals, &[String::from("cow")]);
        // ?rel should be "over"
        let (_, rel_vals) = b.iter().find(|(n, _)| n == "rel").unwrap();
        assert_eq!(rel_vals, &[String::from("over")]);
        // ?other should be "moon"
        let (_, other_vals) = b.iter().find(|(n, _)| n == "other").unwrap();
        assert_eq!(other_vals, &[String::from("moon")]);
    }

    /// Substitute: rest var in Atom string expands to comma-separated values.
    #[test]
    fn substitute_rest_in_atom() {
        let p = vec![Pat::Atom("(print, (Activating ), ?act, ..?fact)".to_string())];
        let b = vec![
            ("act".to_string(), vec!["looking".to_string()]),
            ("fact".to_string(), vec!["say".to_string(), "You look around tentatively.".to_string()]),
        ];
        let f = substitute(&p, &b);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0], "(print, (Activating ), looking, say, (You look around tentatively.))");
    }

    /// Substitute: rest var in Atom string with empty binding removes the ..?var.
    #[test]
    fn substitute_rest_in_atom_empty() {
        let p = vec![Pat::Atom("(had when rule, ..?fact)".to_string())];
        let b = vec![
            ("fact".to_string(), vec![]),
        ];
        let f = substitute(&p, &b);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0], "(had when rule)");
    }

    /// Substitute: rest var in Atom string with single value (no wrapping needed).
    #[test]
    fn substitute_rest_in_atom_single() {
        let p = vec![Pat::Atom("(print, ..?args)".to_string())];
        let b = vec![
            ("args".to_string(), vec!["hello".to_string()]),
        ];
        let f = substitute(&p, &b);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0], "(print, hello)");
    }
}
