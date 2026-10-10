//! Subtitles written by listening, translated into French.
//!
//! The administrator has the model that translates downloaded, as a job like
//! any other so the page shows how far it has got. A task of its own then
//! translates, of a night or when somebody asks, the English subtitles that
//! listening wrote: never during the listening, and never while somebody
//! watches. The translation is kept as one more subtitle of the video, marked
//! as generated like the one it comes from, so the player offers both and the
//! person watching picks.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use melyxar_core::id::{MediaSourceId, TrackId};
use melyxar_core::job::{JobKind, JobPriority, JobStep};
use melyxar_core::library::Library;
use melyxar_core::media::normalise_language;
use melyxar_jobs::JobHandle;
use melyxar_metadata::speech_models::DownloadError;
use melyxar_metadata::translation_models;
use melyxar_translate::{parse, render, translate_cues, Engine};

use crate::speech::generated_track;
use crate::{AppError, AppState, Result};

/// The word the job and the page know the one model by.
const MODEL: &str = "en-fr";

/// How many videos are asked for at a time. They are translated one after the
/// other.
const IN_ONE_BATCH: i64 = 50;

/// A mebibyte, the unit the download is counted in so its numbers stay small.
const MEBIBYTE: u64 = 1024 * 1024;

/// How often the progress of a download is passed on.
const EVERY: Duration = Duration::from_secs(1);

/// What the page of the translation shows.
#[derive(Debug, Clone)]
pub struct Status {
    /// What the model weighs.
    pub bytes: u64,
    /// Whether every file of it is on the disk.
    pub downloaded: bool,
    /// Whether a download of it is under way.
    pub downloading: bool,
}

pub async fn status(state: &AppState) -> Result<Status> {
    Ok(Status {
        bytes: translation_models::total_bytes(),
        downloaded: translation_models::is_whole(&state.config().directories.translation_model()),
        downloading: state
            .database()
            .has_unfinished_job(JobKind::DownloadTranslationModel, Some(MODEL))
            .await?,
    })
}

/// The folder of the model, when all of it is there. Nothing waits for a model
/// that is not: the task counts nought rather than a queue nothing will empty.
pub fn ready(state: &AppState) -> Option<PathBuf> {
    let folder = state.config().directories.translation_model();
    translation_models::is_whole(&folder).then_some(folder)
}

/// Takes the model off the disk. Subtitles already translated stay.
pub async fn forget(state: &AppState) -> Result<()> {
    match tokio::fs::remove_dir_all(state.config().directories.translation_model()).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::Directory(error)),
    }
}

/// Starts downloading the model, as a job the page can follow and stop.
pub async fn download(state: &AppState) -> Result<melyxar_jobs::StartedJob> {
    let owned = state.clone();
    Ok(state
        .jobs()
        .clone()
        .start(
            JobKind::DownloadTranslationModel,
            JobPriority::REQUESTED,
            Some(MODEL.to_string()),
            move |handle| async move { download_it(&owned, &handle).await.map_err(|error| error.to_string()) },
        )
        .await?)
}

async fn download_it(state: &AppState, handle: &JobHandle) -> Result<()> {
    let folder = state.config().directories.translation_model();
    if translation_models::is_whole(&folder) {
        return Ok(());
    }

    handle.at_step(JobStep::DownloadingTranslationModel).await;
    handle.now_working_on(Some(MODEL)).await;
    handle.set_total((translation_models::total_bytes() / MEBIBYTE) as i64 + 1).await;

    // The download reports into a number the job reads once a second: its
    // callback is not async, and a write to the database for every chunk would
    // be a write for every few kilobytes.
    let seen = Arc::new(AtomicU64::new(0));
    let reporting = seen.clone();
    let stopped = handle.clone();
    let mut downloading = std::pin::pin!(translation_models::download(
        &folder,
        move |done| reporting.store(done, Ordering::Relaxed),
        move || stopped.is_cancelled(),
    ));
    let mut reported = 0u64;
    let mut tick = tokio::time::interval(EVERY);
    let outcome = loop {
        tokio::select! {
            outcome = &mut downloading => break outcome,
            _ = tick.tick() => {
                let now = seen.load(Ordering::Relaxed) / MEBIBYTE;
                if now > reported {
                    handle.advance((now - reported) as i64).await;
                    reported = now;
                }
            }
        }
    };
    match outcome {
        Ok(()) => {
            tracing::info!("the model that translates was downloaded");
            Ok(())
        }
        // A stop somebody asked for is not a fault, and the job says so on its
        // own when it ends.
        Err(DownloadError::Stopped) => Ok(()),
        Err(error) => {
            tracing::warn!(%error, "the model that translates could not be downloaded");
            Err(AppError::Domain(melyxar_core::Error::new(
                melyxar_core::error::ErrorCode::Internal,
                error.to_string(),
            )))
        }
    }
}

