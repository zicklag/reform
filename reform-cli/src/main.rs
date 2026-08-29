use std::io::{self, BufRead, Write};
use std::path::PathBuf;

use argh::FromArgs;
use reform::engine::Engine;

/// A Reform rule-engine REPL.
#[derive(FromArgs)]
#[argh(name = "reform")]
struct Cli {
    /// Disallow `$`-prefixed lines as direct facts/commands; treat them as
    /// player prompts instead.
    #[argh(
        switch,
        short = 's',
        long = "safe",
        description = "don't parse $ statements as literal facts"
    )]
    safe: bool,

    /// Trace engine activity to stderr via the `tracing` ecosystem: rules
    /// registered (with computed specificity), rule firings (`fire <name> <-
    /// <matched facts>`) with the facts each firing added/removed indented
    /// beneath it, and file loads / command effects attributed.
    #[argh(
        switch,
        short = 't',
        long = "trace",
        description = "trace engine activity"
    )]
    trace: bool,

    /// Print version and exit.
    #[argh(switch, long = "version", description = "print version and exit")]
    version: bool,

    /// Seed the `random(n)` stream to make `@eval` output deterministic for
    /// the life of the process.
    #[argh(
        option,
        short = 'r',
        long = "seed",
        description = "deterministic random seed"
    )]
    seed: Option<u64>,

    /// Reform files to load before starting the REPL.
    #[argh(positional, description = "reform files to load")]
    files: Vec<PathBuf>,
}

fn main() {
    let cli: Cli = argh::from_env();
    if cli.version {
        println!("reform {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let mut engine = match cli.seed {
        Some(seed) => Engine::new_with_seed(seed),
        None => Engine::new(),
    };
    if cli.trace || std::env::var("REFORM_TRACE").is_ok() {
        let _ = tracing::subscriber::set_global_default(reform::trace::TraceFormat::stderr());
    }

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
        } else if !cli.safe && line.starts_with('$') {
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
