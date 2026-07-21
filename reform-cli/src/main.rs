use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use clap::Parser;
use reform::engine::Engine;

/// A Reform rule-engine REPL.
///
/// Each line you type is fed to the engine. By default every line becomes a
/// `prompt` fact (as if it had been written with a `>` prefix in a file). With
/// `-A`, a line that starts with `$` is inserted directly — as a `$`-prefixed
/// rule/command/fact — instead of being turned into a prompt. Direct `$` facts
/// may span multiple lines: after the first `$` line, indented continuation
/// lines are appended, and a blank line (or the next non-indented line)
/// submits the fact. This lets you enter multi-line rules interactively.
#[derive(Parser)]
#[command(name = "reform", version, about)]
struct Cli {
    /// Allow `$`-prefixed lines to be inserted directly instead of as prompts.
    #[arg(short = 'A')]
    allow_direct: bool,

    /// Trace engine activity to stderr: facts added (`+`) / removed (`-`),
    /// rules registered, and rule firings (`fire <name> -> <body>`).
    #[arg(short = 'v', long = "trace")]
    trace: bool,

    /// Reform files to load before starting the REPL.
    files: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();
    let mut engine = Engine::new();
    let trace = cli.trace || std::env::var("REFORM_TRACE").is_ok();
    engine.set_trace(trace);

    for path in &cli.files {
        if let Err(e) = engine.load_file(path) {
            eprintln!("reform: {}: {e:?}", path.display());
            std::process::exit(1);
        }
        if engine.quit() {
            return;
        }
    }

    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut lines = stdin.lock().lines();
    // Buffer for a multi-line `$` direct fact. While it's `Some`, indented
    // lines append to it; a blank line (or a non-indented line) submits it.
    // This lets you enter multi-line rules at the REPL.
    let mut buffer: Option<String> = None;
    let mut pending: Option<String> = None;

    loop {
        // Show the prompt indicator before reading the next line.
        let _ = write!(stdout, "{}", if buffer.is_some() { "… " } else { "> " });
        let _ = stdout.flush();
        let line = if let Some(p) = pending.take() {
            p
        } else {
            match lines.next() {
                Some(Ok(l)) => l,
                _ => break, // input closed
            }
        };
        let is_blank = line.trim().is_empty();
        let is_indented = line.starts_with(' ') || line.starts_with('\t');

        if let Some(buf) = buffer.as_mut() {
            if is_blank {
                // Blank line: submit the buffered `$` fact.
                let src = std::mem::take(buf);
                if let Err(e) = engine.load_str(&src) {
                    eprintln!("{e:?}");
                }
                buffer = None;
            } else if is_indented {
                // Continuation line.
                buf.push('\n');
                buf.push_str(&line);
            } else {
                // A new non-indented line ends the buffered fact; submit it and
                // reprocess this line on the next iteration.
                let src = std::mem::take(buf);
                if let Err(e) = engine.load_str(&src) {
                    eprintln!("{e:?}");
                }
                buffer = None;
                pending = Some(line);
            }
        } else if is_blank {
            // Ignore blank lines outside a buffer.
        } else if cli.allow_direct && line.starts_with('$') {
            // Start buffering a direct `$` fact.
            buffer = Some(line);
        } else {
            // A prompt: player input, processed immediately.
            if let Err(e) = engine.load_str(&format!("> {line}\n")) {
                eprintln!("{e:?}");
            }
        }
        if engine.quit() {
            break;
        }
    }
    // Flush any buffered fact at EOF.
    if let Some(buf) = buffer {
        let _ = engine.load_str(&buf);
    }
}
