//! Reading the songs of a library for how loud each is, for every song to be
//! played as loud as any other, and for how its sound is spread, for the wave
//! behind the player.
//!
//! A song is read through once for both. One whose tags already say how loud
//! it is (ReplayGain) is read for its sound alone, and one with both is never
//! read. The reading goes a few songs at a time, as a job of its own that a
//! scan of the library starts once it is over and that a restart takes up
//! where it was: each batch asks what is left, so nothing about where it got
//! to has to be written down.

use std::path::Path;

use melyxar_core::job::{JobKind, JobPriority, JobStep};
use melyxar_core::library::Library;
use melyxar_core::media::Loudness;
use melyxar_core::music::Spectrum;
use melyxar_database::Database;
use melyxar_database::music_analysis::SongToAnalyse;
use melyxar_ffmpeg::AskedToStop;
use melyxar_ffmpeg::song_analysis::{Wanted, analyse};
use melyxar_jobs::JobHandle;
use melyxar_sound::{SPECTRUM_BANDS, SPECTRUM_FRAMES_A_SECOND, spectrum_of};

use crate::{AppState, Result};

/// How many songs are asked for at a time.
const IN_ONE_BATCH: i64 = 32;

/// Starts reading the songs of a library nobody read yet, unless none is
/// left, which is most scans.
pub async fn start_when_needed(
    state: &AppState,
    library: Library,
    priority: JobPriority,
) -> Result<()> {
    if state.tools().is_none() || state.database().count_songs_to_analyse(library.id).await? == 0 {
        return Ok(());
    }
    start(state, library, priority).await.map(|_| ())
}

pub async fn start(
    state: &AppState,
    library: Library,
    priority: JobPriority,
) -> Result<melyxar_jobs::StartedJob> {
    let owned = state.clone();
    let target = library.id.to_string();
    Ok(state
        .jobs()
        .clone()
        .start(
            JobKind::AnalyseLoudness,
            priority,
            Some(target),
            move |handle| async move {
                let analysed = analyse_the_songs_of(&owned, &library, &handle)
                    .await
                    .map_err(|error| error.to_string())?;
                tracing::info!(
                    library = library.name,
                    songs = analysed,
                    "the songs of this library were read"
                );
                Ok(())
            },
        )
        .await?)
}

async fn analyse_the_songs_of(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    let Some(tools) = state.tools() else {
        return Ok(0);
    };
    let database = state.database();
    let mut waiting = database.count_songs_to_analyse(library.id).await?;
    if waiting == 0 {
        return Ok(0);
    }
    handle.at_step(JobStep::MeasuringSongs).await;
    handle.set_total(waiting).await;

    let mut analysed = 0;
    loop {
        if handle.is_cancelled() {
            break;
        }
        let batch = database.songs_to_analyse(library.id, IN_ONE_BATCH).await?;
        if batch.is_empty() {
            break;
        }
        let tool = tools.ffmpeg.clone();
        let owned_database = database.clone();
        let owned_handle = handle.clone();
        let asked_to_stop = AskedToStop::when(handle.cancelled_when());
        let done = melyxar_jobs::for_each_bounded(
            batch,
            state.config().limits.concurrent_probes,
            move |song| {
                let tool = tool.clone();
                let database = owned_database.clone();
                let handle = owned_handle.clone();
                let asked_to_stop = asked_to_stop.clone();
                async move {
                    if handle.is_cancelled() {
                        return false;
                    }
                    let name = song
                        .path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned());
                    handle.now_working_on(name.as_deref()).await;
                    let Some(outcome) = read(&tool, &song, asked_to_stop).await else {
                        return false;
                    };
                    let gave_something = outcome.loudness.is_some() || outcome.spectrum.is_some();
                    let written = write_down(&database, &song, outcome).await;
                    handle.advance(1).await;
                    written && gave_something
                }
            },
        )
        .await;
        analysed += done.iter().filter(|analysed| **analysed).count();

        // What is left is asked for again, and is the only sign the reading
        // moves: a batch that wrote nothing down would come back for ever.
        let left = database.count_songs_to_analyse(library.id).await?;
        if left >= waiting {
            if !handle.is_cancelled() {
                tracing::warn!(
                    library = library.name,
                    waiting = left,
                    "nothing of this batch could be written down, so the reading stops here"
                );
            }
            break;
        }
        waiting = left;
    }
    Ok(analysed)
}

/// What a song gave when it was read: nothing of what was asked when no tool
/// could read it.
struct Outcome {
    loudness: Option<Loudness>,
    spectrum: Option<Spectrum>,
}

/// Reads a song for what it lacks, or nothing when the reading was stopped.
async fn read(tool: &Path, song: &SongToAnalyse, asked_to_stop: AskedToStop) -> Option<Outcome> {
    let wanted = Wanted {
        loudness: song.needs_loudness,
        spectrum: song.needs_spectrum,
    };
    match analyse(tool, &song.path, wanted, asked_to_stop).await {
        Ok(analysis) => Some(Outcome {
            loudness: analysis.loudness,
            spectrum: if song.needs_spectrum {
                spectrum_of_samples(analysis.samples).await
            } else {
                None
            },
        }),
        Err(melyxar_ffmpeg::FfmpegError::GivenUp) => None,
        Err(error) => {
            tracing::warn!(
                file = %song.path.display(),
                %error,
                "a song could not be read, and is left as it is"
            );
            Some(Outcome {
                loudness: None,
                spectrum: None,
            })
        }
    }
}

/// The levels of the sound, written down off the threads that serve requests:
/// a song of minutes is some thousands of transforms.
async fn spectrum_of_samples(samples: Vec<i16>) -> Option<Spectrum> {
    let levels = tokio::task::spawn_blocking(move || spectrum_of(&samples))
        .await
        .ok()?;
    (!levels.is_empty()).then(|| Spectrum {
        bands: u8::try_from(SPECTRUM_BANDS).unwrap_or(u8::MAX),
        frames_a_second: u8::try_from(SPECTRUM_FRAMES_A_SECOND).unwrap_or(u8::MAX),
        levels,
    })
}

/// Keeps what was read, for what was asked: either way the file is not read
/// again until it changes. Whether everything could be written.
async fn write_down(database: &Database, song: &SongToAnalyse, outcome: Outcome) -> bool {
    let at = melyxar_core::time::now();
    let mut written = true;
    if song.needs_loudness {
        written &= database
            .write_song_loudness(song.source_id, outcome.loudness.as_ref(), at)
            .await
            .is_ok();
    }
    if song.needs_spectrum {
        written &= database
            .write_song_spectrum(song.source_id, outcome.spectrum.as_ref(), at)
            .await
            .is_ok();
    }
    written
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_tool_and_the_spectrum_read_sound_at_the_same_rate() {
        assert_eq!(
            melyxar_ffmpeg::song_analysis::SPECTRUM_SAMPLES_A_SECOND,
            melyxar_sound::SPECTRUM_SAMPLES_A_SECOND
        );
    }
}
