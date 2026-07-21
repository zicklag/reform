use crate::Arg;
use anyhow::{Context, Result, bail};

/// A parsed rule with its name, pattern, and body.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct Rule {
    /// The rule's name (the second argument of the `rule` fact).
    pub name: Arg,
    /// The pattern to match against engine facts.
    pub pattern: Pattern,
    /// The body to execute when the pattern matches.
    pub body: Body,
    /// Specificity score: higher = more specific. Rules are sorted by this
    /// so that more specific rules fire first.
    pub specificity: u64,
}

impl Rule {
    /// Parse a `rule` fact (4 arguments: `rule`, name, pattern, body) into a [`Rule`].
    pub fn parse(fact: &[&str]) -> Result<Self> {
        if fact.len() != 4 {
            bail!(
                "rule fact must have exactly 4 arguments, got {}",
                fact.len()
            );
        }
        let name = Arg::from(fact[1]);
        let pattern = crate::parser::pattern(fact[2])
            .with_context(|| format!("failed to parse rule pattern: {}", fact[2]))?;
        let body = crate::parser::body(fact[3]);
        let specificity = compute_specificity(&pattern);
        let rule = Rule {
            name,
            pattern,
            body,
            specificity,
        };
        rule.validate()?;
        Ok(rule)
    }

    /// Check structural invariants the parser can't enforce on its own:
    ///
    /// * Every placeholder name is used at a consistent nesting context — the
    ///   same stack of repetition kinds (`?`/`+`/`*`) must enclose every use of
    ///   a given name within the pattern, and within the body. A placeholder
    ///   bound at a given nesting in the pattern may be used at the same or
    ///   deeper nesting in the body (e.g. a flat placeholder may be expanded
    ///   inside a repetition), but a placeholder bound inside a repetition may
    ///   not be used at a shallower nesting (outside that repetition).
    /// * Every placeholder referenced in the body is declared by the pattern,
    ///   at the same or shallower nesting context (so a list-bound placeholder
    ///   is iterated, not dropped in as a scalar).
    pub fn validate(&self) -> Result<()> {
        let pat_ctx = pattern_contexts(&self.pattern)?;
        let body_ctx = body_contexts(&self.body)?;
        for (name, bctx) in &body_ctx {
            match pat_ctx.get(name) {
                None => bail!("body references placeholder `${name}` not declared in pattern"),
                Some(pctx) if !is_prefix(pctx, bctx) => {
                    bail!("placeholder `${name}` has different nesting in body vs pattern");
                }
                Some(_) => {}
            }
        }
        Ok(())
    }
}

/// Compute a specificity score for a pattern. Higher = more specific.
///
/// The score counts:
/// - Each non-negated pattern fact: 1 point
/// - Each literal argument in those facts: 1 point
/// - `?` (optional) and `*` (zero-or-more) repetitions contribute 0 for the
///   block itself (they may match zero), plus their inner args/facts
/// - `+` (one-or-more) repetitions contribute 1 point for the block itself,
///   plus their inner args/facts — so a pattern with more required
///   repeating blocks is more specific than one with fewer, while a
///   catch-all `sentence $( $arg )*` stays less specific than a structured
///   rule with literal constraints.
///
/// This ensures rules with more literal constraints and more required facts
/// fire before more general rules.
pub fn compute_specificity(pattern: &Pattern) -> u64 {
    pattern.iter().map(pattern_item_specificity).sum()
}

fn pattern_item_specificity(item: &PatternItem) -> u64 {
    match item {
        PatternItem::Fact(pf) => fact_score(pf),
        PatternItem::FactRepetition(fr) => match fr.kind {
            // `+` requires at least one match (1 point for the block); `*`
            // may match zero (0 for the block, but inner facts still count);
            // `?` may match zero and contributes 0. Negated inner facts add 0.
            RepetitionKind::Optional => 0,
            RepetitionKind::ZeroOrMore => fr.facts.iter().map(fact_score).sum::<u64>(),
            RepetitionKind::OneOrMore => 1 + fr.facts.iter().map(fact_score).sum::<u64>(),
        },
    }
}

