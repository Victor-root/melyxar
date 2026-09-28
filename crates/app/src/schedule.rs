//! The seven scheduled tasks, each valid for every library.
//!
//! Where there used to be one nightly run of four readings shown library by
//! library, there are seven tasks: the scan, the look up, the ratings from
//! elsewhere, and the four readings of the files. Each runs over every library
//! it concerns, one library after the other, and each has its own switch and
//! time of day. Several falling due together run one after the other in the
//! order they are listed, which is the order each needs the one before it: a
//! file has to be found before it can be named, and named before anybody waits
//! on its pictures or its ratings.
//!
//! What a library wants of the heavy readings is its own affair
//! (`LibraryOptions`): a task passes by a library that does not want it, and
//! nothing there counts as waiting.

use std::collections::HashSet;
use std::sync::Mutex;

use melyxar_core::job::{JobKind, JobPriority, JobState};
use melyxar_core::library::Library;
use melyxar_core::refresh::RefreshMode;
use melyxar_core::time::Timestamp;

use crate::upkeep::UpkeepTask;
use crate::{AppError, AppState, Result};

/// One of the seven scheduled tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScheduledTask {
    /// Walks every library for what was added, moved or removed, and follows
    /// each arrival with what always follows it.
    Scan,
    /// Looks up the works still without a name.
    Identify,
    /// Brings the ratings from IMDb and Rotten Tomatoes up to date.
    Ratings,
    /// One of the readings that go through the files.
    Reading(UpkeepTask),
}

impl ScheduledTask {
    /// Every task, in the order they are shown and in the order several due
    /// together are run.
    pub const ALL: [Self; 7] = [
        Self::Scan,
        Self::Identify,
        Self::Ratings,
        Self::Reading(UpkeepTask::KeyFrames),
        Self::Reading(UpkeepTask::Subtitles),
        Self::Reading(UpkeepTask::Thumbnails),
        Self::Reading(UpkeepTask::Openings),
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Identify => "identify",
            Self::Ratings => "ratings",
            Self::Reading(task) => task.as_str(),
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "scan" => Some(Self::Scan),
            "identify" => Some(Self::Identify),
            "ratings" => Some(Self::Ratings),
            other => UpkeepTask::parse(other).map(Self::Reading),
        }
    }

    /// The kind of job each of its runs on one library is written down as.
    fn job_kind(self) -> JobKind {
        match self {
            Self::Scan => JobKind::ScanLibrary,
            Self::Identify => JobKind::IdentifyWork,
            Self::Ratings => JobKind::FetchRatings,
            Self::Reading(task) => task.job_kind(),
        }
    }

    /// Whether what it counts is seasons rather than files.
    pub fn counts_seasons(self) -> bool {
        matches!(self, Self::Reading(task) if task.counts_seasons())
    }

    /// Whether this task has anything to do with this library at all.
    fn concerns(self, state: &AppState, library: &Library) -> bool {
        match self {
            Self::Scan => true,
            Self::Identify => {
                state.metadata_provider().is_some() && library.kind.is_catalogued()
            }
            Self::Ratings => library.kind.is_catalogued(),
            Self::Reading(task) => task.applies_to(library),
        }
    }

    /// How much this task has waiting on one library. Nothing for the scan,
    /// which cannot know what a disk holds until it has walked it.
    async fn waiting_on(self, state: &AppState, library: &Library) -> Result<Option<i64>> {
        Ok(match self {
            Self::Scan => None,
            Self::Identify => Some(
                state
                    .database()
                    .count_awaiting_identification(Some(library.id))
                    .await?,
            ),
            Self::Ratings => Some(crate::ratings::waiting_on(state, library).await?),
            Self::Reading(task) => {
                Some(crate::upkeep::what_is_waiting_for(state, task, library.id).await?)
            }
        })
    }
}

/// The tasks running right now, so one is never run twice at once.
#[derive(Debug, Default)]
pub struct Running(Mutex<HashSet<ScheduledTask>>);

impl Running {
    /// Takes a task for this run, or answers that it is already under way.
    fn claim(&self, task: ScheduledTask) -> bool {
        self.0
            .lock()
            .expect("the running tasks are never held across an await")
            .insert(task)
    }

    fn release(&self, task: ScheduledTask) {
        self.0
            .lock()
            .expect("the running tasks are never held across an await")
            .remove(&task);
    }

    fn holds(&self, task: ScheduledTask) -> bool {
        self.0
            .lock()
            .expect("the running tasks are never held across an await")
            .contains(&task)
    }
}

/// When a task last ran, and how it went.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastRun {
    pub at: Timestamp,
    /// Succeeded, failed, cancelled, or cut short by a restart.
    pub state: JobState,
    /// How long it took, for a run that ended.
    pub took_seconds: Option<i64>,
}

