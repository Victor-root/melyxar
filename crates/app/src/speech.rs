//! Subtitles written by listening to a video: the models, and the listening.
//!
//! The administrator chooses a model among the few the server offers and has
//! it downloaded, as a job like any other so the page shows how far it has
//! got. A library of personal videos may then ask for its videos that have no
//! subtitle to be listened to, as one more of the upkeep's readings: of a
//! night, one video at a time, at the lowest priority there is.
//!
//! What comes out is kept like a subtitle downloaded for the film, as a file
//! of the server's own and a track marked as generated, so the player offers
//! it with nothing new to learn and says it was not written by a person.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use melyxar_core::id::{MediaSourceId, TrackId};
use melyxar_core::job::{JobKind, JobPriority, JobStep};
use melyxar_core::library::Library;
use melyxar_core::media::{normalise_language, SubtitleDetails, SubtitleLayout, Track, TrackKind};
use melyxar_ffmpeg::speech::{listen, SpeechTool};
use melyxar_ffmpeg::{AskedToStop, FfmpegError};
use melyxar_jobs::JobHandle;
use melyxar_metadata::speech_models::{self, DownloadError, Model};

use crate::{AppError, AppState, Result};

/// How many videos are asked for at a time. They are listened to one after the
/// other: the largest model holds a few gigabytes in memory while it listens.
const IN_ONE_BATCH: i64 = 50;

/// A mebibyte, the unit the download is counted in so its numbers stay small.
const MEBIBYTE: u64 = 1024 * 1024;

/// How often the progress of a download is passed on.
const EVERY: Duration = Duration::from_secs(1);

/// What the page of the models shows.
#[derive(Debug, Clone)]
pub struct Status {
    /// Whether the tool that listens is on this machine.
    pub tool_found: bool,
    /// The model in use, once one has been chosen and is on the disk.
    pub chosen: Option<&'static str>,
    pub effort: Effort,
    pub models: Vec<ModelStatus>,
}

/// How much of the processor listening takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effort {
    /// Half of it, so the rest is left for whoever is watching something.
    Quiet,
    /// Three quarters of it.
    Balanced,
    /// All of it. Whoever is watching something feels it.
    Maximum,
}

impl Effort {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Quiet => "quiet",
            Self::Balanced => "balanced",
            Self::Maximum => "maximum",
        }
    }

    pub fn parse(word: &str) -> Option<Self> {
        [Self::Quiet, Self::Balanced, Self::Maximum]
            .into_iter()
            .find(|effort| effort.as_str() == word)
    }

    /// How many processor threads listening takes on a machine that has
    /// `available`. Past a point more threads do not make a model faster, so
    /// the quiet one stops at eight and the others at twelve.
    pub fn threads(self, available: usize) -> usize {
        match self {
            Self::Quiet => (available / 2).clamp(1, 8),
            Self::Balanced => (available * 3 / 4).clamp(1, 12),
            Self::Maximum => available.clamp(1, 12),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ModelStatus {
    pub id: &'static str,
    pub bytes: u64,
    /// Whether the whole file is on the disk.
    pub downloaded: bool,
    /// Whether a download of it is under way.
    pub downloading: bool,
}

/// What an administrator can be told to put right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// A model the server does not offer.
    UnknownModel,
    /// A model chosen before it was downloaded.
    NotDownloaded,
    /// An effort the server does not know.
    UnknownEffort,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnknownModel => "unknown_model",
            Self::NotDownloaded => "not_downloaded",
            Self::UnknownEffort => "unknown_effort",
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error("refused")]
    Refused(Refused),
    #[error(transparent)]
    Failed(#[from] AppError),
}

impl From<melyxar_database::DatabaseError> for Trouble {
    fn from(error: melyxar_database::DatabaseError) -> Self {
        Self::Failed(error.into())
    }
}

fn path_of(state: &AppState, model: &Model) -> PathBuf {
    state.config().directories.speech_models().join(model.file)
}

fn offered(id: &str) -> std::result::Result<&'static Model, Trouble> {
    speech_models::model(id).ok_or(Trouble::Refused(Refused::UnknownModel))
}

/// The tool that listens, if it is on this machine.
pub fn tool(state: &AppState) -> Option<SpeechTool> {
    SpeechTool::discover(state.config().media_tools.whisper_path.as_deref())
}

