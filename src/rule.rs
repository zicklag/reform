use crate::Arg;
use anyhow::{bail, Context, Result};

/// A parsed rule with its name, pattern, and body.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct Rule {
    /// The rule's name (the second argument of the `rule` fact).
    pub name: Arg,
    /// The pattern to match against engine facts.
    pub pattern: Pattern,
    /// The body to execute when the pattern matches.
    pub body: Body,
}

impl Rule {
    /// Parse a `rule` fact (4 arguments: `rule`, name, pattern, body) into a [`Rule`].
    pub fn parse<S: AsRef<str>>(fact: &[S]) -> Result<Self> {
        if fact.len() != 4 {
            bail!("rule fact must have exactly 4 arguments, got {}", fact.len());
        }
        let name = Arg::from(fact[1].as_ref());
        let pattern = crate::parser::pattern(fact[2].as_ref())
            .with_context(|| format!("failed to parse rule pattern: {}", fact[2].as_ref()))?;
        let body = crate::parser::body(fact[3].as_ref())
            .with_context(|| format!("failed to parse rule body: {}", fact[3].as_ref()))?;
        Ok(Rule { name, pattern, body })
    }
}

/// A rule pattern, matching one or more facts.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, derive_more::Deref)]
pub struct Pattern(pub Vec<PatternItem>);

/// A rule body, producing facts when the pattern matches.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, derive_more::Deref)]
pub struct Body(pub Vec<BodyItem>);

/// A single item in a pattern: a fact or a repeated block of facts.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub enum PatternItem {
    Fact(PatternFact),
    FactRepetition(PatternFactRepetition),
}

/// A fact to match in a pattern, optionally marked for removal.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct PatternFact {
    pub removed: bool,
    pub args: Vec<ArgTemplate>,
}

/// A repeated block of pattern facts.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct PatternFactRepetition {
    pub kind: RepetitionKind,
    pub facts: Vec<PatternFact>,
}

/// How many times a block repeats.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub enum RepetitionKind {
    Optional,
    OneOrMore,
    ZeroOrMore,
}

/// A single item in a rule body: a fact or a repeated block of facts.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub enum BodyItem {
    Fact(BodyFact),
    FactRepetition(BodyFactRepetition),
}

/// A repeated block of body facts.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct BodyFactRepetition {
    pub kind: RepetitionKind,
    pub facts: Vec<BodyFact>,
}

/// A fact to create in a rule body.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, derive_more::Deref)]
pub struct BodyFact(pub Vec<ArgTemplate>);

/// A single argument in a pattern or body: a literal, a placeholder, or a repeated block.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub enum ArgTemplate {
    Literal(Arg),
    Placeholder(String),
    RepeatedArgs(RepeatedArgs),
}

/// A repeated block of arguments.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct RepeatedArgs {
    pub kind: RepetitionKind,
    pub args: Vec<ArgTemplate>,
}

use std::fmt;

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "$ rule {}", self.name)?;
        writeln!(f, "  (")?;
        for item in self.pattern.iter() {
            writeln!(f, "    {item}")?;
        }
        writeln!(f, "  )")?;
        writeln!(f, "  (")?;
        for item in self.body.iter() {
            writeln!(f, "    {item}")?;
        }
        write!(f, "  )")
    }
}

impl fmt::Display for Pattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for item in self.0.iter() {
            write!(f, "{item}")?;
        }
        Ok(())
    }
}

impl fmt::Display for Body {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for item in self.0.iter() {
            write!(f, "{item}")?;
        }
        Ok(())
    }
}

impl fmt::Display for PatternItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatternItem::Fact(fact) => write!(f, "{fact}"),
            PatternItem::FactRepetition(rep) => write!(f, "{rep}"),
        }
    }
}

impl fmt::Display for PatternFact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.removed {
            write!(f, "- ")?;
        }
        for (i, arg) in self.args.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{arg}")?;
        }
        writeln!(f)
    }
}

impl fmt::Display for BodyFact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, arg) in self.0.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{arg}")?;
        }
        writeln!(f)
    }
}

impl fmt::Display for PatternFactRepetition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suffix = match self.kind {
            RepetitionKind::Optional => "?",
            RepetitionKind::OneOrMore => "+",
            RepetitionKind::ZeroOrMore => "*",
        };
        write!(f, "$(")?;
        for fact in &self.facts {
            write!(f, "  {fact}")?;
        }
        writeln!(f, "){suffix}")
    }
}

impl fmt::Display for BodyFactRepetition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suffix = match self.kind {
            RepetitionKind::Optional => "?",
            RepetitionKind::OneOrMore => "+",
            RepetitionKind::ZeroOrMore => "*",
        };
        write!(f, "$(")?;
        for fact in &self.facts {
            write!(f, "  {fact}")?;
        }
        writeln!(f, "){suffix}")
    }
}


impl fmt::Display for BodyItem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BodyItem::Fact(fact) => write!(f, "{fact}"),
            BodyItem::FactRepetition(rep) => write!(f, "{rep}"),
        }
    }
}



impl fmt::Display for ArgTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ArgTemplate::Literal(arg) => write!(f, "{arg}"),
            ArgTemplate::Placeholder(name) => write!(f, "${name}"),
            ArgTemplate::RepeatedArgs(rep) => write!(f, "{rep}"),
        }
    }
}

impl fmt::Display for RepeatedArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suffix = match self.kind {
            RepetitionKind::Optional => "?",
            RepetitionKind::OneOrMore => "+",
            RepetitionKind::ZeroOrMore => "*",
        };
        write!(f, "$(")?;
        for (i, arg) in self.args.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{arg}")?;
        }
        write!(f, "){suffix}")
    }
}
