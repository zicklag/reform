/// A pattern atom: either a literal string or a variable.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Pat {
    Atom(String),
    Var(String),
    Rest(String),
    OptionalBlock(Vec<Pat>),
    RepeatBlock(Vec<Pat>, RepeatKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RepeatKind {
    ZeroOrMore,
    OneOrMore,
}

pub type Pattern = Vec<Pat>;
pub type Fact = Vec<String>;
pub type Bindings = Vec<(String, Vec<String>)>;

/// Collect all variable names from a pattern (for seeding empty bindings on optional skip).
fn collect_vars(pattern: &[Pat]) -> Vec<String> {
    let mut vars = Vec::new();
    for pat in pattern {
        match pat {
            Pat::Var(name) => vars.push(name.clone()),
            Pat::Rest(name) => vars.push(name.clone()),
            Pat::OptionalBlock(inner) => vars.extend(collect_vars(inner)),
            Pat::RepeatBlock(inner, _) => vars.extend(collect_vars(inner)),
            Pat::Atom(_) => {}
        }
    }
    vars
}

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

pub fn match_pattern(pattern: &Pattern, fact: &Fact, outer: &Bindings) -> Option<Bindings> {
    let mut bindings = Bindings::new();
    if try_match(pattern, fact, 0, 0, &mut bindings, outer, false) {
        Some(bindings)
    } else {
        None
    }
}

fn try_match(
    pattern: &Pattern,
    fact: &Fact,
    pat_idx: usize,
    fact_idx: usize,
    bindings: &mut Bindings,
    outer: &Bindings,
    prefix: bool,
) -> bool {
    if pat_idx >= pattern.len() {
        return prefix || fact_idx >= fact.len();
    }
    let pat = &pattern[pat_idx];
    match pat {
        Pat::Atom(s) => {
            if fact_idx >= fact.len() || s != &fact[fact_idx] { return false; }
            try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer, prefix)
        }
        Pat::Var(name) => {
            if let Some((_, existing)) = outer.iter().find(|(n, _)| n == name) {
                if existing.is_empty() {
                    if fact_idx >= fact.len() { return false; }
                    return try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer, prefix);
                }
                if fact_idx >= fact.len() || &fact[fact_idx] != &existing[0] { return false; }
                return try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer, prefix);
            }
            if let Some(idx) = bindings.iter().position(|(n, _)| n == name) {
                let (_, existing) = &bindings[idx];
                if existing.is_empty() { return false; }
                if fact_idx >= fact.len() || existing.len() != 1 || existing[0] != fact[fact_idx] { return false; }
                try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer, prefix)
            } else {
                if fact_idx >= fact.len() { return false; }
                let saved_len = bindings.len();
                bindings.push((name.clone(), vec![fact[fact_idx].clone()]));
                if try_match(pattern, fact, pat_idx + 1, fact_idx + 1, bindings, outer, prefix) { return true; }
                bindings.truncate(saved_len);
                false
            }
        }
        Pat::Rest(name) => {
            let remaining = fact.len() - fact_idx;
            for consume in 0..=remaining {
                let vals: Vec<String> = fact[fact_idx..fact_idx + consume].to_vec();
                if let Some(idx) = bindings.iter().position(|(n, _)| n == name) {
                    let (_, existing) = &bindings[idx];
                    if &existing != &&vals { continue; }
                } else if let Some((_, existing)) = outer.iter().find(|(n, _)| n == name) {
                    if existing.is_empty() {
                        if try_match(pattern, fact, pat_idx + 1, fact_idx, bindings, outer, prefix) { return true; }
                        continue;
                    }
                    if &existing != &&vals { continue; }
                } else {
                    let saved_len = bindings.len();
                    bindings.push((name.clone(), vals));
                    if try_match(pattern, fact, pat_idx + 1, fact_idx + consume, bindings, outer, prefix) { return true; }
                    bindings.truncate(saved_len);
                    continue;
                }
                if try_match(pattern, fact, pat_idx + 1, fact_idx + consume, bindings, outer, prefix) { return true; }
            }
            false
        }
        Pat::OptionalBlock(inner) => {
            let saved_len = bindings.len();
            // First try skipping the optional block entirely
            // Add empty bindings for all variables in the block
            let mut skip_bindings = bindings.clone();
            for var_name in collect_vars(inner) {
                if !skip_bindings.iter().any(|(n, _)| n == &var_name) {
                    skip_bindings.push((var_name, Vec::new()));
                }
            }
            if try_match(pattern, fact, pat_idx + 1, fact_idx, &mut skip_bindings, outer, prefix) {
                *bindings = skip_bindings;
                return true;
            }
            // Then try matching the inner pattern
            for consumed in 1..=(fact.len() - fact_idx) {
                let mut test_bindings = bindings.clone();
                if try_match_exact(inner, fact, 0, fact_idx, fact_idx + consumed, &mut test_bindings, outer) {
                    if try_match(pattern, fact, pat_idx + 1, fact_idx + consumed, &mut test_bindings, outer, prefix) {
                        *bindings = test_bindings;
                        return true;
                    }
                }
            }
            bindings.truncate(saved_len);
            false
        }
        Pat::RepeatBlock(inner, kind) => {
            let saved_len = bindings.len();
            // Greedily match repetitions, accumulating variable bindings
            let mut pos = fact_idx;
            let mut count = 0usize;
            let mut snapshots: Vec<(usize, Bindings)> = Vec::new();
            snapshots.push((pos, bindings.clone()));
            loop {
                if pos >= fact.len() { break; }
                let mut found = false;
                for c in 1..=(fact.len() - pos) {
                    let mut inner_bindings = Bindings::new();
                    if try_match_exact(inner, fact, 0, pos, pos + c, &mut inner_bindings, &Bindings::new()) {
                        // Merge inner bindings by appending (repeat vars accumulate)
                        for (name, vals) in &inner_bindings {
                            if let Some((_, existing)) = bindings.iter_mut().find(|(n, _)| n == name) {
                                existing.extend(vals.clone());
                            } else {
                                bindings.push((name.clone(), vals.clone()));
                            }
                        }
                        pos += c;
                        count += 1;
                        snapshots.push((pos, bindings.clone()));
                        found = true;
                        break;
                    }
                }
                if !found { break; }
            }
            // Backtrack: try from most repetitions down to minimum
            let min_count: usize = match kind {
                RepeatKind::OneOrMore => 1,
                RepeatKind::ZeroOrMore => 0,
            };
            while count >= min_count {
                let (saved_pos, saved_bindings) = &snapshots[count];
                *bindings = saved_bindings.clone();
                if try_match(pattern, fact, pat_idx + 1, *saved_pos, bindings, outer, prefix) {
                    return true;
                }
                if count == 0 { break; }
                count -= 1;
            }
            bindings.truncate(saved_len);
            false
        }
    }
}