/// What the page shows: the tool, the model in use and each model's state.
pub async fn status(state: &AppState) -> Result<Status> {
    let chosen = state.database().speech_model().await?;
    let mut models = Vec::with_capacity(speech_models::MODELS.len());
    for model in &speech_models::MODELS {
        let downloading = state
            .database()
            .has_unfinished_job(JobKind::DownloadSpeechModel, Some(model.id))
            .await?;
        models.push(ModelStatus {
            id: model.id,
            bytes: model.bytes,
            downloaded: speech_models::is_whole(model, &path_of(state, model)),
            downloading,
        });
    }
    let chosen = chosen
        .as_deref()
        .and_then(speech_models::model)
        .filter(|model| models.iter().any(|status| status.id == model.id && status.downloaded))
        .map(|model| model.id);
    Ok(Status {
        tool_found: tool(state).is_some(),
        chosen,
        effort: effort(state).await?,
        models,
    })
}

/// How much of the processor listening takes. The usual one if what is kept
/// is a word this version does not know.
pub async fn effort(state: &AppState) -> Result<Effort> {
    Ok(Effort::parse(&state.database().speech_effort().await?).unwrap_or(Effort::Quiet))
}

/// Chooses how much of the processor listening takes.
pub async fn set_effort(state: &AppState, word: &str) -> std::result::Result<(), Trouble> {
    let effort = Effort::parse(word).ok_or(Trouble::Refused(Refused::UnknownEffort))?;
    state.database().set_speech_effort(effort.as_str()).await?;
    Ok(())
}

/// Chooses the model that listens, or none. One that is not on the disk
/// cannot be chosen: a library would then ask to be listened to and nothing
/// would ever be.
pub async fn choose(state: &AppState, id: Option<&str>) -> std::result::Result<(), Trouble> {
    match id {
        None => state.database().set_speech_model(None).await?,
        Some(id) => {
            let model = offered(id)?;
            if !speech_models::is_whole(model, &path_of(state, model)) {
                return Err(Trouble::Refused(Refused::NotDownloaded));
            }
            state.database().set_speech_model(Some(model.id)).await?;
        }
    }
    Ok(())
}

/// Takes a model off the disk. If it was the one in use, none is.
pub async fn forget(state: &AppState, id: &str) -> std::result::Result<(), Trouble> {
    let model = offered(id)?;
    if state.database().speech_model().await?.as_deref() == Some(model.id) {
        state.database().set_speech_model(None).await?;
    }
    match tokio::fs::remove_file(path_of(state, model)).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(Trouble::Failed(AppError::Directory(error))),
    }
}

/// Starts downloading a model, as a job the page can follow and stop.
pub async fn download(state: &AppState, id: &str) -> std::result::Result<melyxar_jobs::StartedJob, Trouble> {
    let model = offered(id)?;
    let owned = state.clone();
    let started = state
        .jobs()
        .clone()
        .start(
            JobKind::DownloadSpeechModel,
            JobPriority::REQUESTED,
            Some(model.id.to_string()),
            move |handle| async move {
                download_it(&owned, model, &handle)
                    .await
                    .map_err(|error| error.to_string())
            },
        )
        .await
        .map_err(AppError::from)?;
    Ok(started)
}

async fn download_it(state: &AppState, model: &'static Model, handle: &JobHandle) -> Result<()> {
    let to = path_of(state, model);
    if speech_models::is_whole(model, &to) {
        return Ok(());
    }
    tokio::fs::create_dir_all(state.config().directories.speech_models()).await?;

    handle.at_step(JobStep::DownloadingSpeechModel).await;
    handle.now_working_on(Some(model.file)).await;
    handle.set_total((model.bytes / MEBIBYTE) as i64 + 1).await;

    // The download reports into a number the job reads once a second: its
    // callback is not async, and a write to the database for every chunk would
    // be a write for every few kilobytes.
    let seen = Arc::new(AtomicU64::new(0));
    let reporting = seen.clone();
    let stopped = handle.clone();
    let mut downloading = std::pin::pin!(speech_models::download(
        model,
        &to,
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
            tracing::info!(model = model.id, "a model that listens was downloaded");
            Ok(())
        }
        // A stop somebody asked for is not a fault, and the job says so on its
        // own when it ends.
        Err(DownloadError::Stopped) => Ok(()),
        Err(error) => {
            tracing::warn!(model = model.id, %error, "a model that listens could not be downloaded");
            Err(AppError::Domain(melyxar_core::Error::new(
                melyxar_core::error::ErrorCode::Internal,
                error.to_string(),
            )))
        }
    }
}

/// What listening needs, all of it there: the tool, and the model chosen,
/// whole on the disk. Nothing when any is missing.
pub struct Ready {
    pub tool: SpeechTool,
    pub model: PathBuf,
}

pub async fn ready(state: &AppState) -> Result<Option<Ready>> {
    let Some(tool) = tool(state) else {
        return Ok(None);
    };
    let Some(model) = state
        .database()
        .speech_model()
        .await?
        .as_deref()
        .and_then(speech_models::model)
    else {
        return Ok(None);
    };
    let path = path_of(state, model);
    Ok(speech_models::is_whole(model, &path).then_some(Ready { tool, model: path }))
}

