//! The lyrics of a song, looked for from the nearest to the furthest: written
//! inside the song, in a file of the same name beside it, then asked of
//! LRCLIB when its library allows it (see `docs/architecture/06-musique.md`).
//!
//! What is in or beside the song is read afresh each time, so words added to
//! a file are there at the next listen. What LRCLIB answers is kept, found or
//! not, so each song is asked once; a busy answer is not kept, and the song
//! is asked again another time.

use std::path::Path;

use melyxar_core::id::WorkId;
use melyxar_core::music_lyrics::{Lyrics, read_lyrics};
use melyxar_core::user::User;
use melyxar_database::music_lyrics::LookedUpLyrics;
use melyxar_metadata::lrclib::{Asked, LrcLibClient};
pub use melyxar_metadata::lrclib::Offer;
use melyxar_metadata::ProviderError;

use crate::reach::may_read_the_work;
use crate::{AppError, AppState, Result};

/// Where the words of a song were found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// Written inside the song.
    Song,
    /// In a file of the same name beside it.
    Beside,
    Online,
}

impl Source {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Song => "song",
            Self::Beside => "beside",
            Self::Online => "online",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongLyrics {
    pub lyrics: Lyrics,
    pub source: Source,
    /// A song known to have no words, which is said rather than left blank.
    pub instrumental: bool,
}

/// The lyrics of a song, or nothing when none were found anywhere.
pub async fn lyrics_of(state: &AppState, who: &User, song: WorkId) -> Result<Option<SongLyrics>> {
    may_read_the_work(state, who, song).await?;
    let database = state.database();

    let file = database.music_song_file(song).await?;
    tracing::debug!(
        %song,
        file = file.as_ref().map(|file| file.path.display().to_string()),
        "lyrics: asked for, looking in the file and beside it first"
    );
    if let Some(file) = file
        && let Some(found) = near(&file.path).await
    {
        tracing::debug!(%song, source = found.source.as_str(), "lyrics: found near the song, nothing is asked online");
        return Ok(Some(found));
    }
    database
        .forget_unknown_lyrics_out_of_date(song, LOOKUP_METHOD, UNKNOWN_BELIEVED_SECONDS)
        .await?;
    if let Some(kept) = database.looked_up_lyrics(song).await? {
        tracing::debug!(
            %song,
            plain_chars = kept.plain.as_ref().map(|words| words.len()),
            synced_chars = kept.synced.as_ref().map(|words| words.len()),
            instrumental = kept.instrumental,
            "lyrics: an earlier online answer is kept, so LRCLIB is not asked again"
        );
        return Ok(online(&kept));
    }

    let Some(asked) = database.song_to_look_up(song).await? else {
        tracing::debug!(%song, "lyrics: this work is not a song the database can look up, so nothing is asked");
        return Ok(None);
    };
    let online_allowed = database
        .music_library_options(asked.library_id)
        .await?
        .lyrics_online;
    tracing::debug!(
        %song,
        title = %asked.title,
        artist = asked.artist.as_deref(),
        album = asked.album.as_deref(),
        duration_ms = asked.duration_ms,
        online_allowed,
        "lyrics: what the database knows of the song"
    );
    if !online_allowed {
        tracing::debug!(%song, "lyrics: looking up online is switched off for this library, so nothing is asked");
        return Ok(None);
    }
    let Some(artist) = asked.artist.as_deref() else {
        tracing::debug!(%song, "lyrics: the song has no artist credit, and LRCLIB cannot be asked without one");
        return Ok(None);
    };
    let client = match LrcLibClient::new() {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(error = %error, "LRCLIB could not be prepared");
            return Ok(None);
        }
    };
    let answer = client
        .lyrics(&Asked {
            artist,
            title: &asked.title,
            album: asked.album.as_deref(),
            seconds: asked
                .duration_ms
                .and_then(|ms| u64::try_from(ms / 1000).ok()),
        })
        .await;
    let kept = match answer {
        Ok(Some(found)) => {
            tracing::debug!(
                %song,
                plain_chars = found.plain.as_ref().map(|words| words.len()),
                synced_chars = found.synced.as_ref().map(|words| words.len()),
                instrumental = found.instrumental,
                "lyrics: LRCLIB knows the song, and its answer is kept"
            );
            LookedUpLyrics {
                plain: found.plain,
                synced: found.synced,
                instrumental: found.instrumental,
            }
        }
        Ok(None) => {
            tracing::debug!(%song, "lyrics: LRCLIB does not know the song as asked, and that is kept: it is not asked again");
            LookedUpLyrics::default()
        }
        Err(error) => {
            tracing::debug!(
                song = %asked.title,
                error = %error,
                "LRCLIB gave no answer, the song is asked again another time"
            );
            return Ok(None);
        }
    };
    database
        .keep_looked_up_lyrics(song, &kept, melyxar_core::time::now(), LOOKUP_METHOD)
        .await?;
    Ok(online(&kept))
}

