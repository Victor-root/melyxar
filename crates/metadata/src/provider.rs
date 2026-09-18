//! What a metadata provider is, said once so nothing else has to care which
//! one is in use.
//!
//! Everything a provider gives back is plain data. Deciding what to do with it
//! belongs to the layer that assembles the server, which is what lets a test
//! swap the real provider for one that answers from memory.

use std::future::Future;

use melyxar_core::time::Millis;

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    /// The provider could not be reached, or did not answer in time. Worth
    /// trying again later, so it is kept apart from the others.
    #[error("the provider could not be reached: {0}")]
    Unreachable(String),
    /// The provider refused the key. Trying again changes nothing until
    /// someone fixes the configuration.
    #[error("the provider refused the key")]
    Unauthorised,
    /// Too many requests. The provider says when to come back, when it can.
    #[error("the provider asked to be left alone for a while")]
    TooManyRequests { retry_after_seconds: Option<u64> },
    /// The provider answered something that could not be read.
    #[error("the provider answered something unexpected: {0}")]
    Unexpected(String),
}

impl ProviderError {
    /// Whether asking again later stands a chance.
    pub fn is_worth_retrying(&self) -> bool {
        matches!(self, Self::Unreachable(_) | Self::TooManyRequests { .. })
    }
}

pub type Result<T> = std::result::Result<T, ProviderError>;

/// Which of a provider's catalogues a question is about.
///
/// A provider keeps films and series apart and answers a different road for
/// each. Everything above this line is the same question asked twice, which is
/// why the rules that read an answer never learn which one it was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Catalogue {
    Films,
    Series,
}

impl Catalogue {
    /// What a work of this kind is looked up in.
    ///
    /// A season and an episode are never looked up on their own: they are
    /// named by the series they hang under, which is what carries the
    /// identifier the provider answers to.
    pub fn of(kind: melyxar_core::work::WorkKind) -> Option<Self> {
        match kind {
            melyxar_core::work::WorkKind::Movie => Some(Self::Films),
            melyxar_core::work::WorkKind::Series => Some(Self::Series),
            _ => None,
        }
    }
}

/// A work the provider thinks the name might be about.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub external_id: String,
    /// Which catalogue it came out of, so an answer found by an identifier
    /// rather than by a name still says what it is.
    pub catalogue: Catalogue,
    pub title: String,
    /// Title in its own country, which is often how a file is named.
    pub original_title: Option<String>,
    pub release_year: Option<i32>,
    pub overview: Option<String>,
    /// Path of the poster at the provider, not a full address: what the
    /// address looks like is the provider's business, not the caller's.
    pub poster_path: Option<String>,
    /// How sure the provider is, on its own scale. Only ever used to order
    /// candidates against each other, never as a threshold.
    pub popularity: f64,
}

/// Everything the provider knows about one work.
///
/// One shape for a film and for a series, because a page shows the same things
/// about both and the rules that fill it never ask which it is. What a series
/// has and a film has not is its seasons, and those are asked for separately:
/// a series of nine seasons is nine more answers, and a page that only lists
/// them needs none of them.
#[derive(Debug, Clone, PartialEq)]
pub struct Details {
    pub external_id: String,
    pub imdb_id: Option<String>,
    pub title: String,
    pub original_title: Option<String>,
    pub original_language: Option<String>,
    pub tagline: Option<String>,
    pub overview: Option<String>,
    pub release_year: Option<i32>,
    pub runtime: Option<Millis>,
    pub community_rating: Option<f64>,
    /// Age rating as the country writes it, and the country it came from.
    pub age_rating_label: Option<String>,
    pub genres: Vec<String>,
    pub studios: Vec<String>,
    pub credits: Vec<Credit>,
    pub collection: Option<Collection>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
    /// The film's title drawn as the film itself draws it, a picture rather
    /// than a line of text. A kind of its own: it is neither the poster nor
    /// the backdrop, and plenty of films have none.
    pub logo_path: Option<String>,
    pub trailers: Vec<Trailer>,
    /// How many seasons a series holds, as the provider counts them. Absent
    /// for a film, which holds none.
    pub season_count: Option<i32>,
}

/// One season of a series, with the episodes under it.
#[derive(Debug, Clone, PartialEq)]
pub struct SeasonDetails {
    /// What the provider calls this season, which is not what it calls the
    /// series: a season is a page of its own there as it is here.
    pub external_id: String,
    pub season_number: i32,
    /// The name this season goes by, when it has one that is not its number.
    pub name: Option<String>,
    pub overview: Option<String>,
    pub poster_path: Option<String>,
    pub episodes: Vec<EpisodeDetails>,
}

