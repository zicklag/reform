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


