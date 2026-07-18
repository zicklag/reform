use internment::ArcIntern;

pub mod parser;
pub mod rule;

/// An argument in a [`Fact`].
pub type Arg = ArcIntern<str>;

/// A reform fact
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, derive_more::Deref)]
pub struct Fact(Vec<Arg>);

impl Fact {
    pub fn is_rule(&self) -> bool {
        self.len() == 4 && &self[0] == "rule"
    }
}
