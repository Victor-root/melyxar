//! Background work: starting it, following it, and stopping it.
//!
//! Everything heavy the server does happens here rather than inside a request,
//! because a library has to stay browsable while a scan runs. Three promises
//! hold for every job:
//!
//! * it is written down before it starts, so a screen can show it and a
//!   restart can say what became of it;
//! * it can be stopped, and stopping means the work stops, not that the answer
//!   is ignored;
//! * two jobs of the same kind never run on the same subject at once.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use melyxar_core::id::JobId;
use melyxar_core::job::{Job, JobKind, JobPriority, JobState, JobStep};
use melyxar_database::Database;
use tokio::sync::watch;
use tokio::task::{JoinHandle, JoinSet};

#[derive(Debug, thiserror::Error)]
pub enum JobError {
    #[error("a job of this kind is already under way on the same subject")]
    AlreadyUnderWay,
    #[error(transparent)]
    Database(#[from] melyxar_database::DatabaseError),
}

pub type Result<T> = std::result::Result<T, JobError>;

/// A job that has been started.
pub struct StartedJob {
    pub id: JobId,
    /// Resolves with the state the job ended in. Dropping it lets the job run
    /// on, which is what an HTTP handler wants; a command line waits on it.
    pub completion: JoinHandle<JobState>,
}

/// How often progress reaches the database.
///
/// Writing every step would turn a scan of a large collection into tens of
/// thousands of writes for a bar the eye cannot follow anyway.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(500);

/// What a running job is given to report with.
#[derive(Clone)]
pub struct JobHandle {
    id: JobId,
    database: Database,
    cancelled: watch::Receiver<bool>,
    progress: Arc<Mutex<Progress>>,
}

struct Progress {
    done: i64,
    total: Option<i64>,
    last_written: Instant,
}

impl JobHandle {
    pub fn id(&self) -> JobId {
        self.id
    }

    /// Whether someone asked for this job to stop.
    ///
    /// A job is expected to look at this between steps. Stopping between two
    /// files is what makes cancellation leave the library in a sound state.
    pub fn is_cancelled(&self) -> bool {
        *self.cancelled.borrow()
    }

    /// The same answer, in a form something else can wait on.
    ///
    /// Looking between two files is enough for work counted in files. It is
    /// not enough for one file that takes a quarter of an hour: a reading that
    /// takes a whole film past has to be told during the reading, or the stop
    /// is only honoured once the film nobody wants read any more has been read
    /// to its end. What is handed over is the plainest thing that can carry
    /// the news, so that nothing below this crate has to know what a job is.
    pub fn cancelled_when(&self) -> watch::Receiver<bool> {
        self.cancelled.clone()
    }

    /// Says what the job is on at this very moment: the name of one file.
    ///
    /// A bar and a pass name say a scan is reading films and how far along it
    /// is. They do not say that it has been on the same film for four minutes
    /// because that film is four hours long, which is the difference between a
    /// server working and a server stuck.
    ///
    /// Written straight away rather than at the next beat, for the same reason
    /// as the pass name above: it exists to be read while somebody is watching
    /// the screen.
    pub async fn now_working_on(&self, what: Option<&str>) {
        // Written to the log as well as the screen: a pass through heavy
        // files is exactly the kind of thing worth lining up in time against
        // whatever else the machine was doing at the same moment, and the
        // screen alone keeps no history once the next file replaces it.
        if let Some(name) = what {
            tracing::debug!(job = %self.id, file = %name, "a job moved on to a file");
        }
        if let Err(error) = self.database.set_job_doing(self.id, what).await {
            tracing::warn!(job = %self.id, error = %error, "what a job is on could not be recorded");
        }
    }

    /// Says which pass the job has moved on to.
    ///
    /// Written straight away rather than at the next beat: a pass is announced
    /// precisely when the one before it stopped moving, and that is the moment
    /// somebody is looking at the screen wondering whether anything is still
    /// happening.
    ///
    /// The count starts over with it, since the new pass counts something else.
    pub async fn at_step(&self, step: JobStep) {
        {
            let mut progress = self
                .progress
                .lock()
                .expect("the progress lock is never held across an await");
            progress.done = 0;
            progress.total = None;
            progress.last_written = Instant::now();
        }
        // A step's own name says whether it is worth the machine's heavy
        // parts: reading key frames barely touches them, making thumbnails or
        // asking a card to decode a film both do. Written here rather than
        // left to the screen alone, so a moment that turns out to matter can
        // be found again after the fact instead of only while somebody was
        // looking at the time.
        tracing::debug!(job = %self.id, step = step.as_str(), "a job moved on to a step");
        if let Err(error) = self.database.start_job_step(self.id, step).await {
            tracing::warn!(job = %self.id, error = %error, "the step of a job could not be recorded");
        }
    }

