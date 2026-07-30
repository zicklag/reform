//! This contains the fact regex implementation.

use fixedbitset::FixedBitSet;
use smallvec::SmallVec;

use crate::{Arg, Fact, Str};

/// Parse tree of a regex that matches against [`Fact`][crate::Fact]s.
#[derive(PartialEq, Eq, Hash, Debug, Clone, derive_more::Deref)]
pub struct RegexTree(pub Vec<RegexItem>);

/// An item in a regex tree.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone)]
pub enum RegexItem {
    Symbol(NfaSymbol),
    Repetition {
        kind: RepetitionKind,
        items: Vec<RegexItem>,
    },
}

/// How many times a block repeats.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub enum RepetitionKind {
    Optional,
    OneOrMore,
    ZeroOrMore,
}

/// The [Glushkov] NFA construction of a [`RegexTree`].
///
/// [Glushkov]: https://en.wikipedia.org/wiki/Glushkov's_construction_algorithm
pub struct Nfa {
    /// For each position, what symbol does it match
    pub symbols: SmallVec<[NfaSymbol; 8]>,
    /// For each position, what other positions may follow it.
    pub follows: SmallVec<[SmallVec<[u8; 8]>; 8]>,
    /// The other qualities of the NFA.
    pub qualities: Qualities,
}

pub struct Qualities {
    pub first: SmallVec<[u8; 4]>,
    pub last: SmallVec<[u8; 4]>,
    pub nullable: bool,
}

/// A symbol in the glushkov NFA.
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Clone, Copy)]
pub enum NfaSymbol {
    /// A literal argument
    Literal(Arg),
    /// A placeholder ( wildcard )
    Placeholder(Str),
}

impl Nfa {
    pub fn from_tree(tree: RegexTree) -> Self {
        let mut symbols = SmallVec::new();
        let mut follows = SmallVec::new();
        let qualities = Self::accumulate(&mut symbols, &mut follows, None, &tree);
        Self {
            symbols,
            follows,
            qualities,
        }
    }

    fn accumulate(
        symbols: &mut SmallVec<[NfaSymbol; 8]>,
        follows: &mut SmallVec<[SmallVec<[u8; 8]>; 8]>,
        repetition: Option<RepetitionKind>,
        items: &[RegexItem],
    ) -> Qualities {
        // Glushkov construction over a sequence of items, concatenated left to
        // right. `repetition`, when set, wraps the whole sequence in a quantifier
        // and is applied once the sequence's `first`/`last`/`nullable` are known.
        let mut first: SmallVec<[u8; 4]> = SmallVec::new();
        let mut last: SmallVec<[u8; 4]> = SmallVec::new();
        // The empty sequence matches the empty string, hence nullable.
        let mut nullable = true;

        for item in items {
            let q = match item {
                RegexItem::Symbol(sym) => {
                    let pos = symbols.len() as u8;
                    symbols.push(*sym);
                    follows.push(SmallVec::new());
                    let mut first: SmallVec<[u8; 4]> = SmallVec::new();
                    first.push(pos);
                    Qualities {
                        first: first.clone(),
                        last: first,
                        nullable: false,
                    }
                }
                RegexItem::Repetition { kind, items } => {
                    Self::accumulate(symbols, follows, Some(*kind), items)
                }
            };

            // Concatenate the prefix (so far) with `q`: every position that can
            // end the prefix can now be followed by any position that can start
            // `q`.
            for &p in &last {
                extend_unique(&mut follows[p as usize], &q.first);
            }
            // If the prefix can vanish, `q`'s starts are also reachable from the
            // overall start. This is what makes sequential nullable repetitions
            // accumulate the right `first` set: e.g. `(a?)(b?)` => first = {a,b}.
            if nullable {
                first.extend_from_slice(&q.first);
            }
            // If `q` can vanish, the prefix's possible endings survive; otherwise
            // `q`'s endings replace them.
            if q.nullable {
                last.extend_from_slice(&q.last);
            } else {
                last.clear();
                last.extend_from_slice(&q.last);
            }
            nullable = nullable && q.nullable;
        }

        // Wrap the sequence in its quantifier, if any. The loop-back wires each
        // possible ending back to each possible start so the body can repeat.
        if let Some(kind) = repetition {
            match kind {
                RepetitionKind::Optional => nullable = true,
                RepetitionKind::ZeroOrMore => {
                    for &p in &last {
                        extend_unique(&mut follows[p as usize], &first);
                    }
                    nullable = true;
                }
                RepetitionKind::OneOrMore => {
                    for &p in &last {
                        extend_unique(&mut follows[p as usize], &first);
                    }
                }
            }
        }

        Qualities { first, last, nullable }
    }
}

fn extend_unique(dst: &mut SmallVec<[u8; 8]>, src: &[u8]) {
    for &x in src {
        if !dst.contains(&x) {
            dst.push(x);
        }
    }
}

