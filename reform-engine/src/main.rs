use std::env;
use reform_engine::engine::Engine;
use reform_engine::repl;

fn main() -> anyhow::Result<()> {
    let mut engine = Engine::new();

    // Load any files specified as command-line arguments
    let args: Vec<String> = env::args().skip(1).collect();
    for path in &args {
        if let Err(e) = repl::load_script(&mut engine, path) {
            eprintln!("Error loading '{}': {}", path, e);
            std::process::exit(1);
        }
    }

    // If no files were loaded, show a brief usage hint
    if args.is_empty() {
        println!("Usage: reform-engine [file1 file2 ...]");
        println!("  Load files, then enter interactive REPL.");
        println!();
    }

    repl::run_repl(&mut engine)
}