/// Specificity of a single (non-repetition) pattern fact: 1 point for the
/// fact itself plus each arg's specificity. Negated facts contribute 0.
fn fact_score(pf: &PatternFact) -> u64 {
    if pf.negated {
        return 0;
    }
    let mut s = 1; // the fact itself
    for arg in &pf.args {
        s += arg_specificity(arg);
    }
    s
}

/// Specificity contributed by a single arg template.
fn arg_specificity(arg: &ArgTemplate) -> u64 {
    match arg {
        ArgTemplate::Literal(_) => 1,
        ArgTemplate::Placeholder(_) => 0,
        ArgTemplate::RepeatedArgs(ra) => repeated_args_specificity(ra),
    }
}

fn repeated_args_specificity(ra: &RepeatedArgs) -> u64 {
    // A `+` block requires at least one match, so it adds 1 point for the
    // block itself; `*` and `?` may match zero, so they add 0 (only their
    // inner args' specificity counts). This keeps a catch-all
    // `sentence $( $arg )*` less specific than a structured rule with
    // literal constraints, while still ranking `a $( b )+ . $( c )+` above
    // `a $( b )+ .`.
    let base = match ra.kind {
        RepetitionKind::Optional | RepetitionKind::ZeroOrMore => 0,
        RepetitionKind::OneOrMore => 1,
    };
    base + ra.args.iter().map(|a| arg_specificity(a)).sum::<u64>()
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

/// A fact to match in a pattern, optionally marked for removal (`-`) or
/// negation (`!`). A negated fact matches when NO fact in the engine matches
/// it (with the current bindings); it binds nothing and consumes nothing.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub struct PatternFact {
    pub removed: bool,
    pub negated: bool,
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

/// A placeholder's nesting context: the stack of repetition kinds enclosing
/// it, outermost first. Two uses of the same name must share the same context.
type RepContext = Vec<RepetitionKind>;

/// Map from placeholder name to its (unique) nesting context.
type UseMap = std::collections::HashMap<String, RepContext>;

/// Collect every placeholder use in the pattern with its nesting context,
/// erroring if a name is used at two different contexts.
fn pattern_contexts(p: &Pattern) -> Result<UseMap> {
    let mut out: UseMap = std::collections::HashMap::new();
    let mut stack: RepContext = Vec::new();
    collect_pattern(&p.0, &mut stack, &mut out, "pattern")?;
    Ok(out)
}

/// Collect every placeholder use in the body with its nesting context,
/// erroring if a name is used at two different contexts.
fn body_contexts(b: &Body) -> Result<UseMap> {
    let mut out: UseMap = std::collections::HashMap::new();
    let mut stack: RepContext = Vec::new();
    collect_body(&b.0, &mut stack, &mut out, "body")?;
    Ok(out)
}

fn record(out: &mut UseMap, name: &str, ctx: &RepContext, where_: &str) -> Result<()> {
    match out.get(name) {
        Some(existing) if existing != ctx => {
            bail!("placeholder `${name}` used at inconsistent nesting depths in {where_}")
        }
        _ => {
            out.insert(name.to_string(), ctx.clone());
            Ok(())
        }
    }
}

fn collect_pattern(
    items: &[PatternItem],
    stack: &mut RepContext,
    out: &mut UseMap,
    where_: &str,
) -> Result<()> {
    for item in items {
        match item {
            PatternItem::Fact(f) => {
                for a in &f.args {
                    collect_arg(a, stack, out, where_)?;
                }
            }
            PatternItem::FactRepetition(r) => {
                stack.push(r.kind.clone());
                for f in &r.facts {
                    for a in &f.args {
                        collect_arg(a, stack, out, where_)?;
                    }
                }
                stack.pop();
            }
        }
    }
    Ok(())
}

fn collect_arg(
    a: &ArgTemplate,
    stack: &mut RepContext,
    out: &mut UseMap,
    where_: &str,
) -> Result<()> {
    match a {
        ArgTemplate::Placeholder(name) => record(out, name, stack, where_),
        ArgTemplate::RepeatedArgs(r) => {
            stack.push(r.kind.clone());
            for a in &r.args {
                collect_arg(a, stack, out, where_)?;
            }
            stack.pop();
            Ok(())
        }
        ArgTemplate::Literal(_) => Ok(()),
    }
}

fn collect_body(
    chunks: &[BodyChunk],
    stack: &mut RepContext,
    out: &mut UseMap,
    where_: &str,
) -> Result<()> {
    for chunk in chunks {
        match chunk {
            BodyChunk::Placeholder(name) => record(out, name, stack, where_)?,
            BodyChunk::Repeat(r) => {
                stack.push(r.kind.clone());
                collect_body(&r.chunks, stack, out, where_)?;
                stack.pop();
            }
            BodyChunk::Text(_) => {}
        }
    }
    Ok(())
}

/// Check if `prefix` is a prefix of `ctx` — the pattern context must be a
/// prefix of the body context (body may be at same or deeper nesting).
fn is_prefix(prefix: &[RepetitionKind], ctx: &[RepetitionKind]) -> bool {
    if prefix.len() > ctx.len() {
        return false;
    }
    prefix == &ctx[..prefix.len()]
}

use crate::Fact;

// ---------------------------------------------------------------------------
// Matching

use std::collections::HashMap;

/// A bound placeholder value: a single argument, or a list of arguments
/// collected across a repeated match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindValue {
    One(Arg),
    Many(Vec<Arg>),
}

