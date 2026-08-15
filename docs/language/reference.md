# Language Reference

### CLI Built-ins

The Reform CLI defines a small collection of it's own fact conventions that allow
your games to output to the console, debug the game, and load other reform files.

#### `print` and `println`

If you create a fact where the first argument is `print` or `println` it will
output all of the arguments that come afterward.

`println` will make sure there is a newline after what is printed, and `print`
will print it without a newline.

#### `facts`

If you create a fact with a single `facts` argument, it will print out all of
the facts currently in the engine.

This is very useful for debugging.

#### `load`

If you create a fact that has two arguments where the first one is `load`, it
will try to load the reform file specified in the second argument.

This allows you to orgnize your reform program across multiple files:

```rf
# Load the interactive fiction library
$ load ./iflb/lib.rf

# Now I can use any custom syntaxes defined by the interactive fiction library.
```

#### `quit`

This fact makes the reform engine exit.

#### `panic`

This will immediately exit the engine with a panic message set to the arguments
provided.

#### `assert` and `assert-not`

The `assert` fact lets you specify a list of arguments and the engine will make
sure that a fact with those arguments exists. If the fact does not exist, the
engine will panic.

`assert-not` will panic if the fact _does_ exist.

#### `find`

This fact will search the engine for any facts matching the **pattern** that you
specify.