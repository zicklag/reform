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
        let pattern = Pattern::parse(fact[2].as_ref())?;
        let body = Body::parse(fact[3].as_ref())?;
        Ok(Rule { name, pattern, body })
    }
}

/// A rule pattern, matching one or more facts.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, derive_more::Deref)]
pub struct Pattern(pub Vec<PatternItem>);

impl Pattern {
    fn parse(text: &str) -> Result<Self> {
        Ok(Pattern(parse_pattern_items(text)?))
    }
}

/// A rule body, producing facts when the pattern matches.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, derive_more::Deref)]
pub struct Body(pub Vec<BodyItem>);

impl Body {
    fn parse(text: &str) -> Result<Self> {
        Ok(Body(parse_body_items(text)?))
    }
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

// ---------------------------------------------------------------------------
// Pattern parsing
// ---------------------------------------------------------------------------

fn parse_pattern_items(text: &str) -> Result<Vec<PatternItem>> {
    let mut items = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with("$(") {
            // Fact-level repetition block: $( ... )?|+|*
            let mut facts = Vec::new();
            loop {
                let inner = lines
                    .next()
                    .context("unexpected end of input in pattern repetition block")?;
                let inner = inner.trim();
                if inner.starts_with(')') {
                    let rest = inner[1..].trim();
                    let kind = match rest.chars().next() {
                        Some('?') => RepetitionKind::Optional,
                        Some('+') => RepetitionKind::OneOrMore,
                        Some('*') => RepetitionKind::ZeroOrMore,
                        _ => bail!("pattern repetition block must end with ?, +, or *"),
                    };
                    items.push(PatternItem::FactRepetition(PatternFactRepetition {
                        kind,
                        facts,
                    }));
                    break;
                }
                if inner.is_empty() || inner.starts_with('#') {
                    continue;
                }
                let removed = inner.starts_with('-');
                let args_text = if removed {
                    inner[1..].trim_start()
                } else {
                    inner
                };
                let args = parse_arg_templates(args_text)?;
                facts.push(PatternFact { removed, args });
            }
        } else {
            let removed = trimmed.starts_with('-');
            let args_text = if removed {
                trimmed[1..].trim_start()
            } else {
                trimmed
            };
            let args = parse_arg_templates(args_text)?;
            items.push(PatternItem::Fact(PatternFact { removed, args }));
        }
    }

    Ok(items)
}

// ---------------------------------------------------------------------------
// Body parsing
// ---------------------------------------------------------------------------

fn parse_body_items(text: &str) -> Result<Vec<BodyItem>> {
    let mut items = Vec::new();
    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with("$(") {
            // Fact-level repetition block
            let mut facts = Vec::new();
            loop {
                let inner = lines
                    .next()
                    .context("unexpected end of input in body repetition block")?;
                let inner = inner.trim();
                if inner.starts_with(')') {
                    let rest = inner[1..].trim();
                    let kind = match rest.chars().next() {
                        Some('?') => RepetitionKind::Optional,
                        Some('+') => RepetitionKind::OneOrMore,
                        Some('*') => RepetitionKind::ZeroOrMore,
                        _ => bail!("body repetition block must end with ?, +, or *"),
                    };
                    items.push(BodyItem::FactRepetition(BodyFactRepetition {
                        kind,
                        facts,
                    }));
                    break;
                }
                if inner.is_empty() || inner.starts_with('#') {
                    continue;
                }
                let args = parse_arg_templates(inner)?;
                facts.push(BodyFact(args));
            }
        } else {
            let args = parse_arg_templates(trimmed)?;
            items.push(BodyItem::Fact(BodyFact(args)));
        }
    }

    Ok(items)
}

// ---------------------------------------------------------------------------
// Arg template parsing
// ---------------------------------------------------------------------------

