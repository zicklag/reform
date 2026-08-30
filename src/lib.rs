#![forbid(unsafe_code)]

use internment::Intern;

pub mod engine;
pub mod parser;
pub mod regex;
pub mod rule;
pub mod trace;

/// An argument in a [`Fact`].
pub type Arg = Intern<str>;

/// A reform fact
#[derive(PartialEq, Eq, Hash, Debug, Clone, derive_more::Deref)]
pub struct Fact(pub Vec<Arg>);

impl Fact {
    pub fn is_rule(&self) -> bool {
        self.len() >= 4 && &*self[0] == "rule"
    }
}

/// Render a single argument in fact normal form so it survives re-parsing:
/// wrap in parens (with escaping) if it contains whitespace, parens, or
/// trailing punctuation.
pub fn normal_form_arg(a: &Arg) -> String {
    let s: &str = a;
    if s.is_empty() {
        return "()".to_string();
    }
    let needs = s.chars().any(|c| c.is_whitespace() || c == '(' || c == ')' || c == '`')
        || s.ends_with([';', '.', ':', '\'']);
    if !needs {
        return s.to_string();
    }
    format!("({})", escape_arg(a))
}

/// Escape an argument's special characters (`\`, `(`, `)`) so that they come
/// through fact re-parsing as literal characters.
///
/// This is the substitution form used inside a parenthesized argument in a
/// rule body: the user's own parens already group the text into a single
/// argument, so nothing is wrapped — the content is only kept from altering
/// the group structure.
pub fn escape_arg(a: &Arg) -> String {
    let mut out = String::with_capacity(a.len());
    for c in a.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            _ => out.push(c),
        }
    }
    out
}
