//! The activity journal: what people and the server did, one line each.
//!
//! Written as it happens, read by the administration, purged after the number
//! of days the server is set to keep, and never read on the hot path. What a
//! line says beyond its columns is a JSON object this crate does not look
//! inside: the layer above writes it and reads it back.

use melyxar_core::id::{ActivityId, DeviceId, UserId, WorkId};
use melyxar_core::time::Timestamp;
use sqlx::{AssertSqlSafe, Row};

use crate::convert::{parse_id, parse_timestamp, timestamp_to_text};
use crate::{Database, Result};

/// One line about to be written.
#[derive(Debug, Clone, PartialEq)]
pub struct NewActivity<'a> {
    pub at: Timestamp,
    pub kind: &'a str,
    pub user_id: Option<UserId>,
    pub work_id: Option<WorkId>,
    pub device_name: Option<&'a str>,
    /// A JSON object, as text.
    pub details: Option<&'a str>,
}

/// One line as it was written.
#[derive(Debug, Clone, PartialEq)]
pub struct Activity {
    pub id: ActivityId,
    pub at: Timestamp,
    pub kind: String,
    /// Absent once the account is gone.
    pub user_id: Option<UserId>,
    /// Absent once the work is gone.
    pub work_id: Option<WorkId>,
    pub device_name: Option<String>,
    pub details: Option<String>,
}

/// `?, ?, ?` for as many kinds as asked about.
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

