//! LRCLIB, the free library of lyrics: no account and no key, one question a
//! song, answered with the plain words and the words stamped line by line.
//!
//! It asks to be named by who is asking, and to be asked once per song: the
//! answer, found or not, is kept by whoever calls this (see
//! `docs/architecture/06-musique.md`). It is sometimes too busy to answer,
//! which is worth trying again later and never holds up a song.
//!
//! LRCLIB asks of every client that it names itself, that it sends its
//! questions one after the other with a short rest between them, and that it
//! leaves it alone for as long as it says when it answers 429. Every question
//! goes through one gate for that, whoever asks and however many at once.
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

/// The rest between two questions, which LRCLIB asks to be 200 to 500
/// milliseconds.
const REST_BETWEEN_QUESTIONS: Duration = Duration::from_millis(300);

/// How long to leave LRCLIB alone when it says to wait and does not say for how long.
const LEFT_ALONE_BY_DEFAULT: Duration = Duration::from_secs(60);

/// When the next question may go, and whether it is LRCLIB that said to wait.
struct Gate {
    not_before: tokio::time::Instant,
    told_to_wait: bool,
}

/// The one gate every question to LRCLIB goes through, held for the whole of
/// a question so that they follow each other.
fn gate() -> &'static tokio::sync::Mutex<Gate> {
    static GATE: std::sync::OnceLock<tokio::sync::Mutex<Gate>> = std::sync::OnceLock::new();
    GATE.get_or_init(|| {
        tokio::sync::Mutex::new(Gate { not_before: tokio::time::Instant::now(), told_to_wait: false })
    })
}

/// What a song is asked by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked<'a> {
    pub artist: &'a str,
    pub title: &'a str,
    pub album: Option<&'a str>,
    pub seconds: Option<u64>,
}

/// How many entries of a search are offered to choose from.
const MOST_OFFERS: usize = 20;

