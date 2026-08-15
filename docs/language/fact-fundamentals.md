# Fact Fundamentals

The fundamental data structure in Reform is the **fact**. A fact is simply a
list of strings called arguments.

## Typing Facts

Facts can be created directly by writing a line that starts with a `$` with the
fact's arguments separated by spaces.

```
$ Alice is happy.
```

This creates a fact with 4 arguments:

1. `Alice`
2. `is`
3. `happy`
4. `.`

Note that punctuation at the end of a word, such as the period in the example
above, is automatically split into a separate argument.

### Comments

Lines that start with a `#` are comments. They are completely ignored by the
reform engine, but can be handy for documenting your code.

### Multiple Facts

Multiple facts can be added each on their own line.

```
$ Bob is smiling.
$ Carol is fun.
```

A single fact is allowed to span multiple lines by indenting it any amount under
the first character line of the fact.

This example is equivalent to the one above:

```
$ Bob
  is smiling.
$ Carol
  is
    fun.
```

You can do any kind of indentation you want in the fact, as long as it is more
indented than the line that started the fact.

### Grouping With Parenthesis

Sometimes you want to make Reform to put multiple words into a single argument,
you can do this by wrapping the argument in parenthesis:

```
$ the full name of Alice is (Alice Von Schmidt)
```

1. `the`
2. `full`
3. `name`
4. `of`
5. `Alice`
6. `is`
7. `Alice Von Schmidt`

## The Meaning of Facts

Facts do not automatically _mean_ anything. You can crate any facts you need and
deal with them however you want. They are just data.

But different games, libraries, and Reform implementations will interpret
certain facts as having a certain purpose or meaning by convention. Developing
these conventions is a big part of making custom syntaxes.

The Reform CLI defines a small collection of it's own [fact
conventions](reference.md#cli-built-ins) that allow your games to output to the
console, debug the game, and load other reform files.

The most important ones for you to know are [`load`], [`println`], and [`facts`].

[`load`]: reference.md#load
[`println`]: reference.md#print-and-println
[`facts`]: reference.md#facts

