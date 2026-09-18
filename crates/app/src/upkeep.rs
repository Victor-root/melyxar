//! The upkeep: the two readings of a film a scan does not wait for.
//!
//! A scan walks the folders, writes down what moved and asks the analyser what
//! each new file holds. All of that is quick, and all of it is what somebody
//! pressing the button is waiting for. Two other things have to happen to a
//! film, and neither is quick: reading it through for the places its picture
//! can be started, and reading it through again for the thumbnails somebody
//! drags along the playback bar. Each reads the whole file from end to end, so
//! on a collection of several hundred a scan that carried them took days.
//!
//! So they live here, as jobs of their own:
//!
//! * a library may ask for either of them to be done during its scan, which is
//!   what a small library on a machine with time to spare wants. Off by
//!   default, and Jellyfin, which offers the same switch, warns against it on a
//!   large collection;
//! * otherwise they run of a night, when nobody is waiting on anything;
//! * and a button starts either of them now, for somebody who will not wait for
//!   the night.
//!
//! Nothing here holds a list drawn up at the start. Every batch asks what is
//! left to do, which is what lets a reading stopped by a restart carry on
//! exactly where it was without anything having been written down about where
//! that was.

use melyxar_core::id::{LibraryId, MediaSourceId};
use melyxar_core::job::{JobKind, JobPriority, JobStep};
use melyxar_core::library::Library;
use melyxar_core::privacy::MediaName;
use melyxar_database::Database;
use melyxar_jobs::JobHandle;

use crate::{AppState, Result};

/// One of the two readings the upkeep is made of.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpkeepTask {
    /// Reading each film for where its picture can be started.
    KeyFrames,
    /// Reading each film for the thumbnails of its playback bar.
    Thumbnails,
}

impl UpkeepTask {
    /// Both of them, in the order they are worth doing.
    ///
    /// Key frames first: they cost a reading that touches nothing else, and
    /// they are what makes a jump land where it was asked to. A bar with no
    /// pictures on it is a comfort missing; a jump landing six seconds early
    /// is the film itself going wrong.
    pub const ALL: [Self; 2] = [Self::KeyFrames, Self::Thumbnails];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::KeyFrames => "key_frames",
            Self::Thumbnails => "thumbnails",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "key_frames" => Some(Self::KeyFrames),
            "thumbnails" => Some(Self::Thumbnails),
            _ => None,
        }
    }

    /// The job this task is written down as.
    pub fn job_kind(self) -> JobKind {
        match self {
            Self::KeyFrames => JobKind::ReadKeyFrames,
            Self::Thumbnails => JobKind::GenerateThumbnails,
        }
    }

    /// Whether this library has asked its scan to do this one itself.
    pub fn is_done_during_the_scan_of(self, library: &Library) -> bool {
        match self {
            Self::KeyFrames => library.options.key_frames_during_scan,
            Self::Thumbnails => library.options.thumbnails_during_scan,
        }
    }
}

/// How many files one batch asks for.
///
/// A bound on what is held in memory at once rather than on the work: a
/// library of a hundred thousand films must not become a hundred thousand rows
/// to answer one question. Unlike the bound this replaces, reaching the end of
/// a batch is not the end of the run: the next batch asks what is left, and the
/// run goes on until nothing is.
const IN_ONE_BATCH: i64 = 1_000;

/// Where one of the two readings stands for one library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatIsLeft {
    pub task: UpkeepTask,
    pub library: LibraryId,
    pub library_name: String,
    /// Files still waiting. Zero means there is nothing to start.
    pub waiting: i64,
    /// Files already done, so a screen can say four hundred of four hundred
    /// and ten rather than ten.
    pub done: i64,
    /// Whether the scan of this library does this reading itself.
    pub during_the_scan: bool,
    /// Whether a job of this kind is under way on this library right now.
    pub under_way: bool,
    /// The last time this reading ran to an end on this library, if it ever
    /// has and the history still holds it.
    pub last_run: Option<LastRun>,
}

