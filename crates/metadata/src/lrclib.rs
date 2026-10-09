//! LRCLIB, the free library of lyrics: no account and no key, one question a
//! song, answered with the plain words and the words stamped line by line.
//!
//! It asks to be named by who is asking, and to be asked once per song: the
//! answer, found or not, is kept by whoever calls this (see
//! `docs/architecture/06-musique.md`). It is sometimes too busy to answer,
//! which is worth trying again later and never holds up a song.
//!
//! A song is asked for exactly first, by artist, title, album and length. LRCLIB
//! keeps one entry for each album and length a song came out with, so that
//! exact question often finds none while the song is there under another
//! album: it is then searched for by artist and title, and the entry nearest
//! in length is taken.

use std::time::Duration;

use serde::Deserialize;

use crate::provider::{ProviderError, Result};

const BASE_URL: &str = "https://lrclib.net/api";

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
        let mut query = vec![
            ("artist_name", asked.artist.to_string()),
            ("track_name", asked.title.to_string()),
        ];
        if let Some(album) = asked.album {
            query.push(("album_name", album.to_string()));
        }
        if let Some(seconds) = asked.seconds {
            query.push(("duration", seconds.to_string()));
        }
        let (status, body) = self.ask("get", &query).await?;
        if let Some(found) = answer_of(status, &body)? {
            return Ok(Some(found));
        }

        tracing::debug!(
            artist = asked.artist,
            title = asked.title,
            "LRCLIB did not know the song as asked, so it is searched for by artist and title"
        );
        let query = [
            ("artist_name", asked.artist.to_string()),
            ("track_name", asked.title.to_string()),
        ];
        let (status, body) = self.ask("search", &query).await?;
        let found = chosen_of(status, &body, asked)?;
        tracing::debug!(found = found.is_some(), "the search for the song is over");
        Ok(found)
    }

    /// One question to LRCLIB: the status and the body it answered.
    async fn ask(&self, route: &str, query: &[(&str, String)]) -> Result<(u16, String)> {
        let response = self
            .client
            .get(format!("{}/{route}", self.base_url))
            .query(query)
            .send()
            .await
            .map_err(|error| {
                tracing::debug!(route, ?query, %error, "LRCLIB could not be reached");
                ProviderError::Unreachable(error.to_string())
            })?;
        let asked_at = response.url().to_string();
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        tracing::debug!(
            url = %asked_at,
            status,
            body_chars = body.len(),
            body_start = %body.chars().take(300).collect::<String>(),
            "LRCLIB answered"
        );
        Ok((status, body))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Answer {
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    artist_name: Option<String>,
    /// In seconds, with a fraction.
    #[serde(default)]
    duration: Option<f64>,
    plain_lyrics: Option<String>,
    synced_lyrics: Option<String>,
    #[serde(default)]
    instrumental: bool,
}

impl Answer {
    fn into_found(self) -> FoundLyrics {
        let kept = |words: Option<String>| words.filter(|words| !words.trim().is_empty());
        FoundLyrics {
            plain: kept(self.plain_lyrics),
            synced: kept(self.synced_lyrics),
            instrumental: self.instrumental,
        }
    }
}

/// How far a stamped version may be from the length of the file before its
/// times stop lining up with the song.
const SYNCED_TO_WITHIN_SECONDS: f64 = 5.0;

/// Only letters and digits, in lower case: the way two spellings of a name
/// are told to be the same ("Fontaines D.C.", "FONTAINES DC").
fn plain_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The entry of a search that is the song asked for, if there is one.
///
/// Same title and same artist once spelling is set aside, then the one nearest
/// in length. A stamped version far from the length of the file is given as
/// plain words, since its times would not line up.
fn chosen_of(status: u16, body: &str, asked: &Asked<'_>) -> Result<Option<FoundLyrics>> {
    match status {
        200 => {}
        404 => return Ok(None),
        other => return answer_of(other, body),
    }
    let entries: Vec<Answer> = serde_json::from_str(body)
        .map_err(|error| ProviderError::Unexpected(error.to_string()))?;
    let (title, artist) = (plain_name(asked.title), plain_name(asked.artist));
    let away = |entry: &Answer| match (entry.duration, asked.seconds) {
        (Some(there), Some(here)) => (there - here as f64).abs(),
        _ => 0.0,
    };
    let best = entries
        .into_iter()
        .filter(|entry| {
            entry.track_name.as_deref().map(plain_name).as_deref() == Some(title.as_str())
                && entry.artist_name.as_deref().map(plain_name).is_some_and(|name| {
                    !name.is_empty() && (name.contains(&artist) || artist.contains(&name))
                })
        })
        .filter(|entry| {
            entry.instrumental
                || entry.plain_lyrics.as_deref().is_some_and(|words| !words.trim().is_empty())
        })
        .map(|entry| (away(&entry), entry))
        .min_by(|(near, _), (far, _)| near.total_cmp(far));
    Ok(best.map(|(distance, entry)| {
        let mut found = entry.into_found();
        if distance > SYNCED_TO_WITHIN_SECONDS {
            found.synced = None;
        }
        found
    }))
}

/// What an answer says, read apart from asking so it can be tested.
fn answer_of(status: u16, body: &str) -> Result<Option<FoundLyrics>> {
    match status {
        200 => {
            let answer: Answer = serde_json::from_str(body)
                .map_err(|error| ProviderError::Unexpected(error.to_string()))?;
            Ok(Some(answer.into_found()))
        }
        404 => Ok(None),
        429 => Err(ProviderError::TooManyRequests {
            retry_after_seconds: None,
        }),
        500..=599 => Err(ProviderError::Unreachable(format!(
            "LRCLIB answered {status}"
        ))),
        _ => Err(ProviderError::Unexpected(format!(
            "LRCLIB answered {status}"
        ))),
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
        let found = answer_of(
            200,
            r#"{"plainLyrics":"","syncedLyrics":null,"instrumental":true}"#,
        )
        .expect("read")
        .expect("found");
        assert_eq!(
            found,
            FoundLyrics {
                plain: None,
                synced: None,
                instrumental: true
            }
        );
    }

    fn asked(album: Option<&'static str>, seconds: Option<u64>) -> Asked<'static> {
        Asked { artist: "Fontaines D.C.", title: "Starburster", album, seconds }
    }

    const SEARCH: &str = r#"[
        {"trackName":"Starburster","artistName":"Fontaines DC","duration":264.8,"plainLyrics":"far","syncedLyrics":"[00:01.00] far","instrumental":false},
        {"trackName":"starburster","artistName":"fontaines dc","duration":222.0,"plainLyrics":"near","syncedLyrics":"[00:01.00] near","instrumental":false},
        {"trackName":"Starburster (Live)","artistName":"Fontaines D.C.","duration":222.0,"plainLyrics":"live","syncedLyrics":null,"instrumental":false},
        {"trackName":"Starburster","artistName":"Somebody Else","duration":222.0,"plainLyrics":"other","syncedLyrics":null,"instrumental":false}
    ]"#;

    #[test]
    fn a_search_takes_the_same_song_in_another_spelling_nearest_in_length() {
        let found = chosen_of(200, SEARCH, &asked(Some("Romance"), Some(221))).expect("read").expect("found");
        assert_eq!(found.plain.as_deref(), Some("near"));
        assert_eq!(found.synced.as_deref(), Some("[00:01.00] near"));
    }

    #[test]
    fn a_stamped_version_far_from_the_length_of_the_file_is_given_as_plain_words() {
        let found = chosen_of(200, SEARCH, &asked(None, Some(250))).expect("read").expect("found");
        assert_eq!(found.plain.as_deref(), Some("far"));
        assert_eq!(found.synced, None, "its times would not line up");
    }

    #[test]
    fn a_search_with_no_such_song_in_it_finds_nothing() {
        let other = Asked { artist: "Nobody", title: "Starburster", album: None, seconds: None };
        assert_eq!(chosen_of(200, SEARCH, &other).expect("read"), None);
        assert_eq!(chosen_of(200, "[]", &asked(None, None)).expect("read"), None);
        assert!(chosen_of(503, "busy", &asked(None, None)).expect_err("refused").is_worth_retrying());
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
