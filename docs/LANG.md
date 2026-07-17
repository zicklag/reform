# Reform Language Reference

Reform is a reflective rule engine for interactive fiction and dynamic systems. Facts are ground tuples, rules rewrite them, and rules can generate other rules at runtime.

---

## 1. Facts

A fact is a parenthesized tuple of strings. The first element is the **predicate name**; the rest are **arguments**.

```
(john, awesome, 100)
(print, hello world)
(rule, reverse_rel, ((?x, north, of, ?y)), ((?y, south, of, ?x)))
```

Facts are asserted into the engine's fact base. The engine deduplicates — asserting the same fact twice is a no-op.

### 1.1 Sentence Facts

Lines that don't parse as tuples, commands, or prompts are parsed as `sentence` facts. The first word becomes the predicate `"sentence"`; subsequent words become arguments.

```
This is a sentence
```
→ `(sentence, This, is, a, sentence)`

Indented continuation lines are joined to the preceding sentence. Blank lines or comments reset the indentation context.

```
say (hello
  world
  foo)
```
→ `(sentence, say, hello\n  world\n  foo)`

### 1.2 Prompt Facts

Lines starting with `>` are parsed as `prompt` facts.

```
> go north
```
→ `(prompt, go, north)`

In prompt mode (`-p` flag), every input line is automatically prefixed with `> `, turning it into a `(prompt, ...)` fact.

### 1.3 Comments

Lines starting with `#` or `//` are comments and ignored.

```
# This is a comment
// This is also a comment
```

---

## 2. Patterns

Patterns match against facts and bind variables. They appear in rule match conditions and effect templates.

### 2.1 Pattern Syntax

A pattern is a parenthesized tuple where each element is one of:

| Syntax | Kind | Matches |
|--------|------|---------|
| `literal` | Atom | Exactly the string `literal` |
| `?name` | Variable | Any single element; binds to it |
| `..?name` | Rest/Splat | Zero or more remaining elements; binds to `Vec<String>` |
| `[?name]` | Optional variable | Zero or one element; binds if present, skips if absent |
| `[literal]` | Optional literal | Zero or one element matching `literal`; skips if absent |

### 2.2 Variable Binding

- `?name` binds exactly one fact element. The same variable appearing multiple times must bind consistently — the same value must appear at each position.
- `..?name` binds zero or more elements (shortest-match semantics). Multiple rest variables in one pattern are allowed; the first gets the shortest match, the rest get the remainder.
- `[?name]` binds if the element is present. If absent, the variable has an **empty binding** and acts as a wildcard in subsequent patterns (matches any single element without adding a binding).
- `[literal]` matches if the element equals `literal`; skips if absent.

### 2.3 Pattern Examples

```
# Match exactly (pred, a, b, c)
(pred, a, b, c)

# Match any 3-element tuple starting with pred
(pred, a, ?x, ?y)

# Match (pred, a, ...) with any number of trailing elements
(pred, a, ..?rest)

# Match (pred, a, ..., z) with any elements in between
(pred, a, ..?rest, z)

# Match with optional elements
(pred, a, [?x], b, [?y])

# Two rest variables: first gets shortest match
(pred, ..?a, ..?b)
# (pred, x, y, z) → a=[], b=[x, y, z]

# Rest with separator
(pred, ..?a, is, ..?b)
# (pred, big, red, ball, is, fun, toy) → a=[big, red, ball], b=[fun, toy]

# Same rest variable in two positions: must bind consistently
(pred, ..?a, sep, ..?a)
# (pred, x, y, sep, x, y) → a=[x, y]
# (pred, x, y, sep, p, q) → no match
```

---

## 3. Rules

Rules are facts with predicate `"rule"` that the engine interprets as rewrite rules.

### 3.1 Rule Syntax

```
(rule, name, (match1, match2, ...), (effect1, effect2, ...))
```

- **name**: A string identifying the rule (for debugging).
- **match patterns**: Comma-separated list of patterns. Each pattern is a parenthesized tuple.
- **effect patterns**: Comma-separated list of patterns. Each pattern is a parenthesized tuple.

### 3.2 Match Prefixes

Each match pattern can have a prefix:

| Prefix | Meaning |
|--------|---------|
| _(none)_ | Fact must exist; kept after firing |
| `-` | Fact must exist; **consumed** (removed) on firing |
| `!` | Fact must **not** exist (negation) |

### 3.3 Rule Semantics

A rule fires when:
1. All positive match patterns match some fact in the fact base.
2. All negation patterns (`!`) do NOT match any fact with consistent bindings.
3. The rule would actually change something (consumed facts exist OR new effect facts are not already present).

On firing:
1. Consumed facts (`-` prefix) are retracted.
2. Effect facts are asserted (printed immediately if `(print, ...)`, stored otherwise).

The engine runs a **fixed-point loop**: rules fire repeatedly until no more rules match. Rules are rebuilt from `(rule, ...)` facts at the start of each iteration, so rules can generate other rules.

### 3.4 Rule Examples