/// When a reading last ran, and how it went.
///
/// A reading that has never run and one that ran last night and found nothing
/// look exactly alike from a count of what is waiting, and the difference is
/// whether anybody should be worried. Jellyfin puts the same line under each
/// of its tasks, for the same reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastRun {
    pub at: melyxar_core::time::Timestamp,
    /// How it ended, in the words the job layer uses: succeeded, failed,
    /// cancelled, or cut short by a restart.
    pub state: melyxar_core::job::JobState,
    /// How long it took. Absent for a job whose start was never written down,
    /// which is a row from a run that went away under it.
    pub took_seconds: Option<i64>,
}

/// What each library still has waiting, for the screen that shows the upkeep.
///
/// Both readings for every library, including the ones with nothing left:
/// "nothing to do" is an answer somebody came to the screen for, and a row
/// that disappears when it is done looks exactly like a row that was never
/// there.
pub async fn what_is_left(state: &AppState) -> Result<Vec<WhatIsLeft>> {
    let database = state.database();
    let mut left = Vec::new();

    for library in database.list_libraries().await? {
        for task in UpkeepTask::ALL {
            let waiting = what_is_waiting_for(state, task, library.id).await?;
            let done = match task {
                UpkeepTask::KeyFrames => database.count_read_for_key_frames(library.id).await?,
                UpkeepTask::Thumbnails => match crate::thumbnails::wanted(state).await {
                    Some(layout) => database.count_made_thumbnails(library.id, layout).await?,
                    None => 0,
                },
            };

            left.push(WhatIsLeft {
                task,
                library: library.id,
                library_name: library.name.clone(),
                waiting,
                done,
                during_the_scan: task.is_done_during_the_scan_of(&library),
                under_way: database
                    .has_unfinished_job(task.job_kind(), Some(&library.id.to_string()))
                    .await?,
                last_run: database
                    .last_finished_job(task.job_kind(), Some(&library.id.to_string()))
                    .await?
                    .and_then(|job| {
                        let at = job.finished_at?;
                        Some(LastRun {
                            at,
                            state: job.state,
                            took_seconds: job
                                .started_at
                                .map(|started| (at - started).whole_seconds()),
                        })
                    }),
            });
        }
    }
    Ok(left)
}

/// How many films of one library are waiting on one of the two readings.
///
/// The one number a scan says out loud when it hands the work over, and the
/// one the screen counts down. Nothing is ever waiting for thumbnails on a
/// server that has been told not to make any: nought rather than a queue that
/// nothing will ever empty.
pub async fn what_is_waiting_for(
    state: &AppState,
    task: UpkeepTask,
    library: LibraryId,
) -> Result<i64> {
    let database = state.database();
    Ok(match task {
        UpkeepTask::KeyFrames => database.count_awaiting_key_frames(library).await?,
        UpkeepTask::Thumbnails => match crate::thumbnails::wanted(state).await {
            Some(layout) => database.count_awaiting_thumbnails(library, layout).await?,
            None => 0,
        },
    })
}

/// Starts one of the two readings on one library, as a job of its own.
///
/// The priority says who is waiting: somebody who pressed a button, or nobody
/// at all, which is what the nightly run is.
pub async fn start(
    state: &AppState,
    task: UpkeepTask,
    library: Library,
    priority: JobPriority,
) -> Result<melyxar_jobs::StartedJob> {
    let owned = state.clone();
    let target = library.id.to_string();
    let name = library.name.clone();

    let started = state
        .jobs()
        .clone()
        .start(
            task.job_kind(),
            priority,
            Some(target),
            move |handle| async move {
                let read = match task {
                    UpkeepTask::KeyFrames => {
                        read_the_key_frames_of(&owned, &library, &handle).await
                    }
                    UpkeepTask::Thumbnails => {
                        make_the_thumbnails_of(&owned, &library, &handle).await
                    }
                }
                .map_err(|error| error.to_string())?;
                tracing::info!(
                    library = name,
                    task = task.as_str(),
                    films = read,
                    "the upkeep read what was waiting"
                );
                Ok(())
            },
        )
        .await?;
    Ok(started)
}

