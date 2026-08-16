# Two-Phase Glushkov Pattern Matching Architecture

Proposed architecture for replacing reform's backtracking arg-level pattern
matcher with a Glushkov NFA-based engine. This addresses the arg-level
matching bottleneck (the "regex-like" matching within a single fact's args).

For the fact-level join and fixpoint loop bottlenecks, see
[pattern-matching-research.md](./pattern-matching-research.md).

## Background

Reform's arg-level pattern matching (`match_args` in `src/rule.rs`) is
structurally a regex engine: it matches sequences of string tokens (Args)
against patterns containing literals, placeholders (capture groups), and
repetition operators (`?`/`+`/`*`). The current implementation uses recursive
backtracking with eager `Vec`-returning enumeration, which has exponential
worst case on ambiguous patterns with nested repetitions.

Reform's repetition semantics are **lazy-first**: repetitions match the fewest
possible tokens, backtracking to more only if the rest of the pattern fails.
This is analogous to Perl's `*?` lazy quantifiers.

## Why Glushkov instead of Thompson

The standard NFA approach (used by `regex-automata`) is **Thompson
construction** + **PikeVM simulation**. Thompson construction maps each regex
operator to a mini-NFA connected by epsilon transitions. The PikeVM simulates
the NFA by maintaining a set of active threads and computing **epsilon
closures** at every input symbol — the set of all states reachable via epsilon
chains.

The epsilon closure is the PikeVM's dominant cost. Thompson NFAs have epsilon
split/merge states for every `|`, `*`, `?`, `+` — roughly half the states are
epsilon, and the closure work compounds with nested repetitions. The
regex-automata article spends most of its NFA section on optimizations to
*reduce* epsilon transitions (sparse states, literal tries, minimal UTF-8
automata) precisely because they're the bottleneck.

**Glushkov construction** (also called the position automaton) produces an
**epsilon-free** NFA. Each state represents a *position* in the pattern (a
specific occurrence of a literal or placeholder). Transitions connect positions
that can follow each other. No epsilon transitions at all.

| Property | Thompson | Glushkov |
|---|---|---|
| States | O(m), many epsilon | O(m) + 1 start, all positional |
| Transitions | O(m), many epsilon | O(m²), zero epsilon |
| Epsilon transitions | Yes (the main overhead) | **None** |
| Simulation | Must compute epsilon closures per symbol | Direct transition following |
| Bit-parallel | Hard (epsilon states complicate) | **Natural** (one bit per position) |
| Binding extraction | PikeVM threads + epsilon closure | Thread-priority, direct transitions |

For reform's small patterns (5-20 args, a few placeholders, maybe one
repetition), the O(m²) transition count is negligible. The absence of epsilon
closures is the decisive advantage.

## How Glushkov construction works

### Step 1: Number the positions

For a pattern like `$( $x )* .`, assign each literal/placeholder a unique
position number:

```
$x₁  .₂
```

The `*` isn't a position — it's an operator that affects the follow
relationships.

### Step 2: Compute four functions

- **First(r)**: positions that can start a match of r
- **Last(r)**: positions that can end a match of r
- **Nullable(r)**: whether r can match the empty string
- **Follow(p)**: for each position p, which positions can immediately follow

These are computed recursively from the pattern structure. For concatenation
`rs`, the last positions of `r` are followed by the first positions of `s`.
For repetition `r*`, the last positions of `r` are also followed by the first
positions of `r` (the loop). For alternation `r|s`, First and Last are unions.

Example for `$( $x )* .`:

```
First  = {1, 2}     — can start with $x (enter repetition) or . (skip it, since * is nullable)
Last   = {2}        — must end with .
Nullable = false
Follow(1) = {1, 2}  — after $x, either another $x (loop) or . (exit)
Follow(2) = {}      — . is terminal
```

### Step 3: Build the NFA

```
Start ──$x──→ State 1
Start ──.──→  State 2 (accept)
State 1 ──$x──→ State 1
State 1 ──.──→  State 2 (accept)
State 2 (accept)
```

No epsilon transitions. Every transition consumes a symbol (an Arg).

## Lazy semantics via Follow ordering

Standard Glushkov stores Follow as an unordered set. For lazy semantics, store
it as an **ordered list**: `Follow(p) = [exit_position, loop_position]` — try
exiting the repetition before looping.

