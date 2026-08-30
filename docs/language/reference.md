# Language Reference

> **Note:** This is AI generated from the source code. It will be reviewed for
correctness later, but should serve as a good reference while the rest of the
docs are still work-in-progress.

This is a complete, reference guide for the Reform language and its reference
implementation. It describes every syntactic form, every matching and execution
behavior, and every built-in fact the CLI understands.

All data is held in **facts** (lists of string arguments). **Rules** watch the
fact store and, when their **pattern** matches existing facts, fire to
**delete** matched facts and **create** new ones from their **body**. The engine
repeatedly fires rules until no rule changes anything (a fixpoint). There is no
built-in control flow or I/O other than the small set of CLI facts listed below
— everything else is built from facts and rules.

- [Facts](#facts)
- [Fact Syntax](#fact-syntax)
- [Templates](#templates)
- [Fenced Blocks](#fenced-blocks)
- [File Loading and Fact Prefixes](#file-loading-and-fact-prefixes)
- [Rules](#rules)
- [Patterns](#patterns)
- [Bodies](#bodies)
- [Matching Semantics](#matching-semantics)
- [Rule Priority (Specificity)](#rule-priority-specificity)
- [Execution Model](#execution-model)
- [Rule Validation](#rule-validation)
- [CLI](#cli)
- [CLI Built-in Facts](#cli-built-in-facts)
- [Arithmetic: `@eval`](#arithmetic-eval)
- [Normal Form](#normal-form)
- [Embedding](#embedding)

---

## Facts

A **fact** is an ordered list of **arguments**, each of which is a string. For
example, `$ Alice is happy.` creates a fact with four arguments: `Alice`,
`is`, `happy`, `.`.

Facts have no intrinsic meaning. A program's *conventions* define what a fact
means — usually via rules. Two different programs can interpret the same fact
differently. This is what makes Reform a substrate for custom, natural-looking
languages.

Every argument is stored as an interned string; arguments are compared by
value. An argument may be the empty string (written `()`). A fact with zero
arguments exists only through the embedding API — no source syntax produces
one. Facts are unique in the engine — adding a fact that is already present is
a no-op.

---

## Fact Syntax

### Line prefixes

When loading from a file, a line may start with one prefix character. Prefixes
do **not** compose — a line starts with at most one of `$`, `>`, or nothing.

| Prefix | Meaning                                            |
|--------|----------------------------------------------------|
| (none) | The fact gets a leading `parse` argument.          |
| `$`    | Raw fact / rule / command. No prefix added.        |
| `>`    | The fact gets a leading `prompt` argument.          |

So `$ prompt Hello World` and `> Hello World` create the identical fact.
`Hello World` (no prefix) creates `parse Hello World`.

### Arguments and whitespace

Arguments are separated by spaces (any number). A tab is an ordinary word
character — it neither separates arguments nor counts toward indentation.

```rf
$ This   is   a   sentence
```

### Comments

A `#` starts a comment, which runs to the end of the line. Full-line comments
and trailing comments are both ignored.

```rf
# a full-line comment
$ hello world # a trailing comment
```

A `#` inside a parenthesized argument or a template is literal, not a comment.

### Punctuation splitting

Punctuation — `,` `;` `.` `'` `:` — that is followed by whitespace or the end
of the line is split into its own argument.

```rf
$ This is a sentence.
```

produces the arguments `This`, `is`, `a`, `sentence`, `.`. Punctuation
embedded in a word (not followed by whitespace / end of line) stays in the
word: `example.com` is a single argument.

A word that *ends* in punctuation that must stay attached must be wrapped in
parenthesis:

```rf
$ (www.) is a common web domain prefix
```

### Grouping with parentheses

Wrapping content in parentheses makes it a single argument, including spaces:

```rf
$ the full name of Alice is (Alice Von Schmidt)
```

The last argument is `Alice Von Schmidt`.

Inside a parenthesized argument:

- Parentheses may be **nested** if balanced: `(He was pleased (not that he'd admit it).)` keeps the inner parens.
- A literal `(` or `)` is escaped with a backslash: `\(` and `\)`. A smiley `:)` in a string is written `:\)`.
- A literal backslash is escaped as `\\`.
- To include parens as the *value* (e.g. an argument literally `(example)`), use double parentheses: `((example))`.

### Multi-line facts

A fact may continue on subsequent lines if those lines are indented **more**
than the line that started the fact. Indentation is counted in spaces.

```rf
$ Bob
  is smiling.
```

Continuation indentation is relative: each continuation line must be more
indented than the *fact's first line*. Comment-only continuation lines and
blank continuation lines add no arguments.

Indentation counting is suspended while inside a parenthesized argument — all
whitespace inside the parens is taken literally.

A blank or comment-only line adds no arguments and does not by itself end the
fact: a later line indented more than the fact's first line still continues
it. A line at the same or lesser indentation starts a new fact.

---

## Templates

A **template argument** is a multi-line string wrapped in backticks. It is
syntax sugar for mixing literal text with substitution sections and other
arguments.

```rf
$ The description is `There is a gate before you.

It is {if open}open{else}closed{end if}.`
```

A template parses into a sequence of arguments. The opening and closing
backticks each become their own `` ` `` argument, marking the template's
extent. Between them, literal text runs are joined into single arguments,
while `{ ... }` curly-brace sections are split into their own arguments with
normal word splitting.

```rf
(`) (There is a gate before you.

It is ) { if open } open { else } closed { end if } (.) (`)
```

Inside a single-backtick template:

- Literal curly braces are escaped as `\{` and `\}`. Unescaped braces are always substitution delimiters.
- A literal backtick is written `` \` ``.
- A literal backslash is written `\\`.
- Normal word splitting applies *between* backticks and inside `{ ... }`
  sections; literal text runs (including newlines) are preserved verbatim as
  single arguments.

---

## Fenced Blocks

A triple backtick (`` ``` ``) opens a **fenced block**: a multi-line template
convenient for text indented under a fact.

```rf
Before starting first-time-look:
    say ```
        "Kion, wake up."

        Kion stirs, and opens his eyes slowly, "Hmm, what?"
        ```
```

The interior is **dedented to the column of the opening fence** — the leading
whitespace in front of `` ``` `` is stripped from every interior line, so
content indented under the fence comes out flush-left. The leading newline
(right after the opening fence) and the trailing newline (right before the
closing fence) are ignored.

A fenced block expands to the same `` ` `` marker arguments plus interior
chunks as a single-backtick template, so its literal text becomes one
continuous argument (with the paragraph text).

The closing fence is a line consisting only of optional horizontal whitespace
followed by `` ``` ``. Content after the closing fence on the same line is
parsed as regular arguments following the template.

Inside a fenced block:

- Backticks are **literal** — the block is only closed by a dedicated `` ``` ``
  line, not by a single backtick.
- `\{` and `\}` are escapes for literal braces.
- `\\` is a literal backslash.
- `` \``` `` (escaped triple backtick) produces a literal `` ``` `` in the
  content and does not close the fence.
- A `{ ... }` section splits the interior just as in a single-backtick
  template.

---

## File Loading and Fact Prefixes

Facts are loaded from a file by parsing each fact, then applying the line
prefix to decide what to store:

- **No prefix** → the fact is stored with a leading `parse` argument:
  `This is a sentence.` becomes `parse This is a sentence .`.
- **`$`** → the fact is stored as-is, with no prefix. Rules and commands must
  use `$` so they are not treated as parse facts.
- **`>`** → the fact is stored with a leading `prompt` argument. This is how
  player input and file-based test prompts are represented.

The `parse` prefix lets rules post-process plain-looking sentences: a rule can
match `parse ...` and transform it into domain facts. The `prompt` prefix
distinguishes player/user input from other facts.

Prefixes do not compose: `$>` is not meaningful; a line starts with at most one
prefix.

---

## Rules

A **rule** is a special fact whose first argument is exactly `rule`. Rules are
registered with the engine and fired automatically when their pattern matches.
Rules must be written with the `$` prefix (so they aren't stored as `parse`
facts).

A rule fact has **4 or 5 arguments**:

1. `rule` — always exactly this.
2. **name** — any string (usually parenthesized to allow spaces). Names are
   not namespaced; unique, descriptive names help with debugging and removal.
3. **pattern** — matches against facts currently in the engine.
4. **body** — a template producing the facts to create when the pattern matches.
5. **specificity adjustment** (optional) — see below.

```rf
$ rule (say hello when user says hi)
  (
    - prompt hi
  )
  (
    println Hello!
  )
```

The pattern and body are almost always wrapped in parentheses because they
contain facts themselves.

When the pattern matches:

1. Every fact matched by a pattern item prefixed with `-` is **deleted** —
   including `-` inside a fact-level repetition, which deletes exactly the
   facts that repetition consumed (see [Removal and negation
   prefixes](#removal-and-negation-prefixes)).
2. The body is rendered (substituting bindings) and its resulting facts are
   **created**.

### Specificity adjustment (5th argument)

The optional fifth argument is an integer prefixed with `+`, `-`, or `=`:

- `+N` adds N to the rule's computed specificity.
- `-N` subtracts N.
- `=N` sets the specificity to exactly N, ignoring the computed value.

Higher specificity fires first. The adjustment is how you override the default
ordering without changing the pattern. The argument must be a non-empty signed
or `=`-prefixed integer.

---

## Patterns

A **pattern** is a sequence of **pattern items**, one per line (a line's worth
of facts, which may span indented continuation lines). Each item is either a
**pattern fact** or a **repeated block of pattern facts**. The whole pattern is
usually one parenthesized argument with one fact per line.

### Literals

A bare word matches that exact argument.

```rf
( prompt look )
```

### Placeholders

A `$name` placeholder matches any single argument. A placeholder used more than
once — within the same fact or across facts — must bind to the **same value**
in every occurrence.

```rf
$ rule (say hello to user by name)
  (
    - prompt my name is $name
  )
  (
    println (Hello ) $name !
  )
```

`$any` is not a keyword; it is a conventional placeholder name equivalent to
`$x` or any other name.

### Arg repetitions

A `$( ... )` block around arguments repeats (or makes optional) those
arguments within a single fact. Three forms, closed by `)?`, `)+`, or `)*`:

| Form       | Meaning                        |
|------------|--------------------------------|
| `$( ... )?`| optional (zero or one)         |
| `$( ... )+`| one or more                    |
| `$( ... )*`| zero or more                   |

```rf
( - prompt $( $arg )+ )
```

Repetitions may be **nested**, and may wrap whole facts to match multiple facts
at once.

### Negative lookahead

A `$( ... )!` block is a **zero-width negative lookahead** (PEG-style). It
matches at the current position iff the inner args do **not** match starting
there. It binds nothing and consumes nothing — it only asserts the absence of
the inner arguments.

```rf
$ rule (look north)
  (
    - prompt look $( the door is locked )! north
  )
  (
    println You look north.
  )
```

The above rule fires only when `look ... north` does not have `the door is
locked` at the lookahead position (e.g. `look north` matches; `look the door
is locked north` does not). The lookahead's inner args are matched against a
detached copy of the current bindings, so a placeholder inside a lookahead
that is already bound as a **scalar** by the rest of the pattern acts as a
**constraint**: the lookahead succeeds only when no argument run matches that
bound value. A placeholder that appears *only* inside the lookahead (or is a
list-bound placeholder mid-collection) is a fresh local wildcard for the inner
match — nothing the lookahead matches leaks into or changes the rule's
bindings, so such a placeholder is never available to the body.

Because a lookahead is zero-width, the args it guards are still available to
the rest of the pattern, and because it only asserts absence it does not change
what the fact consumes. It is the arg-level analog of the `!` fact prefix,
useful for rejecting a phrase inside a single fact rather than a whole fact.

Like the repetition blocks, a lookahead's interior must contain at least one
argument — an empty `$( )!` is a parse error (a zero-width lookahead over the
always-matching empty sequence would always fail, so it is not expressible).

### Fact repetitions

A `$( ... )?/+/*` block at a fact's base indentation (a sibling item, one per
line) repeats whole **facts**. `$( ... )*` collects every matching fact,
`$( ... )+` requires at least one, and `$( ... )?` is an optional fact
constraint.

```rf
$ rule example2
  (
    # collect all "player is carrying" facts into a list
    $(
      player is carrying $item
    )*
    $(
      - all player items $( $any )*
    )?
  )
  (
    all player items $( $item )*
  )
```

### Removal and negation prefixes

Each pattern fact line may start with a prefix:

| Prefix | Meaning                                                                                  |
|--------|------------------------------------------------------------------------------------------|
| (none) | a fact to match; kept in the engine after firing                                          |
| `-`    | a fact to match **and delete** when the rule fires                                        |
| `!`    | a **negated** fact: matches when *no* fact in the engine matches it (with the current bindings); binds nothing and consumes nothing |

`-` and `!` do not combine into a single marker. The parser accepts `!` then a
literal `-word`, but not a combined `-!`.

`!` is only honored on pattern facts at the **top level** of the pattern.
Inside a fact-level repetition (`$( ... )?/+/*`), the `!` prefix is stripped
and the inner fact is matched as a plain (non-negated) fact.

`-` is honored inside fact-level repetitions as well as at the top level: it
deletes exactly the facts that repetition **consumed** (never facts a sibling
item matched). A fact-level optional `$( - foo )?` consumes and deletes `foo`
when present (fact-level repetitions are always greedy; see [Lazy vs. greedy
repetition](#lazy-vs-greedy-repetition)).

### Greedy repetitions

Within a single fact's arguments, `?`, `+`, and `*` repetitions are **lazy** by
default (see [Matching Semantics](#matching-semantics)). Doubling the marker
makes an arg-level repetition **greedy**: `??`, `++`, `**` prefer *more*
iterations. Fact-level repetitions (`$( ... )?/+/*` on their own lines) are
**always greedy** — the greedy variants `??`/`++`/`**` behave identically to
their single-marked forms there.

---

## Bodies

A **body** is a substitution template. When a rule's pattern matches, the body
is rendered by substituting the pattern's bindings, and the resulting text is
parsed into facts that are created.

A body is composed of:

- **Literal text** — emitted verbatim. This includes parentheses, newlines,
  and the entire contents of generated (inner) rules.
- **`$name` placeholders** — substituted with the matched value. At a bare
  argument position the value renders in normal form (wrapped in parens when
  needed, space-joined if it is a list); inside template parens it is spliced
  in escaped but unwrapped (see below).
- **`$( ... )?/+/*` repetition blocks** — iterated over the bound lists, one
  emission per list element. A block whose placeholders are bound at the same
  nesting depth in the pattern is driven by those lists. If the driver lists
  have inconsistent lengths, the block renders nothing.

Two special escapes:

- `$$` produces a literal `$` in the output. This is how a generated *inner*
  rule writes its own `$x` placeholders or `$( ... )` blocks: `$$x` and
  `$$( ... )` — the outer pattern binds the values, and the emitted rule gets
  its own placeholders.
- A bare `$` not followed by a placeholder name is literal text.

A rule whose body renders to empty output creates nothing.

### Substitution inside parentheses

Parentheses written in a body template group the rendered text into a single
argument, and a placeholder spliced **inside** them is escaped but never
wrapped — your parens are the grouping. At a bare argument position, a
substituted value is wrapped in parens when needed so it stays one argument.

This is also how several captured arguments get merged into one: put a
repetition block inside parentheses. Nothing is added between the substituted
values — separation comes only from the template text, so a captured `(Hello )`
keeps its trailing space and joins the merged argument.

```rf
$ rule (merge args)
  (
    - parse $( $before )* start $( $args )* end $( $after )*
  )
  (
    out $( $before )* ($($args)*) $( $after )*
  )
```

Parsing `one two three start (Hello ) World end four five` produces
`out one two three (Hello World) four five`.

### Body/pattern placeholder alignment

Every placeholder used in a body must be **declared by the pattern**, and must
appear at the **same or deeper nesting** in the body than in the pattern. A
placeholder bound inside a repetition in the pattern must be iterated by a
matching `$( ... )` block in the body; a flat placeholder may be expanded
inside a repetition. Violating this is a validation error (see [Rule
Validation](#rule-validation)).

---

## Matching Semantics

### Multiple facts, one pattern

A multi-line pattern matches only when **all** of its facts exist. Each pattern
fact line matches a **distinct** fact in the engine (the same fact cannot
satisfy two lines). Placeholders shared across facts must bind to the same
value, which lets patterns join facts:

```rf
$ rule (parse the "look" command)
  (
    - prompt look
    player is in $room
    description of $room is $description
  )
  (
    println (You are in the ) $room .
    println $description
  )
```

This only fires when the player is in a room *and* that room has a
description, and `$room` must match in both facts.

### Placeholder binding

- A scalar placeholder (outside any repetition) binds to one argument and is
  consistency-checked across the whole match: `$x is $x` matches only a fact
  where both words are equal.
- A placeholder inside an arg repetition is **list-bound**: it collects a list
  of values, one per iteration, nested one level per enclosing repetition.

A scalar placeholder used again inside a fact-level repetition acts as a
**constraint**, not a collection: it must match the same value in every
iteration (like a literal) and keeps its scalar binding. For example, in
`$prop of car is red` followed by `$( $prop of $x is $old )*`, `$prop` stays
bound to `color` and only facts whose prop is `color` are matched — `$prop` is
not collected into a list.

### Lazy vs. greedy repetition

Within a single fact, `+` and `*` arg repetitions are **lazy** by default:
they match as few iterations as possible. When a fact admits several
full-consumption matches, they are enumerated lazy-first (the one peeling the
fewest arguments from the leftmost repetition first). `?` is also lazy by
default (zero iterations preferred, one as fallback). Doubled markers (`++`,
`**`, `??`) invert to greedy.

The **laziest binding that satisfies the entire pattern** fires. If the
greedier parse fails a later constraint (e.g. an `$( $a is article )?` whose
`$a` has no matching fact), matching backtracks to a lazier parse that does
satisfy it. For a `?` block, a list-bound placeholder with an empty list
"disables" the corresponding fact-level `?` constraint, and a non-empty list
makes it a constraint that only verifies a matching fact exists (without
consuming it).

### Fact repetitions and list collection

`$( ... )*` and `$( ... )+` fact-level repetitions collect all matching facts;
`$( ... )?` matches an optional fact. Fact-level repetitions are **always
greedy**: `*`/`+` consume every matching fact, and `?` consumes the fact when
present, falling back to matching zero facts only when consuming leaves the
rest of the pattern unsatisfiable. The greedy markers `**`/`++`/`??` have no
effect at the fact level — they behave identically to `*`/`+`/`?`. A repeated
block may contain multiple inner pattern facts: each iteration consumes one
group of facts (one fact per inner pattern fact, matched in order), and the
inner facts' placeholders collect one value per group.

---

## Rule Priority (Specificity)

When multiple rules match the same facts, the most specific rule fires first.
Specificity is **word-based** and computed automatically from the pattern.

Each word contributes:

| Element                     | Score |
|-----------------------------|-------|
| literal argument            | 5     |
| placeholder (`$x`)          | 4     |
| required (non-negated) fact | 1     |
| negated fact                | 0     |
| negative lookahead `$( )!`  | 0     |

Repetition blocks add nothing for the block itself, but **penalize** every word
inside them by the block's looseness. Penalties stack across nested blocks and
saturate at zero:

| Repetition | Penalty per enclosed word |
|------------|---------------------------|
| `?`        | 1                         |
| `+`        | 3                         |
| `*`        | 4                         |

For example, the catch-all `parse $( $word )+` scores `1 + 5 + (4-3) = 7`,
while the structured `parse $( $a1 )? $x is $( $a2 )? $y` scores
`1 + 5 + (4-1) + 4 + 5 + (4-1) + 4 = 25` — the structured rule wins. A pattern
with more required repetitions outranks one with fewer.

Rules are sorted by specificity descending, and **ties preserve insertion
order**. This is why a "didn't understand" catch-all rule fires only after a
more specific rule has handled (and, typically, removed) a prompt.

The optional 5th rule argument can add (`+N`), subtract (`-N`), or set (`=N`)
the computed specificity.

---

## Execution Model

### The turn loop

All computation happens by firing rules to a **fixpoint**. Each `turn()`:

1. Iterates rules from most to least specific.
2. For each rule, snapshots the current facts and finds every match.
3. A rule fires at most once per distinct matched-fact set per turn (tracked
   to prevent re-firing on identical facts).
4. On firing, it deletes the `-`-marked facts, renders the body, and creates
   the resulting facts. Any rules created by the body are registered but fire
   on a subsequent turn (the current turn iterates a snapshot of the rule
   list).
5. If any fact changed, the loop **restarts from the most-specific rule**, so
   more-specific rules get first dibs on changed facts.
6. When a full pass changes nothing, the engine has reached a fixpoint.

### Recursive firing

A rule may fire repeatedly within a single turn, including on facts produced
by its own firing (limited only by the per-matched-fact-set tracking in step
3). This is what makes recursive "peel one item per firing" rules work — e.g.
splitting one sentence off a `parse` fact and leaving a shorter `parse` fact
that re-triggers the same rule. A rule is *not* marked fired when its output
restarts the loop.

### Fixpoint bound

Infinite recursion is bounded by a per-turn iteration cap (default 100,000,
configurable in the embedding API). A non-terminating rule set bails with a
fixpoint error.

### Command execution order

Facts that name a registered command (such as `println` or `assert`) are not
stored as data; they are executed. When a command fact is encountered while
loading, the engine **settles rules first** (so `assert` sees the result of
rules that have run), then dispatches the command. Commands produced by rule
bodies execute immediately, mid-turn.

---

## Rule Validation

When a rule is parsed, structural invariants are checked and rejected with an
error:

- The rule fact must have 4 or 5 arguments.
- A placeholder used at **two different nesting depths** (different stacks of
  enclosing repetition kinds) — within the pattern, within the body, or within
  a single repeated arg list — is rejected, **except** in the pattern when one
  of the uses is at the top level (scalar). A top-level use makes the
  placeholder a **native scalar**, which may be used inside fact-level
  repetitions as a constraint; a list-bound placeholder (used at a non-empty
  nesting) at two different nesting depths is still genuinely ambiguous and
  rejected.
- Every body placeholder must be **declared by the pattern**.
- A body placeholder bound at one nesting in the pattern may be used at the
  **same or deeper** nesting in the body, never shallower.

An empty pattern and an empty body are both valid. A pattern consisting only of
negated or only of removal facts is valid.

---

## CLI

The `reform` executable is a REPL plus a file runner.

```
reform [--safe] [--trace] [--seed SEED] [--version] [files...]
```

- Positional **files** are loaded (via `load_file`) before the REPL starts. If
  a file sets `$ quit`, loading stops.
- **`--safe`** disallows `$`-prefixed lines as direct facts/commands at the
  REPL; they are treated as player prompts instead.
- **`--trace`** (or the `REFORM_TRACE` environment variable being set) prints
  trace events to stderr, indented to show causation: rules registered (with
  computed specificity), `file` lines when source loads, rule firings
  (`fire <name>`) with the firing's facts grouped beneath it — `✓` for
  pattern facts that matched and stayed, `-` for pattern facts the firing
  consumed, `+` for facts the body added, and `(via <command>)` suffixes on
  facts changed outside a firing.
- **`--seed SEED`** seeds the `random(n)` stream so `@eval` output is
  deterministic. Omit it to draw a fresh random seed from system entropy.
- **`--version`** prints the version and exits.

At the REPL:

- Lines are treated as prompts (wrapped as `prompt ...` facts).
- A `$`-prefixed line starts buffering a multi-line direct fact/rule; indented
  lines continue it, a blank line (or a non-indented line) submits it. This
  lets you enter multi-line rules at the REPL.
- Blank lines are ignored outside a buffer.
- The engine quits when a `$ quit` fact is processed.

---

## CLI Built-in Facts

These fact conventions are implemented by the reference CLI (and the engine's
default commands). They are triggered by creating facts with the `$` prefix (so
they aren't stored as `parse` facts). Each is a **command**: the fact is not
stored in the engine — it is executed.

### `print` and `println`

A fact whose first argument is `print` or `println` outputs all remaining
arguments.

- `println` concatenates its arguments with **no separator** and appends a
  newline.
- `print` concatenates its arguments with **no separator** and does not append
  a newline.

Because both concatenate without spaces, wrap a multi-word string in a
single parenthesized arg: `$ println (you see a cave)` prints `you see a cave`,
while `$ println you see a cave` prints `youseeacave`.

```rf
$ println (Hello) $name !
```

### `facts`

A fact with a single `facts` argument prints all facts currently in the
engine, one per line in normal form. Useful for debugging.

### `load`

A fact with two arguments whose first is `load` loads the reform file named by
the second argument. Relative paths resolve against the directory of the file
that issued the `load` (or the process working directory if there is none).
`load` issued from a rule body triggers a load mid-turn; cyclic/re-entrant
loading is not specially guarded, so avoid infinite load loops.

```rf
$ load ./iflib/lib.rf
```

### `quit`

This fact makes the engine exit. Loading and rule processing stop at the next
safe point.

### `panic`

Immediately exits the engine with an error whose message is the joined
arguments.

### `assert` and `assert-not`

`assert` takes a list of arguments and panics unless a fact with exactly those
arguments exists. `assert-not` panics if the fact *does* exist.

### `find`

`find` searches the engine for facts matching the given **pattern** and prints
each match in normal form, one per line. `find` only supports a single-fact
pattern (no fact-level repetitions); a multi-fact or repetition pattern is an
error.

### `-` (immediate removal)

A fact whose first argument is `-` removes matching facts immediately. The
remaining arguments are interpreted as a pattern (removing every matching
fact); if that pattern does not parse, they are treated as an exact fact to
remove. `$ -` with no arguments is a no-op. Removing a rule fact also
unregisters that rule.

---

## Arithmetic: `@eval`

`@eval` evaluates the **single argument that directly follows it** as a math
expression (parsed and evaluated as an f64) and substitutes the result. It is
reduced with the highest priority — immediately when a fact is created, before
rules ever see it — so math is computed as soon as it appears. Because `@eval`
interprets only the next single argument, an expression made of multiple words
must be wrapped in parentheses:

```rf
$ the final result is @eval (2 + 2 * 3)
$ half of 7 is @eval (7 / 2)
```

`2 + 2 * 3` evaluates to `8` (multiplication binds before addition) and
`7 / 2` to `3.5`. Values are always f64, so divisions don't truncate.

`@eval` reduces only when the entire expression is a valid, self-contained
arithmetic expression. An `@eval` whose expression fails to parse or fails to
evaluate is left untouched and the fact proceeds unchanged:

```rf
$ a @eval (2 + )      # still a fact with args `a`, `@eval`, `(2 + )`
```

The supported operators are `+`, `-`, `*`, `/`, `%` (remainder), and `^`
(power), plus unary `+`/`-`, and the built-in functions `sqrt`, `abs`, `exp`,
`ln`, `sin`, `cos`, `tan`, `asin`, `acos`, `atan`, `atan2`, `sinh`, `cosh`,
`tanh`, `asinh`, `acosh`, `atanh`, `floor`, `ceil`, `round`, `signum`, `max`,
`min`, and the constants `pi` and `e`.

### Randomness: `random(n)`

`@eval` includes a `random(n)` function. Its argument `n` is the **exclusive
upper bound**: `random(n)` returns a uniformly distributed f64 in `[0, n)`.
So `random(6)` draws from `[0, 6)` and `random(1)` is always in `[0, 1)`. It
draws from a deterministic stream seeded at engine creation, so combining it
with `floor` gives clean integer ranges — e.g. a die roll:

```rf
$ die roll @eval (1 + floor(random(6)))
$ random walk step @eval (floor(random(3)) - 1)
```

The stream is seeded once for the life of the engine. [`Engine::new`] seeds it
from system entropy (nondeterministic); [`Engine::new_with_seed`] or the CLI's
`--seed` flag seed it with a fixed value, making `random(n)` reproducible
across runs.

---

## Normal Form

The **normal form** of a fact is its canonical rendering: arguments separated by
spaces, with each argument wrapped in parentheses if it needs to be. An
argument is wrapped if it contains whitespace, parentheses, or a backtick, or
ends in one of `;` `.` `:` `'`. An empty argument renders as `()`.

Facts in normal form parse back to the identical fact, so normal form
round-trips for arguments that don't contain a comma, a `#`, an unescaped
`{`/`}`, or start with `$` — such arguments do not survive re-parsing (a
comma splits, `#` starts a comment, braces split, and a leading `$` gets the
`parse` prefix). `facts`, `find`, trace output, and body-placeholder
rendering at a bare argument position all use normal form.

```rf
(Grand Canyon) is big
This is a sentence ending in a period .
```

---

## Embedding

The `reform` crate is a library, and nearly all of its types, fields, and
functions are public so it can be embedded and extended:

- `Engine` holds the fact store, registered rules, command handlers, output
  sinks, and load base directory. Use `load_str` / `load_file` to load source,
  `add_fact` / `remove_fact` / `add_rule`, `turn` / `run` to drive
  evaluation, `facts()` / `rules()` to inspect state, and `register_command` /
  `remove_command` to add custom commands. Construct it with `Engine::new`
  (entropy-seeded `random(n)`) or `Engine::new_with_seed(u64)` for a
  deterministic random stream.
- `Fact`, `Arg`, `normal_form_arg`, and `normal_form_fact` provide the data
  model and rendering.
- `parser::facts` / `parser::pattern` / `parser::body` parse source, patterns,
  and rule bodies.
- `rule::compute_specificity` and the `Rule` / `Pattern` / `Body` types expose
  the matching and rendering machinery.
- The engine routes text output through pluggable sinks, which is how the WASM
  build renders into a virtual terminal.

Commands are just registered `CommandHandler` closures (the engine has no
special-cased built-ins beyond what `register_default_commands` installs), so a
host can replace or extend the command set freely.
