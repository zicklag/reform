# Reform Design Constraints

## Fact Normal Form

The normal form of a fact is a space-separated list of arguments, each of which are a string.

If one of the arguments contains whitespace, or a backtick (the template-string delimiter), then it is wrapped in parenthesis.

```rf
(Grand Canyon) is big 
```

A fact normally only spans one line, unless, one of its arguments is a multiline string, for example:

```rf
description is (This is a long description:
 
It has "multiple" lines)
```

Arguments are allowed to have parenthesis inside them if the parens are balanced:

```rf
description is (He was quite pleased (not that he'd admit it), with the way things had gone.)
```

If a right parenthesis needs to occur in a string, then it must be escaped with a backslash.

```rf
Here is a smiley (You can put a smiley in :\) but it has to be escaped.)
```

If you literally want an argument that has parenthesis in it, such as `(example)`, then you can do that with double-parenthesis:

```rf
This is an ((example))
```

If an argument ends with punctuation, such as `;`, `:`, or `.`, then the word needs to be wrapped in parenthesis.

```rf
(www.) is a common web domain prefix
```

Usually punctuation is split into separate arguments:

```rf
This is a sentence ending in a period .
```

## Parsing Facts

Facts in normal form will always parse successfully.

The acceptable fact syntax is more broad than the normal form.

Arguments may be separated by any number of horizontal whitespace characters such as tabs or spaces.

Comments can be added with a `#`.

```rf
# this is a comment
This is a sentence # with a comment at the end
```

Facts can be specified across multiple non-empty, subsequent lines if they are indented under the the first line of the fact:

```rf
This is     a sentence
  that spans multiple   # comments can be here, too
  lines
```

They may be indented by any choice of horizontal whitespace.

The indentation counting suspended completely while parsing arguments wrapped in parenthesis, and all whitespace is taken completely literally until the closing parenthesis of the argument.

For exmaple:

```rf
This is a sentence
  (This is a big sentence

    Indentation is preserved

In parenthesis)
  and I continue typing the same sentence
```

The fifth argument in the fact above is literally:

```rf
This is a big sentence

    Indentation is preserved

In parenthesis
```

Empty lines break up a fact. For example this creates three facts:

```rf
Fact one
  spans two lines

  This is the start of fact two
    which may continue with indentation, too

This is the start of fact three
```

Punctuation with whitespace after it is automatically split into a separate argument.

For example this fact:

```rf
example.com is a website, that is very simple.
```

In normal form would be:

```rf
example.com is a website , that is very simple .
```

Notice how in `example.com` the `.` is included in the argument without splitting, but the comma and the period which are followed by whitespace ( or the end of the line ) are split into separate arguments.

### Template arguments

Template arguments are special syntax sugar for providing possibly multi-line strings with substitutions or other special arguments mixed in more easily.

Template arguments are wrapped in backticks (`` ` ``) and are allowed to span multiple lines, similarly to parenthesis-wrapped arguments.

Different from parenthesis-wrapped arguments, they may contain curly-brace delimited arguments that are split from the template string as separate arguments.

For example:

```rf
The description is `There is a gate before you

It is {if open}open{else}closed{end if}

It is ominous.`
```

In normal form, it would be:

```rf
The description is (`) (There is a gate before you

It is ) { if open } open { else } closed { end if } (

It is ominous.) (`)
```

The backticks get put into their own arguments in order to mark the start and the end of the template string. The curly braces are similarly split. The chunks of literal strings otherwise are joined into one continuous argument as long as neither a curly brace nor the closing backtick is met.

Notice that normal word splitting is done in between backticks such as with `if open` and they split into separate args until the closing curly brace resumes the string chunk parsing.

Inside a backtick template, a literal backtick is written `` \` ``, literal braces as `\{` and `\}`, and a literal backslash as `\\`.

### Fenced blocks

A triple backtick (`` ``` ``) opens a *fenced block*: a multi-line template string that is convenient for blocks of text indented under a fact. The interior is dedented to the column of the opening fence (the leading whitespace in front of `` ``` `` is stripped from every interior line, so content indented under the fence comes out flush-left), and the leading newline (right after the opening fence) and the trailing newline (right before the closing fence) are ignored. A fenced block expands to the same `` ` `` marker args plus interior chunks as a single-backtick template.

The closing fence is a line consisting only of optional horizontal whitespace followed by `` ``` ``. For example:

```rf
Before starting first-time-look:
    say ```
        "Kion, wake up."

        Kion stirs, and opens his eyes slowly, "Hmm, what?"
        ```
```

dedents the interior to the `say ` column and drops the leading/trailing newlines, so the `say` fact receives one continuous template-string argument with the paragraph text. Backticks are literal inside a fenced block (the fence is closed by a dedicated `` ``` `` line, not by a single backtick); only `\{`, `\}`, and `\\` escapes apply.

