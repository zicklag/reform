use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use anyhow::Error;
use clap::Parser;
use conllu::{UPOS, parsers::ParsedDoc};
use crfs::Attribute;
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
    #[arg(short = 'o', default_value = "./data/tagger.bin")]
    output: PathBuf,

    /// The list of conllu word tree files to train the tagger from.
    wordtrees: Vec<PathBuf>,
}

/// Run the tagger on a list of words to predict their part of speech.
#[derive(clap::Args)]
struct CliTaggerRunArgs {
    #[arg(short = 'M', default_value = "./data/tagger.bin")]
    model: PathBuf,

    /// The list of words in the sentence to classify
    words: Vec<String>,
}
/// Evaluate the tagger accuracy against conllu data.
#[derive(clap::Args)]
struct CliTaggerEvalArgs {
    /// The trained tagger model file.
    #[arg(short = 'M', default_value = "./data/tagger.bin")]
    model: PathBuf,
    /// The conllu word tree files to evaluate against.
    wordtrees: Vec<PathBuf>,
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

        // Create a trainer for the pos-tagger
        let mut trainer = crfs::Trainer::averaged_perceptron();
        {
            let params = trainer.params_mut();
            params.set_max_iterations(10)?;
        }
        trainer.verbose(true);

        for doc in wordtrees {
            for sentence in doc.iter() {
                let sentence = sentence.iter().as_slice();
                let (x, y) = extract_features(sentence);
                trainer.append(&x, &y)?;
            }
        }

        trainer.train(&self.output)?;

        Ok(())
    }
}

impl CliTaggerRunArgs {
    /// Execute `tagger train`.
    fn execute(&self) -> anyhow::Result<()> {
        let model_data = std::fs::read(&self.model)?;
        let model = crfs::Model::new(&model_data)?;
        let tagger = model.tagger()?;

        let x = word_features(&self.words);
        let result = tagger.tag(&x)?;

        for (word, upos) in self.words.iter().zip(result) {
            println!("{word} - {upos}");
        }

        Ok(())
    }
}

impl CliTaggerEvalArgs {
    /// Execute `tagger eval`.
    fn execute(&self) -> anyhow::Result<()> {
        let model_data = std::fs::read(&self.model)?;
        let model = crfs::Model::new(&model_data)?;
        let tagger = model.tagger()?;

        let wordtrees = load_wordtrees(&self.wordtrees)?;

        let mut total = 0usize;
        let mut correct = 0usize;

        for doc in wordtrees {
            for sentence in doc.iter() {
                let sentence = sentence.iter().as_slice();
                let (x, y_true) = extract_features(sentence);

                let y_pred = tagger.tag(&x)?;

                for (pred, true_) in y_pred.iter().zip(&y_true) {
                    total += 1;
                    if pred == true_ {
                        correct += 1;
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
        let mut dictionary_file = std::fs::OpenOptions::new()
            .read(true)
            .open(&self.dictionary)?;

        let lemmatizer = Lemmatizer::load(&mut dictionary_file)?;

        let lemma = lemmatizer.lemmatize(&self.word, self.part_of_speech);

        println!("{lemma}");

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

/// Get the suffix of a string.
fn suffix(s: &str, n: usize) -> &str {
    if let Some((i, _char)) = s.char_indices().rev().nth(n - 1) {
        &s[i..]
    } else {
        s
    }
}

/// Build CRF features from a raw sentence.
fn word_features<S: AsRef<str>>(words: &[S]) -> Vec<Vec<Attribute>> {
    let len = words.len();
    let mut x = Vec::with_capacity(len);

    for (i, word) in words.iter().enumerate() {
        let form = word.as_ref();
        let lower = form.to_lowercase();
        let suffix1 = suffix(&lower, 1);
        let suffix2 = suffix(&lower, 2);
        let suffix3 = suffix(&lower, 3);
        let suffix4 = suffix(&lower, 4);

        let mut attrs = vec![
            Attribute::new(format!("form={form}"), 1.0),
            Attribute::new(format!("lowerform={lower}"), 1.0),
            Attribute::new(format!("suffix1={suffix1}"), 1.0),
            Attribute::new(format!("suffix2={suffix2}"), 1.0),
            Attribute::new(format!("suffix3={suffix3}"), 1.0),
            Attribute::new(format!("suffix4={suffix4}"), 1.0),
            Attribute::new("len", form.len() as f64),
            Attribute::new("pos", i as f64 / len as f64),
        ];

        if i == 0 {
            attrs.push(Attribute::new("first", 1.0));
        } else if i == len - 1 {
            attrs.push(Attribute::new("last", 1.0));
        }

        if form.chars().all(|x| x.is_uppercase()) {
            attrs.push(Attribute::new("uppercase", 1.0));
        } else if form
            .chars()
            .next()
            .map(|x| x.is_uppercase())
            .unwrap_or(false)
        {
            attrs.push(Attribute::new("capitalized", 1.0));
        } else {
            attrs.push(Attribute::new("lowercase", 1.0));
        }

        static POS: &[isize] = &[-3, -2, -1, 1, 2, 3];

        for p in POS {
            let n = i as isize + p;
            if n >= 0 && n < len as isize {
                let n = n as usize;
                attrs.push(Attribute::new(
                    format!("relative{n}={}", words[n].as_ref()),
                    1.0,
                ));
            }
        }

        x.push(attrs);
    }

    x
}

/// Extract CRF features and labels from a sentence of conllu tokens.
fn extract_features(sentence: &[conllu::Token]) -> (Vec<Vec<Attribute>>, Vec<String>) {
    let words: Vec<&str> = sentence.iter().map(|t| t.form.as_str()).collect();
    let x = word_features(&words);
    let y: Vec<String> = sentence
        .iter()
        .map(|t| t.upos.unwrap_or(UPOS::X).to_string())
        .collect();
    (x, y)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn str_suffix_helper() {
        let s1 = "testing";
        assert_eq!("ting", suffix(s1, 4));
        assert_eq!("ing", suffix(s1, 3));
        assert_eq!("ng", suffix(s1, 2));
        assert_eq!("g", suffix(s1, 1));
    }
}