/// Starts whatever is waiting, on every library, and says how many jobs that
/// was.
///
/// What the nightly run does, and what one button on the upkeep screen does.
/// A library with nothing waiting is passed over rather than given a job that
/// would end on the spot, because a list of jobs that did nothing is a list
/// nobody reads.
///
/// A library being scanned right now is left alone whatever it has waiting:
/// the scan may be doing this very reading itself, and two readings of one
/// film at the same moment is one reading wasted and one disk working twice.
pub async fn start_what_is_waiting(state: &AppState, priority: JobPriority) -> usize {
    let left = match what_is_left(state).await {
        Ok(left) => left,
        Err(error) => {
            tracing::warn!(%error, "what the upkeep has to do could not be read");
            return 0;
        }
    };

    let libraries = match state.database().list_libraries().await {
        Ok(libraries) => libraries,
        Err(error) => {
            tracing::warn!(%error, "the libraries could not be read, so nothing was started");
            return 0;
        }
    };

    let mut started = 0;
    for entry in left.iter().filter(|entry| entry.waiting > 0) {
        let Some(library) = libraries.iter().find(|library| library.id == entry.library) else {
            continue;
        };

        if state
            .database()
            .has_unfinished_job(JobKind::ScanLibrary, Some(&entry.library.to_string()))
            .await
            .unwrap_or(false)
        {
            tracing::debug!(
                library = entry.library_name,
                task = entry.task.as_str(),
                waiting = entry.waiting,
                "a scan of this library is under way, so the upkeep waits for it to end"
            );
            continue;
        }

        match start(state, entry.task, library.clone(), priority).await {
            Ok(_) => started += 1,
            // A job of this kind already under way on this library is the
            // answer, not a fault: it is doing exactly what this would start.
            Err(crate::AppError::Jobs(melyxar_jobs::JobError::AlreadyUnderWay)) => {}
            Err(error) => tracing::warn!(
                library = entry.library_name,
                task = entry.task.as_str(),
                %error,
                "the upkeep would not start on this library"
            ),
        }
    }
    started
}

/// How often the clock is looked at.
///
/// Every five minutes rather than one long sleep to the exact minute: a sleep
/// of hours is a promise about a machine that may be suspended, moved between
/// hosts or simply slow, and a run missed that way would be missed in silence
/// until somebody noticed a library with no thumbnails in it. What decides a
/// run is the time itself and not the tick, so the tick only sets how late a
/// run can be.
const LOOK_AT_THE_CLOCK_EVERY: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// Keeps the upkeep running of a night, for as long as the server runs.
///
/// The time of day is a setting, read again on every look at the clock, so a
/// change made on a screen takes hold without anybody restarting anything. It
/// is kept in UTC because that is the only clock this server can read with
/// certainty: the hour a machine calls its own depends on a setting no program
/// should be asking for from several threads at once. The screen turns it into
/// the time of whoever is looking at it, which is the only place that
/// conversion can be made honestly.
///
/// A run happens when the time has come round and this server has not run
/// since. What it would have done before it started up is not its to do: it
/// begins as though it had just run, so coming up at ten in the morning does
/// not set three hundred films reading because three o'clock is behind us.
pub fn keep_the_upkeep_running(state: &AppState) -> tokio::task::JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move {
        // Said once, on the way up. The question somebody asks a week later is
        // whether this server is going to read those films at all, and a loop
        // that says nothing until it fires cannot answer it.
        match state.database().library_work().await {
            Ok(work) => tracing::debug!(
                nightly = work.upkeep_nightly,
                at_utc_minutes = work.upkeep_at_utc_minutes,
                "the upkeep is watching the clock"
            ),
            Err(error) => tracing::warn!(%error, "the upkeep could not read when it is due"),
        }

        let mut last_run = melyxar_core::time::now();
        loop {
            tokio::time::sleep(LOOK_AT_THE_CLOCK_EVERY).await;

            let Ok(work) = state.database().library_work().await else {
                continue;
            };
            if !work.upkeep_nightly {
                continue;
            }

            let now = melyxar_core::time::now();
            let due = melyxar_core::time::at_utc_minutes_on(now, work.upkeep_at_utc_minutes);
            // Due, and not run since it fell due. Written against the moment
            // rather than against the day, so a time moved to this evening
            // from a screen is honoured this evening rather than tomorrow.
            if now < due || last_run >= due {
                continue;
            }
            last_run = now;

            let started = start_what_is_waiting(&state, JobPriority::BACKGROUND).await;
            if started > 0 {
                tracing::info!(jobs = started, "the nightly upkeep started");
            } else {
                tracing::debug!("the nightly upkeep found nothing waiting");
            }
        }
    })
}

