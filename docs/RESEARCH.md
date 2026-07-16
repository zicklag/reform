# Reform: Research Report on Fact/Rule Rewriting Systems

## Overview

This report surveys the landscape of production rule systems, Datalog engines, pattern-matching formalisms, and query optimization techniques relevant to Reform's design goals. The analysis is organized around three axes: (1) query-optimizable fact spaces, (2) dynamic rules and runtime optimization, and (3) pattern matching, hygiene, and rewrite semantics.

---

## 1. Query-Optimizable Fact Spaces

Reform is essentially a forward-chaining production system over a tuple store. The core operation is: given a set of patterns (a rule body), find all matching tuples in the fact base, bind variables, and produce new facts. This is isomorphic to evaluating a conjunctive query over a relational database.

### 1.1 The Landscape of Match Algorithms

**Rete (Forgy, 1982)** — The canonical algorithm. Compiles rules into a discrimination network of alpha nodes (single-pattern tests) and beta nodes (join tests). State is saved between cycles: each alpha node stores matching facts, each beta node stores partial join results. On fact insertion/retraction, only the affected nodes are re-evaluated.

- **Strengths:** O(|delta|) per iteration after initial compilation. Excellent for stable rule sets with many shared sub-patterns. ~5000 lines of C in CLIPS.
- **Weaknesses:** Requires fixed predicates (variable predicates need full-scan alpha nodes). Adding a rule at runtime requires re-validating all existing partial matches — O(F x network). High memory: stores all partial join results. Complex implementation.
- **Best for:** 10K+ rules, 1M+ facts, stable rule sets, shared sub-expressions.

**TREAT (Miranker, 1987)** — Removes beta memories from Rete. On each cycle, for each changed fact, re-evaluates joins from scratch using only alpha memories. Uses relational query optimization (seed-ordering, semijoin reduction) to order joins dynamically.

