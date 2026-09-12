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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use melyxar_core::id::JobId;
use melyxar_core::job::{JobKind, JobPriority, JobState};
use melyxar_database::Database;
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
    cancelled: Arc<AtomicBool>,
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
        self.cancelled.load(Ordering::Relaxed)
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

/// Starts background work and keeps track of what is running.
#[derive(Clone)]
pub struct JobRunner {
    database: Database,
    running: Arc<Mutex<HashMap<JobId, Arc<AtomicBool>>>>,
}

impl JobRunner {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            running: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Closes whatever a previous run left hanging.
    ///
    /// Nothing is running when the server comes up, so a row still saying
    /// otherwise is a leftover from a stop or a crash.
    pub async fn close_interrupted(&self, reason: &str) -> Result<u64> {
        let closed = self.database.close_interrupted_jobs(reason).await?;
        if closed > 0 {
            tracing::info!(jobs = closed, "jobs interrupted by a restart were closed");
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
        let cancelled = Arc::new(AtomicBool::new(false));
        self.running
            .lock()
            .expect("the running lock is never held across an await")
            .insert(job.id, Arc::clone(&cancelled));

        let handle = JobHandle {
            id: job.id,
            database: self.database.clone(),
            cancelled: Arc::clone(&cancelled),
            progress: Arc::new(Mutex::new(Progress {
                done: 0,
                total: None,
                last_written: Instant::now(),
            })),
        };

        let database = self.database.clone();
        let running = Arc::clone(&self.running);
        let id = job.id;

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
                _ if cancelled.load(Ordering::Relaxed) => (JobState::Cancelled, None),
                Ok(()) => (JobState::Succeeded, None),
                Err(reason) => (JobState::Failed, Some(reason)),
            };

            if let Err(error) = database.finish_job(id, state, reason.as_deref()).await {
                tracing::error!(job = %id, error = %error, "the end of a job could not be recorded");
            }
            match state {
                JobState::Failed => {
                    tracing::warn!(job = %id, kind = kind.as_str(), reason = reason.unwrap_or_default(), "job failed")
                }
                _ => {
                    tracing::info!(job = %id, kind = kind.as_str(), state = state.as_str(), "job finished")
                }
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
            Some(flag) => {
                flag.store(true, Ordering::Relaxed);
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
    use std::sync::atomic::AtomicI64;

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

        assert_eq!(
            runner
                .close_interrupted("the server restarted")
                .await
                .expect("closed"),
            1
        );
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
