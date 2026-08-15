<script lang="ts">
	import { onMount } from 'svelte';
	import { Button } from '$lib/components/ui/button/index.js';
	import { Card, CardContent, CardHeader, CardTitle } from '$lib/components/ui/card/index.js';
	import { Label } from '$lib/components/ui/label/index.js';
	import { Switch } from '$lib/components/ui/switch/index.js';
	import { Separator } from '$lib/components/ui/separator/index.js';
	import { Badge } from '$lib/components/ui/badge/index.js';
	import { Select, SelectContent, SelectItem, SelectTrigger } from '$lib/components/ui/select/index.js';
	import { Select as SelectPrimitive } from 'bits-ui';
	import { registerReformLanguage } from '$lib/reform-language.js';
	import { ReformTerminal } from '$lib/reform-terminal.js';
	import { Terminal as Xterm } from '@xterm/xterm';
	import { FitAddon } from '@xterm/addon-fit';
	import '@xterm/xterm/css/xterm.css';

	// --- example files ------------------------------------------------------

	const examples: Record<string, { name: string; content: string }> = {
		'hello': {
			name: 'Hello World',
			content: `# A minimal Reform program.
#
# Facts are space-separated arguments. A line prefixed with \`$\` is a direct
# fact (no \`parse\` prefix); \`>\` marks a player prompt.

$ println (Hello from Reform!)

$ the world is round
$ assert the world is round
$ println (assert passed: the world is round)
`,
		},
		'game': {
			name: 'Mini Game',
			content: `# A tiny interactive game: a rule that responds to player prompts.
#
# The \`>\` prefix in a file is a player prompt; typing at the terminal is too.
# Rules match on \`prompt\` facts and can rewrite them into responses.

$ rule (greet)
    (
        - prompt hello
    )
    (
        $ println (Hello, adventurer!)
    )

$ rule (look)
    (
        - prompt look
    )
    (
        $ println (You are in a dark cave. A faint light glows to the north.)
    )

$ rule (quit)
    (
        - prompt quit
    )
    (
        $ println (Goodbye!)
        $ quit
    )

> hello
> look
`,
		},
		'rule': {
			name: 'Rule Rewriting',
			content: `# A rule that rewrites a sentence into a derived fact.
#
# The pattern matches a \`parse\` fact and the body creates a new fact.
# The \`-\` prefix on the pattern removes the matched fact.

$ rule (parse is)
    (
        - parse $x is $y
    )
    (
        $x is $y
    )

the sky is blue
the grass is green

$ facts
`,
		},
	};

	// --- state --------------------------------------------------------------

	let editorEl: HTMLDivElement;
	let termEl: HTMLDivElement;
	let monaco: typeof import('monaco-editor') | null = null;
	let editor: import('monaco-editor').editor.IStandaloneCodeEditor | null = null;
	let term: Xterm | null = null;
	let fitAddon: FitAddon | null = null;
	let reform: ReformTerminal | null = null;

	let trace = $state(false);
	// Default to allowing `$` commands at the terminal (CLI `-s`/`--safe` disables).
	let allowDirect = $state(true);
	let selectedExample = $state('hello');
	let source = $state(examples['hello'].content);
	let status = $state('Ready');

	// --- lifecycle ----------------------------------------------------------

	onMount(() => {
		// Monaco is heavy; load it lazily on first mount.
		(async () => {
			const monacoMod = await import('monaco-editor');
			monaco = monacoMod;
			registerReformLanguage(monacoMod);

			const ed = monacoMod.editor.create(editorEl, {
				value: source,
				language: 'reform',
				theme: 'reform-theme',
				automaticLayout: true,
				minimap: { enabled: false },
				fontSize: 13,
				lineNumbers: 'on',
				scrollBeyondLastLine: false,
				tabSize: 4,
			});
			editor = ed;
			ed.onDidChangeModelContent(() => {
				source = ed.getValue();
			});

			// xterm.js terminal.
			const t = new Xterm({
				cursorBlink: true,
				fontSize: 13,
				theme: {
					background: '#1e1e1e',
					foreground: '#d4d4d4',
				},
				convertEol: true,
			});
			term = t;
			const fit = new FitAddon();
			fitAddon = fit;
			t.loadAddon(fit);
			t.open(termEl);
			fit.fit();

			// Wire the engine to the terminal.
			const rt = new ReformTerminal({ terminal: t, trace, allowDirect });
			reform = rt;
			await rt.init();
			rt.reset();

			// Expose internals for testing/debugging.
			(window as any).__reform = { editor: ed, terminal: t, engine: rt };

			// Terminal input -> engine (player prompt or `$` fact).
			t.onData((data) => {
				// Handle backspace and printable characters.
				if (data === '\r') {
					t.write('\r\n');
					const line = currentLine;
					currentLine = '';
					const err = rt.input(line);
					if (err) {
						t.write(`\x1b[31m${err}\x1b[0m\r\n`);
					}
					t.write('> ');
				} else if (data === '\u007f') {
					if (currentLine.length > 0) {
						currentLine = currentLine.slice(0, -1);
						t.write('\b \b');
					}
				} else if (data >= ' ' || data === '\t') {
					currentLine += data;
					t.write(data);
				}
			});
			t.write('Reform REPL — type a prompt, or use $ commands.\r\n> ');

			// Resize the terminal with the window.
			window.addEventListener('resize', onResize);
		})();

		return () => {
			window.removeEventListener('resize', onResize);
			editor?.dispose();
			term?.dispose();
			reform?.dispose();
		};
	});

	let currentLine = '';
	const onResize = () => fitAddon?.fit();

	// --- actions ------------------------------------------------------------

	function run() {
		if (!reform) return;
		reform.reset();
		currentLine = '';
		term?.clear();
		term?.write('Reform REPL — type a prompt, or use $ commands.\r\n');
		const err = reform.load(source);
		if (err) {
			status = `Error: ${err}`;
			term?.write(`\x1b[31m${err}\x1b[0m\r\n`);
		} else {
			status = 'Loaded';
		}
		// Show the prompt after all load output, then hand focus to the
		// terminal so the user can start typing immediately.
		term?.write('> ');
		term?.focus();
	}

	function onExampleChange(value: string | string[]) {
		const v = Array.isArray(value) ? value[0] : value;
		selectedExample = v;
		source = examples[v].content;
		editor?.setValue(source);
	}

	function onTraceChange(on: boolean) {
		trace = on;
		reform?.setTrace(on);
	}

	function onAllowDirectChange(on: boolean) {
		allowDirect = on;
		reform?.setAllowDirect(on);
	}