    /// Says how much work there turned out to be.
    ///
    /// Called once the job has sized its work up, which is usually after its
    /// first step rather than before it.
    pub async fn set_total(&self, total: i64) {
        {
            let mut progress = self
                .progress
                .lock()
                .expect("the progress lock is never held across an await");
            progress.total = Some(total);
        }
        self.write_progress().await;
    }

    /// Records one more step done.
    pub async fn advance(&self, by: i64) {
        let due = {
            let mut progress = self
                .progress
                .lock()
                .expect("the progress lock is never held across an await");
            progress.done += by;
            progress.last_written.elapsed() >= PROGRESS_INTERVAL
        };
        if due {
            self.write_progress().await;
        }
    }

    async fn write_progress(&self) {
        let (done, total) = {
            let mut progress = self
                .progress
                .lock()
                .expect("the progress lock is never held across an await");
            progress.last_written = Instant::now();
            (progress.done, progress.total)
        };
        if let Err(error) = self.database.set_job_progress(self.id, done, total).await {
            // Progress is a comfort, never a result: failing to write it must
            // not take the work down with it.
            tracing::warn!(job = %self.id, error = %error, "progress could not be recorded");
        }
    }
}

/// A job over, as whoever is told of it hears it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finished {
    pub kind: JobKind,
    pub state: JobState,
    pub reason: Option<String>,
    pub target_id: Option<String>,
    pub took: Duration,
}

/// Told of every job once it is over.
type Listener = Arc<dyn Fn(Finished) + Send + Sync>;

/// Starts background work and keeps track of what is running.
#[derive(Clone)]
pub struct JobRunner {
    database: Database,
    running: Arc<Mutex<HashMap<JobId, watch::Sender<bool>>>>,
    told: Option<Listener>,
}

impl JobRunner {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            running: Arc::new(Mutex::new(HashMap::new())),
            told: None,
        }
    }

    /// The same, telling someone of every job once it is over.
    pub fn telling(mut self, listener: impl Fn(Finished) + Send + Sync + 'static) -> Self {
        self.told = Some(Arc::new(listener));
        self
    }

    /// Closes whatever a previous run left hanging, and says what it was.
    ///
    /// Nothing is running when the server comes up, so a row still saying
    /// otherwise is a leftover from a stop or a crash. What was closed comes
    /// back so the caller can start it again: this is the only moment that
    /// knowledge exists, since the next restart will find these rows long
    /// since closed.
    pub async fn close_interrupted(&self) -> Result<Vec<Job>> {
        let closed = self.database.close_interrupted_jobs().await?;
        if !closed.is_empty() {
            tracing::info!(
                jobs = closed.len(),
                "jobs interrupted by a restart were closed"
            );
        }
        Ok(closed)
    }

    /// Writes a job down, then runs it.
    ///
    /// Refuses when one of the same kind is already under way on the same
    /// subject, which is what stops two scans of one library from walking over
    /// each other.
    pub async fn start<F, Fut>(
        &self,
        kind: JobKind,
        priority: JobPriority,
        target_id: Option<String>,
        body: F,
    ) -> Result<StartedJob>
    where
        F: FnOnce(JobHandle) -> Fut + Send + 'static,
        Fut: Future<Output = std::result::Result<(), String>> + Send + 'static,
    {
        if self
            .database
            .has_unfinished_job(kind, target_id.as_deref())
            .await?
        {
            return Err(JobError::AlreadyUnderWay);
        }

        let job = self
            .database
            .create_job(kind, priority, target_id.as_deref())
            .await?;
        let (asked_to_stop, cancelled) = watch::channel(false);
        self.running
            .lock()
            .expect("the running lock is never held across an await")
            .insert(job.id, asked_to_stop);

        let handle = JobHandle {
            id: job.id,
            database: self.database.clone(),
            cancelled: cancelled.clone(),
            progress: Arc::new(Mutex::new(Progress {
                done: 0,
                total: None,
                last_written: Instant::now(),
            })),
        };

        let database = self.database.clone();
        let running = Arc::clone(&self.running);
        let told = self.told.clone();
        let id = job.id;
        let started = Instant::now();

        let completion = tokio::spawn(async move {
            if let Err(error) = database.mark_job_running(id).await {
                tracing::warn!(job = %id, error = %error, "the start of a job could not be recorded");
            }
            tracing::info!(job = %id, kind = kind.as_str(), "job started");

            let outcome = body(handle.clone()).await;
            handle.write_progress().await;

            let (state, reason) = match outcome {
                // A job that was asked to stop reports a stop, whatever it
                // returned: it did not fail, it was told to stop.
                _ if *cancelled.borrow() => (JobState::Cancelled, None),
                Ok(()) => (JobState::Succeeded, None),
                Err(reason) => (JobState::Failed, Some(reason)),
            };

            if let Err(error) = database.finish_job(id, state, reason.as_deref()).await {
                tracing::error!(job = %id, error = %error, "the end of a job could not be recorded");
            }
            match state {
                JobState::Failed => {
                    tracing::warn!(job = %id, kind = kind.as_str(), reason = reason.as_deref().unwrap_or_default(), "job failed")
                }
                _ => {
                    tracing::info!(job = %id, kind = kind.as_str(), state = state.as_str(), "job finished")
                }
            }
            if let Some(told) = told {
                told(Finished {
                    kind,
                    state,
                    reason,
                    target_id: job.target_id,
                    took: started.elapsed(),
                });
            }

            running
                .lock()
                .expect("the running lock is never held across an await")
                .remove(&id);
            state
        });

        Ok(StartedJob {
            id: job.id,
            completion,
        })
    }

    /// Asks a job to stop. Answers whether it was running at all.
    pub fn cancel(&self, id: JobId) -> bool {
        let running = self
            .running
            .lock()
            .expect("the running lock is never held across an await");
        match running.get(&id) {
            Some(asked_to_stop) => {
                asked_to_stop.send_replace(true);
                tracing::info!(job = %id, "a job was asked to stop");
                true
            }
            None => false,
        }
    }

    /// How many jobs this server is running right now.
    pub fn running_count(&self) -> usize {
        self.running
            .lock()
            .expect("the running lock is never held across an await")
            .len()
    }

    pub fn database(&self) -> &Database {
        &self.database
    }
}

