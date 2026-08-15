/* Reform syntax highlighting for mdBook (highlight.js).
 *
 * Ported from the Monaco Monarch grammar in demo/src/lib/reform-language.ts so
 * the docs site and the web demo highlight Reform the same way. mdBook loads
 * this file after its own highlight pass, so we register the grammar and then
 * re-highlight any ` ```rf ` blocks.
 *
 * Rules are handled specially: after a `$ rule` fact, the pattern and body
 * `( ... )` blocks are highlighted with their placeholders, `-`/`!` prefixes,
 * and `$( ... )` repetition blocks visible, instead of being opaque green
 * literals.
 */
(function () {
  if (typeof hljs === 'undefined') return;

  hljs.registerLanguage('reform', function (hljs) {
    // A `$name` placeholder.
    const PLACEHOLDER = { className: 'variable', begin: /\$[A-Za-z_][A-Za-z0-9_]*/ };
    // A `$( ... )?/+/*` repetition block.
    const REPEAT = {
      className: 'keyword',
      begin: /\$\(/,
      end: /\)(\?\?|\+\+|\*\*|[?+*])?/,
      contains: [PLACEHOLDER],
    };
    // A `-` / `!` pattern-fact prefix at the start of a line.
    const NEGATE = { className: 'keyword', begin: /^[ \t]*[-!](?=\s)/ };

    return {
      name: 'Reform',
      aliases: ['rf'],
      case_insensitive: false,
      contains: [
        // Comments run from `#` to end of line.
        { className: 'comment', begin: /#.*$/ },
        // `$ rule` — a self-contained rule fact. The whole fact (name, pattern,
        // body, optional priority) is one mode so the pattern and body blocks
        // highlight placeholders, `-`/`!` prefixes, and repetition specially.
        // `returnBegin` lets the inner keyword rule color the `$ rule` prefix;
        // `returnEnd` leaves the next `$`/`>` line in the stream so the
        // top-level prefix rule highlights it.
        {
          begin: /^[ \t]*\$ rule\b/,
          returnBegin: true,
          end: /^[ \t]*[$>]/,
          returnEnd: true,
          contains: [
            { className: 'keyword', begin: /^[ \t]*\$ rule\b/ },
            // The rule name — the first `( ... )` block, on the same line as
            // `$ rule`. Rendered green like a literal arg.
            {
              className: 'string',
              begin: /\(/,
              end: /\)/,
              contains: [hljs.BACKSLASH_ESCAPE],
            },
            // The pattern and body blocks — `( ... )` at the start of a line,
            // rendered as plain text with rule syntax highlighted inside.
            {
              begin: /^[ \t]*\(/,
              end: /\)/,
              contains: [
                PLACEHOLDER,
                REPEAT,
                NEGATE,
                // Nested literal, e.g. `(Hello )` inside a body.
                { begin: /\(/, end: /\)/, contains: [hljs.BACKSLASH_ESCAPE] },
                hljs.BACKSLASH_ESCAPE,
              ],
            },
            // Plain words between blocks.
            { begin: /[^\s(){}`$#]+/ },
          ],
        },
        // CLI built-in fact conventions.
        {
          className: 'keyword',
          begin: /^[ \t]*\$ (assert-not|println|assert|print|facts|load|quit|panic|find)\b/,
        },
        // Line prefixes: `$` (direct fact / command) and `>` (prompt).
        { className: 'keyword', begin: /^[ \t]*[$>]/ },
        // Fenced block: ``` ... ``` (multi-line template).
        { className: 'template', begin: /```/, end: /```/ },
        // Single-backtick template string with `{...}` substitution sections.
        {
          className: 'template',
          begin: /`/,
          end: /`/,
          contains: [
            { className: 'keyword', begin: /\{/, end: /\}/ },
            hljs.BACKSLASH_ESCAPE,
          ],
        },
        // Repetition block `$( ... )?/+/*` (outside a rule).
        REPEAT,
        // Placeholder `$name` (outside a rule).
        PLACEHOLDER,
        // Literal argument `( ... )` with escapes.
        { className: 'string', begin: /\(/, end: /\)/, contains: [hljs.BACKSLASH_ESCAPE] },
        // Rule pattern fact prefixes `-` / `!` (outside a rule).
        NEGATE,
        // Numbers.
        { className: 'number', begin: /\b\d+\b/ },
        // Punctuation split into its own argument.
        { className: 'punctuation', begin: /[,;.'":]/ },
        // Everything else is a plain word (an argument).
        { begin: /[^\s(){}`$#]+/ },
      ],
    };
  });

  // book.js already ran its highlight pass before this script loaded, so
  // re-highlight any reform blocks now that the grammar is registered.
  document.querySelectorAll('code.language-rf').forEach(function (block) {
    hljs.highlightBlock(block);
  });
})();
