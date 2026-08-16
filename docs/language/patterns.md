# Patterns

So far we've only seen one pattern:

```rf
(
  - prompt hi
)
```

This pattern looks for a fact that matches `prompt hi` _exactly_. But there are
many times where we will know the _shape_ of a fact that we want to match on,
but not exactly what all the arguments are.

## Simple Placholders

Patterns get _much_ more powerful by allowing us to put in placeholders for
arguments. For example, let's make a small game where you can say `my name is
Caleb` and the game will output `Hello Caleb!`.

The difference from the previous example is that we do not know what the users
name is going to be.

**name.rf:**

```rf
$ rule (say hello to user by name)
  (
    - prompt my name is $name
  )
  (
    println (Hello ) $name !
  )
```

Similar to the previous pattern we can specify the fact that we want to match
on, but this time we add a `$name` placeholder where we expect the name to go.

We can _also_ use the same `$name` in the **body** of the rule, so that we can
say hello with whatever name the user entered.

> ℹ️ **Note:** In this example you can see us providing multiple arguments to
`println`. When you provide multiple arguments to println, it will print them
all out **without spaces between them**. So we wrap "Hello " in parenthesis to
make sure that it prints the space after hello, before the name.

Now we can run it with any name we like:

```
reform name.rf
> my name is Sheri
Hello Sheri!
>
```

This just a simple example; patterns can get far more powerful than that.

## Multiple Facts and Multiple Placeholders

Before we can get into more things you can do with patterns, we'll need some
more facts to play with!

Let's start by describing a few rooms and how they are connected to each-other.

We want to design a small map that looks like this:

```
 ┌──────────────┐    ┌─────────────┐
 │              │    │             │
 │   Kitchen    ┼───►│  Bedroom    │
 │              │    │             │
 └───────▲──────┘    └─────────────┘
         │                          
 ┌───────┼──────┐                   
 │              │                   
 │ Living Room  │                   
 │              │                   
 └──────────────┘                   
```

**simple-rooms.rf:**

```rf
# Set the room the player is in  
$ player is in living-room

# Living room
$ description of living-room is
  (A cozy room with a nice sofa.)

# Kitchen
$ description of kitchen is
  (The place where we cook the food.)
  
$ kitchen is north of living-room

# Bedroom
$ description of bedroom is
  (A nice room with your bed in it.)
  
$ bedroom is east of kitchen
```

All this does is create the facts that describe the world. Now we can create a
rule that parses a `look` prompt and shows the description for the room that the
player is in.

```rf
$ rule (parse the "look" command)
  (
    - prompt look
    player is in $room
    description of $room is $description
  )
  (
    println (You are in the ) $room .
    println
    println $description
  )
```

Here we have, for the first time, multiple facts in the rule's pattern. That
means that the rule will only trigger when all of the facts exists.

Additionally we are able to use the same placeholder in _multiple facts_. When
we do this, Reform will make sure that it only matches if the placeholder has
the _same value_ in all of the matching facts.

This simple setup lets us check which room the player is in, and get the
description of that room.

```
reform simple-rooms.rf
> look
You are in the living-room.

A cozy room with a nice sofa.
>
```

## Rule Priorities

Now would be a good time to fix a little issue in our game: if we prompt it with
something other than `look` it doesn't do anything. It makes sense that we can't
handle _every_ possible input, but we should have a "default" behavior if the
game doesn't understand the input. This is where rule priorities come in.

Every rule in Reform has an automatically calculated priority. The more specific
a rule is, the higher its priority. This means that most of the time it will
automatically do what we want if we create a default rule to catch inputs
that were not understood by a more specific rule.

For example we can add this to `simple-rooms.rf`:

```rf
$ rule (apologize for not understanding prompt)
  (
    - prompt $command
  )
  (
    println (I'm sorry, I didn't understand that command.)
  )
```

And running it:

```
reform simple-rooms.rf
> hi
I'm sorry, I didn't understand that command.
> look
You are in the living-room.

