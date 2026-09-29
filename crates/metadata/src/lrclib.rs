//! LRCLIB, the free library of lyrics: no account and no key, one question a
//! song, answered with the plain words and the words stamped line by line.
//!
//! It asks to be named by who is asking, and to be asked once per song: the
//! answer, found or not, is kept by whoever calls this (see
//! `docs/architecture/06-musique.md`). It is sometimes too busy to answer,
//! which is worth trying again later and never holds up a song.

use std::time::Duration;

use serde::Deserialize;

use crate::provider::{ProviderError, Result};

const BASE_URL: &str = "https://lrclib.net/api/get";

/// Long enough for a slow answer, short enough that a song never waits on it.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);

/// What a song is asked by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked<'a> {
    pub artist: &'a str,
    pub title: &'a str,
    pub album: Option<&'a str>,
    pub seconds: Option<u64>,
}

/// The words LRCLIB holds for a song.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FoundLyrics {
    pub plain: Option<String>,
    /// Stamped line by line, in the form every player reads.
    pub synced: Option<String>,
    /// A song without words, which is an answer too.
    pub instrumental: bool,
}

pub struct LrcLibClient {
    client: reqwest::Client,
    base_url: String,
}

impl LrcLibClient {
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
        Ok(Self {
            client,
            base_url: BASE_URL.to_string(),
        })
    }

    /// The words of a song, or nothing when LRCLIB does not know it.
    pub async fn lyrics(&self, asked: &Asked<'_>) -> Result<Option<FoundLyrics>> {
        let mut query = vec![("artist_name", asked.artist.to_string()), ("track_name", asked.title.to_string())];
        if let Some(album) = asked.album {
            query.push(("album_name", album.to_string()));
        }
        if let Some(seconds) = asked.seconds {
            query.push(("duration", seconds.to_string()));
        }
        let response = self
            .client
            .get(&self.base_url)
            .query(&query)
            .send()
            .await
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        answer_of(status, &body)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Answer {
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
}

/// What an answer says, read apart from asking so it can be tested.
fn answer_of(status: u16, body: &str) -> Result<Option<FoundLyrics>> {
    match status {
        200 => {
            let answer: Answer = serde_json::from_str(body)
                .map_err(|error| ProviderError::Unexpected(error.to_string()))?;
            let kept = |words: Option<String>| words.filter(|words| !words.trim().is_empty());
            Ok(Some(FoundLyrics {
                plain: kept(answer.plain_lyrics),
                synced: kept(answer.synced_lyrics),
                instrumental: answer.instrumental,
            }))
        }
        404 => Ok(None),
        429 => Err(ProviderError::TooManyRequests {
            retry_after_seconds: None,
        }),
        500..=599 => Err(ProviderError::Unreachable(format!("LRCLIB answered {status}"))),
        _ => Err(ProviderError::Unexpected(format!("LRCLIB answered {status}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_song_found_gives_its_words_in_both_forms() {
        let found = answer_of(
            200,
            r#"{"id":1,"trackName":"Tides","plainLyrics":"First line\nSecond line","syncedLyrics":"[00:07.78] First line","instrumental":false}"#,
        )
        .expect("read")
        .expect("found");
        assert_eq!(found.plain.as_deref(), Some("First line\nSecond line"));
        assert_eq!(found.synced.as_deref(), Some("[00:07.78] First line"));
        assert!(!found.instrumental);
    }

    #[test]
    fn a_song_without_words_says_so_and_empty_words_are_none() {
        let found = answer_of(200, r#"{"plainLyrics":"","syncedLyrics":null,"instrumental":true}"#)
            .expect("read")
            .expect("found");
        assert_eq!(found, FoundLyrics { plain: None, synced: None, instrumental: true });
    }

    #[test]
    fn an_unknown_song_is_nothing_and_a_busy_server_is_worth_trying_again() {
        assert_eq!(
            answer_of(404, r#"{"code":404,"name":"TrackNotFound"}"#).expect("read"),
            None
        );
        let busy = answer_of(503, "busy").expect_err("refused");
        assert!(busy.is_worth_retrying());
        assert!(answer_of(429, "").expect_err("refused").is_worth_retrying());
    }
}
