# Features & Use Cases

## Use Cases

We are still figuring out what Reform can do, and what it is good for. It is a very different language!

That said, here are some of the idea's we've had for where it might be useful:

  - **Interactive fiction:** This was main use-case that Reform was
    created for.
  - **High level scripting language for games:** We may end up using this in our [Bones]
    games as a DSL for scripting out things like cut scenes, CPU controlled
    players, state machines, or other high level game elements.
  - **Readable democratic policies for community online spaces:** We've
    considered using it as a language for defining democratic process as
    executable contracts that could integrate with community spaces such as
    [Roomy].
  - **Specification for user experience journeys:** We've run into the need to specify user
    journeys which are conceptually similar to interactive fiction scenarios.
    Reform could serve as a useful format for *executable specifications* in the
    realm of user experience design. It can be a kind of "interactive
    storyboard".
  - Anywhere you want to allow a subset of "plain language" to be used to modify
    a computer program *deterministically* ( i.e. without embedding an LLM ).

[Bones]: https://github.com/fishfolk/bones
[Roomy]: https://a.roomy.space

## Features

Reform's design is focused on 5 major pillars.

### Fun

We want Reform to let you do fun stuff with computers!

The hope is to bring back some of the wonder of computing and show more people
how fun it can be to tinker with something. We don't expect everybody to learn
the rule syntax, but we hope that apps or people who _do_ learn the rule sytax
can use it to make easier syntaxes that more people will be able to do fun stuff
with.

### Simple & Minimalist

Reform tries to be as simple as possible at its core. This has been taken far
enough that it may not be suitable for many use-cases, but that's OK for our
purposes.

The goal is to have a very flexible language in terms of declarative world
modeling, while having very few language concepts built-in.

Because everything is built around matching on string arguments, doing thing like
dealing with external IO, parsing other formats, or FFI, will not be Reform's
strong suit out of the box. Arithmetic is built in via the [`@eval`][@eval]
fact, however, so expressions like `@eval (2 + 2 * 3)` are reduced to `8`
immediately when they appear.

It is possible to integrate those things using Rust. Rust can interact with
the facts in Reform to give it indirect control over systems outside of it, but
Reform is still limited in the kinds of operations it can easily express.

[@eval]: language/reference.md#arithmetic-eval

### Flexible

Reform is meant to be very flexible in the way that it allows you to make your
own syntaxes and rules to influence how they are handled.

The goal is to allow you to make complete, custom language parsers written
completely in Reform. The caveat to this is that Reform languges are mostly
space-sparated with some built-in rules for indentation handling.

It is opinionated at the base level, and flexible at the top layer.

### Portable

Reform is very easy to embed in different languages and can be trivially run in
a web brower. It's not hard to add custom integrations with other languages or
systems, so hopefully it can be useful as a scripting / extension language or
DSL.

### Small

Reform's engine is very small. The core implementation, excluding tests, is
currently less than 2k lines of code. The Reform CLI, compiled for Linux is only
702 kilobytes at the time of writing.

