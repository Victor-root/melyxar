//! What the machine spent, kept for the curves of the administration.
//!
//! One line a minute for a week, then one an hour for a year: the week is
//! where a question about last night is answered, the year only has to show
//! a trend. Lines are written once their span is over and never touched
//! again, except to be folded into hours once they are a week old.

use melyxar_core::time::Timestamp;
use sqlx::Row;

use crate::convert::{parse_timestamp, timestamp_to_text};
use crate::{Database, Result};

/// The two lengths a line can cover.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Span {
    Minute,
    Hour,
}

impl Span {
    fn seconds(self) -> i64 {
        match self {
            Self::Minute => 60,
            Self::Hour => 3600,
        }
    }
}

/// What the machine spent over one stretch of time, on average.
#[derive(Debug, Clone, PartialEq)]
pub struct Measure {
    /// Where the stretch starts.
    pub at: Timestamp,
    /// The share of the processors in use, from nought to one.
    pub processor: Option<f64>,
    pub memory_used: i64,
    pub memory_total: i64,
    pub load: Option<f64>,
    /// Bytes a second, in and out.
    pub received: f64,
    pub sent: f64,
    /// The share of the graphics card in use, when there is one.
    pub card: Option<f64>,
    /// Degrees, when the machine lets them be read.
    pub temperature: Option<f64>,
}

const COLUMNS: &str =
    "processor, memory_used, memory_total, load, received, sent, card, temperature";

