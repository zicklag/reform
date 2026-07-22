# Reform

A programming language based on an Expert System. Language is detailed in docs/DESIGN.md.

## Testing

We would like to maintain 100% test coverage of the `reform` crate. `reform-cli` is not as important to have full test coverage of.

Run `cargo llvm-cov -p reform` for the per-file summary table (Regions / Lines / Cover). For the exact lines and columns of uncovered code, pipe the JSON output through jq:

```sh
cargo llvm-cov -p reform --json 2>/dev/null \
  | jq -r '.data[].files[] | .filename as $f | .segments[] | select(.[2] == 0 and .[4] == true) | "\($f | split("/") | last): line \(.[0]) col \(.[1])"'
```

Each segment is `[line, col, count, covered, hasCount, isBranch]`; filtering on `count == 0 and hasCount == true` keeps only the instrumented regions that never executed (the table's "Missed Regions"), dropping the zero-count region boundaries that llvm-cov emits for closing braces. The text report and this jq filter report the same set of missed regions.

Uncovered regions are a smell: they usually mark an unreachable error condition or a value that "can't happen" at runtime. Prefer to fix the root cause — represent the impossibility in the type system or at an earlier parsing stage instead of unwrapping/propagating it later — rather than adding a test that exercises the dead branch.

## Coding Principles

We are focusing on an extremely lean and focused implementation with as little code as possible.

For this project we prefer to make most fields, functions, and modules public in the library. We want it to be wide open to users of the library.

I want to keep the src dir focused on implementation to, so instead of unit tests, lets put unit tests in the ./tests dir, which should work fine with all the modules, types, and functions public.