/// Placeholder bindings produced by matching a pattern against facts.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bindings {
    pub map: HashMap<String, BindValue>,
}

impl Bindings {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, name: &str) -> Option<&BindValue> {
        self.map.get(name)
    }

    /// Bind `name` to a scalar value. If `name` is already bound to a scalar,
    /// this checks consistency (equal values). If `name` is bound to a list
    /// (a list-bound placeholder inside a repetition), the value is appended
    /// to the list. Returns false on a scalar conflict.
    pub fn bind_scalar(&mut self, name: &str, val: Arg) -> bool {
        match self.map.get_mut(name) {
            Some(BindValue::One(existing)) => existing == &val,
            Some(BindValue::Many(list)) => {
                list.push(val);
                true
            }
            None => {
                self.map.insert(name.to_string(), BindValue::One(val));
                true
            }
        }
    }

    /// Merge another binding set into this one, checking consistency. Returns
    /// false (no change) on conflict.
    pub fn merge(&mut self, other: &Bindings) -> bool {
        for (k, v) in &other.map {
            match v {
                BindValue::One(val) => {
                    if !self.bind_scalar(k, val.clone()) {
                        return false;
                    }
                }
                BindValue::Many(list) => match self.map.get(k) {
                    Some(BindValue::Many(existing)) if existing == list => {}
                    None => {
                        self.map.insert(k.clone(), BindValue::Many(list.clone()));
                    }
                    _ => return false,
                },
            }
        }
        true
    }
}

impl PatternFact {
    /// The first (laziest) full match of this pattern fact against `fact`,
    /// starting from existing `bindings`. The fact matches fully (every arg
    /// consumed). Repetitions (`+`/`*`) are lazy: they match as few iterations
    /// as possible, so when a single fact admits several full matches, the
    /// one with the fewest leading-repetition iterations wins. Returns at
    /// most one binding; different facts each contribute their own binding.
    pub fn match_fact(&self, fact: &Fact, bindings: &Bindings) -> Vec<Bindings> {
        let args: &[Arg] = fact.as_slice();
        let n = args.len();
        let mut out = Vec::new();
        for (end, b) in match_args(&self.args, args, 0, bindings) {
            if end == n {
                out.push(b);
                break;
            }
        }
        out
    }

    /// Whether this pattern fact matches `fact` with no prior bindings.
    pub fn matches_fact(&self, fact: &Fact) -> Option<Bindings> {
        self.match_fact(fact, &Bindings::new()).into_iter().next()
    }
}