- **Strengths:** Lower memory than Rete (no beta memories). Can use dynamic join ordering per cycle. Often outperforms Rete on OPS5 benchmarks (50%+ faster in Miranker's study).
- **Weaknesses:** Re-does join work each cycle. Can be slower than Rete when join results are large and stable.
- **Best for:** Memory-constrained environments, dynamic join patterns.

**LEAPS (Batory, 1994)** — Lazy evaluation. Instead of finding all matches, finds the *first* match for each rule using a stack-based approach. Uses "composite containers" and cursors to lazily enumerate matches. Only computes what's needed to fire one rule.

- **Strengths:** O(1) space per rule (no partial match storage). Very low memory. Used by modern CHR systems.
- **Weaknesses:** Can be slower for exhaustive queries (needs to find all matches). Stack management overhead.
- **Best for:** Memory-constrained, when only one match is needed per cycle.

**Semi-naive evaluation (Datalog standard)** — The standard bottom-up Datalog evaluation. Maintains a "delta" set of newly derived facts per iteration. For each rule, only considers derivations where at least one body atom comes from the delta. Avoids re-deriving the same facts.

- **Strengths:** Simple, well-understood, O(|delta|) per iteration. Handles recursion naturally. Easy to implement.
- **Weaknesses:** Not goal-directed (computes all derivations, not just query-relevant ones). Can compute irrelevant facts.
- **Best for:** General-purpose Datalog, moderate fact counts.

### 1.2 Reform's Current Position

Your current engine is naive forward chaining: O(R x P x F) per iteration. Your PERF.md correctly identifies semi-naive + positional index as the next step. The analysis there is sound.

**Key insight from the literature:** The semi-naive + positional index approach you've proposed is essentially the same strategy used by modern Datalog engines (Souffle, RDFox, VLog). It's not just a stepping stone — it's the production standard. Souffle compiles Datalog to C++ and achieves near-hand-tuned performance using exactly this approach plus aggressive join optimization.

### 1.3 Indexing Strategies

The positional index `HashMap<(usize, String), Vec<Fact>>` from PERF.md is a good start. The Datalog literature suggests several refinements:

**B-tree indexes** (Souffle default) — Allow range queries, not just equality. Useful for ordered predicates like `(age, >, 30)`.

**Brie indexes** — More memory-efficient for sparse, large relations. Souffle uses these for relations with high selectivity.

**Multi-column indexes** — For patterns like `(?x, is, ?y, of, ?z)`, a single index on `(pos, value)` can't efficiently probe the join between `(?x, is, ?y)` and `(?y, is, ?z)`. Multi-column indexes or hash-join strategies are needed.

**Column-store layout** — Store each column of a relation separately. Enables efficient projection without reading full tuples. Used by RDFox and VLog for RDF triple stores.

### 1.4 Recommendation for Reform

The semi-naive + positional index plan is the right next step. But consider:

1. **Add a second backward index** `HashMap<(isize, String), Vec<Fact>>` for fixed-position-from-end probes (as PERF.md notes). This handles patterns like `(..?rest, z)`.

2. **Consider B-tree indexes** instead of hash maps. They support range queries and ordered iteration, which enables:
   - Sorted merge joins
   - Aggregation (min/max/count)
   - Ordered output for debugging

3. **Columnar storage** for the fact base. Instead of `Vec<Fact>` (vec of vecs), store each predicate's facts as a column-oriented structure. This reduces memory and enables SIMD-accelerated matching.

---

## 2. Dynamic Rules and Query Optimization

Reform's killer feature — rules created at runtime via `rule(...)` facts — is the hardest constraint on algorithm choice.

### 2.1 The Dynamic Rule Problem

When a new rule is added at runtime:

- **Rete:** Must graft new alpha/beta nodes onto the existing network. Existing partial matches must be "pushed through" the new nodes. This is O(F x network) in the worst case. YES/OPS (IBM, 1988) implemented this by creating a "mini-RETE" and grafting it, sharing existing nodes where possible. It works but is complex.

- **TREAT:** Simpler — just add the rule's patterns to the rule set. On the next cycle, the new rule's joins are evaluated from scratch using existing alpha memories. No partial match state to update.

- **LEAPS:** Also simple — add the rule to the rule set. The lazy evaluation means the new rule only starts matching when its turn comes on the stack.

- **Semi-naive:** The simplest — add the rule, do one full scan against all facts (as PERF.md notes), then it's delta-driven like everything else.

**The literature is clear:** For dynamic rules, Rete's complexity is a liability. Semi-naive and TREAT handle it naturally. This is why modern Datalog engines (which support runtime rule addition in some form) use semi-naive, not Rete.

### 2.2 Rules as Query Optimizers

Your insight that "rules are our highest priority queries" is powerful and under-exploited in the literature. Here's what it enables:

**Magic Sets transformation** (Souffle, Datalog standard) — A compile-time transformation that makes bottom-up evaluation goal-directed. Given a query pattern, it analyzes which arguments are bound/free, propagates these bindings "upward" through rule dependencies, and generates specialized "magic" rules that only compute relevant facts. This is the standard technique for making Datalog query-optimizable.

**For Reform:** A rule like `(?x, is, ?y)` with a bound `?x` (from a specific query) can be magic-set transformed to only compute `is` facts for that `?x`. This is exactly what you want: rules inform index generation.

**Sideways Information-Passing Strategy (SIPS)** — Determines the order in which body atoms are evaluated. A good SIPS minimizes intermediate result sizes. Souffle's auto-scheduler uses runtime statistics to choose join orders, achieving 12x speedups over naive ordering.

**For Reform:** When a rule is created, you can analyze its patterns to determine:
- Which positions are bound (from earlier patterns in the same rule)
- Which literals can be used as index probes
- The optimal join order (smallest intermediate results first)

This analysis is cheap (O(pattern_length)) and can be done at rule-creation time.

### 2.3 The New Rule Cost

PERF.md correctly identifies the one-time full eval for new rules as O(F). This is unavoidable — a new rule has never seen any facts. But there's a nuance:

**If the new rule's patterns share structure with existing rules**, you can reuse existing index structures. For example, if rule A has `(?x, is, ?y)` and new rule B also has `(?x, is, ?y)`, the index for `is` at position 0 is already built. The full eval is just a scan of the index, not a scan of all facts.

### 2.4 Recommendation for Reform

1. **Stick with semi-naive + positional index.** It handles dynamic rules trivially and is the standard approach in modern systems.

2. **Implement SIPS-based join ordering** at rule-creation time. For each rule, compute the optimal evaluation order of its body patterns. This is a small static analysis that pays off on every firing.

3. **Consider Magic Sets** as a future optimization. When a rule has bound variables (from outer rules or query patterns), generate specialized versions that only compute relevant facts. This is the bridge between "rules as queries" and "rules as index generators."

4. **The one-time full eval for new rules is fine.** It's O(F) and happens once. After that, the rule is delta-driven. For interactive games (Reform's target use case), F is small enough that this is negligible.