The priority is determined at construction time from the pattern structure and
the desired disambiguation policy (lazy or greedy). The NFA structure (states,
transitions) is identical for both policies — only the Follow list ordering
changes.

For `$( $x )* .`:

```
Lazy Follow ordering:
  Follow(start) = [2, 1]   — try . (skip repetition) before $x (enter)
  Follow(1)     = [2, 1]   — try . (exit) before $x (loop)

Greedy Follow ordering:
  Follow(start) = [1, 2]   — try $x (enter) before . (skip)
  Follow(1)     = [1, 2]   — try $x (loop) before . (exit)
```

The NFA accepts the same language either way. Lazy and greedy only affect
*which* accepting path is preferred when multiple exist.

### Tracing lazy against `a b c .`

```
Threads: [{state: start, bindings: {}}]

Arg "a": From start, lazy [2, 1]:
  - .₂: "a" matches "."? No. Dead.
  - $x₁: "a" matches any? Yes. $x = [a].
  Threads: [{state: 1, $x: [a]}]

Arg "b": From state 1, lazy [2, 1]:
  - .₂: "b" matches "."? No. Dead.
  - $x₁: "b" matches any? Yes. $x = [a, b].
  Threads: [{state: 1, $x: [a, b]}]

Arg "c": From state 1, lazy [2, 1]:
  - .₂: "c" matches "."? No. Dead.
  - $x₁: "c" matches any? Yes. $x = [a, b, c].
  Threads: [{state: 1, $x: [a, b, c]}]

Arg ".": From state 1, lazy [2, 1]:
  - .₂: "." matches "."? Yes. (exit, priority 1)
  - $x₁: "." matches any? Yes. $x = [a, b, c, .]. (loop, priority 2)
  Threads: [
    {state: 2, $x: [a, b, c]},      // priority 1 (exit)
    {state: 1, $x: [a, b, c, .]}    // priority 2 (loop)
  ]

Input exhausted. Check in priority order:
  - State 2 is Last (accept) → MATCH. $x = [a, b, c].
```

Lazy correctly stops the repetition at the first opportunity where the rest of
the pattern matches.

### Lazy vs greedy for ambiguous patterns

For `$( $x )* $( $y )*` against `a b`:

- **Lazy**: $x = [], $y = [a, b] (exit first repetition immediately)
- **Greedy**: $x = [a, b], $y = [] (consume everything in first repetition)

Both are valid matches of the same NFA. The Follow ordering determines which
is preferred. Both are correctly produced by the same NFA with different
Follow list ordering.

## The two-phase architecture

```
Phase 1: Bit-parallel Glushkov (structural prefilter)
  Same NFA, bitmask simulation
  "Does any path accept?" — O(n/64) per fact
  Eliminates non-matching facts before binding extraction

Phase 2: Thread-priority Glushkov (binding extraction)
  Same NFA, ordered-thread simulation
  "Which path, with what bindings?" — only on survivors
  Handles lazy semantics + placeholder extraction
  No epsilon closures
```

One NFA construction, two simulation strategies.

### Phase 1: Bit-parallel structural prefilter

Represent the set of active NFA states as a bitmask (one bit per state).
Precompute transition tables as bitmasks: for each Arg value, a bitmask of
"which states can this Arg transition to, from any state." Each step is a
single bitmask operation.

For a pattern with 20 positions, the entire active-state set fits in a `u32`.
Processing each arg in a fact is one bitmask lookup + AND. A fact with 15 args
is ~15 bitmask operations.

The prefilter answers "does any path reach an accept state?" — pure
recognition, no path tracking. Since lazy and greedy accept the same language,
the prefilter is correct regardless of disambiguation policy. It eliminates
facts where no path accepts (genuine non-matches) and passes facts where some
path accepts (possible matches).

