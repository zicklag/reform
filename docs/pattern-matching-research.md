# Pattern Matching Research Report

Research into existing crates, papers, and projects relevant to optimizing
reform's pattern matching engine. Conducted July 2026.

## Background

Reform's pattern matching has two distinct performance problems:

1. **Arg-level matching** (within a single fact): `match_args` in `src/rule.rs`
   is a recursive backtracking regex engine over a string-token alphabet with
   placeholder bindings. It returns all matches eagerly as a `Vec`, has
   exponential worst case on ambiguous patterns with nested repetitions, and
   the callers only need the first match where the rest of the pattern
   succeeds.

2. **Fact-level joins** (across multiple pattern lines): `match_items_detailed`
   does a nested-loop scan over a flat `Vec<Fact>` with no indexing. The
   engine's `turn()` loop re-scans all facts every iteration with no incremental
   evaluation.

---

## Problem 1: Arg-Level Matching (NFA Territory)

Reform's arg-level pattern matching is structurally a regex engine: sequences
of literals, placeholders (capture groups), and repetition operators
(`?`/`+`/`*`) matched against a sequence of string tokens. The current
implementation is recursive backtracking with eager enumeration.

### Key Research Papers

#### Laurikari (2000) — "NFAs with Tagged Transitions"

The foundational paper. Introduces **Tagged NFAs (TNFA)**: NFAs with "tags" on
transitions that record the current input position when traversed. Tags are
the formal basis for regex capture groups — reform's placeholders are exactly
tags. The TNFA simulation maintains a set of `(state, tag-value-vector)`
configurations and processes input left-to-right. Also shows how to determinize
a TNFA into a **Tagged DFA (TDFA)** for O(n) matching with register updates.

- Reference: `laurikari.net/ville/spire2000-tnfa.pdf`
- Relevance: Formal foundation for what reform needs — PikeVM with binding
  threads.

#### Borsotti & Trafimovich (2022) — "A Closer Look at TDFA"

The practical implementation guide. Provides detailed pseudocode for TNFA
construction (Thompson-like with tags and priorities), the simulation
algorithm (Algorithm 1), and epsilon-closure algorithms (LAU, LAU1, GOR1).
Covers both leftmost-greedy and POSIX disambiguation. The GOR1 epsilon-closure
algorithm (Goldberg-Radzak) is recommended for cyclic graphs (which reform's
repetition operators produce).

- Reference: arXiv:2206.01398
- Relevance: **The paper to read when implementing.**

#### Trofimovich (2019) — "Tagged DFA with Lookahead"

Extends Laurikari with one-symbol lookahead, reducing redundant tag operations.
Formalizes the T-language/S-language distinction (S-language = pure
recognition, T-language = with tag values). Relevant if reform ever wants
TDFA determinization for hot patterns.

- Reference: arXiv:1907.08837

#### Barriere & Pit-Claudel (2024) — "Linear Matching of JavaScript Regexes"

Extends PikeVM to handle capture groups *inside quantifiers* — exactly
reform's `$( $x )*` collecting a list of values. Shows this can be done in
O(|r| x |s|), not O(|r|^2 x |s|). The "capture reset" property (capture groups
reset on each iteration of a quantifier) maps to reform's frame-stack
list-binding semantics.

- Reference: arXiv:2311.17620v2
- Relevance: **Most directly relevant to reform's list-bound placeholder
  complication.**

#### Schmid (2019) — "Regular Expressions with Backreferences: Polynomial-Time Matching"

Regex with backreferences is NP-complete in general, but this paper defines
**active variable degree** — a complexity parameter that, when bounded,
yields polynomial-time matching. Also defines **memory-deterministic regex** —
a class matchable in O(|w| * p(|r|)). Reform's placeholder consistency
("`$x` must be the same value everywhere") is analogous to backreferences.

- Reference: arXiv:1903.05896
- Relevance: Formal complexity bounds for reform's patterns.

#### Kutsia (2002) — "Pattern Unification with Sequence Variables and Flexible Arity Symbols"

Formal unification theory for terms with sequence variables (match
zero-or-more terms) and variadic function symbols. Directly models reform's
`$x` (individual variable) and `$( $x )*` (sequence variable). The unification
procedure enumerates minimal complete sets of unifiers.

- Reference: risc.jku.at/people/tkutsia/papers/Kutsia-UNCL02.pdf
- Relevance: Formal semantics foundation.

#### MatchPy (Krebber et al., 2017)

Python library for pattern matching with sequence variables. Uses
**discrimination nets** (decision-tree automata) for many-to-one matching —
sharing common prefixes across patterns. The discrimination net approach is
relevant to reform's fact-level matching.

- Reference: arXiv:1710.06915

### Existing Rust Crates (Arg-Level)