/// Placeholder names appearing directly (not nested in a deeper repetition)
/// in a pattern-arg list — these are list-bound when the list is repeated.
fn top_placeholders(pats: &[ArgTemplate]) -> Vec<String> {
    pats.iter()
        .filter_map(|a| match a {
            ArgTemplate::Placeholder(n) => Some(n.clone()),
            _ => None,
        })
        .collect()
}

fn match_args(
    pats: &[ArgTemplate],
    args: &[Arg],
    start: usize,
    b: &Bindings,
) -> Vec<(usize, Bindings)> {
    if pats.is_empty() {
        return vec![(start, b.clone())];
    }
    let (first, rest) = pats.split_first().unwrap();
    let mut out = Vec::new();
    match first {
        ArgTemplate::Literal(lit) => {
            if start < args.len() && &args[start] == lit {
                out.extend(match_args(rest, args, start + 1, b));
            }
        }
        ArgTemplate::Placeholder(name) => {
            if start < args.len() {
                let mut b2 = b.clone();
                if b2.bind_scalar(name, args[start].clone()) {
                    out.extend(match_args(rest, args, start + 1, &b2));
                }
            }
        }
        ArgTemplate::RepeatedArgs(r) => {
            // Pre-populate the repetition's list-bound placeholders with empty
            // lists, so each iteration appends to them (see bind_scalar).
            let mut b0 = b.clone();
            for n in top_placeholders(&r.args) {
                b0.map.insert(n, BindValue::Many(Vec::new()));
            }
            match r.kind {
                RepetitionKind::Optional => {
                    // Greedy: prefer consuming one iteration; only fall back to
                    // zero when no one-iteration parse lets the rest match.
                    let mut one = Vec::new();
                    for (mid, b2) in match_args(&r.args, args, start, &b0) {
                        one.extend(match_args(rest, args, mid, &b2));
                    }
                    if !one.is_empty() {
                        out.extend(one);
                    } else {
                        out.extend(match_args(rest, args, start, &b0));
                    }
                }
                RepetitionKind::ZeroOrMore => {
                    out.extend(match_reps(&r.args, args, start, &b0, false, rest));
                }
                RepetitionKind::OneOrMore => {
                    out.extend(match_reps(&r.args, args, start, &b0, true, rest));
                }
            }
        }
    }
    out
}

/// Match `inner` repeated (per `at_least_one`) then `rest`, returning all
/// `(end, bindings)` where the whole sequence matches. `b` already has the
/// repetition's list-bound placeholders pre-populated as empty lists; each
/// iteration appends to them. Guards against infinite recursion when `inner`
/// can match zero args.
fn match_reps(
    inner: &[ArgTemplate],
    args: &[Arg],
    start: usize,
    b: &Bindings,
    at_least_one: bool,
    rest: &[ArgTemplate],
) -> Vec<(usize, Bindings)> {
    let mut out = Vec::new();
    if !at_least_one {
        out.extend(match_args(rest, args, start, b));
    }
    for (mid, b2) in match_args(inner, args, start, b) {
        if mid == start {
            // inner matched zero args: stop iterating to avoid a loop.
            // This is unreachable because the parser always produces at least
            // one arg template inside a repetition.
            if at_least_one {
                out.extend(match_args(rest, args, mid, &b2));
            }
        } else {
            out.extend(match_reps(inner, args, mid, &b2, false, rest));
        }
    }
    out
}

impl Pattern {
    /// All ways to match this pattern against the given facts. Each pattern
    /// fact line matches a distinct fact; repetition blocks collect lists.
    pub fn find_matches(&self, facts: &[Fact]) -> Vec<Bindings> {
        let used = vec![false; facts.len()];
        match_items(&self.0, facts, &used, &Bindings::new())
    }
}

