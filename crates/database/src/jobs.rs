//! The job queue as it survives a restart.
//!
//! Background work is written down rather than merely held in memory, for two
//! reasons that both matter to someone watching the server. A screen can show
//! what is running and how far it got, and a restart can say what became of
//! the work that was in flight instead of leaving it hanging for ever.

use melyxar_core::id::JobId;
use melyxar_core::job::{Job, JobKind, JobPriority, JobState, JobStep};
use melyxar_core::time::now;
use sqlx::Row;

use crate::convert::{parse_optional_timestamp, parse_timestamp, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

impl Database {
    /// Writes a job down before anything starts running.
    pub async fn create_job(
        &self,
        kind: JobKind,
        priority: JobPriority,
        target_id: Option<&str>,
    ) -> Result<Job> {
        let id = JobId::new();
        let moment = now();

        sqlx::query(
            "INSERT INTO jobs (id, kind, priority, state, target_id, created_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(kind.as_str())
        .bind(priority.get())
        .bind(JobState::Queued.as_str())
        .bind(target_id)
        .bind(timestamp_to_text(moment))
        .execute(self.writer())
        .await?;

        Ok(Job {
            id,
            kind,
            priority,
            state: JobState::Queued,
            target_id: target_id.map(str::to_string),
            step: None,
            doing: None,
            progress_done: 0,
            progress_total: None,
            failure_reason: None,
            created_at: moment,
            started_at: None,
            finished_at: None,
        })
    }

    pub async fn job(&self, id: JobId) -> Result<Option<Job>> {
        let row = sqlx::query("SELECT * FROM jobs WHERE id = ?")
            .bind(id.to_db_string())
            .fetch_optional(self.reader())
            .await?;
        row.map(|row| job_from_row(&row)).transpose()
    }

    /// Jobs that have not finished, the ones a screen shows at the top.
    pub async fn unfinished_jobs(&self) -> Result<Vec<Job>> {
        let rows = sqlx::query(
            "SELECT * FROM jobs WHERE state IN ('queued', 'running')
             ORDER BY priority DESC, created_at",
        )
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(job_from_row).collect()
    }

    /// The latest jobs whatever their state, newest first.
    pub async fn recent_jobs(&self, limit: i64) -> Result<Vec<Job>> {
        let rows = sqlx::query("SELECT * FROM jobs ORDER BY created_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(self.reader())
            .await?;
        rows.iter().map(job_from_row).collect()
    }

    /// Whether a job of this kind is already under way on the same subject.
    ///
    /// This is what stops two scans of one library from running at once and
    /// walking over each other.
    pub async fn has_unfinished_job(&self, kind: JobKind, target_id: Option<&str>) -> Result<bool> {
        let row: (i64,) = sqlx::query_as(
            "SELECT count(*) FROM jobs
             WHERE kind = ? AND state IN ('queued', 'running')
               AND (target_id IS ? OR (target_id IS NULL AND ? IS NULL))",
        )
        .bind(kind.as_str())
        .bind(target_id)
        .bind(target_id)
        .fetch_one(self.reader())
        .await?;
        Ok(row.0 > 0)
    }

    pub async fn mark_job_running(&self, id: JobId) -> Result<()> {
        sqlx::query(
            "UPDATE jobs SET state = ?, started_at = ?, attempts = attempts + 1 WHERE id = ?",
        )
        .bind(JobState::Running.as_str())
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Says which pass the job has just moved on to, and starts its count over.
    ///
    /// The counters belong to the pass rather than to the job: a scan analyses
    /// four hundred files and then reads three hundred and forty of them for
    /// where they can be started, and adding those together would count the
    /// same films twice. Written in one go with the step so a screen can never
    /// catch the new pass carrying the old numbers.
    pub async fn start_job_step(&self, id: JobId, step: JobStep) -> Result<()> {
        sqlx::query(
            "UPDATE jobs SET step = ?, doing = NULL, progress_done = 0, progress_total = NULL
             WHERE id = ?",
        )
        .bind(step.as_str())
        .bind(id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Says what a job is on at this very moment, or that it is on nothing.
    ///
    /// Written straight away rather than at the next beat, like the pass it
    /// sits under: it exists to be read while somebody is looking at the
    /// screen wondering whether anything is still happening.
    pub async fn set_job_doing(&self, id: JobId, doing: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE jobs SET doing = ? WHERE id = ?")
            .bind(doing)
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Records how far a job got. The size is given once it is known, which is
    /// usually after the first step rather than before it.
    pub async fn set_job_progress(&self, id: JobId, done: i64, total: Option<i64>) -> Result<()> {
        sqlx::query(
            "UPDATE jobs SET progress_done = ?, progress_total = coalesce(?, progress_total)
             WHERE id = ?",
        )
        .bind(done)
        .bind(total)
        .bind(id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Closes a job. A reason is kept only for a failure, since that is the
    /// one an administrator has to read.
    pub async fn finish_job(
        &self,
        id: JobId,
        state: JobState,
        failure_reason: Option<&str>,
    ) -> Result<()> {
        // What it was on goes with it: a name left on a finished job describes
        // work that is over, and the list of finished jobs would read as though
        // every one of them had stopped on a film.
        sqlx::query(
            "UPDATE jobs SET state = ?, failure_reason = ?, finished_at = ?, doing = NULL
             WHERE id = ?",
        )
        .bind(state.as_str())
        .bind(failure_reason)
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Closes the jobs a restart cut short, and says which ones they were.
    ///
    /// Nothing is running when the server comes up, so a row still saying
    /// otherwise is a leftover. Recorded as interrupted rather than failed:
    /// the work was not going wrong, the server went away under it, and that
    /// is the one ending worth taking up again on its own.
    ///
    /// What was closed is handed back rather than counted, because the caller
    /// that means to start these again has no other way of knowing which they
    /// were: a restart later on would find them long since closed.
    ///
    /// No reason is written: the state already says a restart did this, and it
    /// says it in a word a screen can translate, where a sentence stored here
    /// would reach every screen in English whatever language it was set to.
    pub async fn close_interrupted_jobs(&self) -> Result<Vec<Job>> {
        let rows = sqlx::query("SELECT * FROM jobs WHERE state IN ('queued', 'running')")
            .fetch_all(self.reader())
            .await?;
        if rows.is_empty() {
            return Ok(Vec::new());
        }

        // The rows were read as they stood a moment ago, so what comes back
        // carries the ending this call is about to write rather than the state
        // it found. One moment for both, so the two never disagree.
        let moment = now();
        let cut_short: Vec<Job> = rows
            .iter()
            .map(|row| {
                job_from_row(row).map(|job| Job {
                    state: JobState::Interrupted,
                    finished_at: Some(moment),
                    ..job
                })
            })
            .collect::<Result<_>>()?;

        sqlx::query(
            "UPDATE jobs SET state = ?, finished_at = ?
             WHERE state IN ('queued', 'running')",
        )
        .bind(JobState::Interrupted.as_str())
        .bind(timestamp_to_text(moment))
        .execute(self.writer())
        .await?;
        Ok(cut_short)
    }

    /// Forgets the work that is over, and answers how much it forgot.
    ///
    /// The history is there to be read, and a history nobody can clear stops
    /// being readable: what matters is the last failure, not the four hundred
    /// runs before it. What is still running is never touched, since it is not
    /// history yet.
    pub async fn forget_finished_jobs(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM jobs WHERE state NOT IN ('queued', 'running')")
            .execute(self.writer())
            .await?;
        Ok(result.rows_affected())
    }
}

fn job_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Job> {
    let kind_text: String = row.try_get("kind")?;
    let state_text: String = row.try_get("state")?;
    Ok(Job {
        id: row
            .try_get::<String, _>("id")?
            .parse()
            .map_err(|_| DatabaseError::Corrupt("job identifier is malformed".to_string()))?,
        kind: JobKind::parse(&kind_text)
            .ok_or_else(|| DatabaseError::Corrupt(format!("job kind '{kind_text}' is unknown")))?,
        priority: JobPriority::from_stored(row.try_get("priority")?),
        state: JobState::parse(&state_text).ok_or_else(|| {
            DatabaseError::Corrupt(format!("job state '{state_text}' is unknown"))
        })?,
        target_id: row.try_get("target_id")?,
        // A step is a label on a screen, never a result: one this build does
        // not know reads as no step at all, where an unknown kind or state is
        // a row nothing could show and is reported. That is what lets a
        // database written by a newer build still be read by this one.
        step: row
            .try_get::<Option<String>, _>("step")?
            .as_deref()
            .and_then(JobStep::parse),
        doing: row.try_get("doing")?,
        progress_done: row.try_get("progress_done")?,
        progress_total: row.try_get("progress_total")?,
        failure_reason: row.try_get("failure_reason")?,
        created_at: parse_timestamp(&row.try_get::<String, _>("created_at")?)?,
        started_at: parse_optional_timestamp(
            row.try_get::<Option<String>, _>("started_at")?.as_deref(),
        )?,
        finished_at: parse_optional_timestamp(
            row.try_get::<Option<String>, _>("finished_at")?.as_deref(),
        )?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn database() -> Database {
        Database::open_in_memory().await.expect("database opens")
    }

    #[tokio::test]
    async fn a_job_is_written_down_before_anything_runs() {
        let database = database().await;
        let job = database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, Some("films"))
            .await
            .expect("job created");

        assert_eq!(job.state, JobState::Queued);
        assert!(job.started_at.is_none());
        assert_eq!(
            database.job(job.id).await.expect("read").expect("present"),
            job
        );
    }

    #[tokio::test]
    async fn progress_is_recorded_and_its_size_is_only_given_once_it_is_known() {
        let database = database().await;
        let job = database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, Some("films"))
            .await
            .expect("job created");

        database
            .mark_job_running(job.id)
            .await
            .expect("job running");
        database
            .set_job_progress(job.id, 0, Some(50))
            .await
            .expect("size known");
        database
            .set_job_progress(job.id, 25, None)
            .await
            .expect("progress recorded");

        let stored = database.job(job.id).await.expect("read").expect("present");
        assert_eq!(stored.state, JobState::Running);
        assert!(stored.started_at.is_some());
        assert_eq!(stored.progress_done, 25);
        assert_eq!(
            stored.progress_total,
            Some(50),
            "a later report without a size must not erase the one already known"
        );
        assert_eq!(stored.ratio(), Some(0.5));
    }

    #[tokio::test]
    async fn moving_on_to_the_next_pass_starts_its_count_over() {
        let database = database().await;
        let job = database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, Some("films"))
            .await
            .expect("job created");
        assert_eq!(
            database
                .job(job.id)
                .await
                .expect("read")
                .expect("present")
                .step,
            None,
            "a job that has not said where it is says nothing"
        );

        database
            .start_job_step(job.id, JobStep::AnalysingFiles)
            .await
            .expect("pass recorded");
        database
            .set_job_progress(job.id, 400, Some(400))
            .await
            .expect("progress recorded");
        database
            .start_job_step(job.id, JobStep::ReadingKeyFrames)
            .await
            .expect("next pass recorded");

        let stored = database.job(job.id).await.expect("read").expect("present");
        assert_eq!(stored.step, Some(JobStep::ReadingKeyFrames));
        assert_eq!(stored.progress_done, 0);
        assert_eq!(
            stored.progress_total, None,
            "the counters belong to the pass, and counting four hundred analysed \
             files towards three hundred read ones counts the same films twice"
        );
    }

    #[tokio::test]
    async fn what_a_job_is_on_is_kept_while_it_lasts_and_no_longer() {
        let database = database().await;
        let job = database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, Some("films"))
            .await
            .expect("job created");
        database
            .start_job_step(job.id, JobStep::AnalysingFiles)
            .await
            .expect("pass recorded");

        database
            .set_job_doing(job.id, Some("Quiet.Harbour.2019.mkv"))
            .await
            .expect("name recorded");
        assert_eq!(
            database
                .job(job.id)
                .await
                .expect("read")
                .expect("present")
                .doing
                .as_deref(),
            Some("Quiet.Harbour.2019.mkv")
        );

        database
            .start_job_step(job.id, JobStep::ReadingKeyFrames)
            .await
            .expect("next pass recorded");
        assert_eq!(
            database
                .job(job.id)
                .await
                .expect("read")
                .expect("present")
                .doing,
            None,
            "a name from the pass before describes a film this one has not reached"
        );

        database
            .set_job_doing(job.id, Some("Amber.Field.2020.mkv"))
            .await
            .expect("name recorded");
        database
            .finish_job(job.id, JobState::Succeeded, None)
            .await
            .expect("job finished");
        assert_eq!(
            database
                .job(job.id)
                .await
                .expect("read")
                .expect("present")
                .doing,
            None,
            "a finished job stopped on nothing, it stopped because it was done"
        );
    }

    #[tokio::test]
    async fn a_pass_this_build_does_not_know_leaves_the_job_readable() {
        // What a database written by a newer build looks like here. A step is
        // a word on a screen: losing it must not take the whole row with it.
        let database = database().await;
        let job = database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, None)
            .await
            .expect("job created");

        sqlx::query("UPDATE jobs SET step = 'something_new' WHERE id = ?")
            .bind(job.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("value forced");

        let stored = database.job(job.id).await.expect("read").expect("present");
        assert_eq!(stored.step, None);
        assert_eq!(stored.kind, JobKind::ScanLibrary);
    }

    #[tokio::test]
    async fn a_second_scan_of_the_same_library_is_recognised_as_already_under_way() {
        let database = database().await;
        let job = database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, Some("films"))
            .await
            .expect("job created");

        assert!(database
            .has_unfinished_job(JobKind::ScanLibrary, Some("films"))
            .await
            .expect("read"));
        assert!(
            !database
                .has_unfinished_job(JobKind::ScanLibrary, Some("series"))
                .await
                .expect("read"),
            "another library is another subject"
        );

        database
            .finish_job(job.id, JobState::Succeeded, None)
            .await
            .expect("job finished");
        assert!(!database
            .has_unfinished_job(JobKind::ScanLibrary, Some("films"))
            .await
            .expect("read"));
    }

    #[tokio::test]
    async fn a_failure_keeps_the_reason_because_someone_has_to_read_it() {
        let database = database().await;
        let job = database
            .create_job(JobKind::ScanLibrary, JobPriority::BACKGROUND, None)
            .await
            .expect("job created");

        database
            .finish_job(job.id, JobState::Failed, Some("the root is not usable"))
            .await
            .expect("job finished");

        let stored = database.job(job.id).await.expect("read").expect("present");
        assert_eq!(stored.state, JobState::Failed);
        assert_eq!(
            stored.failure_reason.as_deref(),
            Some("the root is not usable")
        );
        assert!(stored.finished_at.is_some());
    }

    #[tokio::test]
    async fn a_restart_closes_the_jobs_it_cut_short_rather_than_leaving_them_hanging() {
        let database = database().await;
        let queued = database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, Some("films"))
            .await
            .expect("job created");
        let running = database
            .create_job(JobKind::FetchImages, JobPriority::BACKGROUND, None)
            .await
            .expect("job created");
        database
            .mark_job_running(running.id)
            .await
            .expect("job running");
        let done = database
            .create_job(JobKind::Backup, JobPriority::BACKGROUND, None)
            .await
            .expect("job created");
        database
            .finish_job(done.id, JobState::Succeeded, None)
            .await
            .expect("job finished");

        let closed = database
            .close_interrupted_jobs()
            .await
            .expect("jobs closed");
        assert_eq!(closed.len(), 2);
        assert!(
            closed.iter().any(|job| job.id == queued.id)
                && closed.iter().any(|job| job.id == running.id),
            "what was closed has to come back, or nothing can start it again"
        );
        assert!(
            !closed.iter().any(|job| job.id == done.id),
            "a job that already ended was not cut short by anything"
        );
        assert!(
            closed
                .iter()
                .all(|job| job.state.is_worth_taking_up_again()),
            "what comes back carries the ending just written, not the state it \
             was read in: on the old state nothing would ever be taken up again"
        );

        assert!(database.unfinished_jobs().await.expect("read").is_empty());
        let cut_short = database
            .job(queued.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            cut_short.state,
            JobState::Interrupted,
            "the work was not going wrong, the server went away under it"
        );
        assert!(cut_short.state.is_worth_taking_up_again());
        assert_eq!(
            cut_short.failure_reason, None,
            "the state says a restart did this, in a word a screen can translate; \
             a sentence stored here would reach every screen in English"
        );
        assert!(cut_short.finished_at.is_some());
        assert_eq!(
            database
                .job(done.id)
                .await
                .expect("read")
                .expect("present")
                .state,
            JobState::Succeeded,
            "a job that already ended is left alone"
        );
    }

    #[tokio::test]
    async fn clearing_the_history_keeps_what_is_still_running() {
        let database = Database::open_in_memory().await.expect("database opens");
        let running = database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, None)
            .await
            .expect("job created");
        database
            .mark_job_running(running.id)
            .await
            .expect("job running");
        for state in [JobState::Succeeded, JobState::Failed, JobState::Cancelled] {
            let job = database
                .create_job(JobKind::Backup, JobPriority::BACKGROUND, None)
                .await
                .expect("job created");
            database
                .finish_job(job.id, state, None)
                .await
                .expect("job finished");
        }

        assert_eq!(
            database
                .forget_finished_jobs()
                .await
                .expect("history cleared"),
            3
        );
        assert_eq!(database.recent_jobs(50).await.expect("read").len(), 1);
        assert_eq!(
            database
                .job(running.id)
                .await
                .expect("read")
                .expect("a scan under way is not history"),
            database
                .unfinished_jobs()
                .await
                .expect("read")
                .into_iter()
                .next()
                .expect("still there")
        );
    }

    #[tokio::test]
    async fn unfinished_jobs_come_back_with_the_awaited_ones_first() {
        let database = database().await;
        database
            .create_job(JobKind::FetchImages, JobPriority::BACKGROUND, None)
            .await
            .expect("job created");
        database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, Some("films"))
            .await
            .expect("job created");

        let unfinished = database.unfinished_jobs().await.expect("read");
        assert_eq!(unfinished.len(), 2);
        assert_eq!(unfinished[0].kind, JobKind::ScanLibrary);
    }

    #[tokio::test]
    async fn the_activity_page_reads_back_what_has_been_running() {
        // Finished or not: this is the list the maintainer looks at when he
        // wants to know what the server has been doing.
        let database = database().await;
        let first = database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, Some("films"))
            .await
            .expect("job created");
        let second = database
            .create_job(JobKind::IdentifyWork, JobPriority::BACKGROUND, None)
            .await
            .expect("job created");
        database
            .finish_job(first.id, JobState::Succeeded, None)
            .await
            .expect("job finished");

        let recent = database.recent_jobs(10).await.expect("read");
        assert_eq!(recent.len(), 2, "a finished job is still worth showing");
        assert!(recent
            .iter()
            .any(|job| job.id == first.id && job.state == JobState::Succeeded));
        assert!(recent.iter().any(|job| job.id == second.id));

        assert_eq!(
            database.recent_jobs(1).await.expect("read").len(),
            1,
            "the page asks for a handful, not for the whole history"
        );
    }

    #[tokio::test]
    async fn a_malformed_stored_kind_is_reported_rather_than_guessed() {
        let database = database().await;
        let job = database
            .create_job(JobKind::ScanLibrary, JobPriority::REQUESTED, None)
            .await
            .expect("job created");

        sqlx::query("UPDATE jobs SET kind = 'something_new' WHERE id = ?")
            .bind(job.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("value forced");

        assert!(matches!(
            database.job(job.id).await,
            Err(DatabaseError::Corrupt(_))
        ));
    }
}
