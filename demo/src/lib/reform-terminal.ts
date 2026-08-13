import type { Terminal } from '@xterm/xterm';
import init, { ReformEngine } from '$lib/reform-wasm/reform_wasm.js';

export interface EngineOptions {
  /** Enable trace logging (engine `--trace`). */
  trace: boolean;
  /** Allow `$`-prefixed lines typed at the terminal as direct facts (CLI `-A`). */
  allowDirect: boolean;
  /** Terminal to render engine stdout/stderr output into. */
  terminal: Terminal;
}

/**
 * A thin wrapper around the wasm ReformEngine that wires its output sinks to
 * an xterm.js terminal and forwards terminal input back into the engine,
 * mirroring the CLI REPL.
 */
export class ReformTerminal {
  private engine: ReformEngine | null = null;
  private terminal: Terminal;
  private trace: boolean;
  private allowDirect: boolean;

  constructor(options: EngineOptions) {
    this.terminal = options.terminal;
    this.trace = options.trace;
    this.allowDirect = options.allowDirect;
  }

  /** Load the wasm module (idempotent). */
  async init(): Promise<void> {
    await init();
  }

  /** Create a fresh engine and wire its output to the terminal. */
  reset(): void {
    this.engine?.free();
    this.engine = new ReformEngine();
    this.engine.set_trace(this.trace);
    this.engine.set_allow_direct(this.allowDirect);
    this.engine.set_output(
      (s: string) => this.terminal.write(s),
      (s: string) => this.terminal.write(s),
    );
  }

  /** Load a reform source string into the engine. */
  load(src: string): string | null {
    if (!this.engine) this.reset();
    try {
      this.engine!.load(src);
      return null;
    } catch (e) {
      return String(e);
    }
  }

  /** Toggle trace logging on the live engine. */
  setTrace(on: boolean): void {
    this.trace = on;
    this.engine?.set_trace(on);
  }

  /** Toggle whether `$`-prefixed terminal lines are direct facts. */
  setAllowDirect(on: boolean): void {
    this.allowDirect = on;
    this.engine?.set_allow_direct(on);
  }

  /** Feed one line of terminal input to the engine (a player prompt or `$` fact). */
  input(line: string): string | null {
    if (!this.engine) this.reset();
    try {
      this.engine!.input(line);
      return null;
    } catch (e) {
      return String(e);
    }
  }

  /** Whether the engine has quit. */
  get quit(): boolean {
    return this.engine?.quit() ?? false;
  }

  /** The current facts as an array of normal-form strings. */
  facts(): string[] {
    if (!this.engine) return [];
    return Array.from(this.engine.facts() as unknown as string[]);
  }

  /** Release the wasm engine. */
  dispose(): void {
    this.engine?.free();
    this.engine = null;
  }
}
