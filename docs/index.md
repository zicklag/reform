# Reform

> ℹ️ **Note:** This documentation is work-in-progress. I hope to expand this in
the near future along with a more sophisticated web IDE.

```
reform is a language.

properties of reform:
    size: tiny
    design: simple
    goal: fun

instead of taking my word for it, because there is a web demo:
    try out reform
```

---

Reform is a very simple, work-in-progress programming language built on just two
main concepts:

- **Facts**: simple lists of string **arguments**.
- **Rules**: describe how new facts are created or deleted, based on which facts
  exist in the world.

A unique feature of reform is that it allows you, with rules, to create custom
syntaxes that read like plain English, or other languages, such as the snippet
above.

It was inspired by [Inform][i7]'s plain English syntax and was an attempt to
find the simplest way to allow you to make your own syntaxes without needing
modifications to the engine itself. These custom syntaxes can then be included
as libraries in reform projects.

Reform's first use-case was interactive fiction, and it works well for world
modeling, but it is very different from most programming languages and is not
going to be a good fit for many things.

The language is an active experiment and we are still learning how to make
certain things work within its particular method of computation.

> **For example:** making sure operations execute in a sequential order, which
is normally the default way that programming languages work, is slightly tricky
in Reform. You can make your own "queue" fact and make rules that pull tasks off
of that queue, but it's not like a built-in feature.
>
> This has intentionally been the approach taken by Reform for most things,
though. It keeps the engine extremely simple, and allows the language to be
customized by the Reform code itself.

This book will walk you through how Reform works, and will also teach you how to
use the included `iflib` library for making interactive fiction you can play on
the commandline or web player.

[re]: https://en.wikipedia.org/wiki/Regular_expression
[i7]: https://inform7.com