/// Match a sequence of pattern items against the fact set, where `used`
/// marks facts already consumed by a single-fact item.
fn match_items(
    items: &[PatternItem],
    facts: &[Fact],
    used: &[bool],
    b: &Bindings,
) -> Vec<Bindings> {
    if items.is_empty() {
        return vec![b.clone()];
    }
    let (first, rest) = items.split_first().unwrap();
    let mut out = Vec::new();
    match first {
        PatternItem::Fact(pf) => {
            if pf.negated {
                // Negation: succeed (with current bindings unchanged) iff NO
                // fact matches. Binds nothing, consumes nothing.
                let any = facts.iter().any(|f| !pf.match_fact(f, b).is_empty());
                if !any {
                    out.extend(match_items(rest, facts, used, b));
                }
            } else {
                for i in 0..facts.len() {
                    if used[i] {
                        continue;
                    }
                    let mut used2 = used.to_vec();
                    used2[i] = true;
                    for b2 in pf.match_fact(&facts[i], b) {
                        out.extend(match_items(rest, facts, &used2, &b2));
                    }
                }
            }
        }
        PatternItem::FactRepetition(rep) => {
            out.extend(match_fact_repetition(rep, facts, used, b, rest));
        }
    }
    out
}

/// Match a fact-level repetition block. Collects all unused facts matching
/// the inner (single) pattern fact consistently, binding the inner's
/// top-level placeholders to paired lists. `?` takes at most one fact.
fn match_fact_repetition(
    rep: &PatternFactRepetition,
    facts: &[Fact],
    used: &[bool],
    b: &Bindings,
    rest: &[PatternItem],
) -> Vec<Bindings> {
    if rep.facts.len() != 1 {
        // Multi-fact inner repetitions aren't supported yet.
        return Vec::new();
    }
    let pf = &rep.facts[0];
    // top-level placeholders in the inner fact become list-bound
    let list_ph: Vec<String> = pf
        .args
        .iter()
        .filter_map(|a| match a {
            ArgTemplate::Placeholder(n) => Some(n.clone()),
            _ => None,
        })
        .collect();

    // matching facts (consistent with b), in fact order
    let mut matched: Vec<Bindings> = Vec::new();
    let mut matched_idx: Vec<usize> = Vec::new();
    for i in 0..facts.len() {
        if used[i] {
            continue;
        }
        for b2 in pf.match_fact(&facts[i], b) {
            matched.push(b2);
            matched_idx.push(i);
        }
    }

    let mut out = Vec::new();
    // A `?` whose list-bound placeholder is an empty list is "disabled": the
    // corresponding arg-level `$( $x )?` matched zero iterations, so there is
    // nothing to verify. It must match zero facts rather than greedily
    // grabbing any matching fact (which could steal a fact needed by a later
    // required `?`). A non-empty list makes it required (see `must_match`);
    // an unbound or literal-only `?` is a free optional that grabs if present.
    let disabled = matches!(rep.kind, RepetitionKind::Optional)
        && list_ph.iter().any(|name| {
            b.get(name)
                .is_some_and(|v| matches!(v, BindValue::Many(list) if list.is_empty()))
        });
    let take: Vec<usize> = match rep.kind {
        RepetitionKind::Optional if !matched_idx.is_empty() && !disabled => vec![matched_idx[0]],
        RepetitionKind::ZeroOrMore | RepetitionKind::OneOrMore => matched_idx.clone(),
        RepetitionKind::Optional => vec![],
    };
    let want_present = !take.is_empty();
    let want_absent = matches!(
        rep.kind,
        RepetitionKind::Optional | RepetitionKind::ZeroOrMore
    ) && !want_present;
    // For `?` repetitions, if any list-bound placeholder is already bound to
    // a non-empty list (from an arg-level repetition), the fact-level `?`
    // must match — it acts as a constraint, not a free optional.
    let must_match = matches!(rep.kind, RepetitionKind::Optional)
        && list_ph.iter().any(|name| {
            b.get(name)
                .is_some_and(|v| matches!(v, BindValue::Many(list) if !list.is_empty()))
        });
    if want_present {
        let mut used2 = used.to_vec();
        for &i in &take {
            used2[i] = true;
        }
        let mut b3 = b.clone();
        for name in &list_ph {
            let list: Vec<Arg> = matched
                .iter()
                .zip(matched_idx.iter())
                .filter(|&(_, i)| take.contains(i))
                .filter_map(|(bf, _)| match bf.get(name) {
                    Some(BindValue::One(v)) => Some(v.clone()),
                    _ => None,
                })
                .collect();
            if !list.is_empty() {
                b3.map.insert(name.clone(), BindValue::Many(list));
            }
        }
        out.extend(match_items(rest, facts, &used2, &b3));
    } else if want_absent && !must_match {
        // No matching facts (or `?` with nothing to take): match zero facts.
        out.extend(match_items(rest, facts, used, b));
    }
    out
}