/// Parse a space-separated sequence of arg templates from `text`.
fn parse_arg_templates(text: &str) -> Result<Vec<ArgTemplate>> {
    let mut args = Vec::new();
    let mut pos = 0;
    let bytes = text.as_bytes();

    while pos < bytes.len() {
        // Skip whitespace
        if bytes[pos].is_ascii_whitespace() {
            pos += 1;
            continue;
        }

        // Comment
        if bytes[pos] == b'#' {
            break;
        }

        // Placeholder or arg repetition block
        if bytes[pos] == b'$' {
            pos += 1;
            if pos < bytes.len() && bytes[pos] == b'(' {
                // Arg repetition block: $( ... )?|+|*
                pos += 1; // skip '('
                let inner = parse_arg_templates_until(text, &mut pos, b')')?;
                if pos >= bytes.len() || bytes[pos] != b')' {
                    bail!("expected ')' to close arg repetition block");
                }
                pos += 1; // skip ')'
                let kind = match bytes.get(pos) {
                    Some(b'?') => {
                        pos += 1;
                        RepetitionKind::Optional
                    }
                    Some(b'+') => {
                        pos += 1;
                        RepetitionKind::OneOrMore
                    }
                    Some(b'*') => {
                        pos += 1;
                        RepetitionKind::ZeroOrMore
                    }
                    _ => bail!("arg repetition block must end with ?, +, or *"),
                };
                args.push(ArgTemplate::RepeatedArgs(RepeatedArgs {
                    kind,
                    args: inner,
                }));
            } else {
                // Placeholder name
                let start = pos;
                while pos < bytes.len()
                    && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'_')
                {
                    pos += 1;
                }
                if pos == start {
                    bail!("expected placeholder name after '$'");
                }
                let name = text[start..pos].to_string();
                args.push(ArgTemplate::Placeholder(name));
            }
        } else {
            // Literal word
            let start = pos;
            while pos < bytes.len()
                && !bytes[pos].is_ascii_whitespace()
                && bytes[pos] != b'#'
            {
                pos += 1;
            }
            let word = text[start..pos].to_string();
            args.push(ArgTemplate::Literal(Arg::from(word)));
        }
    }

    Ok(args)
}

/// Parse arg templates until the byte `end` is encountered (not consumed).
fn parse_arg_templates_until(
    text: &str,
    pos: &mut usize,
    end: u8,
) -> Result<Vec<ArgTemplate>> {
    let mut args = Vec::new();
    let bytes = text.as_bytes();

    while *pos < bytes.len() && bytes[*pos] != end {
        // Skip whitespace
        if bytes[*pos].is_ascii_whitespace() {
            *pos += 1;
            continue;
        }

        // Placeholder
        if bytes[*pos] == b'$' {
            *pos += 1;
            if *pos < bytes.len() && bytes[*pos] == b'(' {
                // Nested arg repetition block
                *pos += 1;
                let inner = parse_arg_templates_until(text, pos, b')')?;
                if *pos >= bytes.len() || bytes[*pos] != b')' {
                    bail!("expected ')' to close nested arg repetition block");
                }
                *pos += 1;
                let kind = match bytes.get(*pos) {
                    Some(b'?') => {
                        *pos += 1;
                        RepetitionKind::Optional
                    }
                    Some(b'+') => {
                        *pos += 1;
                        RepetitionKind::OneOrMore
                    }
                    Some(b'*') => {
                        *pos += 1;
                        RepetitionKind::ZeroOrMore
                    }
                    _ => bail!("arg repetition block must end with ?, +, or *"),
                };
                args.push(ArgTemplate::RepeatedArgs(RepeatedArgs {
                    kind,
                    args: inner,
                }));
            } else {
                let start = *pos;
                while *pos < bytes.len()
                    && (bytes[*pos].is_ascii_alphanumeric() || bytes[*pos] == b'_')
                {
                    *pos += 1;
                }
                if *pos == start {
                    bail!("expected placeholder name after '$'");
                }
                let name = text[start..*pos].to_string();
                args.push(ArgTemplate::Placeholder(name));
            }
        } else {
            // Literal word
            let start = *pos;
            while *pos < bytes.len()
                && !bytes[*pos].is_ascii_whitespace()
                && bytes[*pos] != end
                && bytes[*pos] != b'#'
            {
                *pos += 1;
            }
            let word = text[start..*pos].to_string();
            args.push(ArgTemplate::Literal(Arg::from(word)));
        }
    }

    Ok(args)
}
