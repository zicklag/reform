use clap::Parser;
use reform_engine::engine::Engine;
use reform_engine::repl;

#[derive(Parser)]
#[command(name = "reform-engine", about = "A minimal reflective rule engine")]
struct Args {
    /// Script files to load before entering the REPL
    files: Vec<String>,

    /// Use prompt(...) facts instead of sentence(...) for unrecognized input
    #[arg(short = 'p')]
    prompt_mode: bool,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut engine = Engine::new();

    // Load any files specified as command-line arguments
    for path in &args.files {
        if let Err(e) = repl::load_script_with_mode(&mut engine, path, args.prompt_mode) {
            eprintln!("Error loading '{}': {}", path, e);
            std::process::exit(1);
        }
    }

    // If no files were loaded, show a brief usage hint
    if args.files.is_empty() {
        println!("Usage: reform-engine [file1 file2 ...]");
        println!("  Load files, then enter interactive REPL.");
        println!();
        repl::run_repl(&mut engine, args.prompt_mode)
    } else {
        repl::run_repl_quiet(&mut engine, args.prompt_mode)
    }
}
