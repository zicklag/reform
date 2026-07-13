use std::{
    fs::OpenOptions,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::Error;
use clap::Parser;
use conllu::{UPOS, parsers::ParsedDoc};
use reform::{
    lemmatizer::Lemmatizer,
    tagger::{Tagger, TaggerTrainConfig},
};

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
    Tagger(CliTaggerArgs),
}

/// Run & train the part-of-speech tagger.
#[derive(clap::Args)]
struct CliTaggerArgs {
    #[clap(subcommand)]
    command: CliTaggerCmd,
}
/// Run & train the part-of-speech tagger.
#[derive(clap::Subcommand)]
enum CliTaggerCmd {
    Train(CliTaggerTrainArgs),
    Run(CliTaggerRunArgs),
    Eval(CliTaggerEvalArgs),
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

/// Train the part of speech tagger on one or more conllu wordtrees.
#[derive(clap::Args)]
struct CliTaggerTrainArgs {
    #[arg(long, short = 't', default_value = "./data/tagger.bin")]
    output: PathBuf,

    #[arg(long, short = 'e', default_value = "0.0025")]
    epsilon: f64,

    #[arg(long, short = 'E', default_value = "100")]
    max_epochs: usize,

    /// The list of conllu word tree files to train the tagger from.
    wordtrees: Vec<PathBuf>,
}

/// Run the tagger on a list of words to predict their part of speech.
#[derive(clap::Args)]
struct CliTaggerRunArgs {
    #[arg(long, short = 't', default_value = "./data/tagger.bin")]
    model: PathBuf,