/// Where one task stands, for the screen of the scheduled tasks.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStatus {
    pub task: ScheduledTask,
    /// Whether it runs on its own each day, and when, in minutes since
    /// midnight, UTC.
    pub runs_on_schedule: bool,
    pub at_utc_minutes: i64,
    /// What it has waiting over every library it concerns. Nothing for the
    /// scan.
    pub waiting: Option<i64>,
    /// Whether it is running right now, as a task or on one library by a
    /// button or an arrival.
    pub under_way: bool,
    pub last_run: Option<LastRun>,
}

/// Where every task stands, in the order they are shown.
pub async fn status(state: &AppState) -> Result<Vec<TaskStatus>> {
    let database = state.database();
    let rows = database.scheduled_tasks().await?;
    let libraries = database.list_libraries().await?;

    let mut all = Vec::with_capacity(ScheduledTask::ALL.len());
    for task in ScheduledTask::ALL {
        let Some(row) = rows.iter().find(|row| row.task == task.as_str()) else {
            continue;
        };
        let mut waiting = None;
        for library in libraries.iter().filter(|library| task.concerns(state, library)) {
            if let Some(count) = task.waiting_on(state, library).await? {
                waiting = Some(waiting.unwrap_or(0) + count);
            }
        }
        if task != ScheduledTask::Scan && waiting.is_none() {
            waiting = Some(0);
        }
        let under_way = state.schedule().holds(task)
            || database.has_unfinished_job_of_kind(task.job_kind()).await?;
        let last_run = match (row.last_started_at, row.last_finished_at, row.last_state) {
            (Some(started), Some(finished), Some(state)) => Some(LastRun {
                at: finished,
                state,
                took_seconds: Some((finished - started).whole_seconds()),
            }),
            // Begun and never ended, and not running now: a run the server
            // went down under.
            (Some(started), None, _) if !state.schedule().holds(task) => Some(LastRun {
                at: started,
                state: JobState::Interrupted,
                took_seconds: None,
            }),
            _ => None,
        };
        all.push(TaskStatus {
            task,
            runs_on_schedule: row.runs_on_schedule,
            at_utc_minutes: row.at_utc_minutes,
            waiting,
            under_way,
            last_run,
        });
    }
    Ok(all)
}

/// Says whether a task runs on its own each day, and when.
pub async fn set_schedule(
    state: &AppState,
    task: ScheduledTask,
    runs_on_schedule: bool,
    at_utc_minutes: i64,
) -> Result<()> {
    state
        .database()
        .schedule_task(task.as_str(), runs_on_schedule, at_utc_minutes)
        .await?;
    Ok(())
}

/// Starts one task now, over every library it concerns, and does not wait
/// for it. Refused when it is already under way.
pub fn start(state: &AppState, task: ScheduledTask, priority: JobPriority) -> Result<()> {
    if !state.schedule().claim(task) {
        return Err(AppError::Jobs(melyxar_jobs::JobError::AlreadyUnderWay));
    }
    let state = state.clone();
    tokio::spawn(async move {
        run_claimed(&state, task, priority).await;
    });
    Ok(())
}

/// Starts these tasks one after the other, in the order they are listed, and
/// does not wait for them. One already under way is passed over.
pub fn start_in_turn(state: &AppState, tasks: Vec<ScheduledTask>, priority: JobPriority) {
    let state = state.clone();
    tokio::spawn(async move {
        run_in_turn(&state, &tasks, priority).await;
    });
}

/// Runs these tasks one after the other, in the order they are listed.
pub async fn run_in_turn(state: &AppState, tasks: &[ScheduledTask], priority: JobPriority) {
    for task in ScheduledTask::ALL.into_iter().filter(|task| tasks.contains(task)) {
        if !state.schedule().claim(task) {
            tracing::debug!(task = task.as_str(), "already under way, passed over");
            continue;
        }
        run_claimed(state, task, priority).await;
    }
}

/// Runs a task already claimed, writes down how it went, and lets it go.
async fn run_claimed(state: &AppState, task: ScheduledTask, priority: JobPriority) -> JobState {
    let database = state.database();
    if let Err(error) = database.task_started(task.as_str()).await {
        tracing::warn!(task = task.as_str(), %error, "the start of a task could not be written down");
    }
    let ended = run_over_every_library(state, task, priority).await;
    if let Err(error) = database.task_finished(task.as_str(), ended).await {
        tracing::warn!(task = task.as_str(), %error, "the end of a task could not be written down");
    }
    state.schedule().release(task);
    tracing::info!(task = task.as_str(), state = ended.as_str(), "a scheduled task ended");
    ended
}