/// Reads every film of a library that nobody has read for its key frames.
///
/// Only the film knows where its picture stands on its own, and it is the only
/// thing that decides where a stream carried over untouched may be cut. Read
/// once, here, and never while somebody is watching.
pub(crate) async fn read_the_key_frames_of(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    let Some(tools) = state.tools() else {
        tracing::debug!(
            library = library.name,
            "no media tool here, so no film is read for where its picture can be started"
        );
        return Ok(0);
    };
    let database = state.database();
    let waiting = database.count_awaiting_key_frames(library.id).await?;
    if waiting == 0 {
        return Ok(0);
    }

    handle.at_step(JobStep::ReadingKeyFrames).await;
    // Counted against the whole reading rather than against this run of it. A
    // run picking up where it left off and one starting again from nothing
    // look exactly alike from a bar that always begins at zero, and the
    // difference between them is two hours.
    let already_read = database.count_read_for_key_frames(library.id).await?;
    // Counted before the size is given, because giving the size is what writes
    // both of them down. The other way round, what is already done is held
    // back until the first film of this run has been read through, and a
    // reading that is nine tenths finished says nought per cent for as long as
    // that takes.
    if already_read > 0 {
        handle.advance(already_read).await;
    }
    handle.set_total(already_read + waiting).await;
    tracing::debug!(
        library = library.name,
        waiting,
        already_read,
        "reading the films of this library for where their picture can be started"
    );

    let analyser = tools.ffprobe.clone();
    let mut read = 0;
    let mut still_waiting = waiting;
    loop {
        if handle.is_cancelled() {
            break;
        }
        let batch = database
            .sources_without_key_frames(library.id, IN_ONE_BATCH)
            .await?;
        if batch.is_empty() {
            break;
        }

        let analyser = analyser.clone();
        let owned_database = database.clone();
        let owned_handle = handle.clone();
        // Bounded like the analysis of a scan: this is the disk from end to
        // end, and the upkeep must leave the film somebody is watching alone.
        let done = melyxar_jobs::for_each_bounded(
            batch,
            state.config().limits.concurrent_probes,
            move |source_id| {
                let analyser = analyser.clone();
                let database = owned_database.clone();
                let handle = owned_handle.clone();
                async move {
                    if handle.is_cancelled() {
                        return false;
                    }
                    if let Some(name) = file_name_of(&database, source_id).await {
                        handle.now_working_on(Some(&name)).await;
                    }
                    let done =
                        read_one_film_for_its_key_frames(&database, &analyser, source_id).await;
                    handle.advance(1).await;
                    done
                }
            },
        )
        .await;

        read += done.into_iter().filter(|done| *done).count();

        // What is left is asked for again rather than worked out from what the
        // batch answered, and it is the only thing that says the run is moving.
        // A batch that left the number where it was would come back unchanged
        // for ever: nothing about those files can be written down, and going
        // round again would only fail again in the same way.
        let left = database.count_awaiting_key_frames(library.id).await?;
        if left >= still_waiting {
            tracing::warn!(
                library = library.name,
                waiting = left,
                "nothing of this batch could be written down, so the reading stops here"
            );
            break;
        }
        still_waiting = left;
    }
    Ok(read)
}

