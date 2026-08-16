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
		'rule': {
			name: 'Rules',
			content: `# Rules are how you implement logic in Reform.
#
# A rule has a name, a pattern, and a body. When the pattern matches existing
# facts, the engine fires the rule: it deletes facts marked with \`-\` and
# creates the facts in the body. The \`-\` on the prompt makes sure the rule
# doesn't keep firing on the same input.

$ rule (say hello when user says hi)
    (
        - prompt hi
    )
    (
        println Hello!
    )

# Try it: type \`hi\` in the terminal.
# > hi
`,
		},
		'name': {
			name: 'Placeholders',
			content: `# Patterns can capture arguments with \`$name\` placeholders.
#
# The placeholder matches any single argument, and the same placeholder can be
# reused in the body to substitute the captured value. Here we don't know the
# user's name ahead of time, so we capture it and echo it back.

$ rule (say hello to user by name)
    (
        - prompt my name is $name
    )
    (
        println (Hello ) $name !
    )

# Try it: type your name in the terminal.
# > my name is Sheri
`,
		},
		'rooms': {
			name: 'Simple Rooms',
			content: `# A tiny interactive game: walk around a map of rooms.
#
# Facts describe the world (where the player is, room descriptions, exits).
# Rules react to \`prompt\` facts from the terminal. Notice how rule priority
# works out automatically: the specific \`go north\` rule fires before the
# generic \`fail to go north\` rule, which fires before the catch-all apology.

# Set the room the player is in
$ player is in living-room

# Living room
$ description of living-room is
  (A cozy room with a nice sofa.)

# Kitchen
$ description of kitchen is
  (The place where we cook the food.)

$ kitchen is north of living-room

# Bedroom
$ description of bedroom is
  (A nice room with your bed in it.)

$ bedroom is east of kitchen

$ rule (parse the "look" command)
  (
    - prompt look
    player is in $room
    description of $room is $description
  )
  (
    println (You are in the ) $room .
    println
    println $description
  )

$ rule (apologize for not understanding prompt)
  (
    - prompt $( $arg )+
  )
  (
    println (I'm sorry, I didn't understand your command:) $( ( ) $arg )+
  )

$ rule (go north)
  (
    - prompt north
    - player is in $here
    $there is north of $here
  )
  (
    player is in $there
    prompt look
  )

$ rule (fail to go north)
  (
    - prompt north
  )
  (
    println (You can't go that way.)
  )

$ rule (go south)
  (
    - prompt south
    - player is in $here
    $here is north of $there
  )
  (
    player is in $there
    prompt look
  )

$ rule (fail to go south)
  (
    - prompt south
  )
  (
    println (You can't go that way.)
  )

$ rule (go east)
  (
    - prompt east
    - player is in $here
    $there is east of $here
  )
  (
    player is in $there
    prompt look
  )

$ rule (fail to go east)
  (
    - prompt east
  )
  (
    println (You can't go that way.)
  )

$ rule (go west)
  (
    - prompt west
    - player is in $here
    $here is east of $there
  )
  (
    player is in $there
    prompt look
  )

$ rule (fail to go west)
  (
    - prompt west
  )
  (
    println (You can't go that way.)
  )

# Try it: type \`look\`, \`north\`, \`east\`, etc. in the terminal.
# > look
# > north
`,
		},
		'math': {
			name: 'Math',
			content: `# Arithmetic is built in via the \`@eval\` fact.
#
# \`@eval\` evaluates the single argument that follows it as a math expression
# and substitutes the result immediately. Expressions with multiple words must
# be wrapped in parentheses. Values are always f64, so division doesn't truncate.

$ the final result is @eval (2 + 2 * 3)
$ half of 7 is @eval (7 / 2)
$ a power is @eval (2 ^ 10)
$ a square root is @eval (sqrt(144))
$ a rounded value is @eval (round(3.7))
$ pi is about @eval (pi)

# \`random(n)\` draws a value in [0, n). Combine with floor for a die roll.
$ die roll @eval (1 + floor(random(6)))

$ facts
`,
		},
		'plain': {
			name: 'Plain Language',
			content: `# Reform lets you invent your own syntax with rules.
#
# Lines typed without a \`$\` prefix become \`parse\` facts. Rules can match
# those \`parse\` facts and rewrite them into a world model. This is how you
# build a custom "plain language" parser entirely in Reform.

$ rule (parse a room)
    (
        - parse $( the )? $name is a room
    )
    (
        room $name
    )

$ rule (parse a description)
    (
        - parse the description of $( the )? $obj is $text
    )
    (
        desc $obj $text
    )

$ rule (parse an exit)
    (
        - parse $( the )? $a is $dir of $( the )? $b
    )
    (
        exit $b $dir $a
    )

$ rule (parse the player is in)
    (
        - parse the player is in $( the )? $room
    )
    (
        player is in $room
    )

$ rule (parse a look command)
    (
        - parse look
    )
    (
        prompt look
    )

$ rule (look at the room)
    (
        - prompt look
        player is in $room
        desc $room $text
    )
    (
        $ println (You are in the ) $room .
        $ println $text
    )

$ rule (go somewhere)
    (
        - prompt go $dir
        - player is in $here
        exit $here $dir $there
    )
    (
        player is in $there
        prompt look
    )

$ rule (fail to go)
    (
        - prompt go $dir
    )
    (
        $ println (You can't go that way.)
    )

# The world, written in plain English:
the Kitchen is a room
the description of the Kitchen is (A cozy room with a stove.)
the Bedroom is a room
the description of the Bedroom is (A quiet room with a bed.)
the Bedroom is east of the Kitchen
the player is in the Kitchen

# Try it: type \`look\` or \`go east\` in the terminal.
# > look
# > go east
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
	let selectedExample = $state('rooms');
	let source = $state(examples['rooms'].content);
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
			// Resize the terminal with the window.
			window.addEventListener('resize', onResize);

			// Run the initial example so the console is ready for input.
			run();
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
		run();
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
