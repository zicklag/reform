use internment::ArcIntern;

pub mod parser;

/// An argument in a [`Fact`].
pub type Arg = ArcIntern<str>;

/// A reform fact
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct Fact(Vec<Arg>);


