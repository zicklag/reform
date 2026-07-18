use std::fmt;

pub type Fact = Vec<String>;

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Fact(Fact),
    DeleteFact(Fact),
    Assert(Fact),
    AssertNot(Fact),
    Load(String),
    Find(String),
    Facts,
    Panic(Fact),
    Println(Fact),
    Print(Fact),
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub line: usize,
    pub col: usize,
    pub message: String,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

fn is_hspace(c: char) -> bool { c == ' ' || c == '\t' }
fn is_newline(c: char) -> bool { c == '\n' || c == '\r' }
fn is_split_punct(c: char) -> bool { matches!(c, ':' | ';' | '.' | ',') }
fn is_bracket(c: char) -> bool { matches!(c, '[' | ']' | '{' | '}') }

struct Cursor<'a> {
    chars: std::iter::Peekable<std::str::CharIndices<'a>>,
    rest: &'a str,
    line: usize,
    col: usize,
}

impl<'a> Cursor<'a> {
    fn new(input: &'a str) -> Self {
        Cursor { chars: input.char_indices().peekable(), rest: input, line: 1, col: 1 }
    }
    fn peek(&mut self) -> Option<char> { self.chars.peek().map(|(_, c)| *c) }
    fn advance(&mut self) -> Option<char> {
        match self.chars.next() {
            Some((_, c)) => {
                self.rest = &self.rest[c.len_utf8()..];
                if c == '\n' { self.line += 1; self.col = 1; } else { self.col += 1; }
                Some(c)
            }
            None => None,
        }
    }
    fn save(&self) -> (usize, usize, &'a str) { (self.line, self.col, self.rest) }
    fn restore(&mut self, saved: (usize, usize, &'a str)) {
        let (line, col, rest) = saved;
        self.line = line; self.col = col; self.rest = rest;
        self.chars = rest.char_indices().peekable();
    }
    fn skip_hspace(&mut self) { while let Some(c) = self.peek() { if is_hspace(c) { self.advance(); } else { break; } } }
    fn skip_newline(&mut self) { if self.peek() == Some('\r') { self.advance(); } if self.peek() == Some('\n') { self.advance(); } }
    fn skip_line(&mut self) { while let Some(c) = self.peek() { if is_newline(c) { break; } self.advance(); } }
    fn skip_ws(&mut self) {
        loop { self.skip_hspace(); if self.peek() == Some('#') { self.skip_line(); } else if self.is_eol() { self.skip_newline(); } else { break; } }
    }
    fn skip_hspace_and_comments(&mut self) {
        loop { self.skip_hspace(); if self.peek() == Some('#') { self.skip_line(); } else { break; } }
    }
    fn is_eol(&mut self) -> bool { self.peek().map_or(true, |c| is_newline(c)) }
    fn err(&self, msg: &str) -> ParseError { ParseError { line: self.line, col: self.col, message: msg.to_string() } }
}

pub fn parse_file(input: &str) -> Result<Vec<Stmt>, ParseError> {
    let mut cursor = Cursor::new(input);
    let mut stmts = Vec::new();
    loop {
        cursor.skip_hspace_and_comments();
        if cursor.peek().is_none() { break; }
        if cursor.is_eol() { cursor.skip_newline(); continue; }
        stmts.push(parse_one_stmt(&mut cursor)?);
    }
    Ok(stmts)
}

fn parse_one_stmt(cursor: &mut Cursor) -> Result<Stmt, ParseError> {
    match cursor.peek() {
        Some('$') => { cursor.advance(); cursor.skip_hspace_and_comments(); parse_cmd_or_rule(cursor) }
        Some('>') => {
            cursor.advance(); cursor.skip_hspace_and_comments();
            let fact = parse_fact_line(cursor)?;
            let mut f = vec!["prompt".to_string()]; f.extend(fact); Ok(Stmt::Fact(f))
        }
        Some('-') => {
            cursor.advance();
            if cursor.peek().map_or(true, |c| is_hspace(c) || is_newline(c)) {
                cursor.skip_hspace_and_comments();
                let fact = parse_fact_line(cursor)?;
                Ok(Stmt::DeleteFact(fact))
            } else {
                let mut args = parse_fact_line(cursor)?;
                if args.is_empty() { args.push("-".to_string()); } else { args[0] = format!("-{}", args[0]); }
                let mut f = vec!["sentence".to_string()]; f.extend(args); Ok(Stmt::Fact(f))
            }
        }
        _ => {
            let fact = parse_fact_line(cursor)?;
            let mut f = vec!["sentence".to_string()]; f.extend(fact); Ok(Stmt::Fact(f))
        }
    }
}

fn parse_cmd_or_rule(cursor: &mut Cursor) -> Result<Stmt, ParseError> {
    let mut word = String::new();
    while let Some(c) = cursor.peek() { if is_hspace(c) || is_newline(c) || c == '#' || c == '(' { break; } word.push(c); cursor.advance(); }
    match word.as_str() {
        "assert" => { cursor.skip_hspace_and_comments(); let fact = parse_fact_line(cursor)?; Ok(Stmt::Assert(fact)) }
        "assert-not" | "assert not" => { cursor.skip_hspace_and_comments(); let fact = parse_fact_line(cursor)?; Ok(Stmt::AssertNot(fact)) }
        "load" => { cursor.skip_hspace_and_comments(); let path = parse_rest_of_line(cursor); Ok(Stmt::Load(path.trim().to_string())) }
        "find" => { cursor.skip_hspace_and_comments(); let pattern = parse_rest_of_line(cursor); Ok(Stmt::Find(pattern.trim().to_string())) }
        "facts" => Ok(Stmt::Facts),
        "quit" => Ok(Stmt::Quit),
        "panic" => { cursor.skip_hspace_and_comments(); let args = parse_fact_line(cursor)?; Ok(Stmt::Panic(args)) }
        "println" => { cursor.skip_hspace_and_comments(); let args = parse_fact_line(cursor)?; Ok(Stmt::Println(args)) }
        "print" => { cursor.skip_hspace_and_comments(); let args = parse_fact_line(cursor)?; Ok(Stmt::Print(args)) }
        "rule" => {
            cursor.skip_ws();
            let name = parse_word(cursor);
            cursor.skip_ws();
            let pattern = parse_paren_group(cursor)?;
            cursor.skip_ws();
            let body = parse_paren_group(cursor)?;
            Ok(Stmt::Fact(vec!["rule".to_string(), name, pattern, body]))
        }
        _ => { if word.is_empty() { let args = parse_fact_line(cursor)?; Ok(Stmt::Fact(args)) } else { let mut args = vec![word]; args.extend(parse_fact_line(cursor)?); Ok(Stmt::Fact(args)) } }
    }
}

fn parse_rest_of_line(cursor: &mut Cursor) -> String {
    let mut s = String::new();
    while let Some(c) = cursor.peek() { if is_newline(c) || c == '#' { break; } s.push(c); cursor.advance(); }
    s
}

fn parse_word(cursor: &mut Cursor) -> String {
    let mut s = String::new();
    while let Some(c) = cursor.peek() { if is_hspace(c) || is_newline(c) || c == '#' || is_bracket(c) || c == '(' || c == ')' { break; } s.push(c); cursor.advance(); }
    s
}

fn parse_paren_group(cursor: &mut Cursor) -> Result<String, ParseError> {
    if cursor.peek() != Some('(') { return Err(cursor.err("expected `(`")); }
    cursor.advance();
    let mut depth = 1u32;
    let mut content = String::new();
    loop {
        match cursor.peek() {
            None => return Err(cursor.err("unclosed `(`")),
            Some('(') => { depth += 1; content.push('('); cursor.advance(); }
            Some(')') => { depth -= 1; if depth == 0 { cursor.advance(); return Ok(content); } content.push(')'); cursor.advance(); }
            Some(c) => { content.push(c); cursor.advance(); }
        }
    }
}

fn parse_fact_line(cursor: &mut Cursor) -> Result<Fact, ParseError> {
    let first_indent = cursor.col - 1;
    let mut args = parse_fact_args(cursor)?;
    cursor.skip_newline();
    loop {
        cursor.skip_hspace_and_comments();
        if cursor.peek().is_none() { break; }
        if cursor.is_eol() { break; }
        if cursor.col - 1 <= first_indent { break; }
        args.extend(parse_fact_args(cursor)?);
        cursor.skip_newline();
    }
    Ok(args)
}

fn parse_fact_args(cursor: &mut Cursor) -> Result<Fact, ParseError> {
    let mut args = Fact::new();
    loop {
        cursor.skip_hspace_and_comments();
        if cursor.is_eol() || cursor.peek().is_none() { break; }
        args.push(parse_arg(cursor)?);
    }
    Ok(args)
}

fn parse_arg(cursor: &mut Cursor) -> Result<String, ParseError> {
    // Double-paren literal: ((content)) -> (content)
    if cursor.peek() == Some('(') {
        cursor.advance();
        if cursor.peek() == Some('(') {
            cursor.advance();
            let mut content = String::new();
            let mut depth = 1u32;
            loop {
                match cursor.peek() {
                    None => return Err(cursor.err("unclosed `((`")),
                    Some('(') => { depth += 1; content.push('('); cursor.advance(); }
                    Some(')') => {
                        if depth == 1 {
                            cursor.advance();
                            if cursor.peek() == Some(')') { cursor.advance(); return Ok(format!("({})", content)); }
                            else { return Err(cursor.err("expected `)` to close `((`")); }
                        }
                        depth -= 1; content.push(')'); cursor.advance();
                    }
                    Some(c) => { content.push(c); cursor.advance(); }
                }
            }
        } else {
            // Single paren: balanced group, content is literal
            let mut depth = 1u32;
            let mut content = String::new();
            loop {
                match cursor.peek() {
                    None => return Err(cursor.err("unclosed `(`")),
                    Some('(') => { depth += 1; content.push('('); cursor.advance(); }
                    Some(')') => { depth -= 1; if depth == 0 { cursor.advance(); return Ok(content); } content.push(')'); cursor.advance(); }
                    Some('\\') => { cursor.advance(); match cursor.peek() { Some(c) => { content.push(c); cursor.advance(); } None => return Err(cursor.err("trailing backslash")) } }
                    Some(c) => { content.push(c); cursor.advance(); }
                }
            }
        }
    }

    // Template string [...]
    if cursor.peek() == Some('[') { return parse_template_arg(cursor); }

    // Punctuation that gets auto-split
    if let Some(c) = cursor.peek() {
        if is_split_punct(c) {
            cursor.advance();
            if cursor.peek().map_or(true, |next| is_hspace(next) || is_newline(next) || next == '#' || is_bracket(next)) {
                return Ok(c.to_string());
            }
            let mut word = c.to_string();
            while let Some(next) = cursor.peek() {
                if is_hspace(next) || is_newline(next) || next == '#' || is_bracket(next) || is_split_punct(next) { break; }
                word.push(next); cursor.advance();
            }
            return Ok(word);
        }
    }

    // Regular word with lookahead for punctuation splitting
    let mut word = String::new();
    'word_loop: while let Some(c) = cursor.peek() {
        if is_hspace(c) || is_newline(c) || c == '#' || is_bracket(c) || c == '(' || c == ')' { break; }
        if is_split_punct(c) {
            let saved = cursor.save();
            cursor.advance();
            let next_is_ws = cursor.peek().map_or(true, |next| is_hspace(next) || is_newline(next) || next == '#');
            cursor.restore(saved);
            if next_is_ws { break 'word_loop; }
        }
        word.push(c); cursor.advance();
    }

    if word.is_empty() { return Err(cursor.err("expected argument")); }
    Ok(word)
}

fn parse_template_arg(cursor: &mut Cursor) -> Result<String, ParseError> {
    if cursor.peek() != Some('[') { return Err(cursor.err("expected `[`")); }
    cursor.advance();
    let mut depth = 1u32;
    let mut content = String::new();
    loop {
        match cursor.peek() {
            None => return Err(cursor.err("unclosed `[`")),
            Some('[') => { depth += 1; content.push('['); cursor.advance(); }
            Some(']') => { depth -= 1; if depth == 0 { cursor.advance(); return Ok(format!("[{}]", content)); } content.push(']'); cursor.advance(); }
            Some(c) => { content.push(c); cursor.advance(); }
        }
    }
}

pub fn parse_pattern(input: &str) -> Result<Vec<String>, ParseError> {
    parse_fact_line(&mut Cursor::new(input))
}

pub fn parse_stmt(input: &str) -> Option<Stmt> {
    parse_file(input).ok().and_then(|mut stmts| if stmts.len() == 1 { Some(stmts.remove(0)) } else { None })
}

pub fn parse_fact_str(s: &str) -> Option<Vec<String>> {
    let s = s.trim();
    if !s.starts_with('(') || !s.ends_with(')') { return None; }
    let inner = &s[1..s.len() - 1];
    if inner.is_empty() { return Some(Vec::new()); }
    let mut result = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let mut in_quotes = false;
    let mut in_single = false;
    for (i, c) in inner.char_indices() {
        match c {
            '\'' if !in_quotes => in_single = !in_single,
            '"' if !in_single => in_quotes = !in_quotes,
            '(' if !in_quotes && !in_single => depth += 1,
            ')' if !in_quotes && !in_single => depth -= 1,
            ',' if depth == 0 && !in_quotes && !in_single => { result.push(inner[start..i].trim().to_string()); start = i + 1; }
            _ => {}
        }
    }
    result.push(inner[start..].trim().to_string());
    Some(result)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test] fn simple_words() { let s = parse_file("hello world\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "hello", "world"]); } else { panic!(); } }
    #[test] fn multiple_spaces() { let s = parse_file("hello    world\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "hello", "world"]); } else { panic!(); } }
    #[test] fn tabs_as_whitespace() { let s = parse_file("hello\tworld\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "hello", "world"]); } else { panic!(); } }
    #[test] fn paren_group_single_arg() { let s = parse_file("(Grand Canyon) is big\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "Grand Canyon", "is", "big"]); } else { panic!(); } }
    #[test] fn paren_group_multiline() { let s = parse_file("description is (This is a long description:\n\nIt has \"multiple\" lines)\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f[0], "sentence"); assert_eq!(f[1], "description"); assert_eq!(f[2], "is"); assert_eq!(f[3], "This is a long description:\n\nIt has \"multiple\" lines"); } else { panic!(); } }
    #[test] fn double_paren_literal() { let s = parse_file("This is an ((example))\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "This", "is", "an", "(example)"]); } else { panic!(); } }
    #[test] fn escaped_paren() { let s = parse_file("Here is a smiley (You can put a smiley in :\\) but it has to be escaped.)\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f[0], "sentence"); assert_eq!(f[1], "Here"); assert_eq!(f[2], "is"); assert_eq!(f[3], "a"); assert_eq!(f[4], "smiley"); assert_eq!(f[5], "You can put a smiley in :) but it has to be escaped."); } else { panic!(); } }
    #[test] fn punctuation_splitting() { let s = parse_file("example.com is a website, that is very simple.\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "example.com", "is", "a", "website", ",", "that", "is", "very", "simple", "."]); } else { panic!(); } }
    #[test] fn punctuation_at_end_needs_parens() { let s = parse_file("(www.) is a common web domain prefix\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "www.", "is", "a", "common", "web", "domain", "prefix"]); } else { panic!(); } }
    #[test] fn line_comment() { let s = parse_file("# this is a comment\nhello world\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "hello", "world"]); } else { panic!(); } }
    #[test] fn inline_comment() { let s = parse_file("This is a sentence # with a comment at the end\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "This", "is", "a", "sentence"]); } else { panic!(); } }
    #[test] fn indented_continuation() { let s = parse_file("This is     a sentence\n  that spans multiple   # comments can be here, too\n  lines\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "This", "is", "a", "sentence", "that", "spans", "multiple", "lines"]); } else { panic!(); } }
    #[test] fn empty_line_breaks_fact() { let s = parse_file("Fact one\n  spans two lines\n\n  This is the start of fact two\n    which may continue with indentation, too\n\nThis is the start of fact three\n").unwrap(); assert_eq!(s.len(), 3); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "Fact", "one", "spans", "two", "lines"]); } else { panic!(); } }
    #[test] fn dollar_prefix_no_sentence() { let s = parse_file("$ canyon is big\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["canyon", "is", "big"]); } else { panic!(); } }
    #[test] fn prompt_prefix() { let s = parse_file("> look up\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["prompt", "look", "up"]); } else { panic!(); } }
    #[test] fn rule_definition() { let s = parse_file("$ rule example\n  (\n    - sentence $x\n  )\n  (\n    $x is $y\n  )\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f[0], "rule"); assert_eq!(f[1], "example"); assert!(f[2].contains("- sentence $x")); assert!(f[3].contains("$x is $y")); } else { panic!(); } }
    #[test] fn cmd_assert() { let s = parse_file("$ assert (fact should exist)\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Assert(f) = &s[0] { assert_eq!(f, &["fact should exist"]); } else { panic!(); } }
    #[test] fn cmd_facts() { let s = parse_file("$ facts\n").unwrap(); assert_eq!(s.len(), 1); assert_eq!(s[0], Stmt::Facts); }
    #[test] fn cmd_quit() { let s = parse_file("$ quit\n").unwrap(); assert_eq!(s.len(), 1); assert_eq!(s[0], Stmt::Quit); }
    #[test] fn cmd_load() { let s = parse_file("$ load ./other-file.rf\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Load(p) = &s[0] { assert_eq!(p, "./other-file.rf"); } else { panic!(); } }
    #[test] fn multiple_facts() { let s = parse_file("hello world\nfoo bar\n").unwrap(); assert_eq!(s.len(), 2); }
    #[test] fn template_string() { let s = parse_file("The description is [There is a gate before you\n\nIt is {if open}open{else}closed{end if}\n\nIt is ominous.]\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert!(f[4].starts_with('[')); assert!(f[4].ends_with(']')); } else { panic!(); } }
    #[test] fn empty_input() { assert!(parse_file("").unwrap().is_empty()); }
    #[test] fn only_comments() { assert!(parse_file("# just a comment\n# another\n").unwrap().is_empty()); }
    #[test] fn parse_stmt_simple() { let stmt = parse_stmt("hello world").unwrap(); if let Stmt::Fact(f) = stmt { assert_eq!(f, &["sentence", "hello", "world"]); } else { panic!(); } }
    #[test] fn parse_stmt_prompt() { let stmt = parse_stmt("> look up").unwrap(); if let Stmt::Fact(f) = stmt { assert_eq!(f, &["prompt", "look", "up"]); } else { panic!(); } }
    #[test] fn parse_stmt_dollar() { assert_eq!(parse_stmt("$ facts").unwrap(), Stmt::Facts); }
    #[test] fn parse_fact_str_simple() { let r = parse_fact_str("(hello, world)").unwrap(); assert_eq!(r, vec!["hello", "world"]); }
    #[test] fn parse_fact_str_nested() { let r = parse_fact_str("(print, (hello, world))").unwrap(); assert_eq!(r, vec!["print", "(hello, world)"]); }
    #[test] fn colon_splitting() { let s = parse_file("look: action\n").unwrap(); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "look", ":", "action"]); } else { panic!(); } }
    #[test] fn semicolon_splitting() { let s = parse_file("do this; then that\n").unwrap(); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "do", "this", ";", "then", "that"]); } else { panic!(); } }
    #[test] fn colon_in_word() { let s = parse_file("http://example.com\n").unwrap(); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "http://example.com"]); } else { panic!(); } }
    #[test] fn multiple_punctuation_splits() { let s = parse_file("a, b, c.\n").unwrap(); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "a", ",", "b", ",", "c", "."]); } else { panic!(); } }
    #[test] fn dollar_prefix_rule_with_multiline_body() { let s = parse_file("$ rule multiline\n  (\n    - sentence $x\n    - location $loc\n  )\n  (\n    $x is at $loc\n    $loc contains $x\n  )\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f[0], "rule"); assert_eq!(f[1], "multiline"); assert!(f[2].contains("- sentence $x")); assert!(f[2].contains("- location $loc")); assert!(f[3].contains("$x is at $loc")); assert!(f[3].contains("$loc contains $x")); } else { panic!(); } }
    #[test] fn dollar_prefix_assert_not() { let s = parse_file("$ assert-not (fact exists)\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::AssertNot(f) = &s[0] { assert_eq!(f, &["fact exists"]); } else { panic!(); } }
    #[test] fn dollar_prefix_find() { let s = parse_file("$ find (pattern)\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Find(p) = &s[0] { assert_eq!(p, "(pattern)"); } else { panic!(); } }
    #[test] fn dollar_prefix_panic() { let s = parse_file("$ panic (something went wrong)\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Panic(f) = &s[0] { assert_eq!(f, &["something went wrong"]); } else { panic!(); } }
    #[test] fn dollar_prefix_println() { let s = parse_file("$ println hello world\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Println(f) = &s[0] { assert_eq!(f, &["hello", "world"]); } else { panic!(); } }
    #[test] fn dollar_prefix_print() { let s = parse_file("$ print hello\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Print(f) = &s[0] { assert_eq!(f, &["hello"]); } else { panic!(); } }
    #[test] fn dollar_prefix_unknown_command() { let s = parse_file("$ unknown arg\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["unknown", "arg"]); } else { panic!(); } }
    #[test] fn delete_fact_with_dash() { let s = parse_file("- (fact to delete)\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::DeleteFact(f) = &s[0] { assert_eq!(f, &["fact to delete"]); } else { panic!(); } }
    #[test] fn delete_fact_with_dash_no_parens() { let s = parse_file("- fact\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::DeleteFact(f) = &s[0] { assert_eq!(f, &["fact"]); } else { panic!(); } }
    #[test] fn dash_in_sentence() { let s = parse_file("-hello world\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "-hello", "world"]); } else { panic!(); } }
    #[test] fn nested_parens_in_arg() { let s = parse_file("say (outer (inner) end)\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "say", "outer (inner) end"]); } else { panic!(); } }
    #[test] fn escaped_paren_in_paren_group() { let s = parse_file("say (hello \\) world)\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "say", "hello ) world"]); } else { panic!(); } }
    #[test] fn double_paren_with_nested_parens() { let s = parse_file("say ((nested (parens)))\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "say", "(nested (parens))"]); } else { panic!(); } }
    #[test] fn template_string_with_curly_braces() { let s = parse_file("say [hello {world} end]\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "say", "[hello {world} end]"]); } else { panic!(); } }
    #[test] fn colon_in_middle_of_word() { let s = parse_file("hello:world\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "hello:world"]); } else { panic!(); } }
    #[test] fn semicolon_in_middle_of_word() { let s = parse_file("hello;world\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "hello;world"]); } else { panic!(); } }
    #[test] fn period_in_middle_of_word() { let s = parse_file("example.com\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "example.com"]); } else { panic!(); } }
    #[test] fn multiple_indented_continuations() { let s = parse_file("This is a sentence\n  that spans\n  multiple\n  indented\n  lines\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "This", "is", "a", "sentence", "that", "spans", "multiple", "indented", "lines"]); } else { panic!(); } }
    #[test] fn indented_continuation_with_comments() { let s = parse_file("This is a sentence\n  that spans   # inline comment\n  multiple     # another comment\n  lines\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "This", "is", "a", "sentence", "that", "spans", "multiple", "lines"]); } else { panic!(); } }
    #[test] fn empty_line_after_comment() { let s = parse_file("# a comment\n# another comment\n\nhello world\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "hello", "world"]); } else { panic!(); } }
    #[test] fn parse_fact_str_with_quotes() { let r = parse_fact_str(r#"("hello, world")"#).unwrap(); assert_eq!(r, vec!["\"hello, world\""]); }
    #[test] fn parse_fact_str_empty() { let r = parse_fact_str("()"); assert_eq!(r, Some(vec![])); }
    #[test] fn parse_fact_str_single() { let r = parse_fact_str("(hello)"); assert_eq!(r, Some(vec!["hello".to_string()])); }
    #[test] fn parse_stmt_returns_none_for_multi_stmt() { assert_eq!(parse_stmt("hello world\nfoo bar"), None); }
    #[test] fn parse_file_handles_trailing_newline() { let s = parse_file("hello world\n").unwrap(); assert_eq!(s.len(), 1); if let Stmt::Fact(f) = &s[0] { assert_eq!(f, &["sentence", "hello", "world"]); } else { panic!(); } }
}