/// Bit-parallel structural pre-filter built from an [`Nfa`].
///
/// Implements a single-step-per-arg simulation of the Glushkov automaton using
/// bitsets (one bit per position). Each consumed `Arg` advances the active set:
///
/// ```text
/// active = ( union of Follow(p) for active p ) & char_mask(arg)
/// ```
///
/// where `char_mask(arg) = literal_mask(arg) | any` (`any` is the set of
/// placeholder positions, which match every `Arg`). The match is anchored and
/// must consume the whole fact: it accepts iff the final active set shares a
/// `Last` position, or the pattern is nullable and the fact is empty.
///
/// This is the realization of the Phase-1 design in
/// `docs/glushkov-architecture.md`. The doc's sketch collapses the transition to
/// `follow_bitmask[arg] & active`; that alone over-approximates (it ignores
/// *which* states are active). The correct step unions each active position's
/// `Follow` set before intersecting with `char_mask`, as section 4 of the doc
/// (`follow_bitmask[arg][active_states] -> reachable_states`) requires.
pub struct NfaPrefilter {
    /// Number of positions (states). Bitsets use bits `0..n`.
    n: usize,
    /// Positions reachable from the start state.
    first: FixedBitSet,
    /// Accepting positions (a match ends on one of these).
    last: FixedBitSet,
    /// Whether the whole pattern matches the empty fact.
    nullable: bool,
    /// `follow[p]` = positions reachable from position `p`.
    follow: Vec<FixedBitSet>,
    /// `(arg, mask)` for each literal position in the pattern (one entry per
    /// position; `char_mask` unions every matching entry). Patterns are small,
    /// so the linear scan stays cheap and no dedup bookkeeping is needed.
    literals: Vec<(Arg, FixedBitSet)>,
    /// Placeholder positions, which match any `Arg`.
    any: FixedBitSet,
}

impl NfaPrefilter {
    /// Write `char_mask(arg) = any | literal_mask(arg)` into `out`.
    fn char_mask(&self, arg: &Arg, out: &mut FixedBitSet) {
        out.clear();
        out.union_with(&self.any);
        for (a, mask) in &self.literals {
            if a == arg {
                out.union_with(mask);
            }
        }
    }

    /// True if `fact` could match the pattern (a necessary condition for a full
    /// match; the pre-filter may admit facts that the precise matcher rejects,
    /// but never rejects a fact that would match).
    pub fn matches(&self, fact: &Fact) -> bool {
        let args = &fact.0;
        if args.is_empty() {
            return self.nullable;
        }

        let mut active = FixedBitSet::with_capacity(self.n);
        let mut reach = FixedBitSet::with_capacity(self.n);
        let mut char = FixedBitSet::with_capacity(self.n);

        // First arg: the start state transitions into `first`.
        self.char_mask(&args[0], &mut char);
        active.union_with(&self.first);
        active.intersect_with(&char);

        for arg in &args[1..] {
            if active.is_clear() {
                return false;
            }
            // Positions reachable from the currently active set.
            reach.clear();
            for p in active.ones() {
                reach.union_with(&self.follow[p]);
            }
            // Keep only those reachable positions whose symbol matches `arg`.
            self.char_mask(arg, &mut char);
            reach.intersect_with(&char);
            std::mem::swap(&mut active, &mut reach);
        }

        // Accept iff an active position is a `Last` position.
        active.ones().any(|p| self.last.contains(p))
    }
}

impl From<Nfa> for NfaPrefilter {
    fn from(nfa: Nfa) -> Self {
        let n = nfa.symbols.len();

        let mut first = FixedBitSet::with_capacity(n);
        for &p in &nfa.qualities.first {
            first.insert(p as usize);
        }
        let mut last = FixedBitSet::with_capacity(n);
        for &p in &nfa.qualities.last {
            last.insert(p as usize);
        }

        let follow = nfa
            .follows
            .iter()
            .map(|followers| {
                let mut mask = FixedBitSet::with_capacity(n);
                for &p in followers {
                    mask.insert(p as usize);
                }
                mask
            })
            .collect();

        let mut any = FixedBitSet::with_capacity(n);
        let mut literals: Vec<(Arg, FixedBitSet)> = Vec::new();
        for (p, symbol) in nfa.symbols.iter().enumerate() {
            match symbol {
                NfaSymbol::Placeholder(_) => any.insert(p),
                NfaSymbol::Literal(arg) => {
                    let mut mask = FixedBitSet::with_capacity(n);
                    mask.insert(p);
                    literals.push((*arg, mask));
                }
            }
        }

        NfaPrefilter {
            n,
            first,
            last,
            nullable: nfa.qualities.nullable,
            follow,
            literals,
            any,
        }
    }
}
