# Reform Design Constraints

## Fact Normal Form

The normal form of a fact is a space-separated list of arguments, each of which are a string.

If one of the arguments contains whitespace, then it is wrapped in parenthesis.

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

Template arguments are wrapped in square brackts and are allowed to span muliple lines, similarly to parenthesis-wrapped arguments.

Different from parenthesis-wrapped arguments, they may contain curly-brace delimited arguments that are split from the templte string as separate arguments.

For example:

```rf
The description is [There is a gate before you

It is {if open}open{else}closed{end if}

It is ominous.]
```

In normal form, it would be:

```rf
The description is ([) (There is a gate before you

It is ) { if open } open { else } closed { end if } (

It is ominous.) (])
```

The square brackets get put into their own arguments in order to mark the start and the endof the template string. The curly braces are similarly split. The chunks of literal strings otherwise are joined into one continuous argument as long as neither a curly brace nor the last balanced square bracket are met.

Notice that normal word splitting is done in between brackets such as with `if open` and they split into separate args until the closing curly brace resumes the string chunk parsing.

## Loading Facts

When facts are being loaded from a file into the engine, they are pared and then prefixed with an additional `sentence` argument before being stored in the engine.

For example:

```rf
This is a sentence.
```

Becomes in normal form:

```rf
sentence This is a sentence .
```

If a line is prefixed with a `$`, then the `sentence` prefix argument is not added. For example:

```rf
This is a sentence.
$ canyon is big
```

In normal form is:

```rf
sentence This is a sentence
canyon is big
```

This allows the rule system to intentionally take "normal sentences" and post-process them and parse them into different facts to provide a more natural, parsed definition language as separate from the underlying fact model used by a game.

When a line is prefixed with a `>` then instead of being a `sentence` fact it becomes a `prompt` fact.

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

In a reform file you have to use the `$` prefix to define a rule, to avoid it getting the `sentence` prefix.

A rule fact has 4 arguments:

1. `rule` - The first argument is always exactly `rule`
2. name - any name you want for the rule
3. pattern - a rule _pattern_ that will try to match on other facts existing in the engine
4. body - an effect body defining the new facts to create when this rule's pattern is matched

Because the pattern and body of a rule need to contain facts themselves, they will almost always need to be wrapped in parenthesis.

```rf
$ rule example
  (
    - sentence $( $a1 )? $x is $( $a2 )? $y
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

There are some common commands that may be implemented by different engines, but are not guaranteed everywhere.

These are triggered by just creating new facts, with the `$` syntax to prevent the `sentence` prefix.

- `load ./file.rf` load a file relative to this one
- `assert fact` panic if the provided fact does not exist
- `assert-not fact` panic if the prided fact does exist
- `find (pattern)` list all facts in the engine matching the pattern.
- `quit` exit the program
- `panic message` panic with a message
- `println any number of message args` print all of args to stdout followed by a newline
- `print args` print all the args out without a newline after it
- `- fact` can be used to remove a fact immediately

## Errata

The following clarifications were recorded during implementation and should be worked into the main document above.

1. **Escaping `{` and `}` in templates** — Inside `[...]` template blocks, literal curly braces may be escaped with a backslash: `\{` and `\}`. Unescaped braces are always interpreted as substitution delimiters.

2. **Nested `[...]` in templates** — Square brackets follow the same balance-tracking rules as parentheses. Nested balanced brackets are valid; a lone `]` must be escaped with `\]`.

3. **Rule conflict resolution** — When multiple rules match the same facts, the rule with the highest specificity (most constrained pattern) fires first. Specificity is determined by the pattern structure: more literal arguments, fewer wildcards/optionals, and deeper nesting increase specificity. Exact algorithm TBD.

4. **Character encoding** — UTF-8.

5. **`find` output** — Facts are printed to stdout in normal form, one per line.

6. **`$any` is not a keyword** — It is a conventional placeholder name, equivalent to `$x` or any other name.

7. **Prefixes do not compose** — `$` and `>` cannot be combined. A line starts with at most one prefix character.

8. **`load` from rule bodies** — If a rule body produces a `load` fact, it triggers a load mid-turn. Cyclic/re-entrant loading behavior is not yet specified; implementers should guard against infinite loops.
