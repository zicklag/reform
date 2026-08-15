# Rules

Rules are how you implement your logic in reform. To create them you create a
`rule` fact that has exactly 4 or 5 arguments.

Here is an example rule that will say "Hello!" when you type "hi" into the
prompt.

**hello.rf:**

```rf
$ rule (say hello when user says hi)
  (
    - prompt hi
  )
  (
    println Hello!
  )
```

And we can run it in the CLI to test it:

```
reform hello.rf
> hi
Hello!
>
```

## Rule Arguments

Lets break down each of the arguments in a rule.

#### 1. Rule Marker

```rf
rule
```
All rules start off with the first argument set exactly to `rule`.

#### 2. Rule Name

```rf
(say hello when user says hi)
```

The second argument is the _name_ of the rule. Notice that we wrap it in
parenthesis so that we can put as many words in the name as we want.

Conventionally rule names are quite detailed in reform, unlike function names in
most programming languages. The reason for this is that we don't "call" rules
manually like you do functions: the engine triggers them automatically.

There is also no namespacing, so having unique and detailed rule names can help
if you ever need to be able to remove the rule later, and also for debugging.

#### 3. The Pattern

```rf
(
  - prompt hi
)
```

The third argument of a rule is its **pattern**. If the engine finds facts that
match the pattern, then it will automatically trigger the rule.

Notice that we wrap the whole argument in parenthesis, and we aditionally put
each parenthesis on its own line. This is important if we are matching on
multiple facts in the same pattern: each fact will go on its own line.

The minus sign `-` at the beginning of the fact in the pattern indicates that we
want to match on the `prompt hi` fact **and we want to delete the fact when this
rule triggers**.

So this rule is saying, "if the user types 'hi', then I want to trigger this rule
and delete their prompt". Deleting the prompt makes sure that the rule doesn't
keep triggering every time something happens, because the `prompt hi` fact is
still there.

#### 4. The Body

```rf
(
  println Hello!
)
```

The fourth argument of a rule is its **body**. The body is a set of facts that is
created when the rule triggers.

So when the pattern in argument 3 matches, the facts in argument 4 are created.

#### 5. The Priority ( optional )

The fifth argument, which we didn't use in the example above is a number like
`+15`, `-35`, or `=50` to add to, subtract from, or exactly set the computed
priority of the rule.

As mentioned above, when the pattern matches existing facts, the engine
automatically triggers the rule, deletes any facts marked for deletion by it,
and creates the new facts that are in the body of the rule.

This is an order-sensitive operation, and running rules in different orders
can have different results.

For each rule, Reform automatically calculates a prority for it, based on how
specific its pattern is. This allows more specific rules to automatically take
precedence over very generic rules, and usually that is what you want.

There are special cases, though, where being able to tweak or override the
priority of a rule is useful, so the optional fifth arguments of rules let
you do exactly that. Higher priority rules will run before lower priority rules.

> ℹ️ **Tip:** Running the Reform CLI with the `--trace` option will make it print
out verbose debug information for all of the facts it creates and rules that it
runs. When new rules are added it will also show the computed priority of the rule,
which can be useful if you need to debug rule priority.

## The Big Picture

_All_ of the computation in Reform is built on this design of:

1. look for facts matching a rule
2. allow the rule to optionally delete matching facts
3. allow the rule to optionally create new facts
4. handle any engine integration facts such as `println` that need to interact
   with the outside world.

Despite its simplicity, this is very powerful! But to really get the most out of
it, you've got to learn some of the more powerful features that **patterns**
have.

