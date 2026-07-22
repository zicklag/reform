use internment::Intern;

pub mod engine;
pub mod parser;
pub mod rule;

/// An argument in a [`Fact`].
pub type Arg = Intern<str>;

/// A reform fact
#[derive(PartialEq, Eq, Hash, Debug, Clone, derive_more::Deref)]
pub struct Fact(pub Vec<Arg>);

impl Fact {
    pub fn is_rule(&self) -> bool {
        self.len() == 4 && &*self[0] == "rule"
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
    let needs = s.chars().any(|c| c.is_whitespace() || c == '(' || c == ')')
        || s.ends_with([';', '.', ':', '\'']);
    if !needs {
        return s.to_string();
    }
    let mut out = String::from("(");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            _ => out.push(c),
        }
    }
    out.push(')');
    out
}