impl Database {
    /// Writes one line, and answers its identifier.
    pub async fn record_activity(&self, line: &NewActivity<'_>) -> Result<ActivityId> {
        let id = ActivityId::new();
        sqlx::query(
            "INSERT INTO activity_log (id, occurred_at, user_id, kind, work_id, device_name, details)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(timestamp_to_text(line.at))
        .bind(line.user_id.map(|user| user.to_db_string()))
        .bind(line.kind)
        .bind(line.work_id.map(|work| work.to_db_string()))
        .bind(line.device_name)
        .bind(line.details)
        .execute(self.writer())
        .await?;
        Ok(id)
    }

    /// Gives the line of one device's sign in the browser its page found, and
    /// answers how many lines that changed.
    pub async fn name_the_browser_of_a_sign_in(
        &self,
        kind: &str,
        device: DeviceId,
        browser: &str,
    ) -> Result<u64> {
        let done = sqlx::query(
            "UPDATE activity_log SET details = json_set(details, '$.browser', ?)
             WHERE kind = ? AND json_extract(details, '$.device_id') = ?",
        )
        .bind(browser)
        .bind(kind)
        .bind(device.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(done.rows_affected())
    }

    /// The latest lines, newest first, of the kinds asked about or of every
    /// kind when none is, older than a line already shown when one is given.
    pub async fn activity_page(
        &self,
        kinds: &[&str],
        before: Option<ActivityId>,
        most: i64,
    ) -> Result<Vec<Activity>> {
        let mut conditions = Vec::new();
        if !kinds.is_empty() {
            conditions.push(format!("kind IN ({})", placeholders(kinds.len())));
        }
        if before.is_some() {
            conditions.push("id < ?".to_string());
        }
        let filter = match conditions.is_empty() {
            true => String::new(),
            false => format!("WHERE {}", conditions.join(" AND ")),
        };
        let sql = format!(
            "SELECT id, occurred_at, user_id, kind, work_id, device_name, details
               FROM activity_log {filter}
              ORDER BY id DESC
              LIMIT ?"
        );
        let mut query = sqlx::query(AssertSqlSafe(sql));
        for kind in kinds {
            query = query.bind(*kind);
        }
        if let Some(before) = before {
            query = query.bind(before.to_db_string());
        }
        let rows = query.bind(most).fetch_all(self.reader()).await?;

        rows.iter()
            .map(|row| {
                Ok(Activity {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    at: parse_timestamp(&row.try_get::<String, _>("occurred_at")?)?,
                    kind: row.try_get("kind")?,
                    user_id: row
                        .try_get::<Option<String>, _>("user_id")?
                        .as_deref()
                        .map(parse_id)
                        .transpose()?,
                    work_id: row
                        .try_get::<Option<String>, _>("work_id")?
                        .as_deref()
                        .map(parse_id)
                        .transpose()?,
                    device_name: row.try_get("device_name")?,
                    details: row.try_get("details")?,
                })
            })
            .collect()
    }

    /// How many lines of these kinds were written since an instant.
    pub async fn count_activity_since(&self, kinds: &[&str], since: Timestamp) -> Result<i64> {
        let sql = format!(
            "SELECT count(*) FROM activity_log
              WHERE kind IN ({}) AND occurred_at > ?",
            placeholders(kinds.len())
        );
        let mut query = sqlx::query_as::<_, (i64,)>(AssertSqlSafe(sql));
        for kind in kinds {
            query = query.bind(*kind);
        }
        let (count,) = query
            .bind(timestamp_to_text(since))
            .fetch_one(self.reader())
            .await?;
        Ok(count)
    }

    /// Throws away every line written before an instant, and says how many.
    pub async fn forget_activity_before(&self, at: Timestamp) -> Result<u64> {
        let done = sqlx::query("DELETE FROM activity_log WHERE occurred_at < ?")
            .bind(timestamp_to_text(at))
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected())
    }

    /// How far each point to look at had got when this administrator marked
    /// it as seen.
    pub async fn attention_seen(&self, user_id: UserId) -> Result<Vec<(String, i64)>> {
        Ok(
            sqlx::query_as("SELECT item, mark FROM attention_seen WHERE user_id = ?")
                .bind(user_id.to_db_string())
                .fetch_all(self.reader())
                .await?,
        )
    }

    /// Forgets that this administrator saw a point.
    pub async fn forget_attention_seen(&self, user_id: UserId, item: &str) -> Result<()> {
        sqlx::query("DELETE FROM attention_seen WHERE user_id = ? AND item = ?")
            .bind(user_id.to_db_string())
            .bind(item)
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Writes down that this administrator saw a point as far as this mark.
    pub async fn mark_attention_seen(&self, user_id: UserId, item: &str, mark: i64) -> Result<()> {
        sqlx::query(
            "INSERT INTO attention_seen (user_id, item, mark) VALUES (?, ?, ?)
             ON CONFLICT (user_id, item) DO UPDATE SET mark = excluded.mark",
        )
        .bind(user_id.to_db_string())
        .bind(item)
        .bind(mark)
        .execute(self.writer())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::user::Permissions;
    use time::macros::datetime;

    const MORNING: Timestamp = datetime!(2026-01-01 08:00 UTC);
    const NOON: Timestamp = datetime!(2026-01-01 12:00 UTC);
    const EVENING: Timestamp = datetime!(2026-01-01 20:00 UTC);

    fn line(at: Timestamp, kind: &str) -> NewActivity<'_> {
        NewActivity {
            at,
            kind,
            user_id: None,
            work_id: None,
            device_name: None,
            details: None,
        }
    }

    #[tokio::test]
    async fn the_line_of_a_sign_in_is_given_the_browser_of_its_own_device_only() {
        let database = Database::open_in_memory().await.expect("database opens");
        let here = DeviceId::new();
        let elsewhere = DeviceId::new();
        for device in [here, elsewhere] {
            let details = serde_json::json!({ "device_id": device.to_db_string() }).to_string();
            database
                .record_activity(&NewActivity {
                    details: Some(&details),
                    ..line(NOON, "signed_in")
                })
                .await
                .expect("written");
        }

        assert_eq!(
            database
                .name_the_browser_of_a_sign_in("signed_in", here, "Brave")
                .await
                .expect("named"),
            1
        );
        let page = database.activity_page(&[], None, 10).await.expect("read");
        let browser_of = |device: DeviceId| {
            page.iter()
                .find(|line| {
                    line.details
                        .as_deref()
                        .is_some_and(|details| details.contains(&device.to_db_string()))
                })
                .and_then(|line| {
                    serde_json::from_str::<serde_json::Value>(line.details.as_deref()?).ok()
                })
                .map(|details| details["browser"].clone())
        };
        assert_eq!(browser_of(here), Some(serde_json::json!("Brave")));
        assert_eq!(browser_of(elsewhere), Some(serde_json::Value::Null));
    }

    #[tokio::test]
    async fn a_line_comes_back_as_it_was_written() {
        let database = Database::open_in_memory().await.expect("database opens");
        let user = database
            .create_user("somebody", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account created");
        let written = NewActivity {
            at: NOON,
            kind: "signed_in",
            user_id: Some(user.id),
            work_id: None,
            device_name: Some("a browser"),
            details: Some(r#"{"user_name":"somebody"}"#),
        };
        let id = database.record_activity(&written).await.expect("written");

        let page = database.activity_page(&[], None, 10).await.expect("read");
        assert_eq!(page.len(), 1);
        assert_eq!(page[0].id, id);
        assert_eq!(page[0].at, NOON);
        assert_eq!(page[0].kind, "signed_in");
        assert_eq!(page[0].user_id, Some(user.id));
        assert_eq!(page[0].device_name.as_deref(), Some("a browser"));
        assert_eq!(page[0].details.as_deref(), Some(r#"{"user_name":"somebody"}"#));

        database.delete_user(user.id).await.expect("removed");
        let page = database.activity_page(&[], None, 10).await.expect("read");
        assert_eq!(page[0].user_id, None, "the line outlives the account");
    }

    #[tokio::test]
    async fn a_page_is_newest_first_narrowed_by_kind_and_carried_on_from_the_last_line() {
        let database = Database::open_in_memory().await.expect("database opens");
        let mut written = Vec::new();
        for kind in ["signed_in", "watched", "signed_in", "task_finished", "signed_in"] {
            written.push(database.record_activity(&line(NOON, kind)).await.expect("written"));
        }

        let first = database.activity_page(&[], None, 2).await.expect("read");
        assert_eq!(
            first.iter().map(|one| one.id).collect::<Vec<_>>(),
            vec![written[4], written[3]]
        );
        let next = database
            .activity_page(&[], Some(written[3]), 2)
            .await
            .expect("read");
        assert_eq!(
            next.iter().map(|one| one.id).collect::<Vec<_>>(),
            vec![written[2], written[1]]
        );

        let sign_ins = database
            .activity_page(&["signed_in", "task_finished"], Some(written[4]), 10)
            .await
            .expect("read");
        assert_eq!(
            sign_ins.iter().map(|one| one.id).collect::<Vec<_>>(),
            vec![written[3], written[2], written[0]]
        );
    }

    #[tokio::test]
    async fn lines_are_counted_by_kind_since_an_instant_and_forgotten_before_one() {
        let database = Database::open_in_memory().await.expect("database opens");
        for (at, kind) in [
            (MORNING, "sign_in_refused"),
            (NOON, "sign_in_refused"),
            (EVENING, "sign_in_refused"),
            (EVENING, "task_failed"),
            (EVENING, "signed_in"),
        ] {
            database.record_activity(&line(at, kind)).await.expect("written");
        }

        assert_eq!(
            database
                .count_activity_since(&["sign_in_refused"], MORNING)
                .await
                .expect("counted"),
            2,
            "after the instant, not at it"
        );
        assert_eq!(
            database
                .count_activity_since(&["sign_in_refused", "task_failed"], NOON)
                .await
                .expect("counted"),
            2
        );

        assert_eq!(database.forget_activity_before(NOON).await.expect("forgotten"), 1);
        assert_eq!(database.activity_page(&[], None, 10).await.expect("read").len(), 4);
    }

    #[tokio::test]
    async fn a_line_just_after_a_round_instant_is_counted_after_it() {
        let database = Database::open_in_memory().await.expect("database opens");
        database
            .record_activity(&line(datetime!(2026-01-01 08:00:01.515 UTC), "sign_in_refused"))
            .await
            .expect("written");
        assert_eq!(
            database
                .count_activity_since(&["sign_in_refused"], datetime!(2026-01-01 08:00:01.51 UTC))
                .await
                .expect("counted"),
            1
        );
    }

    #[tokio::test]
    async fn what_an_administrator_saw_is_kept_for_them_alone_and_moves_on() {
        let database = Database::open_in_memory().await.expect("database opens");
        let one = database
            .create_user("one", Some("a stored form"), &Permissions::administrator())
            .await
            .expect("account created");
        let other = database
            .create_user("other", Some("a stored form"), &Permissions::administrator())
            .await
            .expect("account created");

        database.mark_attention_seen(one.id, "unidentified", 12).await.expect("marked");
        database.mark_attention_seen(one.id, "unidentified", 15).await.expect("marked again");
        database.mark_attention_seen(one.id, "failed_tasks", 99).await.expect("marked");

        let mut seen = database.attention_seen(one.id).await.expect("read");
        seen.sort();
        assert_eq!(
            seen,
            vec![("failed_tasks".to_string(), 99), ("unidentified".to_string(), 15)]
        );
        assert!(database.attention_seen(other.id).await.expect("read").is_empty());

        database.forget_attention_seen(one.id, "failed_tasks").await.expect("forgotten");
        assert_eq!(
            database.attention_seen(one.id).await.expect("read"),
            vec![("unidentified".to_string(), 15)]
        );
    }
}
