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
    /// How many lines carry a moment, and the longest time between two of
    /// them in seconds: stamped words with a long gap leave the song running
    /// with nothing lit.
    pub synced_lines: u32,
    pub longest_gap_seconds: Option<u32>,
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
        // A question LRCLIB could not answer is not an answer: the next way of
        // asking may be answered, and when none finds the song and one failed,
        // the song is not known to be absent, only not found, and is asked
        // again another time.
        let mut trouble = None;
        let exact = self.ask("get", &query).await.and_then(|(status, body)| answer_of(status, &body));
        match exact {
            Ok(Some(found)) => return Ok(Some(found)),
            Ok(None) => {}
            Err(error @ ProviderError::TooManyRequests { .. }) => return Err(error),
            Err(error) => trouble = Some(error),
        }

        tracing::debug!(
            artist = asked.artist,
            title = asked.title,
            "LRCLIB did not give the song as asked, so it is searched for"
        );
        for (way, query) in searches(asked.artist, asked.title) {
            match self.search(&query).await {
                Ok(entries) => {
                    tracing::debug!(way, entries = entries.len(), "a search of LRCLIB");
                    if let Some(found) = chosen_among(entries, asked) {
                        tracing::debug!(way, "the song is among them");
                        return Ok(Some(found));
                    }
                }
                Err(error @ ProviderError::TooManyRequests { .. }) => return Err(error),
                Err(error) => {
                    tracing::debug!(way, %error, "a search of LRCLIB was not answered");
                    trouble.get_or_insert(error);
                }
            }
        }
        match trouble {
            Some(error) => Err(error),
            None => {
                tracing::debug!("the song is in none of the searches");
                Ok(None)
            }
        }
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

/// How many lines a stamped text has, and the longest time between two of
/// them in seconds.
fn shape_of(synced: Option<&str>) -> (u32, Option<u32>) {
    let lines = synced.map(melyxar_core::music_lyrics::read_lyrics).unwrap_or_default().synced;
    let longest = lines.windows(2).map(|pair| pair[1].at.get() - pair[0].at.get()).max();
    (lines.len() as u32, longest.map(|gap| (gap / 1000) as u32))
}

/// An entry of a search as it is offered, when it says what it is.
fn offer_of(entry: Answer) -> Option<Offer> {
    let (synced_lines, longest_gap_seconds) = shape_of(entry.synced_lyrics.as_deref());
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
        synced_lines,
        longest_gap_seconds,
    })
}

/// Words that say how a recording was made or published and name no song:
/// searched alone they find every song that has them.
const GENERIC: [&str; 12] = [
    "remix", "version", "remastered", "remaster", "edit", "extended", "original", "radio", "instrumental",
    "acoustic", "cover", "single",
];

/// The pieces of a title worth searching for on their own: what is left of
/// it once what is in brackets is taken out, cut where a dash stands between
/// spaces. A file named after its artist and its song, or a title with the
/// version of the song in brackets, finds nothing as a whole and finds the song
/// by one of its parts.
fn pieces_of(title: &str) -> Vec<String> {
    let mut outside = String::new();
    let mut depth = 0usize;
    for letter in title.chars() {
        match letter {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth = depth.saturating_sub(1),
            _ if depth == 0 => outside.push(letter),
            _ => {}
        }
    }
    let mut pieces: Vec<String> = vec![String::new()];
    for word in outside.split_whitespace() {
        if word == "-" || word == "\u{2013}" {
            pieces.push(String::new());
        } else if let Some(last) = pieces.last_mut() {
            if !last.is_empty() {
                last.push(' ');
            }
            last.push_str(word);
        }
    }
    let mut kept: Vec<String> = Vec::new();
    for piece in pieces {
        if piece.chars().count() >= 3 && piece != title && !kept.contains(&piece) {
            kept.push(piece);
        }
    }
    kept.truncate(MOST_PIECES);
    kept
}

/// How many pieces, and how many single words, of a title are searched for.
const MOST_PIECES: usize = 2;
const MOST_WORDS: usize = 2;

/// The longest words of a title that name something: a rare word finds the
/// song when nothing else does, as long as the entry spells it the same way.
fn rare_words_of(title: &str) -> Vec<String> {
    let mut words: Vec<String> = words_of(title)
        .into_iter()
        .filter(|word| word.chars().count() >= 5 && !GENERIC.contains(&word.as_str()))
        .collect();
    words.sort_by_key(|word| std::cmp::Reverse(word.chars().count()));
    words.truncate(MOST_WORDS);
    words
}