/// Runs a task over every library it concerns, one after the other, and
/// answers the worst of how they ended.
///
/// A library with nothing waiting is passed over, so a run leaves no trail of
/// jobs that did nothing. One the task is already running on, by a button or
/// an arrival, is passed over too: it is being done.
async fn run_over_every_library(
    state: &AppState,
    task: ScheduledTask,
    priority: JobPriority,
) -> JobState {
    let libraries = match state.database().list_libraries().await {
        Ok(libraries) => libraries,
        Err(error) => {
            tracing::warn!(task = task.as_str(), %error, "the libraries could not be read");
            return JobState::Failed;
        }
    };

    let mut worst = JobState::Succeeded;
    for library in libraries.into_iter().filter(|library| task.concerns(state, library)) {
        match task.waiting_on(state, &library).await {
            Ok(Some(0)) => continue,
            Ok(_) => {}
            Err(error) => {
                tracing::warn!(task = task.as_str(), library = library.name, %error, "what is waiting could not be read");
                worst = JobState::Failed;
                continue;
            }
        }
        let name = library.name.clone();
        let ended = match run_on(state, task, library, priority).await {
            Ok(ended) => ended,
            Err(AppError::Jobs(melyxar_jobs::JobError::AlreadyUnderWay)) => continue,
            Err(error) => {
                tracing::warn!(task = task.as_str(), library = name, %error, "the task would not start on this library");
                JobState::Failed
            }
        };
        worst = match (worst, ended) {
            (_, JobState::Failed) | (JobState::Failed, _) => JobState::Failed,
            (_, JobState::Cancelled) | (JobState::Cancelled, _) => JobState::Cancelled,
            (worst, _) => worst,
        };
    }
    worst
}

/// Runs a task on one library and waits for it to end.
async fn run_on(
    state: &AppState,
    task: ScheduledTask,
    library: Library,
    priority: JobPriority,
) -> Result<JobState> {
    Ok(match task {
        ScheduledTask::Scan => {
            crate::scan::scan_and_what_follows(state, library, priority, RefreshMode::default())
                .await?
        }
        ScheduledTask::Identify => {
            let Some(provider) = state.metadata_provider() else {
                return Ok(JobState::Succeeded);
            };
            crate::identify::start_identification(state, provider, library, RefreshMode::default())
                .await?
                .wait()
                .await
                .0
        }
        ScheduledTask::Ratings => crate::ratings::start(state, library, priority)
            .await?
            .completion
            .await
            .unwrap_or(JobState::Failed),
        ScheduledTask::Reading(reading) => crate::upkeep::start(state, reading, library, priority)
            .await?
            .completion
            .await
            .unwrap_or(JobState::Failed),
    })
}

/// How often the clock is looked at.
///
/// Every five minutes rather than one long sleep to the exact minute: a sleep
/// of hours is a promise about a machine that may be suspended, moved between
/// hosts or simply slow, and a run missed that way would be missed in silence.
/// What decides a run is the time itself and not the tick, so the tick only
/// sets how late a run can be.
const LOOK_AT_THE_CLOCK_EVERY: std::time::Duration = std::time::Duration::from_secs(5 * 60);

/// How often the sessions nobody uses any more are thrown away.
///
/// Once a day, and once on the way up. Neither on its own is enough: a server
/// restarted every morning would never reach a timer of its own, and a server
/// left up for months would never come back to a sweep that only ran at
/// startup. It is tied to no task: who may still sign in is not a question
/// about a library.
const FORGET_UNUSED_SESSIONS_EVERY: time::Duration = time::Duration::hours(24);

/// Throws away the sessions nobody has used in a year, and says when it ran.
async fn forget_the_sessions_nobody_uses(state: &AppState) -> Timestamp {
    let at = melyxar_core::time::now();
    match state.database().forget_stale_sessions(at).await {
        Ok(0) => tracing::debug!("no session had gone a year unused"),
        Ok(swept) => tracing::info!(swept, "sessions nobody had used in a year were forgotten"),
        Err(error) => tracing::warn!(%error, "the sessions nobody uses could not be swept"),
    }
    at
}

/// The tasks due at this moment, from each one's schedule and when it was
/// last set going on its own.
///
/// Due when its time has come round today and it has not been set going
/// since. Written against the moment rather than the day, so a time moved to
/// this evening from a screen is honoured this evening rather than tomorrow.
fn due_now(
    rows: &[melyxar_database::schedule::ScheduledTaskRow],
    last_set_going: &std::collections::HashMap<ScheduledTask, Timestamp>,
    now: Timestamp,
    since_start: Timestamp,
) -> Vec<ScheduledTask> {
    ScheduledTask::ALL
        .into_iter()
        .filter(|task| {
            let Some(row) = rows.iter().find(|row| row.task == task.as_str()) else {
                return false;
            };
            if !row.runs_on_schedule {
                return false;
            }
            let due = melyxar_core::time::at_utc_minutes_on(now, row.at_utc_minutes);
            let last = last_set_going.get(task).copied().unwrap_or(since_start);
            now >= due && last < due
        })
        .collect()
}