/// Starts listening to the videos of a library that nobody has listened to,
/// as a job of its own. Listening is one of the upkeep's readings, so it is
/// started the way the others are; this is what they call.
pub(crate) async fn listen_to_the_videos_of(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    let Some(ready) = ready(state).await? else {
        tracing::debug!(
            library = library.name,
            "no tool or no model that listens here, so no video is listened to"
        );
        return Ok(0);
    };
    let Some(tools) = state.tools() else {
        return Ok(0);
    };
    let database = state.database();
    let waiting = database.count_awaiting_speech(library.id).await?;
    if waiting == 0 {
        return Ok(0);
    }

    handle.at_step(JobStep::ListeningToSpeech).await;
    let already_done = database.count_with_speech(library.id).await?;
    handle.size_up(already_done, waiting).await;
    let threads = effort(state)
        .await?
        .threads(std::thread::available_parallelism().map_or(2, usize::from));

    let mut listened = 0;
    let mut still_waiting = waiting;
    loop {
        if handle.is_cancelled() {
            break;
        }
        let batch = database.sources_awaiting_speech(library.id, IN_ONE_BATCH).await?;
        if batch.is_empty() {
            break;
        }
        for source_id in batch {
            if handle.is_cancelled() {
                break;
            }
            if let Ok(source) = crate::playable_file(database, source_id).await {
                let name = source.path.file_name().map(|name| name.to_string_lossy().into_owned());
                handle.now_working_on(name.as_deref()).await;
            }
            let asked_to_stop = AskedToStop::when(handle.cancelled_when());
            match listen_to_one(state, tools, &ready, source_id, threads, asked_to_stop).await {
                Ok(lines) => {
                    listened += 1;
                    tracing::debug!(library = library.name, lines, "a video was listened to");
                }
                // Called off: the video stays waiting, and is not a fault.
                Err(AppError::MediaTools(FfmpegError::GivenUp)) => {}
                // Left waiting too, said once, and the pass says it is stuck
                // if nothing moves: a reading that could not happen is not an
                // answer about the file.
                Err(error) => tracing::warn!(
                    library = library.name,
                    %error,
                    "a video could not be listened to"
                ),
            }
            handle.advance(1).await;
        }
        let left = database.count_awaiting_speech(library.id).await?;
        if left >= still_waiting {
            tracing::warn!(
                library = library.name,
                left,
                "videos are left that could not be listened to, so this pass stops"
            );
            break;
        }
        still_waiting = left;
    }
    if listened > 0 {
        tracing::info!(library = library.name, videos = listened, "the videos of this library were listened to");
    }
    Ok(listened)
}

/// The track of a subtitle that nobody wrote, kept in a file of the server's
/// own, so the player offers it like one downloaded and says where it came from.
pub(crate) fn generated_track(
    id: TrackId,
    source_id: MediaSourceId,
    language: Option<String>,
    file: &str,
) -> Track {
    Track {
        id,
        source_id,
        stream_index: 0,
        language,
        title: None,
        is_default: false,
        is_forced: false,
        kind: TrackKind::Subtitle(SubtitleDetails {
            codec: "subrip".to_string(),
            layout: SubtitleLayout::Text,
            is_hearing_impaired: false,
            is_generated: true,
            is_external: true,
            external_relative_path: None,
            downloaded_file: Some(file.to_string()),
        }),
    }
}

