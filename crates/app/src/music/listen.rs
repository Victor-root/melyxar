//! Sending a song to somebody listening.
//!
//! As it lies on the disk when their browser plays it, which is nearly every
//! song; converted on the way otherwise. Nothing of the sessions films are
//! rebuilt in: a song is one file, sent once.

use std::path::PathBuf;

use melyxar_core::id::WorkId;
use melyxar_core::music::plays_as_it_is;
use melyxar_core::time::Millis;
use melyxar_core::user::User;
use melyxar_ffmpeg::listening::{Converted, ConvertedSong, convert};

use crate::reach::may_read_the_work;
use crate::{AppError, AppState, Result};

/// How a song goes out.
pub enum Sound {
    /// The file itself, which the browser moves about in on its own.
    AsItIs(PathBuf),
    /// Converted from where it was asked, into what the browser plays.
    Converted {
        song: ConvertedSong,
        into: Converted,
    },
}

/// The sound of a song, for a browser that plays the forms named in `plays`,
/// from `start` on when it has to be converted. Nothing for a song that is
/// not there, or whose every file is missing.
pub async fn sound_of(
    state: &AppState,
    who: &User,
    song: WorkId,
    plays: &[&str],
    start: Millis,
) -> Result<Option<Sound>> {
    may_read_the_work(state, who, song).await?;
    let Some(file) = state.database().music_song_file(song).await? else {
        return Ok(None);
    };
    if file
        .codec
        .as_deref()
        .is_some_and(|codec| plays_as_it_is(codec, file.container.as_deref(), plays))
    {
        return Ok(Some(Sound::AsItIs(file.path)));
    }

    let Some(tools) = state.tools() else {
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::DependencyMissing,
            "no media tool to convert this song with",
        )));
    };
    let into = match plays.contains(&"opus") {
        true => Converted::Opus,
        false => Converted::Mp3,
    };
    tracing::debug!(
        file = %file.path.display(),
        codec = file.codec.as_deref().unwrap_or("unknown"),
        into = into.content_type(),
        start_ms = start.get(),
        "a song the browser does not play is converted on the way"
    );
    let song = convert(&tools.ffmpeg, &file.path, start, into)?;
    Ok(Some(Sound::Converted { song, into }))
}