/// `try_match_exact` matches `pattern` against `fact[fact_idx..end]` exactly.
/// `end` is the absolute index (not relative consume) — computed once and passed through.
fn try_match_exact(
    pattern: &Pattern,
    fact: &Fact,
    pat_idx: usize,
    fact_idx: usize,
    end: usize,
    bindings: &mut Bindings,
    outer: &Bindings,
) -> bool {
    if pat_idx >= pattern.len() {
        return fact_idx == end;
    }
    let pat = &pattern[pat_idx];
    match pat {
        Pat::Atom(s) => {
            if fact_idx >= end || s != &fact[fact_idx] { return false; }
            try_match_exact(pattern, fact, pat_idx + 1, fact_idx + 1, end, bindings, outer)
        }
        Pat::Var(name) => {
            if let Some((_, existing)) = outer.iter().find(|(n, _)| n == name) {
                if existing.is_empty() {
                    if fact_idx >= end { return false; }
                    return try_match_exact(pattern, fact, pat_idx + 1, fact_idx + 1, end, bindings, outer);
                }
                if fact_idx >= end || &fact[fact_idx] != &existing[0] { return false; }
                return try_match_exact(pattern, fact, pat_idx + 1, fact_idx + 1, end, bindings, outer);
            }
            if let Some(idx) = bindings.iter().position(|(n, _)| n == name) {
                let (_, existing) = &bindings[idx];
                if existing.is_empty() { return false; }
                if fact_idx >= end || existing.len() != 1 || existing[0] != fact[fact_idx] { return false; }
                try_match_exact(pattern, fact, pat_idx + 1, fact_idx + 1, end, bindings, outer)
            } else {
                if fact_idx >= end { return false; }
                let saved_len = bindings.len();
                bindings.push((name.clone(), vec![fact[fact_idx].clone()]));
                if try_match_exact(pattern, fact, pat_idx + 1, fact_idx + 1, end, bindings, outer) { return true; }
                bindings.truncate(saved_len);
                false
            }
        }
        Pat::Rest(name) => {
            let remaining = end - fact_idx;
            for c in 0..=remaining {
                let vals: Vec<String> = fact[fact_idx..fact_idx + c].to_vec();
                if let Some(idx) = bindings.iter().position(|(n, _)| n == name) {
                    let (_, existing) = &bindings[idx];
                    if &existing != &&vals { continue; }
                } else if let Some((_, existing)) = outer.iter().find(|(n, _)| n == name) {
                    if existing.is_empty() {
                        if try_match_exact(pattern, fact, pat_idx + 1, fact_idx, end, bindings, outer) { return true; }
                        continue;
                    }
                    if &existing != &&vals { continue; }
                } else {
                    let saved_len = bindings.len();
                    bindings.push((name.clone(), vals));
                    if try_match_exact(pattern, fact, pat_idx + 1, fact_idx + c, end, bindings, outer) { return true; }
                    bindings.truncate(saved_len);
                    continue;
                }
                if try_match_exact(pattern, fact, pat_idx + 1, fact_idx + c, end, bindings, outer) { return true; }
            }
            false
        }
        Pat::OptionalBlock(inner) => {
            let saved_len = bindings.len();
            // First try skipping the optional block entirely
            // Add empty bindings for all variables in the block
            let mut skip_bindings = bindings.clone();
            for var_name in collect_vars(inner) {
                if !skip_bindings.iter().any(|(n, _)| n == &var_name) {
                    skip_bindings.push((var_name, Vec::new()));
                }
            }
            if try_match_exact(pattern, fact, pat_idx + 1, fact_idx, end, &mut skip_bindings, outer) {
                *bindings = skip_bindings;
                return true;
            }
            // Then try matching the inner pattern
            for sub in 1..=(end - fact_idx) {
                let mut test_bindings = bindings.clone();
                if try_match_exact(inner, fact, 0, fact_idx, fact_idx + sub, &mut test_bindings, outer) {
                    if try_match_exact(pattern, fact, pat_idx + 1, fact_idx + sub, end, &mut test_bindings, outer) {
                        *bindings = test_bindings;
                        return true;
                    }
                }
            }
            bindings.truncate(saved_len);
            false
        }
        Pat::RepeatBlock(inner, kind) => {
            let saved_len = bindings.len();
            // Greedily match repetitions, accumulating variable bindings
            let mut pos = fact_idx;
            let mut count = 0usize;
            let mut snapshots: Vec<(usize, Bindings)> = Vec::new();
            snapshots.push((pos, bindings.clone()));
            loop {
                if pos >= end { break; }
                let mut found = false;
                for c in 1..=(end - pos) {
                    let mut inner_bindings = Bindings::new();
                    if try_match_exact(inner, fact, 0, pos, pos + c, &mut inner_bindings, &Bindings::new()) {
                        // Merge inner bindings by appending (repeat vars accumulate)
                        for (name, vals) in &inner_bindings {
                            if let Some((_, existing)) = bindings.iter_mut().find(|(n, _)| n == name) {
                                existing.extend(vals.clone());
                            } else {
                                bindings.push((name.clone(), vals.clone()));
                            }
                        }
                        pos += c;
                        count += 1;
                        snapshots.push((pos, bindings.clone()));
                        found = true;
                        break;
                    }
                }
                if !found { break; }
            }
            // Backtrack: try from most repetitions down to minimum
            let min_count: usize = match kind {
                RepeatKind::OneOrMore => 1,
                RepeatKind::ZeroOrMore => 0,
            };
            while count >= min_count {
                let (saved_pos, saved_bindings) = &snapshots[count];
                *bindings = saved_bindings.clone();
                if try_match_exact(pattern, fact, pat_idx + 1, *saved_pos, end, bindings, outer) {
                    return true;
                }
                if count == 0 { break; }
                count -= 1;
            }
            bindings.truncate(saved_len);
            false
        }
    }
}


