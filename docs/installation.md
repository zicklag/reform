# Installation

The reform CLI can be installed locally by first [installing
Rust](https://rust-lang.org/tools/install/), and then running:

```bash
cargo install --git https://github.com/zicklag/reform --bin reform
```

Later we will publish pre-built binaries so you don't have to install Rust
first.

## Running Reform

Generally you will run reform by passing it one or more reform files to load.
These files conventionally end in `.rf`. For example you can create a file:

**hello.rf:**

```rf
$ println (Hello reform!)
```

Then you can run the file:

```
reform hello.rf
Hello world!
> 
```

Reform will give you a `>` prompt, which in this program will do nothing, but
that can be used, for example, to play interactive fiction games. 

You can also use the `$` syntax in the CLI prompt to insert raw facts, which can
be useful for debugging and trying stuff out.
