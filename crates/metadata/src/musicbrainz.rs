//! MusicBrainz and the Cover Art Archive beside it: which album a title and
//! an artist are, and the front cover people gave it. Free, with no key, as
//! long as whoever asks names itself and asks no more than once a second,
//! which the caller keeps to (see `docs/architecture/06-musique.md`).

use std::time::Duration;

use serde::Deserialize;

use crate::provider::{ProviderError, Result};

const SEARCH_URL: &str = "https://musicbrainz.org/ws/2/release-group/";
const COVERS_URL: &str = "https://coverartarchive.org/release-group";

/// How far apart two questions to MusicBrainz are asked, as it asks.
pub const BETWEEN_QUESTIONS: Duration = Duration::from_millis(1100);

/// Long enough for a slow answer, short enough that a job moves on.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// How sure MusicBrainz has to be that the album found is the one asked for,
/// out of a hundred: below this a stranger's cover would be worse than none.
const SURE_ENOUGH: u32 = 90;

pub struct MusicBrainzClient {
    client: reqwest::Client,
}

impl MusicBrainzClient {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!(
                "Melyxar/",
                env!("CARGO_PKG_VERSION"),
                " (self-hosted media server)"
            ))
            .build()
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        Ok(Self { client })
    }

    /// The album MusicBrainz is sure an album title and its artist are, as
    /// the identifier of its release group.
    pub async fn album(&self, title: &str, artist: Option<&str>) -> Result<Option<String>> {
        let mut query = format!("releasegroup:\"{}\"", quoted(title));
        if let Some(artist) = artist {
            query.push_str(&format!(" AND artist:\"{}\"", quoted(artist)));
        }
        let response = self
            .client
            .get(SEARCH_URL)
            .query(&[("query", query.as_str()), ("fmt", "json"), ("limit", "3")])
            .send()
            .await
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        match status {
            200 => album_in(&body),
            503 | 429 => Err(ProviderError::TooManyRequests {
                retry_after_seconds: None,
            }),
            500..=599 => Err(ProviderError::Unreachable(format!(
                "MusicBrainz answered {status}"
            ))),
            _ => Err(ProviderError::Unexpected(format!(
                "MusicBrainz answered {status}"
            ))),
        }
    }

    /// The front cover of an album, 500 points across, when anybody gave
    /// it one.
    pub async fn front_cover(&self, release_group: &str) -> Result<Option<Vec<u8>>> {
        let response = self
            .client
            .get(format!("{COVERS_URL}/{release_group}/front-500"))
            .send()
            .await
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        match response.status().as_u16() {
            200 => Ok(Some(
                response
                    .bytes()
                    .await
                    .map_err(|error| ProviderError::Unreachable(error.to_string()))?
                    .to_vec(),
            )),
            404 => Ok(None),
            status @ 500..=599 => Err(ProviderError::Unreachable(format!(
                "the Cover Art Archive answered {status}"
            ))),
            status => Err(ProviderError::Unexpected(format!(
                "the Cover Art Archive answered {status}"
            ))),
        }
    }
}

/// A value put between quotes in a search, its own quotes and escapes kept
/// from ending it early.
fn quoted(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Debug, Deserialize)]
struct Search {
    #[serde(rename = "release-groups", default)]
    release_groups: Vec<Found>,
}

#[derive(Debug, Deserialize)]
struct Found {
    id: String,
    #[serde(default)]
    score: u32,
}

/// The album of a search, when MusicBrainz is sure enough of it.
fn album_in(body: &str) -> Result<Option<String>> {
    let search: Search =
        serde_json::from_str(body).map_err(|error| ProviderError::Unexpected(error.to_string()))?;
    Ok(search
        .release_groups
        .into_iter()
        .find(|found| found.score >= SURE_ENOUGH)
        .map(|found| found.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_album_found_is_kept_only_when_musicbrainz_is_sure_of_it() {
        let sure =
            r#"{"release-groups":[{"id":"9162580e","score":100,"title":"Northern Lights"}]}"#;
        assert_eq!(album_in(sure).expect("read"), Some("9162580e".to_string()));
        let unsure = r#"{"release-groups":[{"id":"1234","score":61}]}"#;
        assert_eq!(album_in(unsure).expect("read"), None);
        assert_eq!(album_in(r#"{"count":0}"#).expect("read"), None);
        assert!(album_in("not json").is_err());
    }

    #[test]
    fn a_title_with_quotes_cannot_end_the_search_early() {
        assert_eq!(quoted(r#"The "Best" \ Of"#), r#"The \"Best\" \\ Of"#);
    }
}