/// The way songs are looked up, a number that rises each time finding them
/// gets better: an answer of "unknown" kept by an older way is asked again,
/// instead of being believed for good.
const LOOKUP_METHOD: i64 = 2;

/// How long an "unknown" is believed: LRCLIB gains songs every day.
const UNKNOWN_BELIEVED_SECONDS: i64 = 30 * 24 * 3600;

/// What an administrator may do about the lyrics of a song, beyond what is
/// looked up on its own: search LRCLIB by hand, take one of its entries as
/// the words of the song, or forget what was taken so the song is looked up
/// again.
fn may_manage(who: &User) -> Result<()> {
    match who.permissions.is_administrator {
        true => Ok(()),
        false => Err(AppError::Domain(melyxar_core::Error::forbidden(
            "only an administrator looks for lyrics",
        ))),
    }
}

/// What went wrong with LRCLIB, in the words a screen can say.
fn said(error: ProviderError) -> AppError {
    tracing::warn!(%error, "LRCLIB would not answer");
    let code = match error {
        ProviderError::TooManyRequests { .. } => melyxar_core::error::ErrorCode::TooManyAttempts,
        _ => melyxar_core::error::ErrorCode::ExternalServiceUnavailable,
    };
    AppError::Domain(melyxar_core::Error::new(code, error.to_string()))
}

/// What LRCLIB holds under an artist and a title, searched by hand. Looked up
/// whatever the library says about looking up lyrics online: that setting is
/// for what is asked without anybody asking.
pub async fn offers(state: &AppState, who: &User, song: WorkId, artist: &str, title: &str) -> Result<Vec<Offer>> {
    may_manage(who)?;
    may_read_the_work(state, who, song).await?;
    if title.trim().is_empty() {
        return Ok(Vec::new());
    }
    tracing::debug!(%song, artist, title, "lyrics: searched for by hand");
    let client = LrcLibClient::new().map_err(said)?;
    client.offers(artist.trim(), title.trim()).await.map_err(said)
}

/// Takes one entry of LRCLIB as the words of a song, kept like any answer of
/// LRCLIB so the song is not looked up again over it.
pub async fn choose(state: &AppState, who: &User, song: WorkId, entry: i64) -> Result<()> {
    may_manage(who)?;
    may_read_the_work(state, who, song).await?;
    tracing::debug!(%song, entry, "lyrics: an entry of LRCLIB is taken by hand");
    let client = LrcLibClient::new().map_err(said)?;
    let Some(found) = client.entry(entry).await.map_err(said)? else {
        return Err(AppError::Domain(melyxar_core::Error::not_found("that entry of LRCLIB is gone")));
    };
    state
        .database()
        .keep_looked_up_lyrics(
            song,
            &LookedUpLyrics { plain: found.plain, synced: found.synced, instrumental: found.instrumental },
            melyxar_core::time::now(),
            LOOKUP_METHOD,
        )
        .await?;
    Ok(())
}

/// Forgets what was kept for a song, taken by hand or found on its own, so
/// that the next listen looks it up again.
pub async fn forget(state: &AppState, who: &User, song: WorkId) -> Result<()> {
    may_manage(who)?;
    may_read_the_work(state, who, song).await?;
    tracing::debug!(%song, "lyrics: what was kept online is forgotten");
    state.database().forget_looked_up_lyrics(song).await?;
    Ok(())
}

