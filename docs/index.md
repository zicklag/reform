# Reform

> ℹ️ **Note:** This documentation is work-in-progress. I hope to expand this in
the near future along with a more sophisticated web IDE.

Reform is a very simple, work-in-progress programming language built on just two
main concepts:

- A **fact** is simply a list of string **arguments**.
- A **rule** describes how new facts are created or deleted, based on which
  facts exist in the world.

A unique feature of reform is that it allows you, with rules, to create custom
syntaxes that read like plain English, or other languages.

It was inspired by [Inform][i7]'s plain English syntax and was an attempt to
find the simplest way to allow you to make your own syntaxes without needing
modifications to the engine itself. These custom syntaxes can then be included
as libraries in reform projects.

Reform's first use-case was interactive fiction, and it works well for world
modeling, but currently has no arithmatic support and is very different from
most programming languages.

While I will most likely add support for arithmatic somehow later, I'm not sure
about the exact mechanism that I'll use yet. The language is an active
experiment and I am still learning how to make certain things work in it.

> ℹ️ **For example:** making sure that rules execute in a sequential order, which is
normally the default way that programming languages work, is slightly tricky in
Reform. It took some thinking before I realized how to make it work.

This book will walk you through how Reform works, and will also teach you how to
use the included interactive fiction library to make interactive story worlds
that you can play on the commandline.

[re]: https://en.wikipedia.org/wiki/Regular_expression
[i7]: https://inform7.com

## Interactive Web Demo

Reform is very portable and has an [interactive web demo](ide) where you can try
it out without having to install it. It's rudimentary for now and needs to be
filled out with much better examples. It will be extended along with these docs
as I find time.
