use clap::Parser;
use reform_engine::engine::Engine;
use reform_engine::repl;

#[derive(Parser)]
#[command(name = "reform-engine", about = "A minimal reflective rule engine")]
struct Args {
    /// Script files to load before entering the REPL
    files: Vec<String>,

    /// Prepend `> ` to all input lines, turning them into prompt(...) facts
    #[arg(short = 'p')]
    prompt_mode: bool,

    /// Show auto-run firing counts
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Allow commands (facts, run, etc.) in prompt mode instead of treating all input as prompts
    #[arg(short = 'A', long = "allow-commands")]
    allow_commands: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut engine = Engine::new();

    // Load any files specified as command-line arguments
    for path in &args.files {
        if let Err(e) = repl::load_script(&mut engine, path) {
            eprintln!("Error loading '{}': {}", path, e);
            std::process::exit(1);
        }
    }

    if args.files.is_empty() {
        println!("Usage: reform-engine [file1 file2 ...]");
        println!("  Load files, then enter interactive REPL.");
        println!();
        repl::run_repl_full(&mut engine, true, args.prompt_mode, args.verbose, args.allow_commands)
    } else {
        repl::run_repl_full(&mut engine, false, args.prompt_mode, args.verbose, args.allow_commands)
    }
}
