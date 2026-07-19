# Reform

A programming language based on an Expert System. Language is detailed in docs/DESIGN.md.

## Testing

Use `cargo llvm-cov -p reform --json` to analyze test coverage. We would like to maintain 100% test coverage of the `reform` crate. `reform-cli` is not as important to have full test coverage of.

## Coding Principles

We are focusing on an extremely lean and focused implementation with as little code as possible.

For this project we prefer to make most fields, functions, and modules public in the library. We want it to be wide open to users of the library.

I want to keep the src dir focused on implementation to, so instead of unit tests, lets put unit tests in the ./tests dir, which should work fine with all the modules, types, and functions public.
