//! This contains the fact regex implementation.

use smallvec::SmallVec;

use crate::{Arg, Str};

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
