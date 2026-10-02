//! The scheduled tasks: whether each runs on its own each day, when, and how
//! its last run went.
//!
//! Each task is valid for every library and is named by the word the layer
//! above gives it; this layer keeps the rows and knows nothing of what a task
//! does.

use melyxar_core::job::JobState;
use melyxar_core::time::{now, Timestamp};
use sqlx::Row;

use crate::convert::{int_to_bool, parse_optional_timestamp, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

/// Minutes in a day, which a time of day has to stay inside.
const MINUTES_IN_A_DAY: i64 = 24 * 60;

/// One scheduled task, as kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledTaskRow {
    pub task: String,
    /// Whether it runs on its own each day.
    pub runs_on_schedule: bool,
    /// When it does, in minutes since midnight, UTC.
    pub at_utc_minutes: i64,
    pub last_started_at: Option<Timestamp>,
    pub last_finished_at: Option<Timestamp>,
    /// How its last run ended, once one has.
    pub last_state: Option<JobState>,
}

impl Database {
    /// Every scheduled task, in no particular order: the layer above knows
    /// the order they are shown and run in.
    pub async fn scheduled_tasks(&self) -> Result<Vec<ScheduledTaskRow>> {
        let rows = sqlx::query(
            "SELECT task, runs_on_schedule, at_utc_minutes,
                    last_started_at, last_finished_at, last_state
               FROM scheduled_tasks",
        )
        .fetch_all(self.reader())
        .await?;
        rows.iter()
            .map(|row| {
                let state: Option<String> = row.try_get("last_state")?;
                Ok(ScheduledTaskRow {
                    task: row.try_get("task")?,
                    runs_on_schedule: int_to_bool(row.try_get("runs_on_schedule")?),
                    at_utc_minutes: row.try_get("at_utc_minutes")?,
                    last_started_at: parse_optional_timestamp(
                        row.try_get::<Option<String>, _>("last_started_at")?
                            .as_deref(),
                    )?,
                    last_finished_at: parse_optional_timestamp(
                        row.try_get::<Option<String>, _>("last_finished_at")?
                            .as_deref(),
                    )?,
                    last_state: state
                        .map(|state| {
                            JobState::parse(&state).ok_or_else(|| {
                                DatabaseError::Corrupt(format!("task state '{state}' is unknown"))
                            })
                        })
                        .transpose()?,
                })
            })
            .collect()
    }

    /// Says whether a task runs on its own each day, and when.
    ///
    /// The time is brought into a day rather than refused: a screen sending
    /// minus thirty minutes means half past eleven the evening before.
    pub async fn schedule_task(
        &self,
        task: &str,
        runs_on_schedule: bool,
        at_utc_minutes: i64,
    ) -> Result<()> {
        let result = sqlx::query(
            "UPDATE scheduled_tasks SET runs_on_schedule = ?, at_utc_minutes = ? WHERE task = ?",
        )
        .bind(runs_on_schedule)
        .bind(at_utc_minutes.rem_euclid(MINUTES_IN_A_DAY))
        .bind(task)
        .execute(self.writer())
        .await?;
        if result.rows_affected() == 0 {
            return Err(DatabaseError::Corrupt(format!("no scheduled task '{task}'")));
        }
        Ok(())
    }

    /// Writes down that a run of a task began, which forgets how the one
    /// before it ended.
    pub async fn task_started(&self, task: &str) -> Result<()> {
        sqlx::query(
            "UPDATE scheduled_tasks
                SET last_started_at = ?, last_finished_at = NULL, last_state = NULL
              WHERE task = ?",
        )
        .bind(timestamp_to_text(now()))
        .bind(task)
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Writes down how a run of a task ended.
    pub async fn task_finished(&self, task: &str, state: JobState) -> Result<()> {
        sqlx::query(
            "UPDATE scheduled_tasks SET last_finished_at = ?, last_state = ? WHERE task = ?",
        )
        .bind(timestamp_to_text(now()))
        .bind(state.as_str())
        .bind(task)
        .execute(self.writer())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn database() -> Database {
        Database::open_in_memory().await.expect("database opens")
    }

    fn row<'a>(rows: &'a [ScheduledTaskRow], task: &str) -> &'a ScheduledTaskRow {
        rows.iter()
            .find(|row| row.task == task)
            .unwrap_or_else(|| panic!("no row for {task}"))
    }

    #[tokio::test]
    async fn every_task_starts_where_the_nightly_run_stood() {
        let rows = database().await.scheduled_tasks().await.expect("read");
        let mut tasks: Vec<&str> = rows.iter().map(|row| row.task.as_str()).collect();
        tasks.sort_unstable();
        assert_eq!(
            tasks,
            [
                "identify",
                "key_frames",
                "openings",
                "ratings",
                "scan",
                "speech",
                "subtitles",
                "thumbnails",
                "translation"
            ]
        );
        for row in &rows {
            assert!(row.runs_on_schedule, "{}", row.task);
            assert_eq!(row.at_utc_minutes, 3 * 60, "{}", row.task);
            assert_eq!(row.last_started_at, None, "{}", row.task);
            assert_eq!(row.last_state, None, "{}", row.task);
        }
    }

    #[tokio::test]
    async fn a_task_keeps_its_own_schedule_and_its_last_run() {
        let database = database().await;
        database
            .schedule_task("thumbnails", false, -30)
            .await
            .expect("scheduled");
        database.task_started("scan").await.expect("started");

        let rows = database.scheduled_tasks().await.expect("read");
        let thumbnails = row(&rows, "thumbnails");
        assert!(!thumbnails.runs_on_schedule);
        assert_eq!(thumbnails.at_utc_minutes, 23 * 60 + 30, "brought into a day");
        let scan = row(&rows, "scan");
        assert!(scan.last_started_at.is_some());
        assert_eq!(scan.last_state, None, "under way, not ended");
        assert!(row(&rows, "identify").runs_on_schedule, "one task's answer is its own");

        database
            .task_finished("scan", JobState::Succeeded)
            .await
            .expect("finished");
        let rows = database.scheduled_tasks().await.expect("read");
        assert_eq!(row(&rows, "scan").last_state, Some(JobState::Succeeded));
        assert!(row(&rows, "scan").last_finished_at.is_some());

        assert!(
            database.schedule_task("nothing", true, 0).await.is_err(),
            "a task nobody knows is refused rather than quietly ignored"
        );
    }
}