A cozy room with a nice sofa.
>
```

Notice how the priorities worked themselves out automatically.

Even though the "apologize" rule's pattern technically matches the `look`
prompt, because the look rule had a more specific pattern, it ran first and it
removed the prompt before the apologize rule found it.

## Repeating and Optional Blocks

At this point you might have realized that we still have a problem with our
apologize rule: it only works if we put in a single word. If we type `hello
world`, `$command` will only act as a placeholder for the first word, and the
pattern fails to match.

```
reform simple-rooms.rf
> hello
I'm sorry, I didn't understand that command.
> hello world
>
```

What we need is a way to match on **any number of arguments**.

We can do this in Reform with **repeating / optional blocks**, which come in 3 varieties.

**Optional blocks** start with `$(` and end with `)?` and can wrap around arguments that you want to be optional in the pattern.

**Zero-or-more blocks** start with `$(` and end with `)*` and will match if the
arguments inside are not there, or if they are there, and possibly repeated
multiple times.

**One-or-more blocks** start with `$(` and end with `)+` and will match if the
arguments inside are repeated one or more times.

So to fix our apologize rule we can have it match on any prompt with a one-or-more repeating
block:

```rf
$ rule (apologize for not understanding prompt)
  (
    - prompt $( $arg )+
  )
  (
    println (I'm sorry, I didn't understand your command:) $( ( ) $arg )+
  )
```

Now it will print an error message for any number of arguments. There are a
couple things to note here:

- When you use a placeholder in a block in the pattern, you must put it in the
  same kind of block in the body.
- If we want to put a literal, single space in an argument we have to put it in
  parenthesis: `( )`
- Because `println` doesn't put spaces between arguments, we add a space before
  `$arg`, _inside the repeating block_. This way, each time the block repeats
  with a new `$arg`, it also gets it's own space so that it's properly separated.

```
reform simple-rooms.rf
> hello
I'm sorry, I didn't understand your command: hello
> hello world
I'm sorry, I didn't understand your command: hello world
>
```

It works! We get the apology message regardless of how many arguments we put
into our prompt.

Blocks can be used to parser very rich patterns. They can even be nested inside
each-other when necessary or wrapped around whole facts to match on multiple
facts at a time.

### Greedy and Lazy Blocks

By default pattern blocks are **lazy**. This means that they will match as few
arguments as possible, if there is a choice.

For example if we have a pattern like this:

```
(
  $( $before )+ $( $after )*
)
```

If we have a fact like:

```
a b c
```

`$before` will match `a` because it has to match at least once, but because
it is lazy, it will only match `a`, and it will allow `$after` to match
on `b` and `c`.

Even though `$after` is lazy, too, it has to match on `b` and `c` because
if it tried to match on nothing, the pattern wouldn't match at all.

So lazy blocks will try to do as little matching as possible, while still
actually matching the fact, if they can.

Usually lazy matching is good for parsing because it lets us more easily
match on some info at the beginning of a fact, and leave the rest at
the end in a catch all like `$after`. Sometimes, though, you need to make
a block **greedy**.

A greedy block will match as many arguments as possible. You can  make
any of the blocks greedy instead of lazy by doubling the optional / repeat
character at the end of the block, i.e:

- Greedy optional block `$( $example )??`
- Greedy zero-or-more block `$( $example )**`
- Greedy one-or-more block `$( $example )++`

## Almost the Whole Language

Now you've almost learned the entire Reform language!

It may take some getting used to, figuring out how to make different kinds of
logic and flows with only rules and facts. We are still figuring out the best
way to do things ourselves!

But the system is very flexible and we are excited to see what kinds of
things can be made with it.

As we flesh out our interactive fiction library, we'll be extending the docs
with a cookbook to show how to do common things.

The last concept to learn about is [template strings](templates.md), which are
helpful for writing big strings with programmed substitutions in them.

### Plain Language Programming

We also haven't demonstrated how to do "plain language" programming yet! In
fact, you already have all the tools you need to make custom language parsers
in Reform.

Instead of making rules that match on `prompt` facts, you can make rules that
match on `parse` facts, which are generated whenever you type a fact without
the `$` before it.

This lets you extend Reform to create your own syntax just by using rules! In a
[later section](plain-language-parsing.md) section we'll go into some examples
of what this can look like.

But before we get to any of that, lets update our game so we can walk around!

## Walking Around Rooms

In our **simple-rooms.rf** so far, we can `look`, but we can only look at the
room we're in and there's no way to go to the other rooms.

What we want to do is add rules for parsing `north`, `south`, `east`, and `west`
so that they move us into the adjacent room in that direction, if there is one.

Our north rule can look like this:

```rf
$ rule (go north)
  (
    - prompt north
    - player is in $here
    $there is north of $here
  )
  (
    player is in $there
    prompt look
  )