/// One episode, as the provider describes it.
#[derive(Debug, Clone, PartialEq)]
pub struct EpisodeDetails {
    pub external_id: String,
    pub episode_number: i32,
    pub name: Option<String>,
    pub overview: Option<String>,
    /// The picture taken from the episode itself, which is what a list of
    /// episodes shows instead of a poster.
    pub still_path: Option<String>,
    pub release_year: Option<i32>,
    pub runtime: Option<Millis>,
    pub community_rating: Option<f64>,
}

/// One person's part in a film.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credit {
    pub external_id: String,
    pub name: String,
    /// actor, director, writer, producer, composer.
    pub role: String,
    /// Who they played, for an actor.
    pub character: Option<String>,
    /// Position in the billing, which is the order a page shows them in.
    pub ordinal: i32,
    pub photo_path: Option<String>,
}

/// A series of films the provider groups together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collection {
    pub external_id: String,
    pub name: String,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
}

/// A trailer hosted elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trailer {
    pub name: String,
    pub site: String,
    pub key: String,
    pub is_official: bool,
    pub language: Option<String>,
}

impl Trailer {
    /// Where the trailer can be watched.
    ///
    /// Playing it leaves this server, which is why the address is built only
    /// when a viewer asks for it and why a setting decides whether they may.
    pub fn watch_url(&self) -> Option<String> {
        match self.site.to_lowercase().as_str() {
            "youtube" => Some(format!("https://www.youtube.com/watch?v={}", self.key)),
            "vimeo" => Some(format!("https://vimeo.com/{}", self.key)),
            _ => None,
        }
    }
}

/// Where a metadata provider is asked for what it knows.
pub trait MetadataProvider: Send + Sync {
    /// A name the provider goes by, stored alongside every field it supplied.
    fn name(&self) -> &'static str;

    /// Works whose name could be the one asked about, best first.
    fn search(
        &self,
        catalogue: Catalogue,
        title: &str,
        year: Option<i32>,
        language: &str,
    ) -> impl Future<Output = Result<Vec<Candidate>>> + Send;

    /// Everything about one work.
    fn details(
        &self,
        catalogue: Catalogue,
        external_id: &str,
        language: &str,
    ) -> impl Future<Output = Result<Details>> + Send;

    /// One season of a series, with its episodes.
    fn season(
        &self,
        series_id: &str,
        season_number: i32,
        language: &str,
    ) -> impl Future<Output = Result<SeasonDetails>> + Send;

    /// The address a picture the provider named can be fetched from.
    ///
    /// The provider owns the shape of its addresses, so the caller passes the
    /// path it was given back rather than assembling one.
    fn image_url(&self, path: &str) -> String;

    /// Fetches a picture the provider named.
    fn fetch_image(&self, path: &str) -> impl Future<Output = Result<Vec<u8>>> + Send;

    /// The work an identifier from another site stands for.
    ///
    /// This is what makes a description file worth reading: an identifier can
    /// be handed straight to the provider instead of guessing from a name.
    fn by_imdb_id(
        &self,
        imdb_id: &str,
        language: &str,
    ) -> impl Future<Output = Result<Option<Candidate>>> + Send;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_failure_that_could_pass_is_worth_trying_again() {
        assert!(ProviderError::Unreachable("timed out".into()).is_worth_retrying());
        assert!(ProviderError::TooManyRequests {
            retry_after_seconds: Some(3)
        }
        .is_worth_retrying());

        assert!(
            !ProviderError::Unauthorised.is_worth_retrying(),
            "asking again with the same wrong key wastes everyone's time"
        );
        assert!(!ProviderError::Unexpected("not json".into()).is_worth_retrying());
    }

    #[test]
    fn a_trailer_gives_up_an_address_only_for_a_site_that_is_known() {
        let trailer = Trailer {
            name: "Bande annonce".into(),
            site: "YouTube".into(),
            key: "abc123".into(),
            is_official: true,
            language: Some("fre".into()),
        };
        assert_eq!(
            trailer.watch_url().as_deref(),
            Some("https://www.youtube.com/watch?v=abc123")
        );

        let elsewhere = Trailer {
            site: "SomeSite".into(),
            ..trailer
        };
        assert_eq!(
            elsewhere.watch_url(),
            None,
            "an address built from a site nobody knows is a guess"
        );
    }
}
