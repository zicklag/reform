# Reform Engine Language Specification

A minimal reflective rule engine where everything is a fact, rules are facts, and the engine is a single fixed-point loop.

## 1. Core Concepts

### Facts

A fact is a ground atomic sentence: a predicate name followed by zero or more arguments.

```
room(kitchen)
at(apple, kitchen)
here(frontroom)
n
```

Internally: `Vec<String>` where `fact[0]` is the predicate.

### Rules

A rule is a conditional rewrite over the fact base:

```
rule:name:match_patterns:effect_patterns:consume_indices
```

- **name** — identifier for debugging
- **match_patterns** — comma-separated patterns (all must match for the rule to fire)
- **effect_patterns** — comma-separated patterns for new facts to assert
- **consume_indices** — optional, comma-separated indices of matched facts to delete

Example:

```
rule:take:at(?thing,?room),location(player,?room),hands_free(player):carrying(player,?thing):0,2
```

This matches three patterns, consumes patterns 0 and 2 (`at` and `hands_free`), and produces `carrying`.

### Patterns

A pattern is a predicate with arguments. Arguments can be:

- **Atoms** — literal strings that must match exactly: `kitchen`, `player`
- **Variables** — prefixed with `?`, match anything and bind the value: `?thing`, `?room`
- **Variable predicates** — the predicate itself can be a variable: `?rel(?x, ?y)` matches any binary relation

Negation patterns (prefixed with `!`) must NOT match for the rule to fire:

```
rule:infer_domain:relation_sig(?rel,?domain,?range),?rel(?x,?y),!instance(?x,?domain):instance(?x,?domain)
```

This fires only if `instance(?x, ?domain)` does NOT already exist.

### The Fixed-Point Loop

The engine runs a loop:

1. Rebuild executable rules from any `rule(...)` facts in the fact base
2. Find all rules whose patterns match the current fact set
3. For each match: consume the specified facts, assert the effect facts
4. Repeat until no rule produces a change

This is the **only** execution model. There is no separate compile phase.

## 2. Syntax

### Fact syntax

```
predicate
predicate(arg1, arg2, ...)
```

Examples:

```
room(kitchen)
at(apple, kitchen)
n
type(Person)
relation_sig(at, Thing, Room)
```

### Rule syntax

```
rule:name:match1,match2,...:effect1,effect2,...:consume_idx1,consume_idx2
```

The last `:consume_indices` segment is optional. If omitted, nothing is consumed.

Examples:

```
# Single match, single effect, consume match 0
rule:take:at(?thing,?room):carrying(player,?thing):0

# Multiple matches, single effect, consume matches 0 and 2
rule:take:at(?thing,?room),location(player,?room),hands_free(player):carrying(player,?thing):0,2

# Multiple matches, multiple effects, no consumption
rule:infer_domain:relation_sig(?rel,?domain,?range),?rel(?x,?y),!instance(?x,?domain):instance(?x,?domain)

# Single match, single effect, consume match 0
rule:go_north:n,here(?h),north_of(?g,?h):here(?g):0,1
```

### Negation syntax

Prefix a match pattern with `!` to require it NOT to match:

```
rule:infer_domain:relation_sig(?rel,?domain,?range),?rel(?x,?y),!instance(?x,?domain):instance(?x,?domain)
```

### Comments

Lines starting with `#` or `//` are ignored in script files.

## 3. Self-Reflection

### Rules are facts

A rule can be represented as a fact:

```
rule(name, match_pattern, effect_pattern, ...)
```

The engine automatically converts `rule(...)` facts to executable rules at the start of each fixed-point iteration. This enables self-modification:

```
# Learn: create a new rule from a fact
rule:learn:learn(?name,?pat,?eff):rule(?name,?pat,?eff):0

# Override: replace an existing rule
rule:override:override(?name,?new_pat,?new_eff),rule(?name,?old_pat,?old_eff):rule(?name,?new_pat,?new_eff):0,1

# Disable: remove a rule
rule:disable:disable(?name),rule(?name,?pat,?eff):disabled(?name):0,1
```

### All rules are fact-derived

Every rule — whether added via the `rule:` command or created programmatically — is stored as a `rule(...)` fact in the fact base. The engine rebuilds all executable rules from `rule(...)` facts at the start of each fixed-point iteration. If a `rule(...)` fact is consumed, the rule disappears.

This means self-modification works uniformly on all rules:

