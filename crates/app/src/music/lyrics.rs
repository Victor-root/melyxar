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

use crate::reach::may_read_the_work;
use crate::{AppState, Result};

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

    if let Some(file) = database.music_song_file(song).await?
        && let Some(found) = near(&file.path).await
    {
        return Ok(Some(found));
    }
    if let Some(kept) = database.looked_up_lyrics(song).await? {
        return Ok(online(&kept));
    }

    let Some(asked) = database.song_to_look_up(song).await? else {
        return Ok(None);
    };
    if !database
        .music_library_options(asked.library_id)
        .await?
        .lyrics_online
    {
        return Ok(None);
    }
    let Some(artist) = asked.artist.as_deref() else {
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
        Ok(Some(found)) => LookedUpLyrics {
            plain: found.plain,
            synced: found.synced,
            instrumental: found.instrumental,
        },
        Ok(None) => LookedUpLyrics::default(),
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
        .keep_looked_up_lyrics(song, &kept, melyxar_core::time::now())
        .await?;
    Ok(online(&kept))
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
    let beside = tokio::fs::read_to_string(path.with_extension("lrc"))
        .await
        .ok()?;
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