```
# Simple: consume a go fact, move the player
(rule, go_dir,
    ( -(go, ?dir), -(here, is, ?here), (?going, ?dir, of, ?here) ),
    ( (here, is, ?going), (print, (You are in the ), ?going, .) )
)

# Negation: can't go that way
(rule, go_fail,
    ( -(go, ?dir), (here, is, ?here), !(?going, ?dir, of, ?here) ),
    ( (print, (You can't go that way.)) )
)

# Transitivity: if X is Y and Y is Z, then X is Z
(rule, is_transitivity,
    ( (?x, is, ?y), (?y, is, ?z) ),
    ( (?x, is, ?z) )
)
```

---

## 4. Substitution (Effects)

When a rule fires, effect patterns are **substituted** with variable bindings to produce ground facts.

### 4.1 Variable Substitution

| Pattern in effect | Binding | Result |
|-------------------|---------|--------|
| `?name` | single element | Emits that element |
| `?name` | multiple elements | Joins with `", "` into one string |
| `?name` | empty | Emits `?name` literal (unbound) |
| `..?name` | multiple elements | Splats each element as a separate fact argument |
| `..?name` | single element that is a valid tuple `(...)` | Parses the tuple and splats its elements |
| `..?name` | single element that is NOT a valid tuple | Emits the element as-is |
| `..?name` | empty | Emits nothing |
| `[?name]` | bound | Emits the value (no brackets) |
| `[?name]` | unbound | Emits nothing |
| `[literal]` | always | Emits `literal` (no brackets) |

### 4.2 Split/Join Round-Trip

The split and join behaviors are inverses:

```
(pred, a, b, c)  →  ..?args binds ["a", "b", "c"]
?args in output  →  "a, b, c"  (join)
..?args in output  →  (pred, a, b, c)  (split from tuple string)
```

This enables **inline eval**: bind a tuple string with `?expr`, then splat it with `..?expr` to expand it into separate fact elements.

```
(rule, eval,
    ( (sentence, eval, ?arg) ),
    ( (..?arg) )
)
# Input: eval (John, is, cool)
# Output: (John, is, cool)
```

### 4.3 Atom String Substitution

When an effect pattern is an Atom containing `?var` or `..?var` inside a string (for rule-in-rule effects), string replacement is used:

- `..?var` is replaced with the joined values (comma-separated, with parenthesized wrapping for values containing spaces/commas/parens).
- `?var` is replaced with the single bound value.
- `[?var]` is replaced with the value if bound, kept as `[?var]` if unbound.
- `[literal]` has brackets removed.

```
(rule, reverse_rel,
    ( (?thing, is, ?rel1, [?prep1], ?other) ),
    ( (rule, reverse_rel,
        ( (?thing, is, ?rel2, [?prep2], ?other) ),
        ( (?other, is, ?rel1, [?prep1], ?thing) )
    ) )
)
```

---

## 5. Built-in Predicates

### 5.1 `(print, ...)`

When a fact with predicate `"print"` and at least 2 elements is asserted, the engine immediately prints the concatenation of all arguments to stdout. The fact is NOT stored in the fact base.

```
(print, Hello, world!)
# Prints: Hello, world!
```

### 5.2 `(rule, ...)`