/// Makes the thumbnails of the playback bar for every film that has none.
///
/// A film without them shows a bar with no pictures on it, which is what every
/// film did until this had run: nothing here is ever worth holding anything
/// else up for.
pub(crate) async fn make_the_thumbnails_of(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    if state.tools().is_none() {
        // Said rather than passed over in silence: a server with no media tool
        // makes no thumbnail and never will, and the bar staying bare is the
        // only other sign of it.
        tracing::debug!(
            library = library.name,
            "no media tool here, so no thumbnail is made"
        );
        return Ok(0);
    }
    let Some(layout) = crate::thumbnails::wanted(state).await else {
        tracing::debug!(
            library = library.name,
            "the thumbnails of the playback bar are switched off in the configuration"
        );
        return Ok(0);
    };
    let database = state.database();
    let waiting = database
        .count_awaiting_thumbnails(library.id, layout)
        .await?;
    if waiting == 0 {
        return Ok(0);
    }

    handle.at_step(JobStep::MakingThumbnails).await;
    let already_made = database.count_made_thumbnails(library.id, layout).await?;
    if already_made > 0 {
        handle.advance(already_made).await;
    }
    handle.set_total(already_made + waiting).await;
    tracing::debug!(
        library = library.name,
        waiting,
        already_made,
        every_seconds = layout.every.as_seconds_f64(),
        "reading the films of this library for the thumbnails of their bar"
    );

    let mut made = 0;
    let mut still_waiting = waiting;
    loop {
        if handle.is_cancelled() {
            break;
        }
        let batch = database
            .sources_without_thumbnails(library.id, layout, IN_ONE_BATCH)
            .await?;
        if batch.is_empty() {
            break;
        }

        let owned_state = state.clone();
        let owned_handle = handle.clone();
        let done = melyxar_jobs::for_each_bounded(
            batch,
            state.config().limits.concurrent_probes,
            move |source_id| {
                let state = owned_state.clone();
                let handle = owned_handle.clone();
                async move {
                    if handle.is_cancelled() {
                        return false;
                    }
                    if let Some(name) = file_name_of(state.database(), source_id).await {
                        handle.now_working_on(Some(&name)).await;
                    }
                    // Every way this fails has already said so with the file it
                    // was about, which is what a refusal has to carry to be
                    // read.
                    let done = crate::thumbnails::make_for(&state, source_id)
                        .await
                        .is_ok_and(|made| made.counted > 0);
                    handle.advance(1).await;
                    done
                }
            },
        )
        .await;

        made += done.into_iter().filter(|done| *done).count();

        // What is left decides whether the run is moving, for the same reason
        // as the reading above. Counting the films that gave a picture would
        // not do here: a film too short to fill one interval honestly gives
        // none, is written down as having none, and is no longer waiting.
        let left = database
            .count_awaiting_thumbnails(library.id, layout)
            .await?;
        if left >= still_waiting {
            tracing::warn!(
                library = library.name,
                waiting = left,
                "no film of this batch could be written down, so the reading stops here"
            );
            break;
        }
        still_waiting = left;
    }
    Ok(made)
}

/// The name of one file, for the screen that says what a job is on.
///
/// The name rather than the path: a screen says which film, and where it sits
/// on which disk is the report's business.
async fn file_name_of(database: &Database, source_id: MediaSourceId) -> Option<String> {
    let source = database.playable_source(source_id).await.ok()??;
    Some(
        source
            .path
            .file_name()
            .and_then(|name| name.to_str())?
            .to_string(),
    )
}