    /// The list of words in the sentence to classify
    words: Vec<String>,
}
/// Evaluate the tagger accuracy against conllu data.
#[derive(clap::Args)]
struct CliTaggerEvalArgs {
    /// The trained tagger model file.
    #[arg(long, short = 't', default_value = "./data/tagger.bin")]
    model: PathBuf,
    /// Show sentences with errors, marking wrong words with *.
    #[arg(long, short = 'e')]
    show_errors: bool,
    /// The conllu word tree files to evaluate against.
    wordtrees: Vec<PathBuf>,
}

/// Build the lemma dictionary file from a set of conllu word trees.
#[derive(clap::Args)]
struct CliLemmatizerBuildArgs {
    /// The file to output the dictionary to.
    #[arg(long, short = 'o', default_value = "./data/lemmas.bin")]
    output: PathBuf,
    /// The list of conllu word tree files to build the lemma dictionary from.
    wordtrees: Vec<PathBuf>,
}

/// Dump the contents of a lemma dictionary file.
#[derive(clap::Args)]
struct CliLemmatizerDumpArgs {
    /// The Dictionary file to dump
    #[arg(long, short = 'l', default_value = "./data/lemmas.bin")]
    dictionary: PathBuf,
}

/// Lemmatize a word given it's part of speech.
#[derive(clap::Args)]
struct CliLemmatizerRunArgs {
    /// The Dictionary file to use
    #[arg(long, short = 'l', default_value = "./data/lemmas.bin")]
    lemmas: PathBuf,
    #[arg(long, short = 't', default_value = "./data/tagger.bin")]
    tagger: PathBuf,
    // The words to lemmatize.
    words: Vec<String>,
}

//
// CLI command implementations.
//

impl CliArgs {
    /// Execute the CLI.
    fn execute(&self) -> anyhow::Result<()> {
        match &self.command {
            CliCommand::Lemmatizer(args) => args.command.execute(),
            CliCommand::Tagger(args) => args.command.execute(),
        }
    }
}

impl CliTaggerCmd {
    fn execute(&self) -> anyhow::Result<()> {
        match self {
            CliTaggerCmd::Train(args) => args.execute(),
            CliTaggerCmd::Run(args) => args.execute(),
            CliTaggerCmd::Eval(args) => args.execute(),
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

impl CliTaggerTrainArgs {
    /// Execute `tagger train`.
    fn execute(&self) -> anyhow::Result<()> {
        // Load the wordtrees
        let wordtrees = load_wordtrees(&self.wordtrees)?;

        // Extract sentences
        let sentences = wordtrees.into_iter().flatten().map(|x| x.into_iter());

        // Train the model
        Tagger::train(
            sentences,
            TaggerTrainConfig {
                epsilon: Some(self.epsilon),
                max_epochs: Some(self.max_epochs),
            },
            &self.output,
        )?;

        Ok(())
    }
}

impl CliTaggerRunArgs {
    /// Execute `tagger train`.
    fn execute(&self) -> anyhow::Result<()> {
        // Load and start the tagger
        let tagger = Tagger::load(&mut OpenOptions::new().read(true).open(&self.model)?)?;
        let ctx = tagger.start()?;

        // Tag the sentence
        let result = ctx.tag_sentence(&self.words)?;

        // Print the results
        for (word, upos) in self.words.iter().zip(result) {
            println!("{word} - {upos}");
        }

        Ok(())
    }
}

impl CliTaggerEvalArgs {
    /// Execute `tagger eval`.
    fn execute(&self) -> anyhow::Result<()> {
        // Load and start the tagger
        let tagger = Tagger::load(&mut OpenOptions::new().read(true).open(&self.model)?)?;
        let ctx = tagger.start()?;

        // Load the wordtrees we will be evaluating against
        let wordtrees = load_wordtrees(&self.wordtrees)?;

        // Count success and total
        let mut total = 0;
        let mut correct = 0;

        for doc in wordtrees {
            for sentence in doc.iter() {
                let sentence = sentence.iter().as_slice();

                // Get the sentence's plain words
                let plain_sentence = sentence.iter().map(|x| &x.form).collect::<Vec<_>>();
                // Get the true parts of speach for the words
                let y_true = sentence
                    .iter()
                    .map(|x| x.upos.unwrap_or(UPOS::X))
                    .collect::<Vec<_>>();

                // Tag the sentence to predict the parts of speech
                let y_pred = ctx.tag_sentence(&plain_sentence)?;

                // Check each predicted part of speech against it's actual part of speech
                for (i, (pred_pos, true_pos)) in y_pred.iter().zip(&y_true).enumerate() {
                    total += 1;
                    if pred_pos == true_pos {
                        correct += 1;
                    } else if self.show_errors {
                        println!(
                            "{}  true: {}  pred: {}",
                            sentence[i].form, true_pos, pred_pos
                        );
                    }
                }
            }
        }

        let accuracy = correct as f64 / total as f64;
        println!("Accuracy: {correct}/{total} = {:.4}%", accuracy * 100.0);

        Ok(())
    }
}

impl CliLemmatizerBuildArgs {
    /// Execute `lemmatizer build`.
    fn execute(&self) -> anyhow::Result<()> {
        // Load the wordtrees
        let wordtrees = load_wordtrees(&self.wordtrees)?;

        // Build a lemmatizer
        let lemmatizer = Lemmatizer::build(&wordtrees);

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
        let lemmatizer = Lemmatizer::load(&mut OpenOptions::new().read(true).open(&self.lemmas)?)?;
        let tagger = Tagger::load(&mut OpenOptions::new().read(true).open(&self.tagger)?)?;
        let tagger_ctx = tagger.start()?;

        let parts_of_speech = tagger_ctx.tag_sentence(&self.words)?;

        for (word, pos) in self.words.iter().zip(parts_of_speech) {
            let lemma = lemmatizer.lemmatize(word, pos);
            println!("{word} {pos} - {lemma}");
        }

        Ok(())
    }
}

//
// Helpers
//

fn load_wordtrees<P: AsRef<Path>>(paths: &[P]) -> anyhow::Result<Vec<ParsedDoc>> {
    paths
        .iter()
        // Open file for each wordtree
        .map(|path| std::fs::File::open(path.as_ref()).map_err(Error::from))
        // Parse the conllu format
        .map(|f| f.and_then(|file| conllu::parse_file(file).map_err(Error::from)))
        // Collect result
        .collect::<anyhow::Result<Vec<_>>>()
}
