# Reform Performance Evaluation & Benchmarking Plan

This plan is about **measurement**: how to evaluate performance, attribute
time to specific layers, prove an optimization helped, and detect
regressions. The optimization roadmap in
[pattern-matching-research.md](./pattern-matching-research.md) and
[glushkov-architecture.md](./glushkov-architecture.md) defines *what* to
optimize; this plan defines *how to know it worked*.

## 1. The Measurement Gap

The only existing harness is `reform-cli/src/bin/bench.rs`: it runs N
iterations of loading one file (default `examples/demo-3.rf`, 2000×) and
prints a single `ms per iteration` number. Two `perf record` captures exist
(`perf_orig.data`, `perf_baseline.data`) from ad-hoc profiling.

That single number cannot answer any of the questions that matter:

- **Where** is the time going — arg matching, fact joins, or the fixpoint loop?
- **Did** an optimization help, or did noise swamp a 5% gain?
- **Is** the prefilter earning its keep — how many facts does it reject?
- **Are** we regressing — did a refactor quietly make the join 2× slower?

We need three things the current harness lacks: **per-layer attribution**,
**statistical rigor**, and **instrumented counters** that explain *why* a
number moved.

## 2. The Three Cost Centers

Reform's runtime cost decomposes into three independent layers. Each needs a
different measurement strategy because they fail at different scales.

### 2.1 Arg-level matching — `match_args` (`src/rule.rs`)

Recursive backtracking over a string-token alphabet (`&[Arg]`) with
placeholder bindings. Returns **all** matches eagerly as a `Vec`, when
callers only need the first that satisfies the rest of the pattern.
Exponential worst case on ambiguous nested repetitions (`$( $a )* $( $b )*`).

The **Glushkov NFA prefilter** (`NfaPrefilter` in `src/regex.rs`) is already
wired into `PatternFact::match_fact` (line 496): it rejects facts whose arg
shape can't structurally match before running the binding matcher. It is a
sound over-approximation — it admits false positives (e.g. `$x is $x` against
`a is b` matches structurally but fails binding consistency) but never drops a
real match.

**Metric to prove improvements here**: match attempts avoided by the
prefilter, binding-matcher invocations, per-fact match latency.

### 2.2 Fact-level joins — `match_items_detailed` (`src/rule.rs:795`)

Nested-loop scan over a flat `Vec<Fact>` with no indexing. For each pattern
item, it iterates **every** fact (`for i in 0..facts.len()`) and clones the
`used` bitmap per candidate (`used.to_vec()` at line 820). Cost is
O(pattern_items × facts) per rule evaluation, every turn.

**Metric to prove improvements here**: facts scanned per rule evaluation,
candidate facts that pass the prefilter, join selectivity (matches / scans).

### 2.3 Engine fixpoint loop — `turn()` (`src/engine.rs:472`)

Every `turn()` iteration clones **all** facts (`self.facts.clone()` at line
490) as a snapshot for consistent matching, and clones **all** rules (line
473). There is no incremental/delta evaluation: a turn that adds one fact
re-evaluates every rule against every fact. The `fired` dedup (line 499)
allocates a `HashSet<Fact>` per (rule, matched-set) pair.

**Metric to prove improvements here**: turns to reach fixpoint, rule
evaluations (rule × turn), facts cloned, rule firings.

## 3. Workload Corpus

A single workload cannot expose all three cost centers. The corpus spans
real programs (correctness + realism), synthetic scale generators (stress),
and pathological cases (worst-case bounds).

### 3.1 Real workloads (macro, end-to-end)

| File | Lines | Primary stress |
|---|---|---|
| `examples/game.rf` | 33 | Cold-start latency (small game) |
| `examples/lang.rf` | 24 | Minimal bootstrap |
| `examples/demo-1.rf` | 94 | Sentence reification rules |
| `examples/demo-2.rf` | 139 | Optional `?` arg matching |
| `examples/demo-3.rf` | 234 | Mixed `?`/`*`, specificity ordering |
| `examples/demo-4.rf` | 143 | Body-generated inner rules |
| `examples/the-new-guard-ep-1.rf` | 40 | Modular load |
| `examples/cloak-of-darkness.rf` | 498 | Full interactive fiction (integration) |
| `examples/iflib/game.rf` | — | Modular library game |