/// Translates the subtitles of a library that listening wrote in English and
/// nobody has translated, as a job of its own. Translating is one of the
/// upkeep's readings, so it is started the way the others are; this is what
/// they call.
pub(crate) async fn translate_the_subtitles_of(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    let Some(model) = ready(state) else {
        tracing::debug!(library = library.name, "no model that translates here, so nothing is translated");
        return Ok(0);
    };
    let database = state.database();
    let waiting = database.count_awaiting_translation(library.id).await?;
    if waiting == 0 {
        return Ok(0);
    }

    handle.at_step(JobStep::TranslatingSubtitles).await;
    // Counted as itself in per cent when it is the only one, which is a few
    // minutes of listening to a model with nothing to show.
    handle.size_up(0, waiting).await;
    let threads = crate::speech::effort(state)
        .await?
        .threads(std::thread::available_parallelism().map_or(2, usize::from));

    let mut translated = 0;
    let mut still_waiting = waiting;
    loop {
        if handle.is_cancelled() {
            break;
        }
        let batch = database.subtitles_awaiting_translation(library.id, IN_ONE_BATCH).await?;
        if batch.is_empty() {
            break;
        }
        for (source_id, file) in batch {
            if handle.is_cancelled() {
                break;
            }
            if let Ok(source) = crate::playable_file(database, source_id).await {
                let name = source.path.file_name().map(|name| name.to_string_lossy().into_owned());
                handle.now_working_on(name.as_deref()).await;
            }
            match translate_one(state, &model, source_id, &file, threads, handle).await {
                Ok(lines) => {
                    translated += 1;
                    tracing::debug!(library = library.name, lines, "a subtitle was translated");
                }
                // Left waiting, said once, and the pass says it is stuck if
                // nothing moves: a translation that could not happen is not
                // an answer about the file.
                Err(error) => tracing::warn!(
                    library = library.name,
                    %error,
                    "a subtitle could not be translated"
                ),
            }
            handle.advance(1).await;
        }
        let left = database.count_awaiting_translation(library.id).await?;
        if left >= still_waiting {
            tracing::warn!(
                library = library.name,
                left,
                "subtitles are left that could not be translated, so this pass stops"
            );
            break;
        }
        still_waiting = left;
    }
    if translated > 0 {
        tracing::info!(library = library.name, videos = translated, "the subtitles of this library were translated");
    }
    Ok(translated)
}

/// Translates the English subtitle kept in `file` and keeps the result as one
/// more subtitle of the video. Answers how many lines there are.
async fn translate_one(
    state: &AppState,
    model: &std::path::Path,
    source_id: MediaSourceId,
    file: &str,
    threads: usize,
    handle: &JobHandle,
) -> Result<usize> {
    let database = state.database();
    let folder = state.config().directories.downloaded_subtitles();
    let cues = parse(&tokio::fs::read_to_string(folder.join(file)).await?);
    if cues.is_empty() {
        database.store_translation(source_id).await?;
        return Ok(0);
    }

    // The model is opened for the video and let go of with it: it takes less
    // than a second to load, and nothing holds its memory between two videos.
    let model = model.to_path_buf();
    let telling = handle.clone();
    let translated = tokio::task::spawn_blocking(move || -> std::result::Result<_, melyxar_translate::Error> {
        let engine = Engine::open(&model, threads)?;
        translate_cues(&cues, |lines| engine.translate(lines), |share| telling.element_at(share))
    })
    .await
    .map_err(|error| AppError::Domain(melyxar_core::Error::new(melyxar_core::error::ErrorCode::Internal, error.to_string())))?
    .map_err(|error| AppError::Domain(melyxar_core::Error::new(melyxar_core::error::ErrorCode::Internal, error.to_string())))?;
    if translated.is_empty() {
        database.store_translation(source_id).await?;
        return Ok(0);
    }

    let track_id = TrackId::new();
    let written = format!("{track_id}.srt");
    tokio::fs::write(folder.join(&written), render(&translated).as_bytes()).await?;
    let track = generated_track(track_id, source_id, Some(normalise_language("fr")), &written);
    let kept = async {
        database.add_downloaded_subtitle(source_id, &track).await?;
        if let Err(error) = database.store_translation(source_id).await {
            // Nothing says this translation is done, so it must not stay
            // beside the one that will be made next time.
            let _ = database.remove_downloaded_subtitle(source_id, track_id).await;
            return Err(error);
        }
        Ok(())
    }
    .await;
    if let Err(error) = kept {
        // The file is nobody's if its track was not written.
        let _ = tokio::fs::remove_file(folder.join(&written)).await;
        return Err(error.into());
    }
    Ok(translated.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::speech::tests::a_server;

    /// The model on the disk: sparse files of exactly the right sizes.
    fn put_on_the_disk(state: &AppState) {
        let folder = state.config().directories.translation_model();
        std::fs::create_dir_all(&folder).expect("folder");
        for file in translation_models::FILES {
            let made = std::fs::File::create(folder.join(file.name)).expect("created");
            made.set_len(file.bytes).expect("a sparse file");
        }
    }

    #[tokio::test]
    async fn the_model_is_absent_until_every_file_of_it_is_there() {
        let (_directory, state) = a_server().await;
        let shown = status(&state).await.expect("status");
        assert!(!shown.downloaded && !shown.downloading);
        assert_eq!(shown.bytes, translation_models::total_bytes());
        assert!(ready(&state).is_none());

        put_on_the_disk(&state);
        assert!(status(&state).await.expect("status").downloaded);
        assert!(ready(&state).is_some());
    }

    #[tokio::test]
    async fn forgetting_the_model_takes_it_off_the_disk_and_is_no_trouble_when_it_is_not_there() {
        let (_directory, state) = a_server().await;
        forget(&state).await.expect("nothing to forget is not a fault");
        put_on_the_disk(&state);
        forget(&state).await.expect("forgotten");
        assert!(ready(&state).is_none());
        assert!(!status(&state).await.expect("status").downloaded);
    }

    #[tokio::test]
    async fn nothing_waits_for_a_translation_that_has_no_model() {
        let (_directory, state) = a_server().await;
        let library = melyxar_core::library::Library {
            id: melyxar_core::id::LibraryId::new(),
            name: "Home".to_string(),
            kind: melyxar_core::library::LibraryKind::HomeMedia,
            metadata_language: "en".to_string(),
            options: melyxar_core::library::LibraryOptions::default(),
            roots: Vec::new(),
        };
        assert_eq!(
            crate::upkeep::what_is_waiting_for(&state, crate::upkeep::UpkeepTask::Translation, library.id)
                .await
                .expect("counted"),
            0
        );
    }
}
