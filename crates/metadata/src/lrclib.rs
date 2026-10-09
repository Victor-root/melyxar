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
            "LRCLIB did not know the song as asked, so it is searched for"
        );
        for (way, query) in searches(asked.artist, asked.title) {
            let entries = self.search(&query).await?;
            tracing::debug!(way, entries = entries.len(), "a search of LRCLIB");
            if let Some(found) = chosen_among(entries, asked) {
                tracing::debug!(way, "the song is among them");
                return Ok(Some(found));
            }
        }
        tracing::debug!("the song is in none of the searches");
        Ok(None)
    }

    /// One search of LRCLIB: the entries it answers, none when it knows none.
    async fn search(&self, query: &[(&str, String)]) -> Result<Vec<Answer>> {
        let (status, body) = self.ask("search", query).await?;
        match status {
            200 => entries_of(&body),
            404 => Ok(Vec::new()),
            other => answer_of(other, &body).map(|_| Vec::new()),
        }
    }

    /// What LRCLIB holds under an artist and a title, as a person chooses
    /// among them: an entry for each album and length a song came out with.
    /// The artist may be left empty.
    pub async fn offers(&self, artist: &str, title: &str) -> Result<Vec<Offer>> {
        for (way, query) in searches(artist, title) {
            let offers: Vec<Offer> = self.search(&query).await?.into_iter().filter_map(offer_of).take(MOST_OFFERS).collect();
            if !offers.is_empty() {
                tracing::debug!(way, offers = offers.len(), "a search of LRCLIB by hand");
                return Ok(offers);
            }
        }
        Ok(Vec::new())
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

/// The ways a song is searched for, from the narrowest to the widest: by
/// artist and title as they are, then by the title alone in the free text
/// (which finds a song whose title carries its artist), then by both in it.
/// An empty artist is left out.
fn searches(artist: &str, title: &str) -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    let (artist, title) = (artist.trim(), title.trim());
    let mut ways = Vec::new();
    if artist.is_empty() {
        ways.push(("by title", vec![("track_name", title.to_string())]));
    } else {
        ways.push((
            "by artist and title",
            vec![("artist_name", artist.to_string()), ("track_name", title.to_string())],
        ));
    }
    ways.push(("by the title in free text", vec![("q", title.to_string())]));
    if !artist.is_empty() {
        ways.push(("by artist and title in free text", vec![("q", format!("{artist} {title}"))]));
    }
    ways
}

fn entries_of(body: &str) -> Result<Vec<Answer>> {
    serde_json::from_str(body).map_err(|error| ProviderError::Unexpected(error.to_string()))
}

/// Letters without their accents, in lower case.
fn plain_letter(letter: char) -> char {
    match letter {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' => 'a',
        'ç' | 'ć' | 'č' => 'c',
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ę' => 'e',
        'ì' | 'í' | 'î' | 'ï' | 'ī' => 'i',
        'ñ' | 'ń' => 'n',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' => 'o',
        'ù' | 'ú' | 'û' | 'ü' | 'ū' => 'u',
        'ý' | 'ÿ' => 'y',
        'ß' => 's',
        other => other,
    }
}

/// Words that say who made a song or how it was published, and tell nothing
/// of which song it is: they are on one side of a comparison and not the other.
const NOISE: [&str; 11] =
    ["feat", "ft", "featuring", "prod", "by", "x", "official", "video", "audio", "lyrics", "lyric"];

/// The words of a name or a title, in lower case and without accents, the
/// full stops of an abbreviation dropped ("D.C." is "dc") and the rest of
/// the punctuation a gap between words.
fn words_of(text: &str) -> std::collections::BTreeSet<String> {
    let spelled: String = text
        .chars()
        .filter(|c| !matches!(c, '.' | '\'' | '’'))
        .flat_map(char::to_lowercase)
        .map(plain_letter)
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    spelled
        .split_whitespace()
        .filter(|word| !NOISE.contains(word))
        .map(str::to_string)
        .collect()
}

/// How much two sets of words say the same, from nought to one: twice what
/// they share over what they hold together.
fn likeness(asked: &std::collections::BTreeSet<String>, found: &std::collections::BTreeSet<String>) -> f64 {
    let total = asked.len() + found.len();
    if total == 0 {
        return 0.0;
    }
    2.0 * asked.intersection(found).count() as f64 / total as f64
}

/// How alike the words of an entry and the words asked for must be.
const ALIKE_ENOUGH: f64 = 0.8;
/// How alike they must be when the length of the entry is the file's own, to
/// the second or nearly: a title written in its own way, the file name for
/// one, then still finds its entry.
const ALIKE_WHEN_THE_LENGTH_AGREES: f64 = 0.5;
/// How near in length, in seconds, counts as the same recording.
const THE_SAME_LENGTH: f64 = 3.0;