These are the correctness corpus (also run as `tests/cases.rs`) and the
realistic latency baseline. Cloak of Darkness is the integration benchmark —
it exercises every feature.

### 3.2 Synthetic scale workloads (stress, generated)

A generator bin (`reform-cli/src/bin/gen-workload.rs`) emits `.rf` programs at
controlled scale. Each pattern type isolates one cost center.

| Workload type | What it stresses | Generator output |
|---|---|---|
| `catch-all` | Arg repetition scan × fact count | 1 rule `( parse $( $word )* )`, N `parse` facts with growing arg counts |
| `literal-filter` | Prefilter literal rejection | K rules each anchored on a distinct literal first arg, N facts with varied first words |
| `repeated-binding` | Binding consistency (prefilter false positives) | Rules with `$x ... $x`, facts that pass the prefilter but fail binding |
| `multi-fact-join` | Fact-level nested-loop join | Rules with 2–3 fact pattern lines, N facts per predicate |
| `deep-fixpoint` | Turn loop depth / fixpoint iterations | Rule that peels one fact per firing, N queued facts |

Scale axis: N ∈ {100, 1k, 10k, 100k} facts. This reveals the exponent in the
O(R × P × F) scaling — a log-log plot of time vs N should show the slope.

### 3.3 Pathological workloads (worst-case bounds)

These prove the worst-case claim and benchmark whether an optimization
removes the pathological case:

- **Nested ambiguity**: `$( $a )* $( $b )*` against a long fact — the
  backtracking matcher enumerates all split points exponentially. Measures
  the gap between the prefilter (which accepts in O(n)) and the binding
  matcher (which backtracks).
- **Repeated-binding cascade**: `$x is $x is $x ...` with a fact that binds
  inconsistently at the last position — forces the matcher to backtrack
  through the entire chain. Measures binding-comparison cost.
- **Join cross-product**: a rule with two `*` fact repetitions over N matching
  facts each — O(N²) join expansion.

## 4. Instrumentation: Engine Counters

The single highest-leverage addition. A `PerfStats` struct on `Engine` that
counts the operations that determine cost, so a before/after diff explains
*why* a number moved — not just *that* it moved.

```rust
#[derive(Debug, Default, Clone)]
pub struct PerfStats {
    /// Fixpoint iterations in the last `turn()` call.
    pub turns: u64,
    /// `Rule::find_matches_detailed` invocations (rule × turn).
    pub rule_evals: u64,
    /// `PatternFact::match_fact` calls that reached the binding matcher
    /// (i.e. passed the prefilter).
    pub match_attempts: u64,
    /// `NfaPrefilter::matches` calls.
    pub prefilter_calls: u64,
    /// Facts rejected by the prefilter (never reached the binding matcher).
    pub prefilter_rejections: u64,
    /// `match_fact` calls that produced at least one binding.
    pub matches: u64,
    /// Rules that actually fired (produced body facts).
    pub rule_firings: u64,
    /// Total facts iterated inside `match_items_detailed` (the scan cost).
    pub facts_scanned: u64,
    /// `self.facts.clone()` calls (the snapshot cost).
    pub facts_cloned: u64,
}
```

Enabled via `engine.set_perf_stats(true)` (off by default, zero cost when
off). `Engine::perf_stats()` returns the accumulated counters; `reset()` at
the start of each measured region.

**Why each counter matters:**

| Counter | Proves |
|---|---|
| `prefilter_rejections / prefilter_calls` | Prefilter effectiveness — the Glushkov NFA's hit rate |
| `match_attempts` | Binding-matcher load after prefilter (should drop as prefilter improves) |
| `facts_scanned` | Join cost — drops when indexing lands (semi-naive evaluation) |
| `facts_cloned` | Snapshot cost — drops when the per-turn clone is eliminated |
| `turns` / `rule_evals` | Fixpoint efficiency — drops with delta evaluation |
| `rule_firings` | Correctness invariant — must stay constant across optimizations (same rules fire) |

The `rule_firings` counter is the **correctness anchor**: any optimization
that changes the number of rule firings altered semantics, not just
performance. It must match the baseline for every workload.