impl Rule {
    /// All ways this rule's pattern matches the given facts.
    pub fn find_matches(&self, facts: &[Fact]) -> Vec<Bindings> {
        self.pattern.find_matches(facts)
    }

    /// Facts matched by pattern facts marked for removal (`-`), given a set of
    /// bindings. Used to delete the consumed facts when the rule fires.
    pub fn removed_facts(&self, facts: &[Fact], b: &Bindings) -> Vec<Fact> {
        let mut out = Vec::new();
        for item in &self.pattern.0 {
            if let PatternItem::Fact(pf) = item
                && pf.removed
            {
                for f in facts {
                    if !pf.match_fact(f, b).is_empty() {
                        out.push(f.clone());
                    }
                }
            }
        }
        out
    }
}

// ---------------------------------------------------------------------------
// Body rendering (substitution)
// ---------------------------------------------------------------------------

impl Body {
    /// Render the body template with the given bindings, producing reform
    /// source text ready to be parsed by [`crate::parser::facts`].
    pub fn render(&self, b: &Bindings) -> String {
        let mut out = String::new();
        render_chunks(&self.0, b, &mut out);
        out
    }
}

fn render_chunks(chunks: &[BodyChunk], b: &Bindings, out: &mut String) {
    for chunk in chunks {
        match chunk {
            BodyChunk::Text(t) => out.push_str(t),
            BodyChunk::Placeholder(name) => match b.get(name) {
                Some(BindValue::One(v)) => out.push_str(&crate::normal_form_arg(v)),
                Some(BindValue::Many(list)) => {
                    for (i, v) in list.iter().enumerate() {
                        if i > 0 {
                            out.push(' ');
                        }
                        out.push_str(&crate::normal_form_arg(v));
                    }
                }
                None => {}
            },
            BodyChunk::Repeat(r) => render_repeat(r, b, out),
        }
    }
}

fn render_repeat(r: &RepeatBlock, b: &Bindings, out: &mut String) {
    // The list-bound placeholders appearing in this block drive the iteration.
    // Collect each one's bound list exactly once — by construction every entry
    // here is a `Many` value (that's the filter), so there is no second
    // fallible lookup and no dead branch when we read the lengths or elements.
    let driver_lists: Vec<(String, Vec<Arg>)> = list_placeholders(&r.chunks)
        .into_iter()
        .filter_map(|name| match b.get(&name) {
            Some(BindValue::Many(list)) => Some((name, list.clone())),
            _ => None,
        })
        .collect();
    let Some((_, first)) = driver_lists.first() else {
        return;
    };
    let n = first.len();
    // All drivers must have the same length, otherwise the bindings are
    // inconsistent and the block renders nothing.
    for (_, list) in &driver_lists {
        if list.len() != n {
            return;
        }
    }
    for i in 0..n {
        let mut b2 = b.clone();
        for (name, list) in &driver_lists {
            b2.map.insert(name.clone(), BindValue::One(list[i].clone()));
        }
        render_chunks(&r.chunks, &b2, out);
    }
}

/// Names of placeholders appearing (at any depth) in a chunk list.
fn list_placeholders(chunks: &[BodyChunk]) -> Vec<String> {
    let mut out = Vec::new();
    collect_ph_names(chunks, &mut out);
    out
}

fn collect_ph_names(chunks: &[BodyChunk], out: &mut Vec<String>) {
    for chunk in chunks {
        match chunk {
            BodyChunk::Placeholder(name) => out.push(name.clone()),
            BodyChunk::Repeat(r) => collect_ph_names(&r.chunks, out),
            BodyChunk::Text(_) => {}
        }
    }
}
