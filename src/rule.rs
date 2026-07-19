use crate::Arg;
use anyhow::{bail, Context, Result};
use std::collections::HashSet;

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

        // Every `$name` placeholder used in the body must be declared by the
        // pattern. An unbound placeholder is a typo, not a literal — use `$$`
        // to emit a literal `$` (e.g. `$$x` produces `$x` for a generated /
        // inner rule's own placeholder).
        let declared = pattern.placeholders();
        for ph in body.placeholders() {
            if !declared.contains(&ph) {
                bail!("body references placeholder `${ph}` not declared in pattern");
            }
        }

        Ok(Rule { name, pattern, body })
    }
}

/// A rule pattern, matching one or more facts.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, derive_more::Deref)]
pub struct Pattern(pub Vec<PatternItem>);

/// A rule body: a substitution template that produces facts when the pattern
/// matches. The body is a flat template of literal text, `$name` placeholders
/// (substituted from the pattern's bindings at fire time), and
/// `$( ... )?/+/*` repetition blocks (aligned with the pattern's repetitions).
/// After substitution the resulting text is parsed by `facts()` to produce
/// real facts, so any non-placeholder text is opaque — including parens,
/// newlines, and the entire contents of generated (inner) rules.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, derive_more::Deref)]
pub struct Body(pub Vec<BodyChunk>);

/// A single chunk of a rule body template.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub enum BodyChunk {
    /// Literal text, emitted verbatim. A literal `$` is stored as `$` here and
    /// escaped as `$$` on display; `$$` in the source produces a single `$`.
    Text(String),
    /// A `$name` placeholder, substituted with the matched value at fire time.
    Placeholder(String),
    /// A `$( ... )?/+/*` repetition block, iterated over the bound list.
    Repeat(RepeatBlock),
}

/// A repeated block of body chunks.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct RepeatBlock {
    pub kind: RepetitionKind,
    pub chunks: Vec<BodyChunk>,
}

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

/// A single argument in a pattern: a literal, a placeholder, or a repeated block.
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

// ---------------------------------------------------------------------------
// Placeholder collection (used to validate body placeholders against pattern)
// ---------------------------------------------------------------------------

impl Pattern {
    /// All placeholder names declared anywhere in the pattern.
    pub fn placeholders(&self) -> HashSet<String> {
        let mut set = HashSet::new();
        for item in &self.0 {
            collect_pattern_placeholders(item, &mut set);
        }
        set
    }
}

impl Body {
    /// All placeholder names referenced anywhere in the body.
    pub fn placeholders(&self) -> HashSet<String> {
        let mut set = HashSet::new();
        for chunk in &self.0 {
            collect_body_placeholders(chunk, &mut set);
        }
        set
    }
}

fn collect_pattern_placeholders(item: &PatternItem, set: &mut HashSet<String>) {
    match item {
        PatternItem::Fact(f) => {
            for a in &f.args {
                collect_arg_placeholders(a, set);
            }
        }
        PatternItem::FactRepetition(r) => {
            for f in &r.facts {
                for a in &f.args {
                    collect_arg_placeholders(a, set);
                }
            }
        }
    }
}

fn collect_arg_placeholders(a: &ArgTemplate, set: &mut HashSet<String>) {
    match a {
        ArgTemplate::Placeholder(name) => {
            set.insert(name.clone());
        }
        ArgTemplate::RepeatedArgs(r) => {
            for a in &r.args {
                collect_arg_placeholders(a, set);
            }
        }
        ArgTemplate::Literal(_) => {}
    }
}

fn collect_body_placeholders(chunk: &BodyChunk, set: &mut HashSet<String>) {
    match chunk {
        BodyChunk::Placeholder(name) => {
            set.insert(name.clone());
        }
        BodyChunk::Repeat(r) => {
            for c in &r.chunks {
                collect_body_placeholders(c, set);
            }
        }
        BodyChunk::Text(_) => {}
    }
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
        write!(f, "{}", self.body)?;
        writeln!(f, "  )")
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
        for chunk in self.0.iter() {
            write!(f, "{chunk}")?;
        }
        Ok(())
    }
}

impl fmt::Display for BodyChunk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // Escape `$` so the text round-trips through the body parser.
            BodyChunk::Text(s) => {
                for c in s.chars() {
                    if c == '$' {
                        write!(f, "$$")?;
                    } else {
                        write!(f, "{c}")?;
                    }
                }
                Ok(())
            }
            BodyChunk::Placeholder(name) => write!(f, "${name}"),
            BodyChunk::Repeat(r) => write!(f, "{r}"),
        }
    }
}

impl fmt::Display for RepeatBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let suffix = match self.kind {
            RepetitionKind::Optional => "?",
            RepetitionKind::OneOrMore => "+",
            RepetitionKind::ZeroOrMore => "*",
        };
        write!(f, "$(")?;
        for chunk in &self.chunks {
            write!(f, "{chunk}")?;
        }
        write!(f, "){suffix}")
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