/// Keeps the scheduled tasks running at their times, for as long as the
/// server runs.
///
/// The times are read again on every look at the clock, so a change made on a
/// screen takes hold without anybody restarting anything. What was due before
/// the server started is not its to do: it begins as though every task had
/// just run, so coming up at ten in the morning does not set every library
/// reading because three o'clock is behind us.
pub fn keep_the_schedule(state: &AppState) -> tokio::task::JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move {
        let since_start = melyxar_core::time::now();
        let mut last_set_going = std::collections::HashMap::new();
        let mut last_sweep = forget_the_sessions_nobody_uses(&state).await;
        // The activity journal on the same beat: the same kind of
        // housekeeping, owed whatever the libraries are set to do.
        crate::activity::forget_the_old(&state).await;
        loop {
            tokio::time::sleep(LOOK_AT_THE_CLOCK_EVERY).await;

            if melyxar_core::time::now() - last_sweep >= FORGET_UNUSED_SESSIONS_EVERY {
                last_sweep = forget_the_sessions_nobody_uses(&state).await;
                crate::activity::forget_the_old(&state).await;
            }

            let Ok(rows) = state.database().scheduled_tasks().await else {
                continue;
            };
            let now = melyxar_core::time::now();
            let due = due_now(&rows, &last_set_going, now, since_start);
            if due.is_empty() {
                continue;
            }
            for task in &due {
                last_set_going.insert(*task, now);
            }
            tracing::info!(
                tasks = due.iter().map(|task| task.as_str()).collect::<Vec<_>>().join(", "),
                "scheduled tasks are due"
            );
            start_in_turn(&state, due, JobPriority::BACKGROUND);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_database::schedule::ScheduledTaskRow;
    use time::macros::datetime;

    fn row(task: ScheduledTask, runs: bool, at_utc_minutes: i64) -> ScheduledTaskRow {
        ScheduledTaskRow {
            task: task.as_str().to_string(),
            runs_on_schedule: runs,
            at_utc_minutes,
            last_started_at: None,
            last_finished_at: None,
            last_state: None,
        }
    }

    #[test]
    fn every_task_round_trips_through_its_word_and_the_order_is_the_run_order() {
        for task in ScheduledTask::ALL {
            assert_eq!(ScheduledTask::parse(task.as_str()), Some(task));
        }
        assert_eq!(ScheduledTask::parse("nothing"), None);
        assert_eq!(ScheduledTask::ALL[0], ScheduledTask::Scan, "a file is found first");
        assert_eq!(ScheduledTask::ALL[1], ScheduledTask::Identify, "then named");
        assert_eq!(ScheduledTask::ALL[2], ScheduledTask::Ratings, "then rated, by its name");
    }

    #[test]
    fn a_task_is_due_once_its_time_has_come_and_only_once() {
        let since_start = datetime!(2026-09-27 12:00 UTC);
        let rows = vec![
            row(ScheduledTask::Scan, true, 3 * 60),
            row(ScheduledTask::Identify, false, 3 * 60),
            row(ScheduledTask::Reading(UpkeepTask::Thumbnails), true, 4 * 60),
        ];
        let mut last = std::collections::HashMap::new();

        assert!(
            due_now(&rows, &last, datetime!(2026-09-27 15:00 UTC), since_start).is_empty(),
            "three o'clock this morning was before the server came up"
        );
        assert_eq!(
            due_now(&rows, &last, datetime!(2026-09-28 03:02 UTC), since_start),
            vec![ScheduledTask::Scan],
            "a switched off task is never due, and one at four is not yet"
        );
        last.insert(ScheduledTask::Scan, datetime!(2026-09-28 03:02 UTC));
        assert!(
            due_now(&rows, &last, datetime!(2026-09-28 03:07 UTC), since_start).is_empty(),
            "set going once, it is not due again the same day"
        );
        assert_eq!(
            due_now(&rows, &last, datetime!(2026-09-28 04:03 UTC), since_start),
            vec![ScheduledTask::Reading(UpkeepTask::Thumbnails)]
        );
    }

    #[test]
    fn a_task_is_run_by_one_hand_at_a_time() {
        let running = Running::default();
        assert!(running.claim(ScheduledTask::Scan));
        assert!(!running.claim(ScheduledTask::Scan), "already under way");
        assert!(running.claim(ScheduledTask::Identify), "another task is its own");
        running.release(ScheduledTask::Scan);
        assert!(!running.holds(ScheduledTask::Scan));
        assert!(running.claim(ScheduledTask::Scan));
    }
}