## Loading Facts

When facts are being loaded from a file into the engine, they are pared and then prefixed with an additional `parse` argument before being stored in the engine.

For example:

```rf
This is a sentence.
```

Becomes in normal form:

```rf
parse This is a sentence .
```

If a line is prefixed with a `$`, then the `parse` prefix argument is not added. For example:

```rf
This is a sentence.
$ canyon is big
```

In normal form is:

```rf
parse This is a sentence
canyon is big
```

This allows the rule system to intentionally take "normal sentences" and post-process them and parse them into different facts to provide a more natural, parsed definition language as separate from the underlying fact model used by a game.

When a line is prefixed with a `>` then instead of being a `parse` fact it becomes a `prompt` fact.

```rf
> look up
```

Becomes:

```rf
prompt look up
```

Prompts are usually meant for input provided from outside the game, by the player. Putting the `>` in a reform file allows you a clean way to create tests for a game.

## Rules

Rules are a special kind fact. They are stored like any other fact, but they are also evaluated by the engine to pattern match and modify the facts on every turn.

In a reform file you have to use the `$` prefix to define a rule, to avoid it getting the `parse` prefix.

A rule fact has 4 or 5 arguments:

1. `rule` - The first argument is always exactly `rule`
2. name - any name you want for the rule
3. pattern - a rule _pattern_ that will try to match on other facts existing in the engine
4. body - an effect body defining the new facts to create when this rule's pattern is matched
5. specificity adjustment (optional) - an integer optionally prefixed with `+`, `-`, or `=`. `+N` adds N to the rule's computed specificity, `-N` subtracts N, and `=N` sets the specificity to exactly N (ignoring the computed value). Higher specificity fires first. This is useful for overriding the default specificity ordering without changing the pattern.

Because the pattern and body of a rule need to contain facts themselves, they will almost always need to be wrapped in parenthesis.

```rf
$ rule example
  (
    - parse $( $a1 )? $x is $( $a2 )? $y
    $( $a1 is article )?
    $( $a2 is article )?
  )
  (
    $x is $y
  )
```

The pattern and body use special macro syntax for matching on facts.

### Patterns

A pattern's job is to match on facts with possible placeholders. Placeholders have a name and start with a `$` like `$name`. When a placeholder is used multiple times in a pattern it must bind to a single value in all instances, and must be in exactly the same kind and depth of repeating blocks in each appearance.

Patterns may also contain optional and repeating blocks:

- `$( $x is )?` creates a pattern that matches on an optional `$x` placeholder followed by the exact argument `is`.
- `$( $x and )+` creates a pattern that matches on placeholder `$x` followed by literal `and`, repeated one or more times.
- `$( $x and )*` is similar but it repeats zero or more times.

Multiple facts may be matched on simultaneously by putting them on separate lines, similar to facts in a file.

Parts of a fact may be in repeating / optional block to match on repeating or optional arguments.

Entire facts may be put in a repeating or optional block to match on multiple or optional facts.

When a rule line is prefixed with a `-` it means that the rule should be _removed_ whenever this rule matches.

### Bodies

When the rule pattern matches, then the facts in the body are created.

The body is allowed to use any placeholders that were defined in the pattern.

If a placeholder was in a repeating / optional block in the pattern, it must be in a matching block at a matching depth, in the body.

Here is an example rule demonstrating multiple features:

```rf
$ rule example2
  (
    # Find all the items the player is carrying from all the
    # separate "player is carying" facts
    $(
      player is carrying $item
    )*
    # Delete the previous list of all player items if ther was one
    $(
      - all player items $( $any )*
    )?
  )
  (
    # Create a single fact with the full list of items
    all player items $( $item )*
  )
```

## Engine Commands

These are some common commands that may be implemented by different engines, but are not guaranteed everywhere.

These are triggered by just creating new facts, with the `$` syntax to prevent the `parse` prefix.

