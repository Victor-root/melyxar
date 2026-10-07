//! Sending a song to somebody listening.
//!
//! As it lies on the disk when their browser plays it, which is nearly every
//! song; converted on the way otherwise. Nothing of the sessions films are
//! rebuilt in: a song is one file, sent once.

use std::path::PathBuf;

use melyxar_core::id::WorkId;
use melyxar_core::music::Spectrum;
use melyxar_core::music::plays_as_it_is;
use melyxar_core::music_preferences::under_the_ceiling;
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

/// How the sound of a song is spread, or nothing when it was not read yet or
/// could not be.
pub async fn spectrum_of_song(
    state: &AppState,
    who: &User,
    song: WorkId,
) -> Result<Option<Spectrum>> {
    may_read_the_work(state, who, song).await?;
    Ok(state.database().song_spectrum(song).await?)
}

/// The sound of a song, for a browser that plays the forms named in `plays`,
/// from `start` on when it has to be converted. Nothing for a song that is
/// not there, or whose every file is missing.
///
/// A song heavier than the account's ceiling is converted down to it even
/// when the browser plays it as it is: the ceiling is there for a slow way
/// in, where the file itself would stutter.
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
    let ceiling = state
        .database()
        .music_preferences(who.id)
        .await?
        .max_bitrate_kbps;
    if file
        .codec
        .as_deref()
        .is_some_and(|codec| plays_as_it_is(codec, file.container.as_deref(), plays))
        && under_the_ceiling(file.bitrate, ceiling)
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
    let kbps = ceiling.map_or(into.usual_kbps(), |most| most.min(into.usual_kbps()));
    tracing::debug!(
        file = %file.path.display(),
        codec = file.codec.as_deref().unwrap_or("unknown"),
        into = into.content_type(),
        kbps,
        start_ms = start.get(),
        "a song is converted on the way"
    );
    let song = convert(tools, &file.path, start, into, kbps)?;
    Ok(Some(Sound::Converted { song, into }))
}