## 5. Benchmark Harness

### 5.1 Macro benchmarks — `hyperfine`

`hyperfine` is already installed and is language-agnostic with built-in
statistics (warmup, multiple runs, median + CI, outlier detection). It wraps
the existing `bench` binary (and any new ones):

```sh
# Warmup + 10 runs, compare two builds
hyperfine --warmup 3 --runs 10 \
  --export-json baseline.json \
  "target/release/bench 2000 examples/cloak-of-darkness.rf"

hyperfine --warmup 3 --runs 10 \
  --export-json optimized.json \
  "target/release/bench 2000 examples/cloak-of-darkness.rf"

# Statistical comparison
hyperfine --warmup 3 --runs 10 \
  --baseline baseline.json \
  "target/release/bench 2000 examples/cloak-of-darkness.rf"
```

Run every real workload (§3.1) at iteration counts that give 0.5–2 s total
runtime (hyperfine needs measurable wall time). The bench binary already
loops internally; tune N so the binary runs ~1 s.

### 5.2 Micro benchmarks — isolated functions

The library is wide open (all functions public), so each cost center can be
timed in isolation. Each microbenchmark is a small bin that loops the
function and prints `ns/op`; `hyperfine` wraps it for statistics.

| Bin | Isolates | Calls |
|---|---|---|
| `bench-argmatch` | Arg-level matching | `PatternFact::match_fact` on synthetic facts × patterns |
| `bench-prefilter` | Prefilter only | `NfaPrefilter::matches` on N facts |
| `bench-join` | Fact-level join | `Rule::find_matches_detailed` over N facts |
| `bench-parse` | Parsing throughput | `parser::facts` on a large source string |
| `bench-rule-compile` | Rule compilation | `Rule::parse` on K rules |

For sub-millisecond operations, the bin loops internally (e.g. 100k calls)
and prints one `ns/op` line; hyperfine runs the bin 10× for a stable median.
This avoids process-startup noise without a `criterion` dependency.

The instrumented counters (§4) are printed alongside each microbenchmark, so
a `bench-join` run reports both `ns/op` and `facts_scanned` — proving the
time moved *because* the scan count dropped.

### 5.3 Statistical protocol

- **Build**: `cargo build --release` (the release profile already has
  `lto = "fat"`, `codegen-units = 1`, `strip = true`).
- **Profiling build**: `cargo build --profile profiling` (has debug symbols)
  for `perf record` / flamegraph only — never for timing.
- **Warmup**: 3 unmeasured runs (fills caches, JITs nothing but stabilizes
  allocator state).
- **Runs**: 10 measured runs, report median ± CI.
- **Noise floor**: run the same binary twice via hyperfine's `--rerun` to
  confirm the variance band; any "improvement" smaller than the band is noise.
- **Machine**: record CPU model, frequency scaling state, and whether
  turbo-boost is on. Disable frequency scaling for reproducible numbers:
  `sudo cpupower frequency-set -g performance`.

## 6. Profiling Workflow

To find *where* time goes (not just confirm a hypothesis), use sampling
profiling + flamegraphs.

### 6.1 CPU profiling

```sh
# Build with symbols
cargo build --profile profiling

# Sample the macro workload
perf record --call-graph=dwarf -F 999 -- \
  target/profiling/bench 5000 examples/cloak-of-darkness.rf

# Generate a flamegraph (install once: cargo install flamegraph)
cargo flamegraph --profile profiling -- \
  --bin bench -- 5000 examples/cloak-of-darkness.rf
```

`cargo-flamegraph` is the standard wrapper (installs via `cargo install
flamegraph`; pulls in the `flamegraph` perl script). If unavailable, the raw
path is `perf record` → `perf script` → `inferno-collapse-perf` /
`flamegraph.pl`.

Read the flamegraph for the three signatures:
- A wide `match_args` / `match_rep` / `match_reps` bar → arg-level matching
  dominates; the backtracking matcher or eager `Vec` allocation is the cost.
- A wide `match_items_detailed` / `match_fact_repetition_detailed` bar →
  fact-level join dominates; the linear scan or `used.to_vec()` clone is the
  cost.
