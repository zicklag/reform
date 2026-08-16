# Reform Performance Model

## Current Engine: One Fact Asserted

```
Kitchen north of Bedroom
```

1. **Push to `facts` vec** — O(1).
2. **Run fixed-point loop:**
   - `rebuild_rules()` — scan all facts for `rule` facts, parse pattern strings, build `Rule` structs. O(F). If no new `rule` facts, this is a no-op (same rules as last iteration).
   - `find_all_matches` — for each rule, for each pattern, scan all facts. O(R × P × F).
   - Apply matches: retract consumed facts, assert effect facts (push to `facts`).
   - Repeat until no rule fires.

**Cost**: O(R × P × F) per iteration, potentially multiple iterations.

## Proposed: Semi-Naive + Positional Index

### Data structures

```
facts: Vec<Fact>                          // all facts
index: HashMap<(usize, String), Vec<Fact>> // (position, value) → facts with that value at that position
delta: Vec<Fact>                          // facts that changed since last iteration
```

### One fact asserted

```
assert("north_of", "Kitchen", "Bedroom")
```

1. **Insert into index** — for each position `i` in the fact, insert into `index[(i, fact[i])]`. O(length). Also push to `delta`.
2. **Run fixed-point loop:**
   - If any `delta` fact has predicate `"rule"` → `rebuild_rules()` (parse new rules, add to rule list). New rules get a **one-time full eval** against all facts.
   - For each existing rule: find which patterns could match a `delta` fact. For each such pattern, probe the positional index with the delta's values. Hash-join with other patterns (using all facts). O(R × P × |delta|).
   - Apply matches: retract consumed facts (remove from index + delta), assert effect facts (insert into index + delta).
   - `delta = new_delta`. Repeat until delta empty.

**Cost**: O(R × P × |delta|) per iteration. |delta| is typically 1-10 (the new facts from the previous iteration's rule firings).

### The critical path: a `rule` fact

```
assert("rule", "reverse_rel", "$thing is $rel $( $prep )? $other", "$other is $rel2 $( $prep2 )? $thing")
```

1. Insert into index + delta.
2. `delta` contains a `"rule"` fact → `rebuild_rules()` parses it, adds new `Rule`.
3. New rule gets **one full eval** against all facts (it has no prior match state).
4. After that, it's treated like any other rule — only re-evaluated when `delta` contains facts matching its patterns.

The one-time full eval for new rules is the cost of reflection. It's unavoidable — a new rule has never seen any facts, so it must scan everything once. But after that, it's delta-driven like everything else.

### Index probe safety

The positional index is `HashMap<(usize, String), Vec<Fact>>`. A literal at position `p` is safe to probe iff:

1. It's a non-optional atom or var (not `[literal]` or `[?var]`)
2. Its absolute position is fixed (no rest variable before it)

A literal at a fixed position from the end (e.g., `(..?rest, z)` → `z` is at `-1`) can be probed via a second backward index: `HashMap<(isize, String), Vec<Fact>>`.

Patterns with no safe probe (e.g., `(..?a, is, ..?b)`) fall back to a full scan. In practice these are rare — every pattern in the current demos has at least one safe probe.

### Cost comparison

| Operation | Current | Proposed |
|-----------|---------|----------|
| Assert fact | O(1) | O(length + index_insert) |
| Fixed-point iteration | O(R × P × F) | O(R × P × \|delta\|) |
| New rule | O(R × P × F) | O(F) — one full eval, then delta |
| Retract fact | O(F) (scan to find) | O(index_remove) |
| Memory | O(F) | O(F + index_overhead) |

### Why not Rete

Rete shares partial matches across rules, which is valuable at 10K rules and 1M facts. But it requires:

- Fixed predicates (variable predicates need full-scan alpha nodes)
- Stable rule sets (adding a rule requires re-validating all existing partial matches — O(F × network))
- Complex implementation (~5000 lines)

For Reform's use case (interactive games, dynamic rules, variable predicates), semi-naive + positional index is the right tradeoff: simpler, handles dynamic rules trivially, and gives O(|delta|) per iteration.
