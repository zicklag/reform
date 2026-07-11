use std::{path::PathBuf, sync::LazyLock};

use anyhow::Error;
use clap::Parser;
use reform::lemmatizer::Lemmatizer;

/// The parsed CLI arguments.
static ARGS: LazyLock<CliArgs> = LazyLock::new(CliArgs::parse);

/// Parse the arugments and execute the CLI
fn main() {
    if let Err(e) = ARGS.execute() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}

//
// CLI Arg definitions
//

/// Main CLI interface.
#[derive(clap::Parser)]
struct CliArgs {
    #[clap(subcommand)]
    command: CliCommand,
}

#[derive(clap::Subcommand)]
enum CliCommand {
    Lemmatizer(CliLemmatizerArgs),
}

/// Lemmatize words or build / inspect the lemmatizer dictionary.
#[derive(clap::Args)]
struct CliLemmatizerArgs {
    #[clap(subcommand)]
    command: CliLemmatizerCmd,
}

#[derive(clap::Subcommand)]
enum CliLemmatizerCmd {
    Build(CliLemmatizerBuildArgs),
    Dump(CliLemmatizerDumpArgs),
    Run(CliLemmatizerRunArgs),
}

/// Build the lemma dictionary file from a set of conllu word trees.
#[derive(clap::Args)]
struct CliLemmatizerBuildArgs {
    /// The file to output the dictionary to.
    #[arg(short = 'o', default_value = "./data/lemmas.bin")]
    output: PathBuf,
    /// The list of conllu word tree files to build the lemma dictionary from.
    wordtrees: Vec<PathBuf>,
}

/// Dump the contents of a lemma dictionary file.
#[derive(clap::Args)]
struct CliLemmatizerDumpArgs {
    /// The Dictionary file to dump
    #[arg(short = 'D', default_value = "./data/lemmas.bin")]
    dictionary: PathBuf,
}

/// Lemmatize a word given it's part of speech.
#[derive(clap::Args)]
struct CliLemmatizerRunArgs {
    /// The Dictionary file to use
    #[arg(short = 'D', default_value = "./data/lemmas.bin")]
    dictionary: PathBuf,
    /// The part of speech of the word to lemmatize
    part_of_speech: conllu::UPOS,
    /// The word to lemmatize
    word: String,
}

//
// CLI command implementations.
//

impl CliArgs {
    /// Execute the CLI.
    fn execute(&self) -> anyhow::Result<()> {
        match &self.command {
            CliCommand::Lemmatizer(args) => args.command.execute(),
        }
    }
}

impl CliLemmatizerCmd {
    /// Execute `lemmatizer`.
    fn execute(&self) -> anyhow::Result<()> {
        match self {
            CliLemmatizerCmd::Build(args) => args.execute(),
            CliLemmatizerCmd::Dump(args) => args.execute(),
            CliLemmatizerCmd::Run(args) => args.execute(),
        }
    }
}

impl CliLemmatizerBuildArgs {
    /// Execute `lemmatizer build`.
    fn execute(&self) -> anyhow::Result<()> {
        let docs = self
            .wordtrees
            .iter()
            // Open file for each wordtree
            .map(|path| std::fs::File::open(path).map_err(Error::from))
            // Parse the conllu format
            .map(|f| f.and_then(|file| conllu::parse_file(file).map_err(Error::from)))
            // Collect result
            .collect::<anyhow::Result<Vec<_>>>()?;

        // Build a lemmatizer
        let lemmatizer = Lemmatizer::build(&docs);

        // Open the dictionary file
        let mut outfile = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&self.output)?;

        // Save the lemmatizer's dictionary to the out file
        lemmatizer.save(&mut outfile)?;

        Ok(())
    }
}

impl CliLemmatizerDumpArgs {
    /// Execute `lemmatizer dump`.
    fn execute(&self) -> anyhow::Result<()> {
        let mut dictionary_file = std::fs::OpenOptions::new()
            .read(true)
            .open(&self.dictionary)?;

        let lemmatizer = Lemmatizer::load(&mut dictionary_file)?;

        for (word, upos, lemma) in lemmatizer.iter() {
            let upos = upos.to_string();
            println!("{upos:5}: {word:40}{lemma}");
        }

        Ok(())
    }
}

impl CliLemmatizerRunArgs {
    /// Execute `lemmatizer run`.
    fn execute(&self) -> anyhow::Result<()> {
        let mut dictionary_file = std::fs::OpenOptions::new()
            .read(true)
            .open(&self.dictionary)?;

        let lemmatizer = Lemmatizer::load(&mut dictionary_file)?;

        let lemma = lemmatizer.lemmatize(&self.word, self.part_of_speech);

        println!("{lemma}");

        Ok(())
    }
}
