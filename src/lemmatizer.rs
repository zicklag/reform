use std::{
    borrow::Cow,
    collections::HashMap,
    io::{self, Read, Write},
    sync::Arc,
};

use conllu::{UPOS, parsers::ParsedDoc};
use flate2::write::{GzDecoder, GzEncoder};
use rkyv::{collections::swiss_table::ArchivedHashMap, rancor, string::ArchivedString};
use yoke::Yoke;

/// Raw container for the lemmatizer dictionary.
type ArchivedStringMap = ArchivedHashMap<ArchivedString, ArchivedString>;

/// A dictionary containing an archived string map and it's backing data store.
///
/// They are yoked together because the archived string map needs to hold a
/// reference to it's backing data store. Yoke allows us to create a
/// self-referential struct holding both.
type Dictionary = Yoke<&'static ArchivedStringMap, Arc<Vec<u8>>>;

/// A lemmatizer that can return the base form or "lemma" of any word.
///
/// It does this by dictionary lookup first, and by simple transformation rules
/// based on part-of-speech if a word is not found in the dictionary.
pub struct Lemmatizer {
    /// The dictionary containing the lemmas.
    ///
    /// The keys in the dictionary are `{word}|{universal part of speech tag}`
    /// and the value is the lemma.
    dictionary: Dictionary,
}

impl Lemmatizer {
    /// Build a new lemmatizer from a collection of parsed conllu files.
    pub fn build(docs: &[ParsedDoc]) -> Self {
        static EXCLUDE: &[&str] = &["www", "http"];

        // Create a temporary hashmap to store the lemmas
        let mut entries = HashMap::with_capacity(
            // With capacity enough to store every word across all the docs
            docs.iter()
                .fold(0, |a, x| a + x.iter().fold(a, |a, x| a + x.iter().len())),
        );

        // Loop through each token and insert it into the map
        for doc in docs {
            for sentence in doc.iter() {
                'token: for token in sentence.iter() {
                    let form = &token.form;

                    // Skip the token if it matches an exclude pattern
                    for ex in EXCLUDE {
                        if form.find(ex).is_some() {
                            continue 'token;
                        }
                    }

                    let upos = &token.upos.unwrap_or(conllu::UPOS::X);
                    let lemma = token.lemma.as_deref().unwrap_or("_");
                    entries.insert(Self::format_lemma_dict_key(form, *upos), lemma.to_string());
                }
            }
        }

        // Encode the map to bytes using rkyv
        let bytes = Arc::new(
            rkyv::to_bytes::<rancor::Error>(&entries)
                .expect("Encode to rkyv bytes")
                .to_vec(),
        );

        // Create a dictionary from the byte buffer and the rkyv view.
        let dictionary = Yoke::attach_to_cart(bytes, |bytes| {
            rkyv::access::<_, rancor::Error>(bytes).expect("Decode rkyv bytes")
        });

        Self { dictionary }
    }

    /// Get the lemma of a word from the dictionary.
    ///
    /// This does not do any automatic transformation of the word and returns
    /// [`None`] if the word is not found in the dictionary.
    ///
    /// Use [`get_lemma()`][Self::get_lemma] if you want auto lemmatization of
    /// out-of-vocabulary words.
    pub fn get_lemma_from_dict(&self, word: &str, upos: conllu::UPOS) -> Option<&ArchivedString> {
        self.dictionary
            .get()
            .get(Self::format_lemma_dict_key(word, upos).as_str())
    }

    /// Get the lemma of a word using auto-lemmatization for out-of-vocabulary words.
    ///
    /// If you would like to avoid auto-lemmatization, you can use
    /// [`get_lemma_from_dict()`][Self::get_lemma_from_dict].
    pub fn lemmatize(&self, word: &str, upos: conllu::UPOS) -> Cow<'_, str> {
        if let Some(lemma) = self.get_lemma_from_dict(word, upos) {
            Cow::Borrowed(lemma.as_str())
        } else {
            Cow::Owned(Self::auto_lemmatize(word, upos).to_owned())
        }
    }

    /// Get an iterator over every item in the dictionary as `(word, upos, lemma)` tuples.
    pub fn iter(&self) -> impl Iterator<Item = (&str, UPOS, &str)> {
        self.dictionary.get().iter().filter_map(|(k, v)| {
            let (word, upos) = Self::extract_word_upos_from_lemma_dict_key(k)?;
            let lemma = v.as_str();
            Some((word, upos, lemma))
        })
    }

    /// Automatically do a best-effort lemmatization the word based on it's part of speech.
    ///
    /// This is currently implemented by simply stripping common suffixes
    /// associated to different parts of speech.
    pub fn auto_lemmatize(word: &str, upos: conllu::UPOS) -> String {
        // Helper to strip any of a list of suffixes from a word
        fn strip_suffixs<'a>(word: &'a str, suffixes: &[&str]) -> &'a str {
            for suffix in suffixes {
                if let Some(word) = word.strip_suffix(suffix) {
                    return word;
                }
            }
            word
        }

        // Check if the word is capitalized
        let capitalized = word
            .chars()
            .next()
            .map(|x| x.is_uppercase())
            .unwrap_or(false);

        // If the word is capitalized assume a proper noun and just return it
        if capitalized {
            return word.to_owned();
        }

        // Strip suffix depending on part of speech
        match upos {
            UPOS::NOUN => strip_suffixs(word, &["es", "s"]),
            UPOS::VERB => strip_suffixs(word, &["ing", "ed", "s"]),
            UPOS::ADJ => strip_suffixs(word, &["er", "est"]),
            _ => word,
        }
        // Make it lowercase to normalize
        .to_lowercase()
    }

    /// Save the lemmatizer dictionary to a writer.
    pub fn save<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        // Create a gzip encodeer going to the writer.
        let mut gze = GzEncoder::new(writer, flate2::Compression::best());
        // Write our dictionary's raw bytes to the gz encoder.
        gze.write_all(self.dictionary.backing_cart().as_slice())?;
        // Finish the gz encoding.
        gze.finish()?;
        Ok(())
    }

    /// Load the lematizer from a dictionary.
    pub fn load<R: Read>(reader: &mut R) -> std::io::Result<Self> {
        // Create a gz decoder over an in-memory buffer
        let mut gzd = GzDecoder::new(Vec::new());
        // Pipe the reader into the gz encoder
        io::copy(reader, &mut gzd)?;
        // Extract the decoded buffer and wrap it in an Arc
        let bytes = Arc::new(gzd.finish()?);

        // Crate the dictionary by creating an rkyv view over the extracted bytes.
        let dictionary =
            Yoke::try_attach_to_cart(bytes, |bytes| rkyv::access::<_, rancor::Error>(bytes))
                .map_err(std::io::Error::other)?;

        Ok(Self { dictionary })
    }

    /// Format the given word form and universal part of speech ( UPOS ) to a key
    /// into the lemma dictionary.
    fn format_lemma_dict_key(word: &str, upos: conllu::UPOS) -> String {
        format!("{word}|{upos:?}")
    }

    /// Extract the word and part-of-speech from a lemma dictionary key.
    fn extract_word_upos_from_lemma_dict_key(key: &str) -> Option<(&str, UPOS)> {
        let (word, upos) = key.split_once('|')?;
        let upos: UPOS = upos.parse().ok()?;
        Some((word, upos))
    }
}