/// Reads one film, and says whether it gave up anything usable.
async fn read_one_film_for_its_key_frames(
    database: &Database,
    analyser: &std::path::Path,
    source_id: MediaSourceId,
) -> bool {
    let Ok(Some(source)) = database.playable_source(source_id).await else {
        return false;
    };
    if source.missing {
        return false;
    }

    match melyxar_ffmpeg::probe::key_frames(analyser, &source.path).await {
        Ok(found) if !found.is_empty() => {
            match database.store_key_frames(source_id, &found).await {
                Ok(()) => true,
                Err(error) => {
                    tracing::warn!(error = %error, "where a film can be started could not be kept");
                    false
                }
            }
        }
        // A reading that finished and gave nothing is an answer about the
        // file, and it is written down so the file is never read through
        // again for the same nothing. A file that was merely busy is the
        // other branch: the analyser fails there, and nothing is written.
        //
        // Seen on a VC-1 remux of the maintainer's: every packet of the
        // picture carried no time at all, and every one of them claimed to
        // stand on its own, which is what a demuxer says about a stream it
        // cannot read. Reading it again can only say the same, and it cost a
        // minute of every scan. Such a film keeps the usual grid, which is
        // exact for it anyway: no browser plays that codec, so its picture is
        // rebuilt, and a rebuilt picture is cut where this server puts the
        // cuts.
        Ok(_) => {
            tracing::warn!(
                file = %MediaName::new(
                    source
                        .path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                ),
                "this film says nowhere its picture can be started, so it is cut on the usual \
                 grid from now on and never read for this again"
            );
            match database.store_key_frames(source_id, &[]).await {
                Ok(()) => true,
                Err(error) => {
                    tracing::warn!(error = %error, "that answer could not be kept");
                    false
                }
            }
        }
        Err(error) => {
            tracing::warn!(
                file = %MediaName::new(
                    source
                        .path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                ),
                error = %error,
                "this film could not be read for where its picture can be started"
            );
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_task_survives_a_round_trip_through_its_stored_form() {
        for task in UpkeepTask::ALL {
            assert_eq!(UpkeepTask::parse(task.as_str()), Some(task));
        }
        assert_eq!(UpkeepTask::parse("something_new"), None);
    }

    #[test]
    fn each_task_is_written_down_as_a_job_of_its_own() {
        // Two jobs of one kind never run on the same subject at once, so two
        // tasks sharing a kind would mean the thumbnails of a library refusing
        // to start because its key frames are being read.
        assert_eq!(UpkeepTask::KeyFrames.job_kind(), JobKind::ReadKeyFrames);
        assert_eq!(
            UpkeepTask::Thumbnails.job_kind(),
            JobKind::GenerateThumbnails
        );
        assert_ne!(
            UpkeepTask::KeyFrames.job_kind(),
            UpkeepTask::Thumbnails.job_kind()
        );
    }

    #[test]
    fn a_library_answers_for_each_task_whether_its_scan_does_it() {
        let mut library = melyxar_core::library::Library {
            id: LibraryId::new(),
            name: "Films".to_string(),
            kind: melyxar_core::library::LibraryKind::Movies,
            metadata_language: "fr".to_string(),
            options: melyxar_core::library::LibraryOptions::default(),
            roots: Vec::new(),
        };
        assert!(!UpkeepTask::KeyFrames.is_done_during_the_scan_of(&library));
        assert!(!UpkeepTask::Thumbnails.is_done_during_the_scan_of(&library));

        library.options.key_frames_during_scan = true;
        assert!(UpkeepTask::KeyFrames.is_done_during_the_scan_of(&library));
        assert!(
            !UpkeepTask::Thumbnails.is_done_during_the_scan_of(&library),
            "the two switches are two answers, not one"
        );
    }
}