| Crate | What it does | Reusable? | Verdict |
|---|---|---|---|
| **regex-automata** | Thompson NFA, PikeVM, BoundedBacktracker, one-pass DFA, meta engine | Reference only — byte-locked alphabet | Study as blueprint, don't depend on |
| **automata** (v0.0.4) | Generic `Alphabet` trait, `Nfa<A>` with epsilon transitions | Architecturally right, but immature (v0.0.4) | Right idea, too early to build on |
| **rustfst** | Weighted FSTs with `SymbolTable` (string-to-label mapping) | Could map Args to numeric labels | Overcomplicated for reform's needs |
| **inator** | NFA with epsilon transitions, generic token type | Small proof-of-concept | Too minimal |
| **deterministic_automata** | Generic DFA framework with customizable Alphabet trait | DFA-only, no NFA/PikeVM | Shows clean trait design |
| **automaton** (v0.0.2) | Very early, Token trait for input values | Barely documented | Not usable |

**Gap confirmed: no Rust crate provides a ready-made Thompson NFA or PikeVM
over a generic (non-byte) alphabet.** The pragmatic path is to build a
reform-native NFA + PikeVM, using `regex-automata` as the reference
implementation.

### Alternative NFA Construction: Glushkov

Mentioned as future work in the regex-automata article. Produces an **epsilon-free
NFA** directly from the regex — each state corresponds to a position in the
pattern. Better suited to **bit-parallel simulation** (each state = one bit,
process all positions simultaneously with SIMD). Used in **Hyperscan** (Intel,
USENIX NSDI 2019) as the basis for high-throughput multi-pattern matching.

For reform: Glushkov's epsilon-free property means no epsilon-closure
computation — simpler implementation. Worth exploring as an alternative to
Thompson+PikeVM, especially for small patterns (the common case in reform).

---

## Problem 2: Fact-Level Joins and Fixpoint Loop (Datalog/Rete Territory)

Reform's `turn()` re-scans all facts every iteration with no indexing and no
incremental evaluation. The Datalog and expert-system worlds solved this
decades ago.

### Key Techniques and Papers

#### Semi-Naive Evaluation (Datalog, foundational)

Instead of re-evaluating all rules against all facts each iteration, track
**delta facts** (newly added since last iteration) and only join deltas
against the stable set. This is the single biggest optimization for reform's
`turn()` loop. Every Datalog engine does this.

#### Rete Algorithm (Forgy, 1982)

The classic production-system matching algorithm. Compiles all rule patterns
into a **shared DAG network**:

- **Alpha nodes**: filter facts by constant tests (fact type, literal args)
- **Beta nodes**: join across pattern lines, maintaining partial match memories
- **Incremental**: when a fact changes, only affected nodes re-evaluate

Standard in CLIPS, Jess, Drools. Directly addresses reform's fact-level join:
instead of `match_items_detailed` doing a full Cartesian scan per rule, Rete
maintains join state across turns.

- Reference: doi.org/10.1016/0004-3702(82)90020-0

#### TREAT (Miranker, 1987)

Alternative to Rete that drops persistent beta memories — recomputes joins but
avoids memory overhead. Simpler, often faster for small fact sets.

- Reference: AAAI 1987 paper

#### Drools PHREAK

Lazy, goal-oriented Rete variant. Only evaluates nodes when results are
actually needed. Relevant for reform's specificity-ordered firing — rules that
never match don't need their beta networks populated.

- Reference: docs.drools.org

#### Magic Sets (Bancilhon et al.)

Rewrites Datalog rules for goal-directed bottom-up evaluation. Less directly
relevant to reform (which is bottom-up forward chaining, not goal-directed),
but the concept of "only compute what's needed" applies.

- Reference: doi.org/10.1145/6012.15399

#### Souffle (C++ Datalog compiler)

Compiles Datalog to a RAM intermediate representation with automatic **index
selection** (B-trees/Tries per relation). Shows the codegen approach: compile
patterns to indexed lookups rather than interpreting them.

- Reference: souffle-lang.github.io/pdf/cc.pdf

#### DBSP / Differential Datalog

Incremental view maintenance via differential dataflow. Computes *what
changed* mathematically rather than re-running queries. Most sophisticated
incremental approach, but very complex. Overkill for reform's current scale.

- Reference: vldb.org/pvldb/vol16/p1601-budiu.pdf

### Existing Rust Crates (Fact-Level / Engine-Level)

| Crate | What it does | Relevance | Key technique |
|---|---|---|---|
| **datafrog** | Lightweight Datalog engine | **Most directly relevant** | Sorted-relation merge join with galloping search, semi-naive evaluation with delta relations. Used by Rust's own borrow checker (polonius) |
| **crepe** | Proc-macro Datalog compiler | High | Compiles Datalog rules to Rust code with semi-naive eval, auto-generates HashMap indices per relation. Shows codegen approach |
| **ascent** | Feature-rich Datalog | Medium | BYODS (bring-your-own-data-structures), parallel via rayon, lattices. Shows custom index integration |
| **rust-rule-engine** | Full RETE-UL in Rust | **Directly relevant** | Alpha/beta memory indexing, TMS, forward+backward chaining. The Rete implementation to study |
| **clips-sys** | FFI to CLIPS (C) | Reference | Classic RETE engine, shows the approach |
| **kermit** | Leapfrog Triejoin | Research | Worst-case optimal multi-way join. Overkill but shows advanced join theory |
| **egglog** | Datalog + equality saturation | Research | Hash indices, free join planning, parallel. State-of-the-art join planning |
| **polonius** | Rust borrow checker | Reference | Real-world datafrog usage for fixpoint computation |