pub fn substitute(pattern: &Pattern, bindings: &Bindings) -> Fact {
    let mut result = Vec::new();
    for pat in pattern.iter() {
        match pat {
            Pat::Atom(s) => {
                let mut s = s.clone();
                for (name, vals) in bindings {
                    if vals.len() == 1 { s = s.replace(&format!("[?{name}]"), &vals[0]); }
                }
                let mut i = 0;
                while i < s.len() {
                    if s.as_bytes().get(i) == Some(&b'[') {
                        if s[i..].starts_with("[?") { i += 1; continue; }
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
                for (name, vals) in bindings {
                    let rest_pat = format!("..?{name}");
                    if s.contains(&rest_pat) {
                        if vals.is_empty() {
                            s = s.replace(&format!(" {}", rest_pat), "");
                            s = s.replace(&rest_pat, "");
                        } else {
                            let replacement: Vec<String> = vals.iter()
                                .map(|v| if v.contains(' ') || v.contains(',') || v.contains('(') || v.contains(')') {
                                    format!("({})", v)
                                } else { v.clone() })
                                .collect();
                            s = s.replace(&rest_pat, &replacement.join(" "));
                        }
                    }
                }
                for (name, vals) in bindings {
                    if vals.len() == 1 { s = s.replace(&format!("?{name}"), &vals[0]); }
                }
                result.push(s);
            }
            Pat::Var(name) => {
                if let Some((_, vals)) = bindings.iter().find(|(n, _)| n == name) {
                    if vals.len() > 1 { result.push(vals.join(" ")); }
                    else if vals.len() == 1 { result.push(vals[0].clone()); }
                    else { result.push(format!("?{name}")); }
                }
            }
            Pat::Rest(name) => {
                if let Some((_, vals)) = bindings.iter().find(|(n, _)| n == name) {
                    if vals.len() == 1 {
                        if let Some(parsed) = crate::parser::parse_fact_str(&vals[0]) { result.extend(parsed); }
                        else { result.push(vals[0].clone()); }
                    } else { result.extend(vals.clone()); }
                }
            }
            Pat::OptionalBlock(inner) => { let sub = substitute(inner, bindings); if !sub.is_empty() { result.extend(sub); } }
            Pat::RepeatBlock(inner, _kind) => { result.extend(substitute(inner, bindings)); }
        }
    }
    result
}

pub fn format_fact(fact: &Fact) -> String {
    if fact.is_empty() { return String::new(); }
    let args: Vec<String> = fact[1..].iter()
        .map(|a| {
            if a.contains(' ') || a.contains('\n') || a.contains('\t') { format!("({})", a) }
            else if a.ends_with(':') || a.ends_with(';') || a.ends_with('.') { format!("({})", a) }
            else { a.clone() }
        })
        .collect();
    if args.is_empty() { fact[0].clone() }
    else { format!("{} {}", fact[0], args.join(" ")) }
}

pub fn format_pattern(pattern: &Pattern) -> String {
    if pattern.is_empty() { return String::new(); }
    let parts: Vec<String> = pattern.iter().map(|pat| match pat {
        Pat::Atom(s) => s.clone(),
        Pat::Var(s) => format!("${s}"),
        Pat::Rest(s) => format!("..?{s}"),
        Pat::OptionalBlock(inner) => format!("$( {} )?", format_pattern(inner)),
        Pat::RepeatBlock(inner, kind) => {
            let suffix = match kind { RepeatKind::ZeroOrMore => "*", RepeatKind::OneOrMore => "+" };
            format!("$( {} ){suffix}", format_pattern(inner))
        }
    }).collect();
    parts.join(" ")
}

pub fn parse_pattern_from_str(s: &str) -> Option<Pattern> { tokenize_pattern(s) }

fn tokenize_pattern(s: &str) -> Option<Vec<Pat>> {
    let mut result = Vec::new();
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;
    while i < len {
        if chars[i] == ' ' || chars[i] == '\t' { i += 1; continue; }
        if chars[i] == '[' {
            // Bracketed optional block: [inner] is sugar for $( inner )?
            let mut depth = 1;
            let mut k = i + 1;
            while k < len && depth > 0 {
                if chars[k] == '[' { depth += 1; }
                else if chars[k] == ']' { depth -= 1; }
                k += 1;
            }
            if depth != 0 { return None; }
            let inner_str: String = chars[i + 1..k - 1].iter().collect();
            let inner = tokenize_pattern(&inner_str)?;
            result.push(Pat::OptionalBlock(inner));
            i = k; continue;
        }
        // Bare parentheses are not valid in the pattern syntax; reject
        // rather than looping forever.
        if chars[i] == '(' || chars[i] == ')' { return None; }
        if chars[i] == '$' {
            let mut j = i + 1;
            while j < len && (chars[j] == ' ' || chars[j] == '\t') { j += 1; }
            if j < len && chars[j] == '(' {
                let mut depth = 1;
                let mut k = j + 1;
                while k < len && depth > 0 {
                    if chars[k] == '(' { depth += 1; }
                    else if chars[k] == ')' { depth -= 1; }
                    k += 1;
                }
                if depth != 0 { return None; }
                let inner_str: String = chars[j + 1..k - 1].iter().collect();
                let inner = tokenize_pattern(&inner_str)?;
                let kind = if k < len && chars[k] == '?' { k += 1; Some(RepeatKind::ZeroOrMore) }
                else if k < len && chars[k] == '+' { k += 1; Some(RepeatKind::OneOrMore) }
                else if k < len && chars[k] == '*' { k += 1; Some(RepeatKind::ZeroOrMore) }
                else { None };
                match kind {
                    Some(RepeatKind::ZeroOrMore) if k > j + 1 && chars[k - 1] == '?' => { result.push(Pat::OptionalBlock(inner)); }
                    Some(kind) => { result.push(Pat::RepeatBlock(inner, kind)); }
                    None => { result.push(Pat::OptionalBlock(inner)); }
                }
                i = k; continue;
            }
            let mut name = String::new(); i += 1;
            while i < len && !chars[i].is_whitespace() && chars[i] != ')' && chars[i] != '(' { name.push(chars[i]); i += 1; }
            if name.is_empty() { return None; }
            result.push(Pat::Var(name)); continue;
        }
        if i + 2 < len && chars[i] == '.' && chars[i + 1] == '.' && chars[i + 2] == '?' {
            let mut name = String::new(); i += 3;
            while i < len && !chars[i].is_whitespace() && chars[i] != ')' && chars[i] != '(' { name.push(chars[i]); i += 1; }
            if name.is_empty() { return None; }
            result.push(Pat::Rest(name)); continue;
        }
        let mut word = String::new();
        while i < len && !chars[i].is_whitespace() && chars[i] != ')' && chars[i] != '(' { word.push(chars[i]); i += 1; }
        if !word.is_empty() { result.push(Pat::Atom(word)); }
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn pat(s: &str) -> Pattern { parse_pattern_from_str(s).unwrap() }
    fn fact(s: &str) -> Fact { s.split_whitespace().map(|w| w.to_string()).collect() }

    #[test] fn rest_at_end() {
        let p = pat("pred ..?rest"); let f = fact("pred a b c");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["a", "b", "c"]);
    }
    #[test] fn rest_at_start() {
        let p = pat("..?rest z"); let f = fact("x y z");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["x", "y"]);
    }
    #[test] fn rest_in_middle() {
        let p = pat("a ..?rest z"); let f = fact("a b c z");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["b", "c"]);
    }
    #[test] fn two_rests() {
        let p = pat("..?a ..?b"); let f = fact("x y z");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, Vec::<String>::new());
        assert_eq!(b.iter().find(|(n, _)| n == "b").unwrap().1, vec!["x", "y", "z"]);
    }
    #[test] fn two_rests_with_separator() {
        let p = pat("..?a is ..?b"); let f = fact("big red ball is fun toy");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, vec!["big", "red", "ball"]);
        assert_eq!(b.iter().find(|(n, _)| n == "b").unwrap().1, vec!["fun", "toy"]);
    }
    #[test] fn rest_matches_zero() {
        let p = pat("..?rest z"); let f = fact("z");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, Vec::<String>::new());
    }
    #[test] fn rest_matches_all() {
        let p = pat("..?rest"); let f = fact("x y z");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["x", "y", "z"]);
    }
    #[test] fn rest_consistent_binding() {
        let p = pat("..?a sep ..?a"); let f = fact("x y sep x y");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, vec!["x", "y"]);
    }
    #[test] fn rest_inconsistent_binding() {
        let p = pat("..?a sep ..?a"); let f = fact("x y sep p q");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }
    #[test] fn no_rest_exact_length() {
        let p = pat("a b"); let f = fact("a b");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn no_rest_wrong_length() {
        let p = pat("a b"); let f = fact("a b c");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }
    #[test] fn atom_mismatch() {
        let p = pat("a ..?rest"); let f = fact("x b c");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }
    #[test] fn var_consistency() {
        let p = pat("$x ..?rest $x"); let f = fact("hello a b hello");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "x").unwrap().1, vec!["hello"]);
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["a", "b"]);
    }
    #[test] fn var_inconsistency() {
        let p = pat("$x ..?rest $x"); let f = fact("hello a b world");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }
    #[test] fn optional_block_present() {
        let p = pat("a $( b )?"); let f = fact("a b");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn optional_block_absent() {
        let p = pat("a $( b )?"); let f = fact("a");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn optional_block_absent_extra_fails() {
        let p = pat("a $( b )?"); let f = fact("a c");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }
    #[test] fn optional_block_var_present() {
        let p = pat("a $( $x )?"); let f = fact("a hello");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "x").unwrap().1, vec!["hello"]);
    }
    #[test] fn optional_block_var_absent() {
        let p = pat("a $( $x )?"); let f = fact("a");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert!(b.iter().find(|(n, _)| n == "x").unwrap().1.is_empty());
    }
    #[test] fn multiple_optional_blocks_present() {
        let p = pat("$( $a )? prefix and $( $b )?"); let f = fact("x prefix and y");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a").unwrap().1, vec!["x"]);
        assert_eq!(b.iter().find(|(n, _)| n == "b").unwrap().1, vec!["y"]);
    }
    #[test] fn multiple_optional_blocks_some_absent() {
        let p = pat("$( $a )? prefix and $( $b )?"); let f = fact("prefix and");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert!(b.iter().find(|(n, _)| n == "a").unwrap().1.is_empty());
        assert!(b.iter().find(|(n, _)| n == "b").unwrap().1.is_empty());
    }
    #[test] fn repeat_zero_matches_zero() {
        let p = pat("a $( b )*"); let f = fact("a");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn repeat_zero_matches_one() {
        let p = pat("a $( b )*"); let f = fact("a b");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn repeat_zero_matches_many() {
        let p = pat("a $( b )*"); let f = fact("a b b b");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn repeat_one_fails_zero() {
        let p = pat("a $( b )+"); let f = fact("a");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }
    #[test] fn repeat_one_matches_one() {
        let p = pat("a $( b )+"); let f = fact("a b");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn repeat_one_matches_many() {
        let p = pat("a $( b )+"); let f = fact("a b b b");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn repeat_block_var_binds_all() {
        let p = pat("a $( $x )*"); let f = fact("a b c d");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "x").unwrap().1, vec!["b", "c", "d"]);
    }
    #[test] fn repeat_block_multi_element() {
        let p = pat("a $( $x and )+"); let f = fact("a b and c and");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "x").unwrap().1, vec!["b", "c"]);
    }
    #[test] fn rule_pattern_from_design() {
        let p2 = pat("sentence $( $a1 )? $x is $( $a2 )? $y");
        let f = fact("sentence The cow is the moon");
        let b = match_pattern(&p2, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a1").unwrap().1, vec!["The"]);
        assert_eq!(b.iter().find(|(n, _)| n == "x").unwrap().1, vec!["cow"]);
        assert_eq!(b.iter().find(|(n, _)| n == "a2").unwrap().1, vec!["the"]);
        assert_eq!(b.iter().find(|(n, _)| n == "y").unwrap().1, vec!["moon"]);
    }
    #[test] fn optional_block_multi_word() {
        let p = pat("$( $a1 is article )?"); let f = fact("the is article");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "a1").unwrap().1, vec!["the"]);
    }
    #[test] fn substitute_optional_var_bound() {
        let p = vec![Pat::OptionalBlock(vec![Pat::Var("x".to_string())])];
        let b = vec![("x".to_string(), vec!["hello".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f, vec!["hello"]);
    }
    #[test] fn substitute_optional_var_unbound() {
        let p = vec![Pat::OptionalBlock(vec![Pat::Var("x".to_string())])];
        let b = vec![]; let f = substitute(&p, &b); assert!(f.is_empty());
    }
    #[test] fn substitute_optional_atom() {
        let p = vec![Pat::OptionalBlock(vec![Pat::Atom(".".to_string())])];
        let b = vec![]; let f = substitute(&p, &b); assert_eq!(f, vec!["."]);
    }
    #[test] fn format_optional_block() {
        let p = vec![Pat::Atom("pred".to_string()), Pat::OptionalBlock(vec![Pat::Atom("x".to_string())])];
        assert_eq!(format_pattern(&p), "pred $( x )?");
    }
    #[test] fn format_repeat_block() {
        let p = vec![Pat::Atom("pred".to_string()), Pat::RepeatBlock(vec![Pat::Var("x".to_string())], RepeatKind::ZeroOrMore)];
        assert_eq!(format_pattern(&p), "pred $( $x )*");
    }
    #[test] fn optional_var_skipped_empty_binding() {
        let p = pat("$( $x )?"); let f = fact("");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert!(b.iter().find(|(n, _)| n == "x").unwrap().1.is_empty());
    }
    #[test] fn substitute_bound_optional_in_atom() {
        let p = vec![Pat::Atom("(?thing, is, ?rel, [?prep], ?other)".to_string())];
        let b = vec![("thing".to_string(), vec!["cow".to_string()]), ("rel".to_string(), vec!["over".to_string()]), ("prep".to_string(), vec!["from".to_string()]), ("other".to_string(), vec!["moon".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f.len(), 1); assert_eq!(f[0], "(cow, is, over, from, moon)");
    }
    #[test] fn substitute_unbound_optional_in_atom() {
        let p = vec![Pat::Atom("(?thing, is, ?rel, [?prep], ?other)".to_string())];
        let b = vec![("thing".to_string(), vec!["cow".to_string()]), ("rel".to_string(), vec!["over".to_string()]), ("other".to_string(), vec!["moon".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f.len(), 1); assert_eq!(f[0], "(cow, is, over, [?prep], moon)");
    }
    #[test] fn optional_var_should_try_skip_when_binding_wrong() {
        let p = pat("sentence $( $a1 )? $thing is $rel $( $prep )? $( $a2 )? $other");
        let f = fact("sentence The cow is over the moon");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert!(b.iter().find(|(n, _)| n == "prep").unwrap().1.is_empty());
        assert_eq!(b.iter().find(|(n, _)| n == "a1").unwrap().1, vec!["The"]);
        assert_eq!(b.iter().find(|(n, _)| n == "a2").unwrap().1, vec!["the"]);
        assert_eq!(b.iter().find(|(n, _)| n == "thing").unwrap().1, vec!["cow"]);
        assert_eq!(b.iter().find(|(n, _)| n == "rel").unwrap().1, vec!["over"]);
        assert_eq!(b.iter().find(|(n, _)| n == "other").unwrap().1, vec!["moon"]);
    }
    #[test] fn substitute_rest_in_atom() {
        let p = vec![Pat::Atom("(print, (Activating ), ?act, ..?fact)".to_string())];
        let b = vec![("act".to_string(), vec!["looking".to_string()]), ("fact".to_string(), vec!["say".to_string(), "You look around tentatively.".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f.len(), 1); assert_eq!(f[0], "(print, (Activating ), looking, say, (You look around tentatively.))");
    }
    #[test] fn substitute_rest_in_atom_empty() {
        let p = vec![Pat::Atom("(had when rule, ..?fact)".to_string())];
        let b = vec![("fact".to_string(), vec![])]; let f = substitute(&p, &b);
        assert_eq!(f.len(), 1); assert_eq!(f[0], "(had when rule)");
    }
    #[test] fn substitute_rest_in_atom_single() {
        let p = vec![Pat::Atom("(print, ..?args)".to_string())];
        let b = vec![("args".to_string(), vec!["hello".to_string()])]; let f = substitute(&p, &b);
        assert_eq!(f.len(), 1); assert_eq!(f[0], "(print, hello)");
    }
    #[test] fn substitute_rest_splits_tuple_string() {
        let p = vec![Pat::Rest("args".to_string())];
        let b = vec![("args".to_string(), vec!["(hello, world)".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f, vec!["hello", "world"]);
    }
    #[test] fn substitute_rest_keeps_non_tuple_string() {
        let p = vec![Pat::Rest("x".to_string())];
        let b = vec![("x".to_string(), vec!["hello".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f, vec!["hello"]);
    }
    #[test] fn substitute_rest_splits_nested_tuple() {
        let p = vec![Pat::Rest("args".to_string())];
        let b = vec![("args".to_string(), vec!["(print, (hello, world))".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f, vec!["print", "(hello, world)"]);
    }
    #[test] fn substitute_var_joins_multi_element() {
        let p = vec![Pat::Var("args".to_string())];
        let b = vec![("args".to_string(), vec!["a".to_string(), "b".to_string(), "c".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f, vec!["a, b, c"]);
    }
    #[test] fn substitute_var_single_element() {
        let p = vec![Pat::Var("x".to_string())];
        let b = vec![("x".to_string(), vec!["hello".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f, vec!["hello"]);
    }
    #[test] fn substitute_var_empty_binding() {
        let p = vec![Pat::Var("x".to_string())];
        let b = vec![("x".to_string(), vec![])]; let f = substitute(&p, &b);
        assert_eq!(f, vec!["?x"]);
    }
    #[test] fn substitute_rest_empty_binding() {
        let p = vec![Pat::Rest("x".to_string())];
        let b = vec![("x".to_string(), vec![])]; let f = substitute(&p, &b);
        assert!(f.is_empty());
    }
    #[test] fn substitute_rest_multi_element() {
        let p = vec![Pat::Rest("args".to_string())];
        let b = vec![("args".to_string(), vec!["a".to_string(), "b".to_string(), "c".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f, vec!["a", "b", "c"]);
    }
    #[test] fn optional_block_skips_when_inner_var_mismatches() {
        let outer = vec![("x".to_string(), vec!["hello".to_string()])];
        let p = pat("a $( $x )? b"); let f = fact("a c b");
        assert!(match_pattern(&p, &f, &outer).is_none());
    }
    #[test] fn optional_block_skips_when_inner_atom_mismatches() {
        let p = pat("a $( b )? c"); let f = fact("a d c");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_none());
    }
    #[test] fn repeat_block_zero_matches_empty_fact() {
        let p = pat("$( a )*"); let f = fact("");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn repeat_block_one_matches_exact() {
        let p = pat("$( a )+"); let f = fact("a");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn repeat_block_var_accumulates_across_repetitions() {
        let p = pat("$( $x and )*"); let f = fact("a and b and");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "x").unwrap().1, vec!["a", "b"]);
    }
    #[test] fn repeat_block_backtracks_when_rest_fails() {
        let p = pat("a $( b )* c"); let f = fact("a b b c");
        assert!(match_pattern(&p, &f, &Bindings::new()).is_some());
    }
    #[test] fn rest_matches_partial_with_trailing_atoms() {
        let p = pat("..?rest z"); let f = fact("x y z");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, vec!["x", "y"]);
    }
    #[test] fn rest_matches_empty_with_trailing_atoms() {
        let p = pat("..?rest z"); let f = fact("z");
        let b = match_pattern(&p, &f, &Bindings::new()).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "rest").unwrap().1, Vec::<String>::new());
    }
    #[test] fn var_bound_by_outer_skips_optional() {
        let outer = vec![("x".to_string(), vec!["hello".to_string()])];
        let p = pat("$( $x )? $y"); let f = fact("world");
        let b = match_pattern(&p, &f, &outer).unwrap();
        assert_eq!(b.iter().find(|(n, _)| n == "y").unwrap().1, vec!["world"]);
        assert_eq!(outer.iter().find(|(n, _)| n == "x").unwrap().1, vec!["hello"]);
    }
    #[test] fn substitute_optional_block_with_mixed_vars() {
        let p = vec![Pat::OptionalBlock(vec![Pat::Var("x".to_string()), Pat::Atom("is".to_string()), Pat::Var("y".to_string())])];
        let b = vec![("x".to_string(), vec!["a".to_string()]), ("y".to_string(), vec!["b".to_string()])];
        let f = substitute(&p, &b); assert_eq!(f, vec!["a", "is", "b"]);
    }
    #[test] fn substitute_optional_block_skipped() {
        let p = vec![Pat::OptionalBlock(vec![Pat::Var("x".to_string())])];
        let b = vec![]; let f = substitute(&p, &b); assert!(f.is_empty());
    }
    #[test] fn format_pattern_empty() {
        assert_eq!(format_pattern(&Pattern::new()), "");
    }
    #[test] fn format_pattern_single_atom() {
        assert_eq!(format_pattern(&vec![Pat::Atom("hello".to_string())]), "hello");
    }
    #[test] fn format_pattern_rest() {
        assert_eq!(format_pattern(&vec![Pat::Rest("args".to_string())]), "..?args");
    }
    #[test] fn parse_pattern_from_str_empty() {
        assert_eq!(parse_pattern_from_str(""), Some(Pattern::new()));
    }
    #[test] fn parse_pattern_from_str_whitespace() {
        assert_eq!(parse_pattern_from_str("   "), Some(Pattern::new()));
    }
    #[test] fn parse_pattern_from_str_var_only() {
        assert_eq!(parse_pattern_from_str("$x"), Some(vec![Pat::Var("x".to_string())]));
    }
    #[test] fn parse_pattern_from_str_rest_only() {
        assert_eq!(parse_pattern_from_str("..?x"), Some(vec![Pat::Rest("x".to_string())]));
    }
    #[test] fn parse_pattern_from_str_optional_block() {
        assert_eq!(parse_pattern_from_str("$( a )?"), Some(vec![Pat::OptionalBlock(vec![Pat::Atom("a".to_string())])]));
    }
    #[test] fn parse_pattern_from_str_repeat_block() {
        assert_eq!(parse_pattern_from_str("$( a )*"), Some(vec![Pat::RepeatBlock(vec![Pat::Atom("a".to_string())], RepeatKind::ZeroOrMore)]));
    }
    #[test] fn parse_pattern_from_str_repeat_one() {
        assert_eq!(parse_pattern_from_str("$( a )+"), Some(vec![Pat::RepeatBlock(vec![Pat::Atom("a".to_string())], RepeatKind::OneOrMore)]));
    }
    #[test] fn match_pattern_prefix_mode() {
        let p = pat("a"); let f = fact("a b c");
        let mut b = Bindings::new();
        assert!(try_match(&p, &f, 0, 0, &mut b, &Bindings::new(), true));
    }
    #[test] fn match_pattern_prefix_mode_fails() {
        let p = pat("a b"); let f = fact("a");
        let mut b = Bindings::new();
        assert!(!try_match(&p, &f, 0, 0, &mut b, &Bindings::new(), true));
    }
}