---

## 3. Pattern Matching, Hygiene, and Rewrite Semantics

This is where Reform's design is most novel and most in need of formal grounding.

### 3.1 Current Pattern System

Reform's patterns are tuples of `Pat` atoms (literals or variables) with:
- `?var` — single-element variable binding
- `..?var` — rest/splat variable (matches 0+ elements)
- `[?var]` — optional binding (matches 0 or 1, skips if absent)
- `[literal]` — optional literal
- `-(pattern)` — negation (fact must not exist)
- `!(pattern)` — negation (fact must not exist, different syntax?)

The matching algorithm uses backtracking with shortest-match semantics for rest variables.

### 3.2 Hygiene Concerns

Your instinct about hygiene is correct. The current system has several ambiguity sources:

**1. Variable capture in nested rules.** When a rule creates another rule (e.g., `def_reverse_rel` in `iflib.rf`), the inner rule's patterns are strings embedded in the outer rule's effects. Variable names like `?thing`, `?rel1`, `?other` appear in both the outer and inner rule. The `substitute()` function in `fact.rs` handles this by recursively parsing and substituting pattern strings, but:

```
(rule, reverse_rel,
    ( (?thing, is, ?rel1, [?prep1], ?other) ),
    ( (?other, is, ?rel2, [?prep2], ?thing) )
)
```

Here `?thing` in the inner rule's match pattern is bound by the outer rule's match. But `?rel2` and `?prep2` are *new* variables that only exist in the inner rule. The distinction is implicit — you have to know which variables appear in the outer rule's bindings to understand the inner rule.

**2. Optional binding semantics.** `[?var]` matches 0 or 1 elements. When it matches 0, the variable is unbound. Later patterns that reference the same `?var` see an empty binding, which acts as a wildcard. This is clever but subtle — the same variable name can mean "bound to a value" or "wildcard" depending on whether an optional matched.

**3. Rest variable scope.** `..?rest` in a pattern like `(?cmd, ..?args)` captures all remaining elements. But in a rule effect like `(print, ..?args)`, the `..?args` splats the bound list into the output fact. The `..` prefix means different things in match vs. effect contexts.

**4. Negation semantics.** `-(pattern)` means "this pattern must not match." But the scope of variables in negated patterns is unclear — can a negated pattern introduce new bindings? (In standard Datalog, negated atoms cannot bind new variables — they must be ground by positive atoms first.)

### 3.3 Formal Approaches

**Datalog's approach** — The cleanest formal semantics. Rules are Horn clauses:
```
ancestor(X, Z) :- parent(X, Y), ancestor(Y, Z).
```
- Variables are universally quantified over the rule.
- Every variable in the head must appear in a positive body atom (range restriction).
- Negation is stratified: negated atoms can only reference variables bound by positive atoms earlier in the body.
- No optional/rest patterns — fixed arity predicates.

