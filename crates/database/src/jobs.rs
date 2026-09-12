//! The job queue as it survives a restart.
//!
//! Background work is written down rather than merely held in memory, for two
//! reasons that both matter to someone watching the server. A screen can show
//! what is running and how far it got, and a restart can say what became of
//! the work that was in flight instead of leaving it hanging for ever.

use melyxar_core::id::JobId;
use melyxar_core::job::{Job, JobKind, JobPriority, JobState};
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
        sqlx::query("UPDATE jobs SET state = ?, failure_reason = ?, finished_at = ? WHERE id = ?")
            .bind(state.as_str())
            .bind(failure_reason)
            .bind(timestamp_to_text(now()))
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Closes the jobs a restart cut short.
    ///
    /// Nothing is running when the server comes up, so a row still saying
    /// otherwise is a leftover. Calling it failed with a plain reason beats
    /// showing a progress bar that will never move again.
    pub async fn close_interrupted_jobs(&self, reason: &str) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE jobs SET state = ?, failure_reason = ?, finished_at = ?
             WHERE state IN ('queued', 'running')",
        )
        .bind(JobState::Failed.as_str())
        .bind(reason)
        .bind(timestamp_to_text(now()))
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
            .close_interrupted_jobs("the server restarted")
            .await
            .expect("jobs closed");
        assert_eq!(closed, 2);

        assert!(database.unfinished_jobs().await.expect("read").is_empty());
        assert_eq!(
            database
                .job(queued.id)
                .await
                .expect("read")
                .expect("present")
                .failure_reason
                .as_deref(),
            Some("the server restarted")
        );
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