```
# Learn: create a new rule from a fact
rule:learn:learn(?name,?pat,?eff):rule(?name,?pat,?eff):0

# Override: replace an existing rule
rule:override:override(?name,?new_pat,?new_eff),rule(?name,?old_pat,?old_eff):rule(?name,?new_pat,?new_eff):0,1

# Disable: remove a rule
rule:disable:disable(?name),rule(?name,?pat,?eff):disabled(?name):0,1
```
## 4. Script Commands

Script files and the REPL support these commands:

| Command | Effect |
|---|---|
| `pred(arg1, ...)` | Assert a fact |
| `rule:name:pat:eff:del` | Add a rule (asserts a `rule(...)` fact) |
| `run` | Run the fixed-point loop to completion |
| `step` | Run one iteration of the fixed-point loop |
| `facts` | Print all current facts |
| `rules` | Print all current rules |
| `assert pred(arg1, ...)` | Crash if the fact does not exist |
| `assert not pred(arg1, ...)` | Crash if the fact exists |
| `checkpoint` | Save a state snapshot |
| `restore` | Restore to the last checkpoint |
| `load <file>` | Load and execute a script file |
| `quit` | Exit |
## 5. Execution Model

### Fixed-point semantics

Starting from the current fact set, the engine repeatedly:

1. Converts `rule(...)` facts to executable rules
2. Finds all rules whose patterns match (including negation checks)
3. For each matching rule:
   - Checks if the rule would actually change the state (consumes existing facts or produces new ones)
   - If so: consumes the specified matched facts, asserts the effect facts
4. Repeats until no rule produces a change

### Consumption

Consumption is destructive — a consumed fact is removed from the fact base. This models state change: `at(apple, kitchen)` is consumed, `carrying(player, apple)` is produced.

Rules that don't consume their matched facts will fire every iteration (unless guarded by `!not`). This is useful for inference rules that should fire once per fact.

### Ordering

All matching rules fire in a single iteration. There is no guaranteed order. "Rulebook" behavior (before/instead/check/carry out) is achieved through consumption patterns: a "before" rule consumes facts that a "carry out" rule needs, effectively blocking it.

## 6. Type System (as Rules)

The type system is not built in — it's defined as rules:

```
# Declare types
type(Person)
type(Thing)

# Declare relation signatures
relation_sig(at, Thing, Room)
relation_sig(location, Person, Room)

# Type inference rules
rule:infer_domain:relation_sig(?rel,?domain,?range),?rel(?x,?y),!instance(?x,?domain):instance(?x,?domain)
rule:infer_range:relation_sig(?rel,?domain,?range),?rel(?x,?y),!instance(?y,?range):instance(?y,?range)

# Subtype propagation
rule:infer_subtype:instance(?x,?type),subtype(?type,?super),!instance(?x,?super):instance(?x,?super)
```

When `at(apple, kitchen)` is asserted, the inference rules automatically derive `instance(apple, Thing)` and `instance(kitchen, Room)`.

## 7. Checkpoints

Checkpoints save the full engine state (facts). They enable LSP-style incremental editing:

```
checkpoint    # save state
...           # make changes
restore       # undo to last checkpoint
```

## 8. CLI Usage

```
reform-engine [file1 file2 ...]
```

Loads the specified files in order, then enters an interactive REPL.

## 9. Examples

### Minimal game

```
room(kitchen)
room(bedroom)
north_of(kitchen, bedroom)
here(frontroom)

rule:go_north:n,here(?h),north_of(?g,?h):here(?g):0,1

n
run
facts
```

Expected output: `here(bedroom)` — the player moved north.

### Type inference

```
type(Person)
type(Thing)
type(Room)
relation_sig(at, Thing, Room)
relation_sig(location, Person, Room)

rule:infer_domain:relation_sig(?rel,?domain,?range),?rel(?x,?y),!instance(?x,?domain):instance(?x,?domain)
rule:infer_range:relation_sig(?rel,?domain,?range),?rel(?x,?y),!instance(?y,?range):instance(?y,?range)

at(apple, kitchen)
run
facts
```

Expected: `instance(apple, Thing)` and `instance(kitchen, Room)` are inferred.

### Self-modification

```
rule:learn:learn(?name,?pat,?eff):rule(?name,?pat,?eff):0
learn(my_rule, at(?x,?y), found(?x,?y))
run
at(hello, world)
run
facts
```

Expected: `found(hello, world)` — the learned rule fired.

## 10. Current Limitations

- No string concatenation in effects
- No arithmetic
- No ordered rule application (all matching rules fire in one iteration)
- No persistent storage
- Naive O(R × F^k) matching (no Rete network yet)
- Single-threaded
