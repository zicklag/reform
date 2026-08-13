# Reform Demo

A web playground for the [Reform](..) rule engine. It combines a Monaco editor
with a custom Reform syntax highlighter, and an xterm.js virtual terminal wired
to the engine compiled to WASM via wasm-bindgen.

## Layout

- `src/lib/reform-wasm/` — wasm-bindgen bindings (`.js`, `.wasm`, `.d.ts`),
  copied from `reform-wasm/pkg`. **Regenerate, don't hand-edit** (see below).
- `src/lib/reform-language.ts` — Monaco language definition + theme for Reform.
- `src/lib/reform-terminal.ts` — bridges the wasm engine's output sinks to an
  xterm.js terminal.
- `src/routes/+page.svelte` — the playground: editor + terminal + flag toggles
  (trace logging, `$`-command handling) + example files.

## Build

Prereqs: `wasm-bindgen-cli`, `wasm32-unknown-unknown` target, Node/npm.

```sh
# 1. Build the wasm bindings (from the repo root)
cd reform-wasm
cargo build --release --target wasm32-unknown-unknown -p reform-wasm
wasm-bindgen --target web --out-dir pkg \
  ../target/wasm32-unknown-unknown/release/reform_wasm.wasm
# copy into the demo
cp pkg/reform_wasm.js pkg/reform_wasm_bg.wasm pkg/reform_wasm.d.ts ../demo/src/lib/reform-wasm/

# 2. Build / run the demo
cd ../demo
npm install
npm run dev      # dev server
npm run build    # static site -> build/
npm run preview  # serve the static build
```

The page is `ssr = false` (Monaco, xterm, and wasm are browser-only) and
`prerender = true` (adapter-static), so the static build serves a shell that
hydrates in the browser.

## Flags

- **Trace** — routes the engine's trace events (`+`/`-` facts, rule registration,
  `fire <name>`) to the terminal, matching `--trace` / `REFORM_TRACE`.
- **Allow $ commands** — when on, `$`-prefixed lines typed at the terminal are
  inserted as direct facts/commands; when off they are treated as player prompts,
  mirroring the CLI `-A`.