The rule-definition predicate. See [Rules](#3-rules) above.

### 5.3 `(sentence, ...)`

Implicit fact from non-tuple input lines. See [Sentence Facts](#11-sentence-facts).

### 5.4 `(prompt, ...)`

Fact from `>`-prefixed input lines. See [Prompt Facts](#12-prompt-facts).
---

## 6. Commands

Commands are prefixed with `$` and are processed by the REPL, not the engine.

### 6.1 `$ assert (fact)`

Panics (exits with code 1) if the given fact does NOT exist in the fact base.

```
$ assert (john, awesome, 100)
```

### 6.2 `$ assert not (fact)`

Panics (exits with code 1) if the given fact DOES exist.

```
$ assert not (john, lame, 0)
```

### 6.3 `$ load <path>`

Loads and executes a `.rf` script file. Relative paths are resolved against the directory of the currently executing file.

```
$ load ./iflib/iflib.rf
```

### 6.4 `$ find <pattern>`

Finds and prints all facts matching the given pattern(s). Supports `!` prefix for negation. Uses the same matching logic as rule execution.

```
$ find (?x, is, ?y)
$ find (?x, is, ?y), !(?x, is, thing)
```

### 6.5 `$ facts`

Prints all current facts in the fact base.

```
$ facts
```

### 6.6 `$ quit`

Exits the REPL.

### 6.7 Fact Deletion

A fact prefixed with `-` retracts it from the fact base.

```
-(john, awesome, 100)
```

---

## 7. Engine

### 7.1 Fixed-Point Loop

The engine runs a fixed-point loop:

1. **Rebuild rules** from all `(rule, ...)` facts in the fact base.
2. **Collect matches** — for each rule, find all combinations of facts that satisfy its patterns.
3. **Apply** — for each match, retract consumed facts and assert effect facts.
4. **Repeat** until no rule fires.

Rules are rebuilt every iteration, so rules that generate other rules take effect immediately.

### 7.2 Checkpoints

The engine supports save/restore checkpoints for incremental editing:

- `save_checkpoint()` — saves all facts.
- `restore_checkpoint()` — restores to the last checkpoint.
- `restore_to(index)` — restores to a specific checkpoint.

Rules are NOT saved in checkpoints — they are rebuilt from `(rule, ...)` facts on the next `run_fixedpoint()`.

### 7.3 Auto-Run

After asserting or retracting a fact (including during script loading), `run_fixedpoint()` is automatically called. Other commands (`$ assert`, `$ find`, `$ facts`) do NOT trigger auto-run.

---

## 8. Command-Line Interface

```
reform-engine [file1 file2 ...]
```

### Options

| Flag | Description |
|------|-------------|
| `-p` | Prompt mode: prepend `> ` to all input lines, turning them into `(prompt, ...)` facts |
| `-v`, `--verbose` | Show auto-run firing counts |
| `-A`, `--allow-commands` | In prompt mode, allow `$`, `(`, and `-` lines as commands instead of prompts |

### Examples

```
# Load a game and enter interactive mode
reform-engine demo/game1.rf

# Run in prompt mode (for external prompt sources)
reform-engine -p demo/game1.rf

# Load a game with verbose output
reform-engine -v demo/game1.rf
```

---

## 9. Complete Example

### `demo/test.rf`
```
$ load ./iflib/iflib.rf

(rule, eval,
    (
        (sentence, eval, ?arg),
    ),
    (
        (..?arg),
    )
)

eval (John, is, cool)

$ facts
```

### `demo/game1.rf`
```
$ load ./iflib/iflib.rf

# Core world modeling
bedroom is a room
The player is a person

# Direction relationships
north of is the reverse of south of
east of is the reverse of west of

# Game world
The Kitchen is a room
The Kitchen is north of the (Master Bedroom)
The description of the kitchen is (The place where things are cooked.)

The (tea kettle) is a thing
The (tea kettle) is in the Kitchen
The description of the (tea kettle) is (A cute little kettle for tea.)
The print-name of the (tea kettle) is (little kettle)

(My Room) is east of the Kitchen
(My Room) is containing the player

# Start the game with a message
say (
    You wake up.
    Who you are, and what you are doing here is a mystery...
)

# Game actions
looking is an action
understand look as looking
understand l as looking

When looking
    say (You look around tentatively.)

moosing is an action
understand moose as moosing

when moosing
    now the player is a moose and
    say (Now you are a moose 🦛 👀)

unmoosing is an action
understand unmoose as unmoosing

When unmoosing
    say (You are not a moose anymore.) and
    now the player is not a moose
```

---

## 10. Pattern Matching Algorithm

### 10.1 Shortest-Match Semantics

Rest variables (`..?name`) use shortest-match semantics: the engine tries consuming 0 elements first, then 1, 2, etc., backtracking if the remainder of the pattern fails.

```
(pred, ..?a, ..?b) matching (pred, x, y, z):
  Try a=[], b=[x, y, z] → match
  (Never tries a=[x], b=[y, z] because first attempt succeeds)
```

### 10.2 Optional Variable Semantics

Optional variables (`[?name]`) try skipping first (shortest match). If skipping leads to a pattern failure, the engine backtracks and tries binding.

```
(sentence, [?a1], ?thing, is, ?rel, [?prep], [?a2], ?other)
matching (sentence, The, cow, is, over, the, moon):
  ?prep tries skip first → succeeds (no preposition needed)
  ?a1 binds to "The", ?a2 binds to "the"
```

### 10.3 Backtracking

The engine backtracks on:
- Variable binding conflicts (same variable, different values)
- Rest variable length choices
- Optional variable skip/bind choices

### 10.4 Outer Bindings

When a rule has multiple match patterns, bindings from earlier patterns are carried as **outer bindings** to later patterns. Variables with empty bindings (from skipped optionals) act as wildcards in later patterns — they match any single element without adding a binding.

---

## 11. Library System

### 11.1 File Loading

Files are loaded with `$ load <path>`. Relative paths are resolved against the directory of the currently executing file. The library system is file-based, not module-based — all facts and rules share a global namespace.

### 11.2 Standard Library

The IF library is at `iflib/iflib.rf` and provides:

- **Article definitions**: `a`, `an`, `the`, `A`, `An`, `The` are articles
- **Preposition definitions**: `of`, `to`, `from`, `in` are prepositions
- **`is` transitivity**: if X is Y and Y is Z, then X is Z
- **Property assignment**: `[The] ?prop of [the] ?thing is ?value` → `(?value, is, ?prop, of, ?thing)`
- **Relation parsing**: `[The] ?thing is ?rel [of] [a] ?other` → `(?thing, is, ?rel, [?prep], ?other)`
- **`is not` elimination**: `?thing is not ?other` retracts `?thing is ?other`
- **Reverse relations**: `?rel1 of is the reverse of ?rel2 of` generates bidirectional rules
- **Say command**: `say ...` → `(print, ...)`
- **Action definition**: `?act is an action` → `(?act, is, action)`
- **Understand command**: `understand ?word as ?act` generates a rule mapping prompts to actions
- **When command**: `When ?act ...` generates rules that fire on action enactment