/// Runs one task per item, never more than `limit` at a time, and gives the
/// answers back in the order the items came in.
///
/// Bounding matters more than speed here: a scan that probes a hundred files
/// at once starves the very playback it is supposed to leave untouched.
pub async fn for_each_bounded<T, F, Fut, R>(items: Vec<T>, limit: usize, task: F) -> Vec<R>
where
    T: Send + 'static,
    F: Fn(T) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = R> + Send,
    R: Send + 'static,
{
    let limit = limit.max(1);
    let task = Arc::new(task);
    let mut ordered: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
    let mut set: JoinSet<(usize, R)> = JoinSet::new();

    for (index, item) in items.into_iter().enumerate() {
        if set.len() >= limit {
            if let Some(Ok((done, value))) = set.join_next().await {
                ordered[done] = Some(value);
            }
        }
        let task = Arc::clone(&task);
        set.spawn(async move { (index, task(item).await) });
    }

    while let Some(joined) = set.join_next().await {
        if let Ok((done, value)) = joined {
            ordered[done] = Some(value);
        }
    }

    ordered.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::job::JobKind;
    use std::sync::atomic::{AtomicI64, Ordering};

    async fn runner() -> JobRunner {
        JobRunner::new(Database::open_in_memory().await.expect("database opens"))
    }

    #[tokio::test]
    async fn a_job_that_finishes_is_recorded_as_having_succeeded() {
        let runner = runner().await;
        let started = runner
            .start(
                JobKind::ScanLibrary,
                JobPriority::REQUESTED,
                Some("films".to_string()),
                |handle| async move {
                    handle.set_total(2).await;
                    handle.advance(2).await;
                    Ok(())
                },
            )
            .await
            .expect("job started");

        assert_eq!(
            started.completion.await.expect("the task ran"),
            JobState::Succeeded
        );
        let stored = runner
            .database()
            .job(started.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.state, JobState::Succeeded);
        assert_eq!(stored.progress_done, 2);
        assert_eq!(stored.progress_total, Some(2));
        assert!(stored.finished_at.is_some());
        assert_eq!(runner.running_count(), 0);
    }

    #[tokio::test]
    async fn progress_is_not_written_down_once_per_step() {
        // Fifty steps in a few microseconds must not mean fifty writes: the
        // screen refreshes a few times a second, and the writer is single.
        let runner = runner().await;
        let database = runner.database().clone();

        let started = runner
            .start(
                JobKind::FetchImages,
                JobPriority::BACKGROUND,
                None,
                move |handle| async move {
                    handle.set_total(50).await;
                    for _ in 0..50 {
                        handle.advance(1).await;
                    }

                    // Read our own row, by the identifier the handle carries.
                    let midway = database
                        .job(handle.id())
                        .await
                        .expect("read")
                        .expect("the row of a running job is there");
                    assert_eq!(
                        midway.progress_done, 0,
                        "nothing was written since the total: the steps were too close together"
                    );
                    assert_eq!(midway.progress_total, Some(50));
                    Ok(())
                },
            )
            .await
            .expect("job started");

        assert_eq!(
            started.completion.await.expect("the task ran"),
            JobState::Succeeded
        );
        let stored = runner
            .database()
            .job(started.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            stored.progress_done, 50,
            "what was held back during the run is written down at the end"
        );
    }

    #[tokio::test]
    async fn a_job_that_moves_on_says_so_at_once_and_counts_the_new_pass() {
        // The defect this exists for: the long passes of a scan are at the
        // end, and a bar that fills up, drops back to nothing and sets off
        // again with no word anywhere looks like a server that crashed.
        let runner = runner().await;
        let database = runner.database().clone();

        let started = runner
            .start(
                JobKind::ScanLibrary,
                JobPriority::REQUESTED,
                Some("films".to_string()),
                move |handle| async move {
                    handle.at_step(JobStep::AnalysingFiles).await;
                    handle.set_total(400).await;
                    handle.advance(400).await;

                    // What a pass picking up where it left off does: it counts
                    // what is already done, then says how much there is. The
                    // size is what writes both down, so the screen never shows
                    // nought while nine tenths of the work is behind it.
                    handle.at_step(JobStep::ReadingKeyFrames).await;
                    handle.advance(317).await;
                    handle.set_total(347).await;
                    let caught_up = database
                        .job(handle.id())
                        .await
                        .expect("read")
                        .expect("the row of a running job is there");
                    assert_eq!(
                        (caught_up.progress_done, caught_up.progress_total),
                        (317, Some(347)),
                        "what is already done has to reach the screen at once, not \
                         once the first film of this run has been read through"
                    );

                    handle.at_step(JobStep::ReadingKeyFrames).await;
                    let moved = database
                        .job(handle.id())
                        .await
                        .expect("read")
                        .expect("the row of a running job is there");
                    assert_eq!(
                        moved.step,
                        Some(JobStep::ReadingKeyFrames),
                        "a pass is written down the moment the one before it stopped moving"
                    );
                    assert_eq!(moved.progress_done, 0);
                    assert_eq!(moved.progress_total, None);

                    handle.set_total(340).await;
                    handle.advance(340).await;
                    Ok(())
                },
            )
            .await
            .expect("job started");

        assert_eq!(
            started.completion.await.expect("the task ran"),
            JobState::Succeeded
        );
        let stored = runner
            .database()
            .job(started.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.progress_done, 340, "and not the two passes added up");
        assert_eq!(stored.progress_total, Some(340));
    }

    #[tokio::test]
    async fn a_job_that_gives_up_keeps_the_reason_it_gave() {
        let runner = runner().await;
        let started = runner
            .start(
                JobKind::ScanLibrary,
                JobPriority::REQUESTED,
                None,
                |_| async move { Err("the root is not usable".to_string()) },
            )
            .await
            .expect("job started");

        assert_eq!(
            started.completion.await.expect("the task ran"),
            JobState::Failed
        );
        let stored = runner
            .database()
            .job(started.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            stored.failure_reason.as_deref(),
            Some("the root is not usable")
        );
    }

    #[tokio::test]
    async fn a_job_asked_to_stop_stops_the_work_rather_than_ignoring_the_answer() {
        let runner = runner().await;
        let steps = Arc::new(AtomicI64::new(0));
        let counted = Arc::clone(&steps);

        let started = runner
            .start(
                JobKind::ScanLibrary,
                JobPriority::REQUESTED,
                None,
                move |handle| async move {
                    for _ in 0..1_000 {
                        if handle.is_cancelled() {
                            return Ok(());
                        }
                        counted.fetch_add(1, Ordering::Relaxed);
                        tokio::time::sleep(Duration::from_millis(1)).await;
                    }
                    Ok(())
                },
            )
            .await
            .expect("job started");

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(runner.cancel(started.id), "the job was running");

        assert_eq!(
            started.completion.await.expect("the task ran"),
            JobState::Cancelled,
            "a job that was told to stop did not fail"
        );
        assert!(
            steps.load(Ordering::Relaxed) < 1_000,
            "stopping has to stop the work, not just the answer"
        );
    }

    /// Looking between two files is enough for work counted in files. One file
    /// that takes a quarter of an hour has to be told during the reading, so
    /// the same answer is also handed out in a form something can wait on.
    #[tokio::test]
    async fn a_job_told_to_stop_wakes_whatever_is_waiting_on_its_long_reading() {
        let runner = runner().await;
        let (told, was_told) = tokio::sync::oneshot::channel();

        let started = runner
            .start(
                JobKind::GenerateThumbnails,
                JobPriority::BACKGROUND,
                None,
                move |handle| async move {
                    let mut asked_to_stop = handle.cancelled_when();
                    // What a reading of one whole film does: it waits here
                    // rather than between two films.
                    while !*asked_to_stop.borrow_and_update() {
                        asked_to_stop.changed().await.expect("the runner is alive");
                    }
                    let _ = told.send(());
                    Ok(())
                },
            )
            .await
            .expect("job started");

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert!(runner.cancel(started.id), "the job was running");

        // Answered without the reading having to look for itself, which is the
        // whole point: nothing here polls.
        tokio::time::timeout(Duration::from_secs(5), was_told)
            .await
            .expect("the news reached the reading")
            .expect("the reading was still there to hear it");

        assert_eq!(
            started.completion.await.expect("the task ran"),
            JobState::Cancelled
        );
    }

    #[tokio::test]
    async fn cancelling_something_that_is_not_running_answers_plainly() {
        let runner = runner().await;
        assert!(!runner.cancel(JobId::new()));
    }

    #[tokio::test]
    async fn two_scans_of_one_library_never_run_at_once() {
        let runner = runner().await;
        let first = runner
            .start(
                JobKind::ScanLibrary,
                JobPriority::REQUESTED,
                Some("films".to_string()),
                |_| async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok(())
                },
            )
            .await
            .expect("job started");

        let refused = runner
            .start(
                JobKind::ScanLibrary,
                JobPriority::REQUESTED,
                Some("films".to_string()),
                |_| async move { Ok(()) },
            )
            .await;
        assert!(matches!(refused, Err(JobError::AlreadyUnderWay)));

        let other_library = runner
            .start(
                JobKind::ScanLibrary,
                JobPriority::REQUESTED,
                Some("series".to_string()),
                |_| async move { Ok(()) },
            )
            .await;
        assert!(other_library.is_ok(), "another library is another subject");

        first.completion.await.expect("the task ran");
        assert!(runner
            .start(
                JobKind::ScanLibrary,
                JobPriority::REQUESTED,
                Some("films".to_string()),
                |_| async move { Ok(()) },
            )
            .await
            .is_ok());
    }

    #[tokio::test]
    async fn a_restart_closes_what_it_cut_short() {
        let runner = runner().await;
        runner
            .database()
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, Some("films"))
            .await
            .expect("job created");

        assert_eq!(runner.close_interrupted().await.expect("closed").len(), 1);
        assert!(runner
            .database()
            .unfinished_jobs()
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn bounded_work_keeps_the_order_of_the_items_it_was_given() {
        let answers = for_each_bounded(vec![1, 2, 3, 4, 5], 2, |value| async move {
            tokio::time::sleep(Duration::from_millis(10 - value)).await;
            value * 10
        })
        .await;
        assert_eq!(answers, vec![10, 20, 30, 40, 50]);
    }

    #[tokio::test]
    async fn bounded_work_never_runs_more_than_it_was_allowed_to() {
        let live = Arc::new(AtomicI64::new(0));
        let peak = Arc::new(AtomicI64::new(0));
        let counted = Arc::clone(&live);
        let highest = Arc::clone(&peak);

        for_each_bounded((0..20).collect(), 3, move |_| {
            let counted = Arc::clone(&counted);
            let highest = Arc::clone(&highest);
            async move {
                let now = counted.fetch_add(1, Ordering::SeqCst) + 1;
                highest.fetch_max(now, Ordering::SeqCst);
                tokio::time::sleep(Duration::from_millis(5)).await;
                counted.fetch_sub(1, Ordering::SeqCst);
            }
        })
        .await;

        assert!(
            peak.load(Ordering::SeqCst) <= 3,
            "a scan that probes everything at once starves the playback it should leave alone"
        );
    }

    #[tokio::test]
    async fn bounded_work_on_nothing_is_not_a_special_case() {
        let answers: Vec<i32> =
            for_each_bounded(Vec::new(), 4, |value: i32| async move { value }).await;
        assert!(answers.is_empty());
    }
}
