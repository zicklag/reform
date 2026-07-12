use std::{
    fs::OpenOptions,
    io::{Read, Write},
    path::Path,
    sync::Arc,
};

use conllu::UPOS;
use crfs::{Attribute, Model};
use flate2::{
    Compression,
    write::{GzDecoder, GzEncoder},
};
use yoke::Yoke;

/// A part-of-speech tagger
pub struct Tagger {
    model: Yoke<YokableModel<'static>, Arc<[u8]>>,
}

pub struct TaggerCtx<'a>(crfs::Tagger<'a>);

/// Helper to make [`Model`] "yokable".
#[derive(yoke::Yokeable, derive_more::Deref)]
struct YokableModel<'a>(Model<'a>);

/// Configuration for traing a [`Tagger`].
#[derive(Default)]
pub struct TaggerTrainConfig {
    /// Once the loss gets below this level training will automatically stop.
    pub epsilon: Option<f64>,
    /// Training will stop automatically after reaching the max_epochs.
    pub max_epochs: Option<usize>,
}

impl Tagger {
    /// Create a new tagger by providing it sentences of [`conllu::Token`]s.
    ///
    /// This will write a model file to the provided output path, which can be
    /// loaded into a new tagger.
    pub fn train<
        Sentences: Iterator<Item = Sentence>,
        Sentence: AsRef<[conllu::Token]>,
        P: AsRef<Path>,
    >(
        sentences: Sentences,
        config: TaggerTrainConfig,
        out_model_file: P,
    ) -> std::io::Result<()> {
        // Create a trainer for the pos-tagger
        let mut trainer = crfs::Trainer::averaged_perceptron();
        {
            let params = trainer.params_mut();
            params.set_shuffle_seed(Some(42));
            if let Some(epsilon) = config.epsilon {
                params.set_epsilon(epsilon)?;
            }
            if let Some(max_epochs) = config.max_epochs {
                params.set_max_iterations(max_epochs)?;
            }
        }
        trainer.verbose(true);

        for sentence in sentences {
            let (x, y) = conllu_to_training_pair(sentence.as_ref());
            trainer.append(&x, &y)?;
        }

        // Train the model
        trainer.train(out_model_file.as_ref())?;

        // Load the trained model and gzip it
        let output_file_data = std::fs::read(out_model_file.as_ref())?;
        let mut gze = GzEncoder::new(
            OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(out_model_file)?,
            Compression::best(),
        );
        gze.write_all(&output_file_data)?;

        Ok(())
    }

    /// Load a tagger from a trained tagger model file.
    pub fn load<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        let buf = Vec::new();
        let mut gzd = GzDecoder::new(buf);
        std::io::copy(reader, &mut gzd)?;
        let buf = gzd.finish()?;
        let buf: Arc<[u8]> = buf.into();

        let model = Yoke::try_attach_to_cart(buf, |buf| {
            Ok::<_, std::io::Error>(YokableModel(Model::new(buf)?))
        })?;

        Ok(Tagger { model })
    }

    /// Save the tagger's model to a file.
    pub fn save<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writer.write_all(self.model.backing_cart())
    }

    /// Start the tagger, returning a context that can be used to tag sentences.
    pub fn start(&self) -> std::io::Result<TaggerCtx<'_>> {
        Ok(TaggerCtx(self.model.get().tagger()?))
    }
}

impl<'a> TaggerCtx<'a> {
    /// Tag the parts of speech of each word in the provided sentence.
    pub fn tag_sentence<S: AsRef<str>>(&self, sentence: &[S]) -> std::io::Result<Vec<UPOS>> {
        let x = sentence_features(sentence);
        let y = self.0.tag(&x)?;

        Ok(y.into_iter()
            .map(|x| x.parse().unwrap_or(UPOS::X))
            .collect())
    }
}

/// Build CRF features from a raw sentence.
pub fn sentence_features<S: AsRef<str>>(words: &[S]) -> Vec<Vec<Attribute>> {
    let len = words.len();
    let mut x = Vec::with_capacity(len);

    for (i, word) in words.iter().enumerate() {
        let word = word.as_ref();
        let mut attrs = vec![
            Attribute::new(format!("form={}", word), 1.0),
            Attribute::new(format!("form.lowercase={}", word.to_lowercase()), 1.0),
            Attribute::new(format!("suffix1={}", suffix(word, 1)), 1.0),
            Attribute::new(format!("suffix2={}", suffix(word, 2)), 1.0),
            Attribute::new(format!("suffix3={}", suffix(word, 3)), 1.0),
            Attribute::new(format!("suffix4={}", suffix(word, 4)), 1.0),
            Attribute::new(format!("prefix3={}", prefix(word, 3)), 1.0),
            Attribute::new(format!("prefix2={}", prefix(word, 2)), 1.0),
        ];

        if word.chars().all(|x| x.is_uppercase()) {
            attrs.push(Attribute::new("uppercase", 1.0));
        } else if word
            .chars()
            .next()
            .map(|x| x.is_uppercase())
            .unwrap_or(false)
        {
            attrs.push(Attribute::new("capitalized", 1.0));
        } else {
            attrs.push(Attribute::new("lowercase", 1.0));
        }

        // Thes haven't improved accuracy in tests yet, so we comment them out
        // for now.
        //
        // if word.chars().any(|x| x.is_ascii_digit()) {
        //     attrs.push(Attribute::new("has-digit", 1.0));
        // }
        // if word.chars().any(|x| x.is_ascii_punctuation()) {
        //     attrs.push(Attribute::new("has-punctuation", 1.0));
        // }

        static POS: &[isize] = &[-1, 1];

        for p in POS {
            let n = i as isize + p;
            if n >= 0 && n < len as isize {
                let n = n as usize;
                attrs.push(Attribute::new(
                    format!("relative.{n}={}", words[n].as_ref()),
                    1.0,
                ));
            }
        }

        x.push(attrs);
    }

    x
}

/// Extract CRF features and labels from a sentence of conllu tokens.
fn conllu_to_training_pair(sentence: &[conllu::Token]) -> (Vec<Vec<Attribute>>, Vec<String>) {
    let words: Vec<&str> = sentence.iter().map(|t| t.form.as_str()).collect();
    let x = sentence_features(&words);
    let y: Vec<String> = sentence
        .iter()
        .map(|t| t.upos.unwrap_or(UPOS::X).to_string())
        .collect();
    (x, y)
}

/// Get the suffix of a string.
fn suffix(s: &str, n: usize) -> &str {
    if let Some((i, _char)) = s.char_indices().rev().nth(n - 1) {
        &s[i..]
    } else {
        s
    }
}

/// Get the prefix of a string.
fn prefix(s: &str, n: usize) -> &str {
    if let Some((i, _char)) = s.char_indices().nth(n) {
        &s[..i]
    } else {
        s
    }
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

    #[test]
    fn str_prefix_helper() {
        let s1 = "testing";
        assert_eq!("test", prefix(s1, 4));
        assert_eq!("tes", prefix(s1, 3));
        assert_eq!("te", prefix(s1, 2));
        assert_eq!("t", prefix(s1, 1));
    }
}