```

Let's break it down step by step. First the pattern:

- `-prompt north`: If the player typed north, we match on it and remove the
  prompt.
- `- player is in $here`: If the player is in any room `$here` then we match on
  that and remove the fact. We remove it so that we can add a new fact for where
  the player is that will replace the old one.
- `$there is north of $here`: We match on a fact that says some other room is
  north of the `$here` room that the player is in.

If all those facts exist, then we are able to go north in the body:

- `player is in $there`: We put the player in the room that was north of `$here`.
- `prompt look`: We trigger a look prompt so that the player can immediately see
  the description of the Room that they moved to.

Now we can run it!

```
reform examples/simple-rooms.rf
> look
You are in the living-room.

A cozy room with a nice sofa.
> north
You are in the kitchen.

The place where we cook the food.
> north
I'm sorry, I didn't understand your command: north
>
```

Notice that when there is no room north of the room that we are in, our north
rule fails to fire and the apologize rule fires because nothing consumed `prompt
north`. That's confusing to the player, so let's improve it by adding default
rule for going north that gives a more helpful message.

```rf
$ rule (fail to go north)
  (
    - prompt north
  )
  (
    println (You can't go that way.)
  )
```

And running it:

```
reform examples/simple-rooms.rf
> north
You are in the kitchen.

The place where we cook the food.
> north
You can't go that way.
>
```

The automatic rule priority works out great for us again: the "go north" rule
is more specific than "fail to go north" because it matches on multiple facts.

Now, when parsing a `north` prompt, there are actually three different rules now
that would match on the prompt:

- "go north"
- "fail to go north", and
- "apologize for not understanding prompt"

But Reform is smart enough to order them by how specific they are so that
they each fire only when they should.

Finally, we can do the same thing for all of the remaining directions:

```rf
$ rule (go south)
  (
    - prompt south
    - player is in $here
    $here is north of $there
  )
  (
    player is in $there
    prompt look
  )
  
$ rule (fail to go south)
  (
    - prompt south
  )
  (
    println (You can't go that way.)
  )
  
$ rule (go east)
  (
    - prompt east
    - player is in $here
    $there is east of $here
  )
  (
    player is in $there
    prompt look
  )
  
$ rule (fail to go east)
  (
    - prompt east
  )
  (
    println (You can't go that way.)
  )

$ rule (go west)
  (
    - prompt west
    - player is in $here
    $here is east of $there
  )
  (
    player is in $there
    prompt look
  )
  
$ rule (fail to go west)
  (
    - prompt west
  )
  (
    println (You can't go that way.)
  )
```

One thing worth noticing is that when going south and west, just swap `$here` and
`$there` while still chekcing the `north of` or `east of` rules.

There are multiple ways of handling this, for example, we could create rules that
automatically create `west of` rules whenever there is a corresponding `east of`.
In our experience so far it seems best to decide on an "official" way to write
any given fact, so that if you ever need to change which room is `east of` another
you only have to worry about updating one rule, instead of both the `east of` and
`west of` rules.

Anyway, now we can navigate our whole map!

```
reform examples/simple-rooms.rf
> look
You are in the living-room.

A cozy room with a nice sofa.
> east
You can't go that way.
> north
You are in the kitchen.

The place where we cook the food.
> west
You can't go that way.
> east
You are in the bedroom.

A nice room with your bed in it.
> east
You can't go that way.
>
```