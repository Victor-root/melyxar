//! Requests for titles the server does not hold: whether accounts may make
//! them, looking a title up at the provider, asking for it, the
//! administrator's decision, and noticing when it arrives. Nothing outside
//! this module knows how a request is made or kept: the rest of the server
//! only starts its round.
//!
//! See the decisions on title requests in `docs/architecture/README.md`.

pub mod access;
pub mod asking;
pub mod deciding;
pub mod live;
pub mod rounds;
mod rules;
pub mod search;
pub mod title;
#[cfg(test)]
mod testing;
mod telling;

pub use melyxar_database::requests::{Decision, TitleRequest};
pub use melyxar_metadata::Catalogue;

use crate::AppError;

/// Why a request was refused, in a word the interface puts into a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Requests are off, or this account was not given the right.
    NotAllowed,
    /// The title, or every season asked for, is in a library already.
    AlreadyHere,
    /// This account already asked for it and is waiting.
    AlreadyAsked,
    /// A season the provider does not know, or seasons asked of a film.
    NoSuchSeason,
    NoteTooLong,
    /// The administrator accepted it: it is no longer the account's to take
    /// back.
    Accepted,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAllowed => "not_allowed",
            Self::AlreadyHere => "already_here",
            Self::AlreadyAsked => "already_asked",
            Self::NoSuchSeason => "no_such_season",
            Self::NoteTooLong => "note_too_long",
            Self::Accepted => "accepted",
        }
    }

    /// Every one of them, so a test can check each has words on the screen.
    pub const ALL: [Self; 6] = [
        Self::NotAllowed,
        Self::AlreadyHere,
        Self::AlreadyAsked,
        Self::NoSuchSeason,
        Self::NoteTooLong,
        Self::Accepted,
    ];
}

/// What can go wrong: a refusal to put into words, or the server itself.
#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error("refused: {}", .0.as_str())]
    Refused(Refused),
    #[error(transparent)]
    Failed(#[from] AppError),
}

impl From<melyxar_database::DatabaseError> for Trouble {
    fn from(error: melyxar_database::DatabaseError) -> Self {
        Self::Failed(AppError::from(error))
    }
}

impl From<melyxar_metadata::ProviderError> for Trouble {
    /// The provider out of reach is a delay, said as such.
    fn from(error: melyxar_metadata::ProviderError) -> Self {
        Self::Failed(
            melyxar_core::Error::new(
                melyxar_core::error::ErrorCode::ExternalServiceUnavailable,
                error.to_string(),
            )
            .into(),
        )
    }
}

pub type Result<T> = std::result::Result<T, Trouble>;

/// The word a catalogue is kept under, and said by.
pub fn word_of(catalogue: Catalogue) -> &'static str {
    match catalogue {
        Catalogue::Films => "films",
        Catalogue::Series => "series",
    }
}

/// The catalogue a kept word names.
pub fn catalogue_of(word: &str) -> Option<Catalogue> {
    match word {
        "films" => Some(Catalogue::Films),
        "series" => Some(Catalogue::Series),
        _ => None,
    }
}

/// How the language an account reads in is asked of the provider.
fn provider_language(language: &str) -> &str {
    match melyxar_core::user::is_a_language(language) {
        true => language,
        false => "en",
    }
}