</script>

<svelte:head>
	<title>Reform Demo</title>
</svelte:head>

<div class="flex h-screen flex-col gap-4 p-4">
	<!-- Toolbar -->
	<Card class="shrink-0">
		<CardHeader class="flex flex-row items-center justify-between gap-4">
			<div class="flex items-center gap-3">
				<CardTitle class="text-lg">Reform Playground</CardTitle>
				<Badge variant="outline">wasm</Badge>
			</div>
			<div class="flex items-center gap-6">
				<div class="flex items-center gap-2">
					<Label for="trace-switch" class="text-sm">Trace</Label>
					<Switch id="trace-switch" bind:checked={trace} onCheckedChange={onTraceChange} />
				</div>
				<div class="flex items-center gap-2">
					<Label for="direct-switch" class="text-sm">Allow $ commands</Label>
					<Switch id="direct-switch" bind:checked={allowDirect} onCheckedChange={onAllowDirectChange} />
				</div>
				<Separator orientation="vertical" class="h-6" />
				<Select type="single" value={selectedExample} onValueChange={onExampleChange}>
					<SelectTrigger class="w-44">
						<SelectPrimitive.Value placeholder="Example" />
					</SelectTrigger>
					<SelectContent>
						{#each Object.entries(examples) as [key, ex]}
							<SelectItem value={key}>{ex.name}</SelectItem>
						{/each}
					</SelectContent>
				</Select>
				<Button onclick={run}>Run</Button>
			</div>
		</CardHeader>
	</Card>

	<!-- Editor + Terminal -->
	<div class="grid min-h-0 flex-1 grid-cols-2 gap-4">
		<Card class="min-h-0">
			<CardHeader class="py-2">
				<CardTitle class="text-sm">Editor</CardTitle>
			</CardHeader>
			<CardContent class="min-h-0 flex-1 p-2">
				<div bind:this={editorEl} class="h-full w-full overflow-hidden rounded-md border"></div>
			</CardContent>
		</Card>
		<Card class="min-h-0">
			<CardHeader class="py-2">
				<CardTitle class="text-sm">Terminal</CardTitle>
			</CardHeader>
			<CardContent class="min-h-0 flex-1 p-2">
				<div bind:this={termEl} class="h-full w-full overflow-hidden rounded-md border bg-[#1e1e1e]"></div>
			</CardContent>
		</Card>
	</div>

	<!-- Status bar -->
	<div class="flex shrink-0 items-center gap-2 text-sm text-muted-foreground">
		<Badge variant="secondary">{status}</Badge>
		<span>Reform {source.length} chars</span>
	</div>
</div>