**False positives** are facts that structurally match but fail binding
consistency (e.g., `$x is $x` against `a is b` — shape matches, bindings
don't). The prefilter passes these; phase 2 rejects them. This is the same
kind of false positive that any structural prefilter has (including regex's
literal prefilter) — conservative (never rejects a real match), may pass some
non-matches.

### Phase 2: Thread-priority binding extraction

Only runs on facts that passed phase 1. Maintains an ordered list of threads,
each carrying `(NFA_state, bindings)`. For each input Arg:

1. For each active thread (in priority order), look up `Follow(state)` 
   transitions matching the current Arg
2. Spawn a new thread at each matching target, carrying forward bindings
3. If the target is a placeholder position, update bindings (set for scalar,
   append for list-bound)
4. If a scalar placeholder already has a value, check consistency (must be
   equal) — kill the thread if inconsistent
5. Deduplicate threads with the same (state, bindings), keeping the
   higher-priority one

When input is exhausted, check threads in priority order. The first thread at
an accept (Last) state produces the match with its bindings.

No epsilon closures. No epsilon-closure deduplication. Direct transition
following with thread priority for lazy semantics.

### Binding extraction details

Each Glushkov state is a position. If position 3 is a placeholder `$x`, then
when a thread transitions to state 3 after consuming arg[2], the binding for
`$x` is arg[2].

**Scalar placeholders** (not in repetitions): when the thread enters the
placeholder's position, set the binding. If the placeholder already has a
value, check consistency (must be equal). This is reform's `bind_scalar`.

**List-bound placeholders** (in repetitions like `$( $x )*`): each time the
thread passes through the $x position, append the current arg to the list.
Whether a position is "list-bound" or "scalar" is determined at NFA
construction time by checking whether the position is inside a repetition.
This replaces reform's frame-stack with a simple per-position flag.

### Phase 2 simulation pseudocode

The core loop is a tight synchronous iteration over args, branching threads per
transition target, then deduplicating. No async, no OS threads, no generators
— the problem is too small for those tools to help, and their overhead only
makes it slower.

```rust
struct Thread {
    state: u8,           // position index (≤ 20)
    bindings: Bindings,  // small, copy-on-write
    priority: u16,       // insertion order
}

// Per arg: iterate, branch, deduplicate
let mut threads: SmallVec<[Thread; 8]> = vec![Thread::start()];
for arg in fact.args {
    let mut next: SmallVec<[Thread; 8]> = SmallVec::new();
    for t in &threads {
        for target in follow[t.state].matching(arg) {
            let mut b = t.bindings.clone();
            if let Some(ph) = placeholder_at(target) {
                b.set_or_check(ph, arg)?; // None if inconsistent
            }
            next.push(Thread { state: target, bindings: b, .. });
        }
    }
    // Dedup: keep highest-priority thread per (state, bindings)
    threads = dedup(next);
}
threads.iter().find(|t| is_accept(t.state)).map(|t| t.bindings)
```

#### Key optimizations

| Technique | Why it works |
|---|---|
| **`SmallVec<[T; 8]>`** | Thread count ≤ state count (~20). No heap alloc for typical case. |
| **`u8` state index** | Fits in a byte. Array-indexed dedup: `best[state]` instead of HashMap. |
| **Copy-on-write bindings** | `Arc<HashMap<Placeholder, Value>>` — cloning is O(1) until a thread actually diverges. |
| **Priority = insertion order** | No explicit priority queue. First thread pushed wins on ties. |
| **Inline dedup array** | `[Option<Thread>; MAX_STATES]` — zero alloc, O(states) per arg. |

#### The real cost center

It's not the thread management — it's **binding comparison** for scalar
consistency (`$x is $x`). Two threads with different bindings for `$x` must be
checked for equality. That's a string comparison per placeholder per thread per
arg. The optimization there is **interning** — compare `u32` IDs instead of
`str` slices.

## Comparison with Thompson + PikeVM

| | Thompson + PikeVM | Two-phase Glushkov |
|---|---|---|
| Epsilon transitions | Many (half the states) | **Zero** |
| Epsilon closure per step | Yes (main cost) | **None** |
| States | ~2m (with epsilon nodes) | m+1 (positions only) |
| Structural prefilter | No (PikeVM can't do bit-parallel) | **Yes** (phase 1, O(n/64)) |
| Binding extraction | PikeVM threads + epsilon closure | Thread-priority, direct transitions |
| Lazy semantics | Epsilon split ordering | Follow list ordering |
| Binding tracking | Capture slots updated during closure | Capture on direct transition (simpler) |
| Same NFA for both phases | N/A | **Yes** (one construction, two simulations) |

The epsilon closure is the PikeVM's fundamental cost. Glushkov eliminates it.
The thread-priority second phase is structurally simpler than the PikeVM —
fewer states, no closure, simpler deduplication — while preserving all the
semantic capabilities reform needs (lazy ordering, scalar/list bindings,
consistency checking). The bit-parallel first phase gives a prefilter faster
than anything Thompson-based can offer.

## What needs to be built

### 1. Glushkov construction with Follow priority (~200 lines)

Compute First, Last, Nullable, Follow as ordered lists (not unordered sets).
The ordering encodes the lazy policy. Standard Glushkov algorithm, extended
with priority tracking.

Each `ArgTemplate` variant maps to Glushkov operations:
- `Literal(lit)` → one position, matching Arg equal to lit
- `Placeholder(name)` → one position, matching any Arg, with binding capture
- `RepeatedArgs { kind: Optional }` → nullable, Follow includes exit before
  enter (lazy)
- `RepeatedArgs { kind: OneOrMore }` → not nullable, Follow includes exit
  before loop (lazy)
- `RepeatedArgs { kind: ZeroOrMore }` → nullable, Follow includes exit before
  loop (lazy)

Mark each position as scalar or list-bound (inside a repetition or not).

### 2. Bit-parallel simulation for phase 1 (~100 lines)

Represent active states as a bitmask (`u32` or `u64` for small patterns).
Precompute `follow_bitmask: HashMap<Arg, StateMask>` mapping each Arg to the
bitmask of states reachable from any state via that Arg. For placeholder
positions (match any Arg), maintain a separate "any" bitmask OR'd into every
lookup.

Each step: `active = follow_bitmask[arg] & active` (with the "any" mask OR'd
in). After processing all args, check if any accept-state bit is set in
`active`.

### 3. Thread-priority simulation for phase 2 (~300 lines)

Ordered thread list, direct transition following, binding extraction on
placeholder positions, consistency checking for scalar placeholders,
list-append for repetition placeholders, deduplication of threads with same
(state, bindings).

### 4. Follow bitmask table for phase 1 (~50 lines)

The bit-parallel prefilter needs `follow_bitmask[arg][active_states] →
reachable_states`. Since reform's alphabet is interned strings (not bytes),
this is a `HashMap<Arg, StateMask>` per NFA, or a sorted array for binary
search. For placeholder positions, a separate "match any" bitmask is OR'd
into every lookup.

**Total: ~650 lines** for a complete arg-level matching engine.

## Complexity

- **Phase 1 (prefilter)**: O(n / word_size) per fact, where n = number of
  args. For a 15-arg fact with a `u64` bitmask, that's ~1-2 operations per
  arg. This is the fastest possible structural matching.

- **Phase 2 (binding extraction)**: O(m × n × k) per fact, where m = pattern
  positions, n = fact args, k = number of distinct (state, bindings) thread
  combinations alive simultaneously. k is bounded by m (at most one thread per
  state) times the number of distinct binding combinations. In practice, k
  stays small (most patterns have 2-4 placeholders, and facts constrain values
  quickly).

- **Current backtracking**: O(2^n) worst case for ambiguous patterns with
  nested repetitions, plus eager enumeration of all matches when only the first
  is needed.

## Research references

- **Glushkov (1961)** — Original construction algorithm. The position
  automaton.
- **Hyperscan (Wang et al., 2019, USENIX NSDI)** — Production regex engine
  using Glushkov NFA with bit-parallel SIMD matching. Demonstrates the
  bit-parallel approach at scale.
- **Laurikari (2000)** — "NFAs with Tagged Transitions." Formal foundation
  for tags/capture groups in NFAs. Reform's placeholders are tags.
- **Borsotti & Trafimovich (2022)** — "A Closer Look at TDFA" (arXiv:2206.01398).
  Practical implementation guide for tagged NFA simulation, including
  disambiguation policies (leftmost-greedy and POSIX).
- **Barriere & Pit-Claudel (2024)** — "Linear Matching of JavaScript Regexes"
  (arXiv:2311.17620v2). Capture groups inside quantifiers with linear-time
  guarantees. Relevant to reform's list-bound placeholders.
- **regex-automata article** (burntsushi.net/regex-internals/) — Architecture
  overview. Mentions Glushkov as future work: "A Glushkov NFA has a worse time
  complexity for compilation, but it comes with the advantage of not having
  any epsilon transitions... possibly more amenable to bit-parallel
  techniques."