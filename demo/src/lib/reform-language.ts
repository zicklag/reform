import type * as monaco from 'monaco-editor';

type Monaco = typeof monaco;

/**
 * A Monaco language definition for the Reform syntax.
 *
 * Reform is a fact-based language. A file is a sequence of facts, each a
 * space-separated list of arguments. Key syntax:
 *
 *   - `#` starts a comment to end of line
 *   - `$` prefix marks a direct fact / engine command (no `sentence` prefix)
 *   - `>` prefix marks a player prompt
 *   - `( ... )` a literal argument (parens balanced, `\(`/`\)` escapes)
 *   - `` `...` `` a template string; `{ ... }` a substitution section inside
 *   - ```` ```...``` ```` a fenced block (multi-line template)
 *   - `$name` a placeholder; `$( ... )?/+/*` a repetition block (in rules)
 *   - `-` / `!` prefixes on a rule pattern fact (remove / negate)
 *   - punctuation `, ; . ' :` split into their own arguments
 */
export function registerReformLanguage(monaco: Monaco): void {
  if (monaco.languages.getLanguages().some((l) => l.id === 'reform')) {
    return;
  }

  monaco.languages.register({ id: 'reform', extensions: ['.rf'], aliases: ['Reform', 'reform'] });

  monaco.languages.setMonarchTokensProvider('reform', {
    // Comments run from `#` to end of line.
    comments: [
      { line: '#' },
    ],

    // The `$` and `>` line prefixes.
    prefixes: ['$', '>'],

    // Punctuation that splits into its own argument.
    punctuation: [',', ';', '.', "'", ':'],

    // Repetition markers on `$( ... )` blocks.
    repetition: ['?', '+', '*', '??', '++', '**'],

    tokenizer: {
      root: [
        // Comments.
        [/#.*$/, 'comment'],

        // Line prefixes: `$` (direct fact / command) and `>` (prompt).
        [/^[ \t]*[$>]/, 'keyword.prefix'],

        // Fenced block: ``` ... ``` (multi-line template). Consume to the
        // closing fence line.
        [/```/, { token: 'string.fence', next: '@fence' }],

        // Single-backtick template string.
        [/`/, { token: 'string.template', next: '@template' }],

        // Repetition block `$( ... )?/+/*` — the `$(` opener.
        [/\$\(/, { token: 'keyword.repeat', next: '@repeat' }],

        // Placeholder `$name`.
        [/\$[A-Za-z_][A-Za-z0-9_]*/, 'variable.placeholder'],

        // Literal argument `( ... )`.
        [/\(/, { token: 'string.literal', next: '@literal' }],

        // Rule pattern fact prefixes `-` / `!` (only meaningful in rules,
        // but harmless to highlight anywhere).
        [/^[ \t]*[-!](?=\s)/, 'keyword.negate'],

        // Numbers.
        [/\b\d+\b/, 'number'],

        // Punctuation split into its own argument.
        [/[,;.'":]/, 'punctuation'],

        // Everything else is a plain word (an argument).
        [/[^\s(){}`$#]+/, 'string.arg'],
      ],

      // Inside a fenced block: everything is literal text until a line that
      // is only optional whitespace followed by ```.
      fence: [
        [/^[ \t]*```/, { token: 'string.fence', next: '@pop' }],
        [/[^`]*/, 'string.fence'],
        [/`/, 'string.fence'],
      ],

      // Inside a single-backtick template: literal text with `{...}`
      // substitution sections and `\`` / `\{` / `\}` escapes.
      template: [
        [/\\[`{}]/, 'string.escape'],
        [/`/, { token: 'string.template', next: '@pop' }],
        [/\{/, { token: 'keyword.subst', next: '@curly' }],
        [/[^`{}]+/, 'string.template'],
      ],

      // Inside a `{ ... }` substitution section: word-split arguments.
      curly: [
        [/\}/, { token: 'keyword.subst', next: '@pop' }],
        [/[^\s}]+/, 'string.arg'],
        [/\s+/, 'white'],
      ],

      // Inside a `$( ... )` repetition block: placeholders, literals, and
      // nested blocks, until the closing `)` followed by a repetition marker.
      repeat: [
        [/\$\(/, { token: 'keyword.repeat', next: '@repeat' }],
        [/\$[A-Za-z_][A-Za-z0-9_]*/, 'variable.placeholder'],
        [/\(/, { token: 'string.literal', next: '@literal' }],
        [/\)(\?\?|\+\+|\*\*|[?+*])?/, { token: 'keyword.repeat', next: '@pop' }],
        [/[^\s()$]+/, 'string.arg'],
        [/\s+/, 'white'],
      ],

      // Inside a literal `( ... )` argument: balanced parens with escapes.
      literal: [
        [/\\[()\\]/, 'string.escape'],
        [/\(/, { token: 'string.literal', next: '@literal' }],
        [/\)/, { token: 'string.literal', next: '@pop' }],
        [/[^()\\]+/, 'string.literal'],
      ],
    },
  });

  // Theme for the dark demo. Plain words (arguments) render in a neutral gray
  // so that the two "string-like" constructs stand out against them:
  // parenthesis-wrapped literal args (green) and backtick/fenced templates
  // (orange).
  monaco.editor.defineTheme('reform-theme', {
    base: 'vs-dark',
    inherit: true,
    rules: [
      { token: 'comment', foreground: '6a737d', fontStyle: 'italic' },
      { token: 'keyword.prefix', foreground: 'c678dd', fontStyle: 'bold' },
      { token: 'keyword.repeat', foreground: 'c678dd', fontStyle: 'bold' },
      { token: 'keyword.negate', foreground: 'e06c75', fontStyle: 'bold' },
      { token: 'keyword.subst', foreground: 'd19a66' },
      { token: 'variable.placeholder', foreground: '61afef' },
      // Plain word argument (default text color).
      { token: 'string.arg', foreground: 'd4d4d4' },
      // Parenthesis-wrapped literal argument.
      { token: 'string.literal', foreground: '98c379' },
      // Backtick template string and fenced block.
      { token: 'string.template', foreground: 'ce9178' },
      { token: 'string.fence', foreground: 'ce9178' },
      { token: 'string.escape', foreground: 'd19a66' },
      { token: 'number', foreground: 'd19a66' },
      { token: 'punctuation', foreground: 'abb2bf' },
    ],
    colors: {},
  });
}