/// The ways a song is searched for, from the narrowest to the widest: by
/// artist and title as they are, by the title alone in the free text (which
/// finds a song whose title carries its artist), by both in it, then by the
/// pieces of the title and by its rarest words, for a title that finds
/// nothing as a whole. An empty artist is left out.
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
    let pieces = pieces_of(title);
    for piece in &pieces {
        ways.push(("by a piece of the title in free text", vec![("q", piece.clone())]));
    }
    for word in rare_words_of(title) {
        let already = |text: &str| text.eq_ignore_ascii_case(&word);
        if !already(title) && !pieces.iter().any(|piece| already(piece)) {
            ways.push(("by a rare word of the title in free text", vec![("q", word)]));
        }
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

/// The letters and digits of a text, run together: how a name is spelled
/// when nobody agrees where the spaces and the pictures go.
fn run_together(text: &str) -> Vec<char> {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(plain_letter)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// How much two texts say the same letter by letter, from nought to one:
/// twice the pairs of letters in a row they share over all the pairs they
/// hold. A title written with a picture for two letters, or with its words
/// run together, still comes near.
fn spelled_alike(asked: &str, found: &str) -> f64 {
    let pairs = |text: &str| {
        let letters = run_together(text);
        let mut counted = std::collections::BTreeMap::<(char, char), u32>::new();
        for pair in letters.windows(2) {
            *counted.entry((pair[0], pair[1])).or_default() += 1;
        }
        counted
    };
    let (asked, found) = (pairs(asked), pairs(found));
    let total: u32 = asked.values().sum::<u32>() + found.values().sum::<u32>();
    if total == 0 {
        return 0.0;
    }
    let shared: u32 = asked.iter().map(|(pair, count)| (*count).min(*found.get(pair).unwrap_or(&0))).sum();
    2.0 * f64::from(shared) / f64::from(total)
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
/// The artist and the title, taken together, must be alike enough, by their
/// words or by their letters, whichever comes nearer: a title that carries its artist, a file name for a title,
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
    let wanted_text = format!("{} {}", asked.artist, asked.title);
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
            let found_text = format!(
                "{} {}",
                entry.artist_name.as_deref().unwrap_or_default(),
                entry.track_name.as_deref().unwrap_or_default()
            );
            let alike = likeness(&wanted, &found).max(spelled_alike(&wanted_text, &found_text));
            let distance = away(&entry);
            let agrees = distance.is_some_and(|distance| distance <= THE_SAME_LENGTH);
            let enough = alike >= ALIKE_ENOUGH || (agrees && alike >= ALIKE_WHEN_THE_LENGTH_AGREES);
            // Of the entries of the length of the file, the one with the most
            // stamped lines is the one that has all of them: the same song is
            // often stamped with lines missing, and the song then runs on
            // with nothing lit.
            let stamped = if agrees { shape_of(entry.synced_lyrics.as_deref()).0 } else { 0 };
            enough.then_some((has_words, alike, agrees, stamped, distance, entry))
        })
        .max_by(|(words, alike, agrees, stamped, distance, _), (other_words, other_alike, other_agrees, other_stamped, other_distance, _)| {
            words
                .cmp(other_words)
                .then(alike.total_cmp(other_alike))
                .then(agrees.cmp(other_agrees))
                .then(stamped.cmp(other_stamped))
                .then(other_distance.unwrap_or(f64::MAX).total_cmp(&distance.unwrap_or(f64::MAX)))
        });
    best.map(|(_, _, _, _, distance, entry)| {
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

    /// A server answering by route: each answers with a status and a body,
    /// whoever asks and however often.
    async fn serving_by_route(routes: Vec<(&'static str, &'static str, &'static str)>) -> String {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("bound");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut asked = [0u8; 2048];
                let read = stream.read(&mut asked).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&asked[..read]).to_string();
                let path = head.split_whitespace().nth(1).unwrap_or_default().to_string();
                let (status, body) = routes
                    .iter()
                    .find(|(route, _, _)| path.starts_with(route))
                    .map_or(("404 Not Found", "{}"), |(_, status, body)| (*status, *body));
                let answer =
                    format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                let _ = stream.write_all(answer.as_bytes()).await;
            }
        });
        format!("http://{address}")
    }

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

    /// One test for what asks LRCLIB, since they share its one gate: a
    /// question told to wait would hold up the others for as long as it says.
    #[tokio::test]
    async fn what_asks_lrclib_goes_on_past_a_busy_answer_names_itself_and_leaves_it_alone_when_told() {
        // Busy for the exact question and for one search, answered by another.
        let song = r#"[{"trackName":"Castaner - C🅰️C🅰️ Staner (REMIX)","artistName":"Khaled Freak","duration":155.0,"plainLyrics":"this","syncedLyrics":null,"instrumental":false}]"#;
        let url = serving_by_route(vec![
            ("/get", "503 Service Unavailable", "busy"),
            ("/search?q=Castaner+-+CacaStaner", "200 OK", "[]"),
            ("/search?artist_name", "503 Service Unavailable", "busy"),
            ("/search?q=Khaled", "200 OK", song),
            ("/search", "200 OK", "[]"),
        ])
        .await;
        let client = LrcLibClient { base_url: url, ..LrcLibClient::new().expect("client") };
        let asked = Asked { artist: "Khaled Freak", title: "Castaner - CacaStaner (REMIX)", album: None, seconds: Some(156) };
        let found = client.lyrics(&asked).await.expect("answered").expect("found by a later search");
        assert_eq!(found.plain.as_deref(), Some("this"));

        // Busy for everything and nothing found: unasked, never "unknown".
        let url = serving_by_route(vec![("/get", "503 Service Unavailable", "busy"), ("/search", "200 OK", "[]")]).await;
        let client = LrcLibClient { base_url: url, ..LrcLibClient::new().expect("client") };
        let error = client.lyrics(&asked).await.expect_err("not known to be absent");
        assert!(error.is_worth_retrying(), "{error:?}");

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
    fn of_the_entries_of_the_length_of_the_file_the_one_with_the_most_stamped_lines_wins() {
        let entries = r#"[
            {"trackName":"Tides","artistName":"Amber Field","duration":177.0,"plainLyrics":"a","syncedLyrics":"[00:01.00] a\n[00:30.00] b","instrumental":false},
            {"trackName":"Tides","artistName":"Amber Field","duration":176.0,"plainLyrics":"a","syncedLyrics":"[00:01.00] a\n[00:10.00] b\n[00:20.00] c\n[00:30.00] d","instrumental":false},
            {"trackName":"Tides","artistName":"Amber Field","duration":260.0,"plainLyrics":"a","syncedLyrics":"[00:01.00] a\n[00:05.00] b\n[00:09.00] c\n[00:13.00] d\n[00:17.00] e","instrumental":false}
        ]"#;
        let asked = Asked { artist: "Amber Field", title: "Tides", album: None, seconds: Some(177) };
        let found = chosen_among(entries_of(entries).expect("read"), &asked).expect("found");
        assert!(found.synced.as_deref().is_some_and(|text| text.contains("[00:20.00] c")), "{found:?}");
        assert!(!found.synced.as_deref().is_some_and(|text| text.contains("[00:17.00] e")), "too far in length");
    }

    #[test]
    fn what_a_stamped_text_leaves_without_a_line_is_counted() {
        assert_eq!(shape_of(Some("[00:01.00] a\n[00:08.50] b\n[00:30.00] c")), (3, Some(21)));
        assert_eq!(shape_of(Some("no stamps")), (0, None));
        assert_eq!(shape_of(None), (0, None));
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
    fn a_title_with_a_picture_for_letters_is_the_same_song_by_its_spelling() {
        let entries = r#"[
            {"trackName":"Castaner - C🅰️C🅰️ Staner (REMIX)","artistName":"Khaled Freak","duration":155.0,"plainLyrics":"this","syncedLyrics":null,"instrumental":false},
            {"trackName":"Jean Marie Bigard (SYNTHWAVE REMIX)","artistName":"Khaled Freak","duration":155.0,"plainLyrics":"other","syncedLyrics":null,"instrumental":false}
        ]"#;
        let asked = Asked { artist: "Khaled Freak", title: "Castaner - CacaStaner (REMIX)", album: None, seconds: None };
        let found = chosen_among(entries_of(entries).expect("read"), &asked).expect("found");
        assert_eq!(found.plain.as_deref(), Some("this"));
        assert!(spelled_alike("Khaled Freak Castaner - CacaStaner (REMIX)", "Khaled Freak Jean Marie Bigard (SYNTHWAVE REMIX)") < 0.5);
    }

    #[test]
    fn a_title_that_finds_nothing_as_a_whole_is_searched_by_its_pieces_and_its_rare_words() {
        let ways = searches("Khaled Freak", "Castaner - CacaStaner (REMIX)");
        let free: Vec<&str> = ways
            .iter()
            .filter_map(|(_, query)| match query.as_slice() {
                [("q", text)] => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(free.contains(&"Castaner"), "{free:?}");
        assert!(free.contains(&"CacaStaner"), "{free:?}");
        assert!(free.contains(&"castaner") || free.contains(&"Castaner"));
        assert!(!free.contains(&"remix"), "a word every remix has finds every remix");
        assert_eq!(pieces_of("A Title"), Vec::<String>::new(), "a title without a dash or a bracket has no pieces");
        assert_eq!(
            pieces_of("Chimbala x Omega - Se Me Nota - (Agarrame)"),
            ["Chimbala x Omega", "Se Me Nota"]
        );
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
                synced_lines: 1,
                longest_gap_seconds: None,
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