/// The words written inside the song, or in a file of the same name beside
/// it.
async fn near(path: &Path) -> Option<SongLyrics> {
    let inside = {
        let path = path.to_path_buf();
        tokio::task::spawn_blocking(move || melyxar_tags::lyrics(&path))
            .await
            .ok()
            .and_then(|read| read.ok())
            .flatten()
    };
    tracing::debug!(
        file = %path.display(),
        chars = inside.as_ref().map(|text| text.len()),
        "lyrics: what the file carries inside it"
    );
    if let Some(found) = inside
        .map(|text| read_lyrics(&text))
        .filter(|found| !found.is_empty())
    {
        return Some(SongLyrics {
            lyrics: found,
            source: Source::Song,
            instrumental: false,
        });
    }
    let beside_path = path.with_extension("lrc");
    let beside = tokio::fs::read_to_string(&beside_path).await;
    tracing::debug!(
        file = %beside_path.display(),
        result = ?beside.as_ref().map(|text| text.len()).map_err(|error| error.kind()),
        "lyrics: the file of the same name beside the song"
    );
    let beside = beside.ok()?;
    let found = read_lyrics(&beside);
    (!found.is_empty()).then_some(SongLyrics {
        lyrics: found,
        source: Source::Beside,
        instrumental: false,
    })
}

/// What LRCLIB answered, as lyrics: the stamped words when it has them.
fn online(kept: &LookedUpLyrics) -> Option<SongLyrics> {
    let lyrics = kept
        .synced
        .as_deref()
        .or(kept.plain.as_deref())
        .map(read_lyrics)
        .unwrap_or_default();
    (!lyrics.is_empty() || kept.instrumental).then_some(SongLyrics {
        lyrics,
        source: Source::Online,
        instrumental: kept.instrumental,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn words_beside_a_song_are_found_under_its_name() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let song = directory.path().join("01 Tides.mp3");
        std::fs::write(&song, b"not really a song").expect("written");
        assert_eq!(near(&song).await, None);

        std::fs::write(
            directory.path().join("01 Tides.lrc"),
            "[00:01.00]Low tide\n",
        )
        .expect("written");
        let found = near(&song).await.expect("found");
        assert_eq!(found.source, Source::Beside);
        assert_eq!(found.lyrics.plain, "Low tide");
    }

    #[tokio::test]
    async fn only_an_administrator_changes_the_lyrics_of_a_song() {
        let (_directory, state) = crate::an_empty_server().await;
        let viewer = crate::a_viewer(&state).await;
        let song = WorkId::new();

        let refused = |outcome: Result<()>| {
            matches!(outcome, Err(AppError::Domain(error)) if error.to_string().contains("only an administrator"))
        };
        assert!(refused(offers(&state, &viewer, song, "Artist", "Title").await.map(|_| ())));
        assert!(refused(choose(&state, &viewer, song, 7).await));
        assert!(refused(forget(&state, &viewer, song).await));

        let administrator = state
            .database()
            .create_user("Boss", None, &melyxar_core::user::Permissions::administrator())
            .await
            .expect("account created");
        forget(&state, &administrator, song).await.expect("nothing kept, nothing to forget");
        assert_eq!(
            offers(&state, &administrator, song, "Artist", "  ").await.expect("nothing asked"),
            Vec::new(),
            "without a title nothing is searched for"
        );
    }

    #[test]
    fn an_online_answer_prefers_the_stamped_words_and_says_a_song_has_none() {
        let both = online(&LookedUpLyrics {
            plain: Some("Plain".to_string()),
            synced: Some("[00:02.00]Stamped".to_string()),
            instrumental: false,
        })
        .expect("found");
        assert_eq!(both.lyrics.synced.len(), 1);
        assert_eq!(both.lyrics.plain, "Stamped");

        let wordless = online(&LookedUpLyrics {
            instrumental: true,
            ..LookedUpLyrics::default()
        })
        .expect("said");
        assert!(wordless.instrumental);
        assert_eq!(online(&LookedUpLyrics::default()), None);
    }
}