- A wide `turn` / `clone` / `sort_by_key` bar → the fixpoint loop overhead
  (snapshot clone, rule re-sort) dominates.

### 6.2 Allocation profiling

`match_args` returns `Vec<(usize, State)>` and `match_items_detailed` clones
`used` per candidate. Allocation pressure is a likely hidden cost:

```sh
# Count allocations (jemalloc's heap profiling or valgrind massif)
valgrind --tool=massif --stacks=yes \
  target/profiling/bench 500 examples/cloak-of-darkness.rf
ms_print massif.out.*
```

Or add a temporary `#[track_caller]` allocation counter to confirm the
`Vec`/`to_vec` hotspots the flamegraph suggests.

## 7. Baseline & Regression Protocol

### 7.1 Record the baseline

Before any optimization, on a fixed machine with frequency scaling locked:

```sh
./scripts/perf-baseline.sh   # runs the full corpus, writes docs/PERF-BASELINE.md
```

`docs/PERF-BASELINE.md` records, per workload:
- `hyperfine` median ± CI (ms/op)
- `PerfStats` counters (turns, rule_evals, facts_scanned, prefilter hit rate,
  rule_firings)
- flamegraph snapshot link (optional)

### 7.2 Prove an improvement

For each optimization:

1. Re-run `hyperfine --baseline` on the affected workloads.
2. Re-run the instrumented counters; diff against baseline.
3. Assert `rule_firings` is unchanged (correctness anchor).
4. Update `docs/PERF-BASELINE.md` with the new numbers and the ratio.
5. If the optimization targets a worst-case (§3.3), run the pathological
   workload and show it no longer blows up.

### 7.3 Detect regressions

A CI-runnable `scripts/perf-check.sh` (or a test) runs the core corpus, feeds
`PerfStats` + timing into a tolerance check against the committed baseline.
If `facts_scanned` or `ns/op` exceeds baseline × 1.1, fail. Keep the
threshold loose (10–15%) to survive CI noise; tight comparisons use the local
`hyperfine --baseline` workflow.

## 8. Improvement Roadmap (mapped to measurements)

The research docs define the optimizations. This table maps each to the
metric that proves it worked, so we measure *toward* the improvement, not
after the fact.

| Optimization (from research) | Target layer | Metric that proves it |
|---|---|---|
| Lazy `match_args` (iterator, not `Vec`) | Arg-level | Allocation count drops; `ns/op` on `bench-argmatch` drops; pathological nested-ambiguity no longer exponential |
| Glushkov thread-priority binding extraction (phase 2) | Arg-level | `match_attempts` unchanged but `ns/op` drops; worst-case becomes linear (measured on §3.3) |
| Fact index (HashMap by first-arg literal) | Fact-level join | `facts_scanned` drops from O(F) to O(matches); `bench-join` `ns/op` scales sub-linearly with N |
| Semi-naive evaluation (delta tracking) | Fixpoint loop | `rule_evals` and `facts_scanned` drop proportional to `|delta|`, not `|F|`; `turns` may rise (more, cheaper turns) but total work falls |
| Eliminate per-turn `facts.clone()` snapshot | Fixpoint loop | `facts_cloned` → 0; macro `ns/op` drops on large-fact workloads |
| Eliminate per-candidate `used.to_vec()` | Fact-level join | Allocation count drops; `bench-join` `ns/op` drops |

Each is independently measurable: an optimization in one layer should move its
own metric while leaving the others' counters unchanged — confirming the
attribution is correct.

## 9. Implementation Order

1. **Instrumentation first** (§4) — `PerfStats` on `Engine`. No behavior
   change; unlocks every subsequent measurement. ~30 lines.
2. **Workload generator** (§3.2) — `gen-workload.rs` bin. ~80 lines.
3. **Microbenchmark bins** (§5.2) — one per cost center. ~40 lines each.
4. **Baseline capture** (§7.1) — `scripts/perf-baseline.sh` + commit
   `docs/PERF-BASELINE.md`.
5. **Then** optimize, measuring against the baseline at each step.

The instrumentation and harness are the foundation: once in place, every
optimization in the research docs becomes a measurable, provable change
rather than a guess.