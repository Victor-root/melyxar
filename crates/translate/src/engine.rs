//! The model that translates, run in this process.

use std::path::Path;

use ct2rs::{Config, TranslationOptions, Translator};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the translation model could not be opened: {0}")]
    Opening(String),
    #[error("a translation failed: {0}")]
    Translating(String),
    #[error("the model answered {answered} lines to {asked}")]
    Mismatch { asked: usize, answered: usize },
}

/// A translation model, loaded. Loading takes a moment and the memory of the
/// model, so it is opened once for a whole video rather than for each line.
pub struct Engine {
    translator: Translator<ct2rs::tokenizers::auto::Tokenizer>,
}

impl Engine {
    /// Opens the model in `folder`, working on `threads` processor threads.
    pub fn open(folder: &Path, threads: usize) -> Result<Self, Error> {
        let config = Config { num_threads_per_replica: threads.max(1), ..Config::default() };
        let translator =
            Translator::new(folder, &config).map_err(|error| Error::Opening(error.to_string()))?;
        Ok(Self { translator })
    }

    /// The translation of each line, in the same order.
    pub fn translate(&self, lines: &[String]) -> Result<Vec<String>, Error> {
        if lines.is_empty() {
            return Ok(Vec::new());
        }
        let options = TranslationOptions { beam_size: 4, ..TranslationOptions::default() };
        let answered = self
            .translator
            .translate_batch(lines, &options, None)
            .map_err(|error| Error::Translating(error.to_string()))?;
        if answered.len() != lines.len() {
            return Err(Error::Mismatch { asked: lines.len(), answered: answered.len() });
        }
        Ok(answered.into_iter().map(|(text, _)| text).collect())
    }
}