/// One entry of LRCLIB offered for a song: what it is, never its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub id: i64,
    pub artist: String,
    pub title: String,
    pub album: Option<String>,
    pub seconds: Option<u64>,
    /// Stamped line by line.
    pub synced: bool,
    pub plain: bool,
    pub instrumental: bool,
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
            // LRCLIB requires the name, the version and a link of whoever asks.
            .user_agent(concat!(
                "Melyxar/",
                env!("CARGO_PKG_VERSION"),
                " (",
                env!("CARGO_PKG_REPOSITORY"),
                ")"
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

    /// What LRCLIB holds under an artist and a title, as a person chooses
    /// among them: an entry for each album and length a song came out with.
    /// The artist may be left empty.
    pub async fn offers(&self, artist: &str, title: &str) -> Result<Vec<Offer>> {
        let mut query = vec![("track_name", title.to_string())];
        if !artist.trim().is_empty() {
            query.push(("artist_name", artist.to_string()));
        }
        let (status, body) = self.ask("search", &query).await?;
        match status {
            200 => Ok(entries_of(&body)?.into_iter().filter_map(offer_of).take(MOST_OFFERS).collect()),
            404 => Ok(Vec::new()),
            other => answer_of(other, &body).map(|_| Vec::new()),
        }
    }

    /// The words of one entry of LRCLIB, chosen from its offers.
    pub async fn entry(&self, id: i64) -> Result<Option<FoundLyrics>> {
        let (status, body) = self.ask(&format!("get/{id}"), &[]).await?;
        answer_of(status, &body)
    }

    /// One question to LRCLIB: the status and the body it answered.
    ///
    /// Asked after the rest the one before it left, and not at all while
    /// LRCLIB has said to leave it alone: that is an answer of its own, for
    /// the song to be asked again another time.
    async fn ask(&self, route: &str, query: &[(&str, String)]) -> Result<(u16, String)> {
        let mut gate = gate().lock().await;
        let now = tokio::time::Instant::now();
        if gate.not_before > now {
            if gate.told_to_wait {
                let wait = (gate.not_before - now).as_secs() + 1;
                tracing::debug!(wait_seconds = wait, "LRCLIB said to leave it alone, so it is not asked");
                return Err(ProviderError::TooManyRequests { retry_after_seconds: Some(wait) });
            }
            tokio::time::sleep_until(gate.not_before).await;
        }
        let answer = self.send(route, query).await;
        gate.told_to_wait = false;
        gate.not_before = tokio::time::Instant::now() + REST_BETWEEN_QUESTIONS;
        let (status, retry_after, body) = answer?;
        if status == 429 {
            let wait = retry_after.unwrap_or(LEFT_ALONE_BY_DEFAULT);
            gate.told_to_wait = true;
            gate.not_before = tokio::time::Instant::now() + wait;
            tracing::debug!(wait_seconds = wait.as_secs(), "LRCLIB answered 429 and is left alone for that long");
            return Err(ProviderError::TooManyRequests { retry_after_seconds: Some(wait.as_secs()) });
        }
        Ok((status, body))
    }

    /// The question itself: the status, the time LRCLIB asked to be left
    /// alone for, and the body.
    async fn send(&self, route: &str, query: &[(&str, String)]) -> Result<(u16, Option<Duration>, String)> {
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
        let retry_after = response
            .headers()
            .get(reqwest::header::RETRY_AFTER)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
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
        Ok((status, retry_after, body))
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Answer {
    #[serde(default)]
    id: Option<i64>,
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(default)]
    album_name: Option<String>,
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

/// An entry of a search as it is offered, when it says what it is.
fn offer_of(entry: Answer) -> Option<Offer> {
    let has_words = |words: &Option<String>| words.as_deref().is_some_and(|words| !words.trim().is_empty());
    Some(Offer {
        id: entry.id?,
        artist: entry.artist_name?,
        title: entry.track_name?,
        album: entry.album_name.filter(|album| !album.trim().is_empty()),
        seconds: entry.duration.map(|seconds| seconds.round() as u64),
        synced: has_words(&entry.synced_lyrics),
        plain: has_words(&entry.plain_lyrics),
        instrumental: entry.instrumental,
    })
}

fn entries_of(body: &str) -> Result<Vec<Answer>> {
    serde_json::from_str(body).map_err(|error| ProviderError::Unexpected(error.to_string()))
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
    let entries = entries_of(body)?;
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

    /// A server answering its first question with a song, and its second with
    /// a request to be left alone for two seconds. It tells what it was asked.
    async fn serving_a_song_then_asking_to_be_left_alone() -> (String, tokio::sync::oneshot::Receiver<String>) {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bound");
        let address = listener.local_addr().expect("address");
        let (told, heard) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let mut told = Some(told);
            for answer in [
                "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}",
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: 2\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            ] {
                let Ok((mut stream, _)) = listener.accept().await else { return };
                let mut asked = [0u8; 2048];
                let read = stream.read(&mut asked).await.unwrap_or(0);
                if let Some(told) = told.take() {
                    let _ = told.send(String::from_utf8_lossy(&asked[..read]).to_lowercase());
                }
                let _ = stream.write_all(answer.as_bytes()).await;
            }
        });
        (format!("http://{address}"), heard)
    }

    #[tokio::test]
    async fn lrclib_is_told_who_asks_and_is_left_alone_for_as_long_as_it_says() {
        let (url, heard) = serving_a_song_then_asking_to_be_left_alone().await;
        let client = LrcLibClient { base_url: url, ..LrcLibClient::new().expect("client") };
        let query = [("track_name", "Tides".to_string())];

        let (status, _) = client.ask("get", &query).await.expect("answered");
        assert_eq!(status, 200);
        let head = heard.await.expect("told");
        assert!(head.contains("user-agent: melyxar/") && head.contains("https://"), "{head}");

        let told = client.ask("get", &query).await.expect_err("asked to wait");
        assert!(matches!(told, ProviderError::TooManyRequests { retry_after_seconds: Some(2) }), "{told:?}");

        // Nobody is listening any more: a question that reached the network
        // would be unreachable, not told to wait.
        let again = client.ask("get", &query).await.expect_err("still asked to wait");
        assert!(matches!(again, ProviderError::TooManyRequests { .. }), "{again:?}");
    }

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
    fn an_entry_is_offered_with_what_tells_it_apart_and_only_when_it_says_what_it_is() {
        let body = r#"[
            {"id":7,"trackName":"Starburster","artistName":"Fontaines D.C.","albumName":"Romance","duration":221.4,"plainLyrics":"words","syncedLyrics":"[00:01.00] words","instrumental":false},
            {"id":8,"trackName":"Starburster","artistName":"Fontaines D.C.","albumName":" ","duration":null,"plainLyrics":"","syncedLyrics":null,"instrumental":true},
            {"trackName":"No Id","artistName":"Nobody","plainLyrics":"x","syncedLyrics":null,"instrumental":false}
        ]"#;
        let offers: Vec<Offer> = entries_of(body).expect("read").into_iter().filter_map(offer_of).collect();
        assert_eq!(offers.len(), 2, "the entry without an identifier cannot be chosen");
        assert_eq!(
            offers[0],
            Offer {
                id: 7,
                artist: "Fontaines D.C.".into(),
                title: "Starburster".into(),
                album: Some("Romance".into()),
                seconds: Some(221),
                synced: true,
                plain: true,
                instrumental: false,
            }
        );
        assert_eq!(offers[1].album, None);
        assert!(offers[1].instrumental && !offers[1].plain);
        assert!(entries_of("not json").is_err());
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