**CHR's approach** — Multi-set rewriting with guards:
```
leq(X, Y), leq(Y, X) <=> X = Y.    % antisymmetry
leq(X, Y), leq(Y, Z) ==> leq(X, Z). % transitivity
```
- Three rule types: simplification (`<=>`), propagation (`==>`), simpagation (`\`).
- Guards are tests, not bindings.
- Variables in the head are matched; guards can test but not bind.
- The refined operational semantics (Duck et al., 2004) gives a deterministic execution order.

**Egison's approach** — Extensible pattern matching with matchers:
```haskell
matchAll [1,2,3] (Multiset Integer) [[mc| $x : $xs -> (x, xs) |]]
-- [(1,[2,3]),(2,[1,3]),(3,[1,2])]
```
- Patterns are interpreted by *matchers* — user-definable objects that control decomposition.
- Non-linear patterns (same variable appearing multiple times) are handled with backtracking.
- Matchers can be composed: `Multiset Integer` means "treat the list as a multiset of integers."
- This gives clean semantics for optional/splat patterns: they're just different matchers.

### 3.4 Concrete Recommendations

**1. Adopt explicit variable scoping for nested rules.**

Instead of relying on implicit capture from the outer rule's bindings, consider a syntax that makes the distinction clear:

```
(rule, reverse_rel,
    ( (?thing, is, ?rel1, [?prep1], ?other) ),
    ( (rule, reverse_rel_inner,
        ( (?thing, is, ?rel1, [?prep1], ?other) ),   % ?thing, ?rel1, ?prep1, ?other from outer
        ( (?other, is, ?rel2, [?prep2], ?thing) )     % ?rel2, ?prep2 are new
      )
    )
)
```

Options for making this explicit:
- **Lambda-style:** `(rule, name, (args), (body))` where `args` declares the rule's formal parameters.
- **Scope annotations:** `$outer.thing` vs `$inner.rel2` to distinguish variable sources.
- **Fresh variable prefix:** New variables in inner rules must use a different prefix or be explicitly declared.

**2. Formalize optional binding semantics.**

The current `[?var]` semantics (empty binding = wildcard) is elegant but fragile. Consider:

- **Explicit optional marker:** `?var?` or `?var=optional` to make the optionality syntactically visible at the use site, not just the definition site.
- **No wildcard from empty binding:** An unbound optional variable should cause a match failure if referenced in a later pattern, not silently act as a wildcard. This is safer and more predictable.
- **Alternative:** Use `?var` everywhere and add a separate `skip` pattern for optional elements: `(skip, ?var)` matches 0 or 1 elements and binds `?var` to a list of 0 or 1 values.

**3. Adopt range restriction for negated patterns.**

Formalize: every variable in a negated pattern must be bound by a positive pattern earlier in the rule body. This is standard Datalog practice and prevents ambiguity about whether negation introduces bindings.

**4. Consider a more structured pattern language.**

The current string-based pattern representation (`"(?x, is, ?y)"`) is flexible but makes analysis hard. A structured representation (like Datalog's atoms or CHR's head patterns) would enable:

- **Static analysis** for join ordering, index selection, and magic set transformation.
- **Better error messages** for malformed patterns.
- **Pattern compilation** to efficient matching code.

This doesn't mean abandoning the tuple syntax — just parsing it into a richer internal representation at rule-creation time, rather than re-parsing strings on every match.

**5. Consider CHR's rule types.**

CHR's three rule types map well to Reform's use cases:

| CHR | Reform equivalent | Use case |
|-----|------------------|----------|
| `Head <=> Body` (simplification) | `(-pattern), (effect)` | Consume facts, produce new ones |
| `Head ==> Body` (propagation) | `(pattern), (effect)` without consume | Derive new facts without removing triggers |
| `Head1 \ Head2 <=> Body` (simpagation) | `(-pattern1), (pattern2), (effect)` | Keep some matched facts, replace others |

Adopting this taxonomy would make the semantics of consume vs. non-consume patterns explicit and formal.

---

## 4. Synthesis: A Roadmap for Reform

### Phase 1 (Near-term): Semi-naive + Positional Index

As outlined in PERF.md. This is the highest-impact change and is well-understood.

- Implement `delta: Vec<Fact>` and `index: HashMap<(usize, String), Vec<Fact>>`
- On assert: insert into index + delta
- On fixed-point iteration: for each rule, probe index with delta facts
- New rules: one full eval, then delta-driven

### Phase 2 (Medium-term): Structured Pattern Representation

Parse patterns into a richer internal form at rule-creation time:

```rust
struct RulePattern {
    predicate: Option<String>,  // None for variable predicates
    args: Vec<ArgPattern>,
}

enum ArgPattern {
    Literal(String),
    Variable(String),
    Rest(String),        // ..?var
    Optional(Box<ArgPattern>),  // [?var] or [literal]
}
```

This enables:
- Static analysis for join ordering
- Index selection (which positions to index)
- Better error messages
- Pattern compilation

### Phase 3 (Medium-term): SIPS-based Join Ordering

At rule-creation time, compute the optimal evaluation order:

1. Identify which patterns have safe index probes (fixed-position literals or bound variables)
2. Order patterns to minimize intermediate result sizes
3. Generate a query plan for the rule body

### Phase 4 (Long-term): Magic Sets for Goal-Directed Evaluation

When a rule has bound variables (from a query or outer rule), generate specialized versions:

- Analyze which arguments are bound/free
- Propagate bindings through rule dependencies
- Generate "magic" rules that only compute relevant facts

This is the bridge between "rules as queries" and "rules as index generators."

### Phase 5 (Long-term): Formal Pattern Semantics

- Adopt range restriction for negated patterns
- Formalize optional binding semantics (no wildcard from empty binding)
- Consider CHR-style rule types (simplification/propagation/simpagation)
- Document the variable scoping rules for nested rules

---

## 5. Key References

| Paper | Key Idea | Relevance |
|-------|----------|-----------|
| Forgy, "Rete" (1982) | Discrimination network for production rules | Baseline for comparison |
| Miranker, "TREAT" (1987) | No beta memories, dynamic join ordering | Lower memory alternative |
| Batory, "LEAPS" (1994) | Lazy evaluation, O(1) space | Memory-constrained use cases |
| Bancilhon, "Semi-naive evaluation" (1986) | Delta-driven Datalog evaluation | Reform's next step |
| Souffle project (Scholz et al., 2016+) | Compiling Datalog to C++ | Reference implementation |
| Frühwirth, "CHR" (1998) | Multi-set rewriting with guards | Pattern semantics inspiration |
| Van Weert, "Efficient Lazy Evaluation" (2009) | CHR lazy matching vs Rete | Dynamic rule performance |
| Egi, "Non-linear Pattern Matching" (2018) | Extensible matchers for non-free types | Pattern system design |
| Herman & Wand, "Hygienic Macros" (2007) | Formal hygiene for macro systems | Variable capture in nested rules |
| Alvarez-Picallo, "Fixing Incremental Computation" (2018) | Derivatives of Datalog fixpoints | Formal foundation for incremental eval |

---

## 6. Open Questions

1. **Variable predicates.** Reform allows `(?rel, of, ?thing)` where `?rel` is a variable predicate. This is unusual in Datalog (predicates are usually fixed). How does this interact with indexing? (PERF.md notes this forces full-scan alpha nodes in Rete.)

2. **Consume semantics.** Reform's `consumes` field selects which matched facts to remove. This is more expressive than standard Datalog (which only adds facts). How does this interact with semi-naive evaluation? (Delta must track removals too.)

3. **Fixed-point termination.** With consume semantics, the fact base can shrink. Does the fixed-point loop always terminate? (With range-restricted rules and no function symbols, yes — the fact space is finite.)

4. **Conflict resolution.** When multiple rules match, which fires? Current code fires all matches (no conflict resolution). This is fine for pure Datalog but may cause issues with side-effecting rules (like `print`).

5. **Stratified negation.** Reform's negation `-(pattern)` is not stratified — a rule can negate a fact that the same rule produces. This can lead to non-termination or non-determinism. Consider requiring stratified negation (negated predicates must be computable without the current rule's output).