- `load ./file.rf` load a file relative to this one
- `assert fact` panic if the provided fact does not exist
- `assert-not fact` panic if the prided fact does exist
- `find (pattern)` list all facts in the engine matching the pattern.
- `quit` exit the program
- `panic message` panic with a message
- `println any number of message args` concatenate all args with no separator and print to stdout followed by a newline. To print text containing spaces, wrap it in a single parenthesized arg (e.g. `$ println (you see a cave)`); bare word args run together (`$ println you see a cave` prints `youseeacave`).
- `print args` print all the args separated by spaces to stdout without a trailing newline.
- `- fact` can be used to remove a fact immediately

## Errata

The following clarifications were recorded during implementation and should be worked into the main document above.

1. **Escaping `{` and `}` in templates** — Inside `` `...` `` template blocks, literal curly braces may be escaped with a backslash: `\{` and `\}`. Unescaped braces are always interpreted as substitution delimiters.

2. **Escaping backticks in templates** — Inside a single-backtick `` `...` `` template, a literal backtick is written `` \` ``. (Inside a triple-backtick fenced block, backticks are literal already.)

3. **Rule conflict resolution** — When multiple rules match the same facts, the rule with the highest specificity (most constrained pattern) fires first. Specificity is word-based: a literal argument scores 5, a placeholder (`$x`) scores 4 (it still fixes a position in the pattern's shape), and each required (non-negated) fact adds 1. Repetition blocks add nothing for the block itself but penalize the words inside them by the block's looseness — `?` subtracts 1, `+` subtracts 2, `*` subtracts 3 — with penalties stacking across nested blocks and saturating at zero. Negated facts contribute 0. This ranks literals above wildcards, structured rules above catch-alls, and patterns with more required repetitions above those with fewer. Ties preserve insertion order (stable sort).

4. **Character encoding** — UTF-8.

5. **`find` output** — Facts are printed to stdout in normal form, one per line.

6. **`$any` is not a keyword** — It is a conventional placeholder name, equivalent to `$x` or any other name.

7. **Prefixes do not compose** — `$` and `>` cannot be combined. A line starts with at most one prefix character.

8. **`load` from rule bodies** — If a rule body produces a `load` fact, it triggers a load mid-turn. Cyclic/re-entrant loading behavior is not yet specified; implementers should guard against infinite loops.
9. **Lazy repetitions** — Arg-level `+`/`*` repetitions are lazy: they match as few iterations as possible. When a single fact admits several full-consumption matches, they are enumerated lazy-first (the one peeling fewest arguments from the leftmost repetition first). Optional `?` blocks are greedy (one iteration preferred, zero as fallback) but enumerate both alternatives in that order. The laziest binding that satisfies the *entire* pattern — including later constraint facts, e.g. `$( $a is article )?` — fires; if the greedier parse fails a downstream constraint, matching backtracks to the next-lazier parse rather than dropping the fact. Fact-level repetitions still collect all matching facts.
10. **Recursive rule firing** — A rule may fire repeatedly within a single turn, including on facts produced by its own firing (there is no single-fire-per-turn limit). This lets a rule recursively peel one item per firing (e.g. split one sentence off a `parse` fact, leaving a shorter `parse` fact that re-triggers the same rule). Infinite recursion is bounded by a per-turn iteration cap; non-terminating rules bail with a fixpoint error. More-specific rules always get first dibs on changed facts (the turn restarts from the most-specific rule whenever any fact changes).
11. **Specificity of repeating blocks** — A repetition block (`$( ... )?`, `$( ... )+`, `$( ... )*`) adds 0 for the block itself; the words inside it are worth less the looser the block is, because a looser block constrains the match less. The per-block penalty, subtracted from each enclosed word's base score (literal 5, placeholder 4), is: `?` → 1, `+` → 2, `*` → 3. Penalties stack across nested blocks and saturate at zero. So a catch-all `parse $( $word )+` scores 1 + 5 + (4-2) = 8, while `parse $( $a1 )? $x is $( $a2 )? $y` scores 1 + 5 + (4-1) + 4 + 5(is) + (4-1) + 4 = 25 — the structured rule wins. More required repetitions still outrank fewer: `parse $( $a )+ . $( $b )+` (1 + 5 + (4-2) + 5 + (4-2) = 15) beats `parse $( $a )+ .` (1 + 5 + (4-2) + 5 = 13).