/// The entry among these that is the song asked for, if there is one.
///
/// The words of the artist and the title, taken together, must be alike
/// enough in both: a title that carries its artist, a file name for a title,
/// a spelling with or without accents all come to the same words. Then the
/// likest wins, with words before no words, and the nearest in length
/// among equals. A stamped version far from the length of the file is given
/// as plain words, since its times would not line up.
fn chosen_among(entries: Vec<Answer>, asked: &Asked<'_>) -> Option<FoundLyrics> {
    let wanted = {
        let mut words = words_of(asked.artist);
        words.extend(words_of(asked.title));
        words
    };
    let away = |entry: &Answer| match (entry.duration, asked.seconds) {
        (Some(there), Some(here)) => Some((there - here as f64).abs()),
        _ => None,
    };
    let best = entries
        .into_iter()
        .filter_map(|entry| {
            let has_words = entry.plain_lyrics.as_deref().is_some_and(|words| !words.trim().is_empty());
            if !has_words && !entry.instrumental {
                return None;
            }
            let mut found = words_of(entry.artist_name.as_deref().unwrap_or_default());
            found.extend(words_of(entry.track_name.as_deref().unwrap_or_default()));
            let alike = likeness(&wanted, &found);
            let distance = away(&entry);
            let agrees = distance.is_some_and(|distance| distance <= THE_SAME_LENGTH);
            let enough = alike >= ALIKE_ENOUGH || (agrees && alike >= ALIKE_WHEN_THE_LENGTH_AGREES);
            enough.then_some((has_words, alike, distance, entry))
        })
        .max_by(|(words, alike, distance, _), (other_words, other_alike, other_distance, _)| {
            words
                .cmp(other_words)
                .then(alike.total_cmp(other_alike))
                .then(other_distance.unwrap_or(f64::MAX).total_cmp(&distance.unwrap_or(f64::MAX)))
        });
    best.map(|(_, _, distance, entry)| {
        let mut found = entry.into_found();
        if distance.is_some_and(|distance| distance > SYNCED_TO_WITHIN_SECONDS) {
            found.synced = None;
        }
        found
    })
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
        let found = chosen_among(entries_of(SEARCH).expect("read"), &asked(Some("Romance"), Some(221))).expect("found");
        assert_eq!(found.plain.as_deref(), Some("near"));
        assert_eq!(found.synced.as_deref(), Some("[00:01.00] near"));
    }

    #[test]
    fn a_stamped_version_far_from_the_length_of_the_file_is_given_as_plain_words() {
        let found = chosen_among(entries_of(SEARCH).expect("read"), &asked(None, Some(250))).expect("found");
        assert_eq!(found.plain.as_deref(), Some("far"));
        assert_eq!(found.synced, None, "its times would not line up");
    }

    #[test]
    fn a_file_name_for_a_title_and_a_folder_for_an_artist_still_find_the_entry() {
        let entries = r#"[
            {"trackName":"Se Me Nota","artistName":"Hansel Y Raul","duration":279.0,"plainLyrics":"other song","syncedLyrics":null,"instrumental":false},
            {"trackName":"Se Me Nota (Agárrame)","artistName":"Chimbala & Omega","duration":176.0,"plainLyrics":"this","syncedLyrics":"[00:01.00] this","instrumental":false},
            {"trackName":"Se Me Nota - Agarrame","artistName":"Chimbala x Omega","duration":177.0,"plainLyrics":"best","syncedLyrics":"[00:01.00] best","instrumental":false}
        ]"#;
        let asked = Asked {
            artist: "Stream Chimbala X Omega",
            title: "Chimbala x Omega - Se Me Nota - (Agarrame)",
            album: Some("Stream Chimbala X Omega"),
            seconds: Some(177),
        };
        let found = chosen_among(entries_of(entries).expect("read"), &asked).expect("found");
        assert_eq!(found.plain.as_deref(), Some("best"));
    }

    #[test]
    fn words_are_compared_without_accents_capitals_or_the_full_stops_of_an_abbreviation() {
        assert_eq!(words_of("Fontaines D.C."), words_of("FONTAINES DC"));
        assert_eq!(words_of("Se Me Nota (Agárrame)"), words_of("se me nota - agarrame"));
        assert_eq!(words_of("Tides feat. Somebody"), words_of("Tides Somebody"));
        assert!(likeness(&words_of("a b c d"), &words_of("a b c d")) > 0.99);
        assert!(likeness(&words_of("a b"), &words_of("c d")) < 0.01);
    }

    #[test]
    fn an_entry_with_words_wins_over_one_without_and_a_song_without_any_is_still_known() {
        let entries = r#"[
            {"trackName":"Starburster","artistName":"Fontaines D.C.","duration":221.0,"plainLyrics":null,"syncedLyrics":null,"instrumental":true},
            {"trackName":"Starburster","artistName":"Fontaines D.C.","duration":225.0,"plainLyrics":"words","syncedLyrics":null,"instrumental":false}
        ]"#;
        let found = chosen_among(entries_of(entries).expect("read"), &asked(None, Some(221))).expect("found");
        assert_eq!(found.plain.as_deref(), Some("words"));

        let only = r#"[{"trackName":"Starburster","artistName":"Fontaines D.C.","duration":221.0,"plainLyrics":null,"syncedLyrics":null,"instrumental":true}]"#;
        assert!(chosen_among(entries_of(only).expect("read"), &asked(None, Some(221))).expect("found").instrumental);
    }

    #[test]
    fn the_searches_go_from_the_narrowest_to_the_widest_and_leave_out_a_missing_artist() {
        let ways = searches("Artist", "Title");
        assert_eq!(ways.len(), 3);
        assert_eq!(ways[0].1[0].0, "artist_name");
        assert_eq!(ways[1].1, [("q", "Title".to_string())]);
        assert_eq!(ways[2].1, [("q", "Artist Title".to_string())]);
        let without = searches(" ", "Title");
        assert_eq!(without.len(), 2);
        assert_eq!(without[0].1, [("track_name", "Title".to_string())]);
    }

    #[test]
    fn a_search_with_no_such_song_in_it_finds_nothing() {
        let other = Asked { artist: "Nobody", title: "Starburster", album: None, seconds: None };
        assert_eq!(chosen_among(entries_of(SEARCH).expect("read"), &other), None);
        assert_eq!(chosen_among(Vec::new(), &asked(None, None)), None);
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