/// Listens to one video and keeps what it hears as one more subtitle of it.
/// Answers how many lines there are, nought when nothing was said.
async fn listen_to_one(
    state: &AppState,
    tools: &melyxar_ffmpeg::ToolPaths,
    ready: &Ready,
    source_id: MediaSourceId,
    threads: usize,
    asked_to_stop: AskedToStop,
) -> Result<usize> {
    let database = state.database();
    let source = crate::playable_file(database, source_id).await?;
    let directories = &state.config().directories;
    tokio::fs::create_dir_all(directories.speech_scratch()).await?;

    let heard = listen(
        tools,
        &ready.tool,
        &source.path,
        &ready.model,
        &directories.speech_scratch(),
        threads,
        asked_to_stop,
    )
    .await?;

    if heard.lines == 0 {
        database.store_speech(source_id, 0).await?;
        return Ok(0);
    }

    let track_id = TrackId::new();
    let file = format!("{track_id}.srt");
    let folder = directories.downloaded_subtitles();
    tokio::fs::create_dir_all(&folder).await?;
    tokio::fs::write(folder.join(&file), heard.subrip.as_bytes()).await?;

    let track = generated_track(track_id, source_id, heard.language.as_deref().map(normalise_language), &file);
    let kept = async {
        database.add_downloaded_subtitle(source_id, &track).await?;
        database.store_speech(source_id, heard.lines).await
    }
    .await;
    if let Err(error) = kept {
        // The file is nobody's if its track was not written.
        let _ = tokio::fs::remove_file(folder.join(&file)).await;
        return Err(error.into());
    }
    Ok(heard.lines)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) async fn a_server() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = melyxar_config::Config {
            directories: melyxar_config::Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
                ..Default::default()
            },
            ..melyxar_config::Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let database = melyxar_database::Database::open_in_memory().await.expect("database opens");
        (directory, AppState::new(config, database, None, None))
    }

    /// A model that is on the disk: a sparse file of exactly the right size.
    fn put_on_the_disk(state: &AppState, id: &str) {
        let model = speech_models::model(id).expect("a model that is offered");
        let file = std::fs::File::create(path_of(state, model)).expect("created");
        file.set_len(model.bytes).expect("a sparse file");
    }

    #[tokio::test]
    async fn a_model_is_chosen_only_once_it_is_on_the_disk() {
        let (_directory, state) = a_server().await;
        assert!(matches!(
            choose(&state, Some("everything")).await,
            Err(Trouble::Refused(Refused::UnknownModel))
        ));
        assert!(matches!(
            choose(&state, Some("small")).await,
            Err(Trouble::Refused(Refused::NotDownloaded))
        ));
        assert_eq!(status(&state).await.expect("read").chosen, None);

        put_on_the_disk(&state, "small");
        choose(&state, Some("small")).await.expect("chosen");
        let shown = status(&state).await.expect("read");
        assert_eq!(shown.chosen, Some("small"));
        assert!(shown.models.iter().any(|model| model.id == "small" && model.downloaded));
        assert!(shown.models.iter().any(|model| model.id == "medium" && !model.downloaded));

        choose(&state, None).await.expect("none");
        assert_eq!(status(&state).await.expect("read").chosen, None);
    }

    #[tokio::test]
    async fn a_file_that_is_cut_is_not_a_model_and_one_forgotten_is_no_longer_chosen() {
        let (_directory, state) = a_server().await;
        let model = speech_models::model("small").expect("offered");
        std::fs::write(path_of(&state, model), b"cut short").expect("written");
        assert!(matches!(
            choose(&state, Some("small")).await,
            Err(Trouble::Refused(Refused::NotDownloaded))
        ));

        put_on_the_disk(&state, "small");
        choose(&state, Some("small")).await.expect("chosen");
        forget(&state, "small").await.expect("forgotten");
        assert!(!path_of(&state, model).exists());
        let shown = status(&state).await.expect("read");
        assert_eq!(shown.chosen, None, "what is gone is not in use");
        forget(&state, "small").await.expect("forgetting what is not there is no fault");
        assert!(matches!(
            forget(&state, "everything").await,
            Err(Trouble::Refused(Refused::UnknownModel))
        ));
    }

    #[tokio::test]
    async fn the_tool_is_found_where_the_configuration_says() {
        let (directory, state) = a_server().await;
        assert!(!status(&state).await.expect("read").tool_found);

        let tool = directory.path().join("whisper-cli");
        std::fs::write(&tool, "#!/bin/sh\n").expect("written");
        let config = melyxar_config::Config {
            media_tools: melyxar_config::MediaToolsConfig {
                whisper_path: Some(tool),
                ..Default::default()
            },
            directories: state.config().directories.clone(),
            ..melyxar_config::Config::default()
        };
        let database = melyxar_database::Database::open_in_memory().await.expect("database opens");
        let configured = AppState::new(config, database, None, None);
        assert!(super::tool(&configured).is_some());
        assert!(status(&configured).await.expect("read").tool_found);
    }

    #[test]
    fn each_effort_takes_its_share_of_the_processor_and_never_less_than_one_thread() {
        for effort in [Effort::Quiet, Effort::Balanced, Effort::Maximum] {
            assert_eq!(effort.threads(1), 1, "{effort:?}");
        }
        assert_eq!(Effort::Quiet.threads(12), 6);
        assert_eq!(Effort::Balanced.threads(12), 9);
        assert_eq!(Effort::Maximum.threads(12), 12);
        assert_eq!(Effort::Quiet.threads(64), 8, "more threads do not make a model faster");
        assert_eq!(Effort::Maximum.threads(64), 12);
    }

    #[test]
    fn an_effort_is_known_by_its_word_and_only_by_it() {
        for effort in [Effort::Quiet, Effort::Balanced, Effort::Maximum] {
            assert_eq!(Effort::parse(effort.as_str()), Some(effort));
        }
        assert_eq!(Effort::parse("frantic"), None);
    }
}