**datafrog is the standout.** Used by Rust's own borrow checker (polonius),
lightweight, and its core technique — sorted-relation merge join with galloping
search + semi-naive delta evaluation — is exactly what reform's fact-level
matching needs. The galloping merge join is O(n log m) for joining a small
delta against a large stable set, which is the common case in reform's
fixpoint loop.

---

## The Hybrid Reform Needs

No single existing crate or technique solves both problems. Reform needs a
hybrid:

```
+---------------------------------------------------+
|              Engine turn() loop                    |
|  Semi-naive: track delta facts, only re-evaluate  |
|  rules whose patterns could match new facts       |
+---------------------------------------------------+
|         Fact-level join (across pattern lines)     |
|  Indexed fact store (HashMap<Arg, Vec<Fact>>)     |
|  Galloping merge join for multi-fact patterns     |
|  (datafrog's approach)                            |
+---------------------------------------------------+
|      Arg-level matching (within a single fact)     |
|  Tagged NFA (Laurikari) over Arg alphabet          |
|  PikeVM simulation with binding-aware threads      |
|  (regex-automata as reference)                     |
|  List-bound placeholders: capture-reset in         |
|  quantifiers (Barriere & Pit-Claudel 2024)        |
+---------------------------------------------------+
```

---

## Recommended Implementation Path

### Phase 1: Lazy matching + indexing (highest bang/buck)

1. **Lazy matching**: Convert `match_args`/`match_fact` from `Vec`-returning
   to lazy iteration. Callers already only need the first match where `rest`
   succeeds. Few hundred lines, no algorithm change.

2. **Fact indexing**: Add `HashMap<Arg, Vec<usize>>` index on the first
   argument (fact predicate). Replace linear scan in `match_items_detailed`
   with indexed lookup. Also index by arity.

3. **Literal prefiltering**: Extract literal Args from patterns (like regex's
   literal extraction) and use them as a fast pre-scan before running the full
   matcher.

### Phase 2: Semi-naive fixpoint + galloping joins

1. **Semi-naive evaluation**: Track delta facts between iterations. Only
   re-evaluate rules whose patterns could match new facts. Study datafrog's
   approach.

2. **Galloping merge join**: For multi-fact patterns, use sorted-relation
   merge joins with galloping search (datafrog's technique) instead of
   nested-loop scans.

### Phase 3: Tagged NFA + PikeVM (the real NFA solution)

1. **Build a Thompson NFA** from reform's `ArgTemplate` AST with Arg-level
   transitions. Each `ArgTemplate` variant maps directly to NFA states.
   Use regex-automata's `thompson::Builder` as the reference.

2. **Implement PikeVM simulation** with binding-aware capture slots: scalar
   consistency check (like `bind_scalar`) + list-append for repetitions
   (capture-reset semantics from Barriere & Pit-Claudel 2024).

3. **Study Glushkov NFA** as an alternative for small patterns — epsilon-free,
   amenable to bit-parallel simulation.

---

## Prioritized Reading List

1. **Borsotti & Trafimovich (2022)** — "A Closer Look at TDFA" (arXiv:2206.01398)
   — The implementation guide for tagged NFA simulation.

2. **Barriere & Pit-Claudel (2024)** — "Linear Matching of JavaScript Regexes"
   (arXiv:2311.17620v2) — Capture groups inside quantifiers. Directly addresses
   reform's `$( $x )*` list-binding complication.

3. **Laurikari (2000)** — "NFAs with Tagged Transitions" — The formal
   foundation. Tags = reform's placeholders.

4. **datafrog source** (github.com/rust-lang/datafrog) — Rust Datalog engine
   used by polonius. Study galloping merge join and semi-naive delta evaluation.

5. **rust-rule-engine source** (github.com/KSD-CO/rust-rule-engine) — Rust
   RETE-UL implementation. Study the alpha/beta network structure.

6. **Forgy (1982)** — Rete algorithm — The classic paper.

7. **Schmid (2019)** — "Regex with Backreferences" (arXiv:1903.05896) —
   Complexity theory for placeholder consistency.

8. **Kutsia (2002)** — Sequence unification — Formal semantics for reform's
   pattern language.

9. **Hyperscan paper** (Wang et al., 2019, USENIX NSDI) — Glushkov NFA +
   bit-parallel SIMD. Epsilon-free NFA alternative.

10. **regex-automata article** (burntsushi.net/regex-internals/) — The
    architecture overview for the NFA/PikeVM/DFA engine composition approach.