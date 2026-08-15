# Fact Fundamentals

The fundamental data structure in Reform is the **fact**. A fact is simply a
list of strings called arguments.

## Typing Facts

Facts can be created directly by writing a line that starts with a `$` with the
fact's arguments separated by spaces.

```rf
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

```rf
$ Bob is smiling.
$ Carol is fun.
```

A single fact is allowed to span multiple lines by indenting it any amount under
the first character line of the fact.

This example is equivalent to the one above:

```rf
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

```rf
$ the full name of Alice is (Alice Von Schmidt)
```

1. `the`
2. `full`
3. `name`
4. `of`
5. `Alice`
6. `is`
7. `Alice Von Schmidt`

## `parse` and `prompt` Facts

When you input a fact with a `$` sign before it, it tells the engine to create a
raw fact directly. But there are two other ways to input facts that are built-in
to the engine for convenience.

**`prompt` facts** are created in two different scenarios:

1. When you type a line into the reform CLI after the `>` sign.
2. When you start a line in a reform file with `>`.

Prompt facts are created almost identically to raw facts, but they automatically
have a `prompt` argument added to the beginning of them.

For example, these two lines create the exact same fact in a reform file:

```rf
> Hello World
$ prompt Hello World
```

This simple convention gives rules a way to distinguish user input from other
kinds of facts.

**`parse` facts** are similar to prompt facts: they are just normal facts that
start with a `parse` argument.

Parse facts are created when you type a line in a reform file _without_ starting
the line with a `$`. For example, these two facts are identical:

```rf
Hello World
$ parse Hello World
```

Parse facts are one of the keys to reform's "natural language" parsing. It
allows you to type plain looking sentences that are automatically converted into
`parse` facts. Reform rules can then transform those `parse` facts to modify the
other facts in any way that might be needed.

## The Meaning of Facts

You might notice that the example facts above are just whatever we want to say,
in no particular way. Actually, facts do not automatically _mean_ anything. They
are just data.

We could just as well create facts like `Bob is green` or `The color of Bob is
green` and the engine doesn't care: it doesn't know what any of it means anyway.
But different games, libraries, and Reform implementations will interpret some
facts as having a certain meaning by convention.

Once we get into creating **rules** we can define what different facts mean in
the context of our program, by defining how different facts interact with
each-other.

The Reform CLI also has a small collection of it's own [fact
conventions](reference.md#cli-built-ins). The most important ones for you to
know are [`load`], [`println`], and [`facts`].

- `$ load file.rf` an be used to load another reform file, so you can organize a
  project across multiple files easily.
- `$ println (Text to print)` can be used to print a line of output to the
  console.
- `$ facts` can be used to see the complete list of facts in the engine at the
  moment. It's a crucial debugging tool.

[`load`]: reference.md#load
[`println`]: reference.md#print-and-println
[`facts`]: reference.md#facts

Combined with `prompt` facts, these few facts give us what we need to make
interactive console games.