impl Database {
    /// Writes the average of one span once it is over.
    ///
    /// Written again rather than refused when the span is already there: a
    /// server restarted within a minute writes that minute twice, and the
    /// second is the one that saw the end of it.
    pub async fn keep_measure(&self, span: Span, measure: &Measure) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO system_measures
                 (at, span_seconds, processor, memory_used, memory_total, load,
                  received, sent, card, temperature)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(timestamp_to_text(measure.at))
        .bind(span.seconds())
        .bind(measure.processor)
        .bind(measure.memory_used)
        .bind(measure.memory_total)
        .bind(measure.load)
        .bind(measure.received)
        .bind(measure.sent)
        .bind(measure.card)
        .bind(measure.temperature)
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Everything kept since an instant, averaged into stretches of the length
    /// asked for, oldest first.
    ///
    /// Averaged here rather than on the page: a month is forty thousand
    /// minutes, and a curve a few hundred points wide has no use for more
    /// than one of them in a hundred. A stretch where nothing was kept, the
    /// server being down, has no line at all, which is what a curve needs to
    /// show the gap.
    pub async fn measures_since(
        &self,
        since: Timestamp,
        stretch_seconds: i64,
    ) -> Result<Vec<Measure>> {
        let rows = sqlx::query(
            "SELECT min(at) AS at,
                    avg(processor) AS processor,
                    CAST(avg(memory_used) AS INTEGER) AS memory_used,
                    max(memory_total) AS memory_total,
                    avg(load) AS load,
                    avg(received) AS received,
                    avg(sent) AS sent,
                    avg(card) AS card,
                    avg(temperature) AS temperature
             FROM system_measures
             WHERE at >= ?
             GROUP BY CAST(strftime('%s', at) AS INTEGER) / ?
             ORDER BY at",
        )
        .bind(timestamp_to_text(since))
        .bind(stretch_seconds.max(1))
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                Ok(Measure {
                    at: parse_timestamp(&row.try_get::<String, _>("at")?)?,
                    processor: row.try_get("processor")?,
                    memory_used: row.try_get("memory_used")?,
                    memory_total: row.try_get("memory_total")?,
                    load: row.try_get("load")?,
                    received: row.try_get("received")?,
                    sent: row.try_get("sent")?,
                    card: row.try_get("card")?,
                    temperature: row.try_get("temperature")?,
                })
            })
            .collect()
    }

    /// Folds the minutes older than one instant into hours, and forgets the
    /// hours older than another.
    ///
    /// The first instant has to fall on the hour, which the caller sees to: an
    /// hour folded while half its minutes were still to come would be folded
    /// again an hour later from the other half alone.
    pub async fn fold_measures(
        &self,
        minutes_before: Timestamp,
        hours_before: Timestamp,
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "INSERT OR REPLACE INTO system_measures (at, span_seconds, {COLUMNS})
             SELECT substr(at, 1, 13) || ':00:00Z', 3600,
                    avg(processor), CAST(avg(memory_used) AS INTEGER), max(memory_total),
                    avg(load), avg(received), avg(sent), avg(card), avg(temperature)
             FROM system_measures
             WHERE span_seconds = 60 AND at < ?
             GROUP BY substr(at, 1, 13)"
        )))
        .bind(timestamp_to_text(minutes_before))
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM system_measures WHERE span_seconds = 60 AND at < ?")
            .bind(timestamp_to_text(minutes_before))
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM system_measures WHERE span_seconds = 3600 AND at < ?")
            .bind(timestamp_to_text(hours_before))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn a_minute(at: Timestamp, processor: f64) -> Measure {
        Measure {
            at,
            processor: Some(processor),
            memory_used: 1000,
            memory_total: 4000,
            load: Some(0.5),
            received: 100.0,
            sent: 10.0,
            card: None,
            temperature: None,
        }
    }

    #[tokio::test]
    async fn minutes_come_back_averaged_into_the_stretches_asked_for() {
        let database = Database::open_in_memory().await.expect("database opens");
        let start = datetime!(2026-09-23 14:00 UTC);
        for (minute, processor) in [(0, 0.2), (1, 0.4), (2, 0.6), (3, 0.8)] {
            database
                .keep_measure(Span::Minute, &a_minute(start + time::Duration::minutes(minute), processor))
                .await
                .expect("kept");
        }

        let pairs = database.measures_since(start, 120).await.expect("read");
        assert_eq!(pairs.len(), 2);
        assert_eq!(pairs[0].at, start);
        assert!((pairs[0].processor.expect("a share") - 0.3).abs() < 1e-9);
        assert!((pairs[1].processor.expect("a share") - 0.7).abs() < 1e-9);
        assert_eq!(pairs[1].memory_total, 4000);
        assert_eq!(pairs[1].card, None, "nothing measured stays nothing");

        let later = database
            .measures_since(start + time::Duration::minutes(2), 60)
            .await
            .expect("read");
        assert_eq!(later.len(), 2, "only what was kept since the instant asked");
    }

    #[tokio::test]
    async fn a_minute_written_twice_is_kept_once() {
        let database = Database::open_in_memory().await.expect("database opens");
        let at = datetime!(2026-09-23 14:00 UTC);
        database.keep_measure(Span::Minute, &a_minute(at, 0.1)).await.expect("kept");
        database.keep_measure(Span::Minute, &a_minute(at, 0.9)).await.expect("kept");
        let kept = database.measures_since(at, 60).await.expect("read");
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].processor, Some(0.9));
    }

    #[tokio::test]
    async fn old_minutes_are_folded_into_hours_and_old_hours_forgotten() {
        let database = Database::open_in_memory().await.expect("database opens");
        let long_ago = datetime!(2025-01-01 10:00 UTC);
        let last_week = datetime!(2026-09-15 10:00 UTC);
        let today = datetime!(2026-09-23 10:00 UTC);
        database
            .keep_measure(Span::Hour, &a_minute(long_ago, 0.5))
            .await
            .expect("kept");
        for (minute, processor) in [(0, 0.2), (30, 0.4)] {
            database
                .keep_measure(Span::Minute, &a_minute(last_week + time::Duration::minutes(minute), processor))
                .await
                .expect("kept");
        }
        database.keep_measure(Span::Minute, &a_minute(today, 0.9)).await.expect("kept");

        database
            .fold_measures(datetime!(2026-09-16 10:00 UTC), datetime!(2025-09-23 10:00 UTC))
            .await
            .expect("folded");

        let kept = database.measures_since(long_ago, 60).await.expect("read");
        assert_eq!(kept.len(), 2, "the year old hour is gone");
        assert_eq!(kept[0].at, last_week, "a week old minutes are one hour now");
        assert!((kept[0].processor.expect("a share") - 0.3).abs() < 1e-9);
        assert_eq!(kept[1].at, today, "today's minute is left alone");

        let rows: (i64,) = sqlx::query_as("SELECT count(*) FROM system_measures WHERE span_seconds = 60")
            .fetch_one(database.reader())
            .await
            .expect("counted");
        assert_eq!(rows.0, 1);
    }
}
