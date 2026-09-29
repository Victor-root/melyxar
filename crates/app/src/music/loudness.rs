//! Measuring how loud each song of a library is, for every song to be
//! played as loud as any other.
//!
//! A song whose tags already say it (ReplayGain) is never read. The others
//! are read through, a few at a time, as a job of its own that a scan of the
//! library starts once it is over and that a restart takes up where it was:
//! each batch asks what is left, so nothing about where it got to has to be
//! written down.

use melyxar_core::job::{JobKind, JobPriority, JobStep};
use melyxar_core::library::Library;
use melyxar_ffmpeg::AskedToStop;
use melyxar_ffmpeg::song_loudness::measure;
use melyxar_jobs::JobHandle;

use crate::{AppState, Result};

/// How many songs are asked for at a time.
const IN_ONE_BATCH: i64 = 32;

/// Starts measuring the songs of a library nobody measured yet, unless none
/// is left, which is most scans.
pub async fn start_when_needed(
    state: &AppState,
    library: Library,
    priority: JobPriority,
) -> Result<()> {
    if state.tools().is_none() || state.database().count_songs_to_measure(library.id).await? == 0 {
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
                let measured = measure_the_songs_of(&owned, &library, &handle)
                    .await
                    .map_err(|error| error.to_string())?;
                tracing::info!(
                    library = library.name,
                    songs = measured,
                    "the songs of this library were measured"
                );
                Ok(())
            },
        )
        .await?)
}

async fn measure_the_songs_of(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    let Some(tools) = state.tools() else {
        return Ok(0);
    };
    let database = state.database();
    let mut waiting = database.count_songs_to_measure(library.id).await?;
    if waiting == 0 {
        return Ok(0);
    }
    handle.at_step(JobStep::MeasuringSongs).await;
    handle.set_total(waiting).await;

    let mut measured = 0;
    loop {
        if handle.is_cancelled() {
            break;
        }
        let batch = database.songs_to_measure(library.id, IN_ONE_BATCH).await?;
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
                    let loudness = match measure(&tool, &song.path, asked_to_stop).await {
                        Ok(loudness) => Some(loudness),
                        Err(melyxar_ffmpeg::FfmpegError::GivenUp) => return false,
                        Err(error) => {
                            tracing::warn!(
                                file = %song.path.display(),
                                %error,
                                "a song could not be measured, and is left as loud as it is"
                            );
                            None
                        }
                    };
                    let written = database
                        .write_song_loudness(
                            song.source_id,
                            loudness.as_ref(),
                            melyxar_core::time::now(),
                        )
                        .await
                        .is_ok();
                    handle.advance(1).await;
                    written && loudness.is_some()
                }
            },
        )
        .await;
        measured += done.iter().filter(|measured| **measured).count();

        // What is left is asked for again, and is the only sign the reading
        // moves: a batch that wrote nothing down would come back for ever.
        let left = database.count_songs_to_measure(library.id).await?;
        if left >= waiting {
            if !handle.is_cancelled() {
                tracing::warn!(
                    library = library.name,
                    waiting = left,
                    "nothing of this batch could be written down, so the measuring stops here"
                );
            }
            break;
        }
        waiting = left;
    }
    Ok(measured)
}
