//! Notifications: what each account is told and keeps, what it chose to be
//! told, and what the server has already announced of each library. Also the
//! points an administrator marked as seen.
//!
//! What a notification says is a JSON object this crate does not look
//! inside: the layer above writes it and reads it back.

use std::collections::HashMap;

use melyxar_core::id::{LibraryId, NotificationId, UserId, WorkId};
use melyxar_core::time::Timestamp;
use sqlx::{AssertSqlSafe, Row};

use crate::convert::{
    bool_to_int, int_to_bool, parse_id, parse_optional_timestamp, parse_timestamp,
    timestamp_to_text,
};
use crate::{Database, Result};

/// One notification about to be written, the same for every account that
/// receives it.
#[derive(Debug, Clone, PartialEq)]
pub struct NewNotification<'a> {
    pub kind: &'a str,
    pub level: &'a str,
    /// A JSON object, as text.
    pub data: &'a str,
    pub work_id: Option<WorkId>,
    pub priority: bool,
    pub mandatory: bool,
    pub sticky: bool,
    pub shown_for_ms: Option<i64>,
    pub due_at: Option<Timestamp>,
    /// Already recalled: a maintenance announced less than the recall ahead
    /// of its time has nothing left to recall.
    pub reminded: bool,
    pub created_at: Timestamp,
}

/// One notification as one account keeps it.
#[derive(Debug, Clone, PartialEq)]
pub struct Notification {
    pub id: NotificationId,
    pub user_id: UserId,
    pub kind: String,
    pub level: String,
    pub data: String,
    /// Absent once the work is gone.
    pub work_id: Option<WorkId>,
    pub priority: bool,
    pub mandatory: bool,
    pub sticky: bool,
    pub shown_for_ms: Option<i64>,
    pub due_at: Option<Timestamp>,
    pub created_at: Timestamp,
    pub read_at: Option<Timestamp>,
}

/// What became of asking to remove a notification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Removed,
    /// Mandatory or urgent: kept whatever the account asks.
    Kept,
    /// Not one of this account's.
    Missing,
}

/// Whether one kind of notification reaches the bell and the screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Channels {
    pub bell: bool,
    pub screen: bool,
}

/// An account's quiet hours and what still shows through them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationSettings {
    /// Minutes after midnight, on the clock of the device.
    pub quiet_from: Option<i32>,
    pub quiet_until: Option<i32>,
    pub priority_while_playing: bool,
    pub priority_while_quiet: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            quiet_from: None,
            quiet_until: None,
            priority_while_playing: true,
            priority_while_quiet: true,
        }
    }
}

/// A library, as far as announcing what arrives in it goes.
#[derive(Debug, Clone, PartialEq)]
pub struct AnnouncingLibrary {
    pub id: LibraryId,
    pub name: String,
    /// movies, series, and so on.
    pub kind: String,
    pub created_at: Timestamp,
    /// Absent until the administrator chose: it announces.
    pub announces: Option<bool>,
    /// Absent until the first look at it.
    pub announced_until: Option<Timestamp>,
}

/// A film or an episode added to a library.
#[derive(Debug, Clone, PartialEq)]
pub struct Arrival {
    pub work_id: WorkId,
    /// movie or episode.
    pub kind: String,
    pub title: String,
    pub added_at: Timestamp,
    /// The series an episode hangs under.
    pub series: Option<(WorkId, String)>,
    /// How the work that names it stands: the series for an episode, the
    /// film itself otherwise. pending, identified, unidentified, manual.
    pub identification: String,
    /// Whether the work that names it wears a poster.
    pub has_poster: bool,
    /// The age rating of the work that names it, when it has one.
    pub age_rating: Option<i32>,
}

const WHAT_A_NOTIFICATION_IS: &str = "id, user_id, kind, level, data, work_id, priority, \
     mandatory, sticky, shown_for_ms, due_at, created_at, read_at";

fn notification_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Notification> {
    Ok(Notification {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        user_id: parse_id(&row.try_get::<String, _>("user_id")?)?,
        kind: row.try_get("kind")?,
        level: row.try_get("level")?,
        data: row.try_get("data")?,
        work_id: row
            .try_get::<Option<String>, _>("work_id")?
            .as_deref()
            .map(parse_id)
            .transpose()?,
        priority: int_to_bool(row.try_get("priority")?),
        mandatory: int_to_bool(row.try_get("mandatory")?),
        sticky: int_to_bool(row.try_get("sticky")?),
        shown_for_ms: row.try_get("shown_for_ms")?,
        due_at: parse_optional_timestamp(row.try_get::<Option<String>, _>("due_at")?.as_deref())?,
        created_at: parse_timestamp(&row.try_get::<String, _>("created_at")?)?,
        read_at: parse_optional_timestamp(row.try_get::<Option<String>, _>("read_at")?.as_deref())?,
    })
}

/// `?, ?, ?` for as many values as asked about.
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

impl Database {
    /// Writes one notification for each account, all or none of them, and
    /// answers them as written.
    pub async fn add_notifications(
        &self,
        recipients: &[UserId],
        new: &NewNotification<'_>,
    ) -> Result<Vec<Notification>> {
        let mut written = Vec::with_capacity(recipients.len());
        let mut transaction = self.begin().await?;
        for user in recipients {
            let row = sqlx::query(AssertSqlSafe(format!(
                "INSERT INTO notifications (id, user_id, kind, level, data, work_id, priority,
                     mandatory, sticky, shown_for_ms, due_at, reminded, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                 RETURNING {WHAT_A_NOTIFICATION_IS}"
            )))
            .bind(NotificationId::new().to_db_string())
            .bind(user.to_db_string())
            .bind(new.kind)
            .bind(new.level)
            .bind(new.data)
            .bind(new.work_id.map(|work| work.to_db_string()))
            .bind(bool_to_int(new.priority))
            .bind(bool_to_int(new.mandatory))
            .bind(bool_to_int(new.sticky))
            .bind(new.shown_for_ms)
            .bind(new.due_at.map(timestamp_to_text))
            .bind(bool_to_int(new.reminded))
            .bind(timestamp_to_text(new.created_at))
            .fetch_one(&mut *transaction)
            .await?;
            written.push(notification_from_row(&row)?);
        }
        transaction.commit().await?;
        Ok(written)
    }

    /// An account's notifications, newest first, older than one already
    /// shown when one is given.
    pub async fn notifications_of(
        &self,
        user: UserId,
        before: Option<NotificationId>,
        most: i64,
    ) -> Result<Vec<Notification>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_NOTIFICATION_IS} FROM notifications
              WHERE user_id = ? AND (? IS NULL OR id < ?)
              ORDER BY id DESC
              LIMIT ?"
        )))
        .bind(user.to_db_string())
        .bind(before.map(|id| id.to_db_string()))
        .bind(before.map(|id| id.to_db_string()))
        .bind(most)
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(notification_from_row).collect()
    }

    /// How many of an account's notifications it has not read.
    pub async fn unread_notifications(&self, user: UserId) -> Result<i64> {
        Ok(sqlx::query_scalar(
            "SELECT COUNT(*) FROM notifications WHERE user_id = ? AND read_at IS NULL",
        )
        .bind(user.to_db_string())
        .fetch_one(self.reader())
        .await?)
    }

    /// Marks some of an account's notifications read at an instant, or
    /// unread when none is given, every one of them when none is named.
    /// Answers the ones that changed.
    pub async fn mark_notifications(
        &self,
        user: UserId,
        which: Option<&[NotificationId]>,
        read_at: Option<Timestamp>,
    ) -> Result<Vec<NotificationId>> {
        let named = match which {
            Some([]) => return Ok(Vec::new()),
            Some(ids) => format!("AND id IN ({})", placeholders(ids.len())),
            None => String::new(),
        };
        let unchanged = match read_at {
            Some(_) => "read_at IS NULL",
            None => "read_at IS NOT NULL",
        };
        // Assembled from question marks and two fixed conditions only.
        let mut query = sqlx::query_scalar::<_, String>(AssertSqlSafe(format!(
            "UPDATE notifications SET read_at = ?
              WHERE user_id = ? AND {unchanged} {named}
              RETURNING id"
        )))
        .bind(read_at.map(timestamp_to_text))
        .bind(user.to_db_string());
        for id in which.unwrap_or_default() {
            query = query.bind(id.to_db_string());
        }
        query
            .fetch_all(self.writer())
            .await?
            .iter()
            .map(|id| parse_id(id))
            .collect()
    }

    /// Removes one of an account's notifications, unless it may not be.
    pub async fn remove_notification(&self, user: UserId, id: NotificationId) -> Result<Removal> {
        let done = sqlx::query(
            "DELETE FROM notifications
              WHERE id = ? AND user_id = ? AND mandatory = 0 AND priority = 0",
        )
        .bind(id.to_db_string())
        .bind(user.to_db_string())
        .execute(self.writer())
        .await?;
        if done.rows_affected() > 0 {
            return Ok(Removal::Removed);
        }
        let there: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM notifications WHERE id = ? AND user_id = ?")
                .bind(id.to_db_string())
                .bind(user.to_db_string())
                .fetch_optional(self.reader())
                .await?;
        Ok(match there {
            Some(_) => Removal::Kept,
            None => Removal::Missing,
        })
    }

    /// Erases the read notifications that arrived before an instant, and
    /// answers how many.
    pub async fn forget_read_notifications_before(&self, at: Timestamp) -> Result<u64> {
        let done = sqlx::query(
            "DELETE FROM notifications WHERE read_at IS NOT NULL AND created_at < ?",
        )
        .bind(timestamp_to_text(at))
        .execute(self.writer())
        .await?;
        Ok(done.rows_affected())
    }

    /// Marks unread, once, every maintenance whose time comes before an
    /// instant and is still to come, and answers them.
    pub async fn recall_maintenance(&self, by: Timestamp, now: Timestamp) -> Result<Vec<Notification>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "UPDATE notifications SET reminded = 1, read_at = NULL
              WHERE due_at IS NOT NULL AND reminded = 0 AND due_at <= ? AND due_at > ?
              RETURNING {WHAT_A_NOTIFICATION_IS}"
        )))
        .bind(timestamp_to_text(by))
        .bind(timestamp_to_text(now))
        .fetch_all(self.writer())
        .await?;
        rows.iter().map(notification_from_row).collect()
    }

    /// Erases every maintenance whose time has passed, and answers whose
    /// they were.
    pub async fn forget_past_maintenance(
        &self,
        now: Timestamp,
    ) -> Result<Vec<(UserId, NotificationId)>> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "DELETE FROM notifications WHERE due_at IS NOT NULL AND due_at <= ?
             RETURNING user_id, id",
        )
        .bind(timestamp_to_text(now))
        .fetch_all(self.writer())
        .await?;
        rows.iter()
            .map(|(user, id)| Ok((parse_id(user)?, parse_id(id)?)))
            .collect()
    }

    /// What an account gets of each kind when it has not chosen.
    pub async fn notification_defaults(&self) -> Result<HashMap<String, Channels>> {
        let rows: Vec<(String, i64, i64)> =
            sqlx::query_as("SELECT kind, bell, screen FROM notification_defaults")
                .fetch_all(self.reader())
                .await?;
        Ok(channels_by_kind(rows))
    }

    pub async fn set_notification_default(&self, kind: &str, channels: Channels) -> Result<()> {
        sqlx::query(
            "INSERT INTO notification_defaults (kind, bell, screen) VALUES (?, ?, ?)
             ON CONFLICT (kind) DO UPDATE SET bell = excluded.bell, screen = excluded.screen",
        )
        .bind(kind)
        .bind(bool_to_int(channels.bell))
        .bind(bool_to_int(channels.screen))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// What an account chose, kind by kind.
    pub async fn notification_choices(&self, user: UserId) -> Result<HashMap<String, Channels>> {
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT kind, bell, screen FROM notification_choices WHERE user_id = ?",
        )
        .bind(user.to_db_string())
        .fetch_all(self.reader())
        .await?;
        Ok(channels_by_kind(rows))
    }

    /// What every account that chose about one kind chose.
    pub async fn notification_choices_for(&self, kind: &str) -> Result<HashMap<UserId, Channels>> {
        let rows: Vec<(String, i64, i64)> = sqlx::query_as(
            "SELECT user_id, bell, screen FROM notification_choices WHERE kind = ?",
        )
        .bind(kind)
        .fetch_all(self.reader())
        .await?;
        rows.into_iter()
            .map(|(user, bell, screen)| {
                Ok((
                    parse_id(&user)?,
                    Channels {
                        bell: int_to_bool(bell),
                        screen: int_to_bool(screen),
                    },
                ))
            })
            .collect()
    }

    pub async fn set_notification_choice(
        &self,
        user: UserId,
        kind: &str,
        channels: Channels,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO notification_choices (user_id, kind, bell, screen) VALUES (?, ?, ?, ?)
             ON CONFLICT (user_id, kind) DO UPDATE SET bell = excluded.bell, screen = excluded.screen",
        )
        .bind(user.to_db_string())
        .bind(kind)
        .bind(bool_to_int(channels.bell))
        .bind(bool_to_int(channels.screen))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    pub async fn notification_settings(&self, user: UserId) -> Result<NotificationSettings> {
        let row: Option<(Option<i32>, Option<i32>, i64, i64)> = sqlx::query_as(
            "SELECT quiet_from, quiet_until, priority_while_playing, priority_while_quiet
               FROM notification_settings WHERE user_id = ?",
        )
        .bind(user.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        Ok(row.map_or_else(
            NotificationSettings::default,
            |(quiet_from, quiet_until, playing, quiet)| NotificationSettings {
                quiet_from,
                quiet_until,
                priority_while_playing: int_to_bool(playing),
                priority_while_quiet: int_to_bool(quiet),
            },
        ))
    }

    pub async fn set_notification_settings(
        &self,
        user: UserId,
        settings: &NotificationSettings,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO notification_settings
                 (user_id, quiet_from, quiet_until, priority_while_playing, priority_while_quiet)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (user_id) DO UPDATE SET
                 quiet_from = excluded.quiet_from,
                 quiet_until = excluded.quiet_until,
                 priority_while_playing = excluded.priority_while_playing,
                 priority_while_quiet = excluded.priority_while_quiet",
        )
        .bind(user.to_db_string())
        .bind(settings.quiet_from)
        .bind(settings.quiet_until)
        .bind(bool_to_int(settings.priority_while_playing))
        .bind(bool_to_int(settings.priority_while_quiet))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Every library, with whether it announces what arrives and up to when.
    pub async fn announcing_libraries(&self) -> Result<Vec<AnnouncingLibrary>> {
        let rows = sqlx::query(
            "SELECT l.id, l.name, l.kind, l.created_at, n.announces, n.announced_until
               FROM libraries l
               LEFT JOIN notification_libraries n ON n.library_id = l.id
              ORDER BY l.name",
        )
        .fetch_all(self.reader())
        .await?;
        rows.iter()
            .map(|row| {
                Ok(AnnouncingLibrary {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    name: row.try_get("name")?,
                    kind: row.try_get("kind")?,
                    created_at: parse_timestamp(&row.try_get::<String, _>("created_at")?)?,
                    announces: row.try_get::<Option<i64>, _>("announces")?.map(int_to_bool),
                    announced_until: parse_optional_timestamp(
                        row.try_get::<Option<String>, _>("announced_until")?.as_deref(),
                    )?,
                })
            })
            .collect()
    }

    /// Whether a library announces what arrives. A library looked at for
    /// the first time starts from the instant given.
    pub async fn set_library_announces(
        &self,
        library: LibraryId,
        announces: bool,
        from: Timestamp,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO notification_libraries (library_id, announces, announced_until)
             VALUES (?, ?, ?)
             ON CONFLICT (library_id) DO UPDATE SET announces = excluded.announces",
        )
        .bind(library.to_db_string())
        .bind(bool_to_int(announces))
        .bind(timestamp_to_text(from))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Writes down that what arrived in a library is announced up to here.
    pub async fn set_announced_until(&self, library: LibraryId, until: Timestamp) -> Result<()> {
        sqlx::query(
            "INSERT INTO notification_libraries (library_id, announced_until) VALUES (?, ?)
             ON CONFLICT (library_id) DO UPDATE SET announced_until = excluded.announced_until",
        )
        .bind(library.to_db_string())
        .bind(timestamp_to_text(until))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// The libraries an account chose about, and what it chose.
    pub async fn library_announcement_choices(&self, user: UserId) -> Result<HashMap<LibraryId, bool>> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            "SELECT library_id, announced FROM notification_library_choices WHERE user_id = ?",
        )
        .bind(user.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.into_iter()
            .map(|(library, announced)| Ok((parse_id(&library)?, int_to_bool(announced))))
            .collect()
    }

    /// The accounts that chose not to hear about a library.
    pub async fn deaf_to_library(&self, library: LibraryId) -> Result<Vec<UserId>> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT user_id FROM notification_library_choices
              WHERE library_id = ? AND announced = 0",
        )
        .bind(library.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(|user| parse_id(user)).collect()
    }

    pub async fn set_library_announcement_choice(
        &self,
        user: UserId,
        library: LibraryId,
        announced: bool,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO notification_library_choices (user_id, library_id, announced)
             VALUES (?, ?, ?)
             ON CONFLICT (user_id, library_id) DO UPDATE SET announced = excluded.announced",
        )
        .bind(user.to_db_string())
        .bind(library.to_db_string())
        .bind(bool_to_int(announced))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// The films and episodes added to a library after an instant, oldest
    /// first, each with the work that names it.
    pub async fn arrivals_since(&self, library: LibraryId, since: Timestamp) -> Result<Vec<Arrival>> {
        let rows = sqlx::query(
            "SELECT w.id, w.kind, w.title, w.added_at,
                    s.id AS series_id, s.title AS series_title,
                    COALESCE(s.identification, w.identification) AS identification,
                    CASE WHEN s.id IS NULL THEN w.age_rating ELSE s.age_rating END AS age_rating,
                    EXISTS (SELECT 1 FROM images i
                             WHERE i.owner_kind = 'work' AND i.image_kind = 'poster'
                               AND i.owner_id = COALESCE(s.id, w.id)) AS has_poster
               FROM works w
               LEFT JOIN works p ON p.id = w.parent_id
               LEFT JOIN works s ON s.kind = 'series'
                                AND s.id = CASE p.kind WHEN 'season' THEN p.parent_id ELSE p.id END
              WHERE w.library_id = ? AND w.added_at > ? AND w.kind IN ('movie', 'episode')
              ORDER BY w.added_at",
        )
        .bind(library.to_db_string())
        .bind(timestamp_to_text(since))
        .fetch_all(self.reader())
        .await?;
        rows.iter()
            .map(|row| {
                let series = match row.try_get::<Option<String>, _>("series_id")? {
                    Some(id) => Some((parse_id(&id)?, row.try_get("series_title")?)),
                    None => None,
                };
                Ok(Arrival {
                    work_id: parse_id(&row.try_get::<String, _>("id")?)?,
                    kind: row.try_get("kind")?,
                    title: row.try_get("title")?,
                    added_at: parse_timestamp(&row.try_get::<String, _>("added_at")?)?,
                    series,
                    identification: row.try_get("identification")?,
                    has_poster: int_to_bool(row.try_get("has_poster")?),
                    age_rating: row.try_get("age_rating")?,
                })
            })
            .collect()
    }

    /// When anything was last added to a library.
    pub async fn latest_addition(&self, library: LibraryId) -> Result<Option<Timestamp>> {
        let latest: Option<String> =
            sqlx::query_scalar("SELECT MAX(added_at) FROM works WHERE library_id = ?")
                .bind(library.to_db_string())
                .fetch_one(self.reader())
                .await?;
        parse_optional_timestamp(latest.as_deref())
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

fn channels_by_kind(rows: Vec<(String, i64, i64)>) -> HashMap<String, Channels> {
    rows.into_iter()
        .map(|(kind, bell, screen)| {
            (
                kind,
                Channels {
                    bell: int_to_bool(bell),
                    screen: int_to_bool(screen),
                },
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use melyxar_core::library::LibraryKind;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::WorkKind;
    use time::macros::datetime;

    use super::*;

    const NOON: Timestamp = datetime!(2026-10-03 12:00 UTC);

    async fn two_accounts() -> (Database, UserId, UserId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let one = database
            .create_user("one", Some("a stored form"), &Permissions::administrator())
            .await
            .expect("account created");
        let other = database
            .create_user("other", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account created");
        (database, one.id, other.id)
    }

    fn a_message(at: Timestamp) -> NewNotification<'static> {
        NewNotification {
            kind: "message",
            level: "ok",
            data: r#"{"title":"Hello"}"#,
            work_id: None,
            priority: false,
            mandatory: false,
            sticky: false,
            shown_for_ms: None,
            due_at: None,
            reminded: false,
            created_at: at,
        }
    }

    #[tokio::test]
    async fn a_notification_is_written_once_per_account_and_read_back_as_written() {
        let (database, one, other) = two_accounts().await;
        let new = NewNotification {
            priority: true,
            sticky: true,
            shown_for_ms: Some(4_000),
            due_at: Some(NOON + time::Duration::hours(3)),
            ..a_message(NOON)
        };
        let written = database
            .add_notifications(&[one, other], &new)
            .await
            .expect("written");
        assert_eq!(written.len(), 2);
        assert_ne!(written[0].id, written[1].id);

        let mine = database.notifications_of(one, None, 10).await.expect("read");
        assert_eq!(mine, vec![written[0].clone()]);
        let kept = &mine[0];
        assert_eq!(kept.kind, "message");
        assert_eq!(kept.data, r#"{"title":"Hello"}"#);
        assert!(kept.priority && kept.sticky && !kept.mandatory);
        assert_eq!(kept.shown_for_ms, Some(4_000));
        assert_eq!(kept.due_at, Some(NOON + time::Duration::hours(3)));
        assert_eq!(kept.created_at, NOON);
        assert_eq!(kept.read_at, None);
        assert_eq!(database.unread_notifications(other).await.expect("counted"), 1);
    }

    #[tokio::test]
    async fn the_history_is_read_newest_first_a_page_at_a_time() {
        let (database, one, _) = two_accounts().await;
        for minute in 0..5 {
            database
                .add_notifications(&[one], &a_message(NOON + time::Duration::minutes(minute)))
                .await
                .expect("written");
        }
        let first = database.notifications_of(one, None, 2).await.expect("read");
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].created_at, NOON + time::Duration::minutes(4));
        let next = database
            .notifications_of(one, Some(first[1].id), 10)
            .await
            .expect("read");
        assert_eq!(next.len(), 3);
        assert_eq!(next[0].created_at, NOON + time::Duration::minutes(2));
    }

    #[tokio::test]
    async fn read_and_unread_change_only_what_they_name_and_say_what_changed() {
        let (database, one, other) = two_accounts().await;
        let first = database.add_notifications(&[one], &a_message(NOON)).await.expect("written");
        let second = database.add_notifications(&[one], &a_message(NOON)).await.expect("written");
        let theirs = database.add_notifications(&[other], &a_message(NOON)).await.expect("written");

        let changed = database
            .mark_notifications(one, Some(&[first[0].id, theirs[0].id]), Some(NOON))
            .await
            .expect("marked");
        assert_eq!(changed, vec![first[0].id], "another account's is never touched");
        assert_eq!(database.unread_notifications(one).await.expect("counted"), 1);

        let all = database.mark_notifications(one, None, Some(NOON)).await.expect("marked");
        assert_eq!(all, vec![second[0].id], "what was already read is not said again");
        assert_eq!(database.unread_notifications(one).await.expect("counted"), 0);

        let back = database
            .mark_notifications(one, Some(&[first[0].id]), None)
            .await
            .expect("marked");
        assert_eq!(back, vec![first[0].id]);
        assert_eq!(database.unread_notifications(one).await.expect("counted"), 1);
        assert!(database.mark_notifications(one, Some(&[]), None).await.expect("marked").is_empty());
    }

    #[tokio::test]
    async fn what_is_mandatory_or_urgent_cannot_be_removed() {
        let (database, one, other) = two_accounts().await;
        let plain = database.add_notifications(&[one], &a_message(NOON)).await.expect("written");
        let urgent = database
            .add_notifications(&[one], &NewNotification { priority: true, ..a_message(NOON) })
            .await
            .expect("written");
        let mandatory = database
            .add_notifications(&[one], &NewNotification { mandatory: true, ..a_message(NOON) })
            .await
            .expect("written");

        let remove = |id| database.remove_notification(one, id);
        assert_eq!(remove(urgent[0].id).await.expect("asked"), Removal::Kept);
        assert_eq!(remove(mandatory[0].id).await.expect("asked"), Removal::Kept);
        assert_eq!(
            database.remove_notification(other, plain[0].id).await.expect("asked"),
            Removal::Missing
        );
        assert_eq!(remove(plain[0].id).await.expect("asked"), Removal::Removed);
        assert_eq!(remove(plain[0].id).await.expect("asked"), Removal::Missing);
        assert_eq!(database.notifications_of(one, None, 10).await.expect("read").len(), 2);
    }

    #[tokio::test]
    async fn only_what_was_read_is_forgotten_with_age() {
        let (database, one, _) = two_accounts().await;
        let old = database.add_notifications(&[one], &a_message(NOON)).await.expect("written");
        database.add_notifications(&[one], &a_message(NOON)).await.expect("written");
        let recent = database
            .add_notifications(&[one], &a_message(NOON + time::Duration::days(2)))
            .await
            .expect("written");
        database
            .mark_notifications(one, Some(&[old[0].id, recent[0].id]), Some(NOON))
            .await
            .expect("marked");

        let forgotten = database
            .forget_read_notifications_before(NOON + time::Duration::days(1))
            .await
            .expect("forgotten");
        assert_eq!(forgotten, 1, "the unread one and the recent one stay");
        assert_eq!(database.notifications_of(one, None, 10).await.expect("read").len(), 2);
    }

    #[tokio::test]
    async fn a_maintenance_is_recalled_once_before_its_time_and_gone_after() {
        let (database, one, other) = two_accounts().await;
        let due = NOON + time::Duration::hours(2);
        let maintenance = NewNotification {
            kind: "maintenance",
            mandatory: true,
            due_at: Some(due),
            ..a_message(NOON)
        };
        let written = database
            .add_notifications(&[one, other], &maintenance)
            .await
            .expect("written");
        database.mark_notifications(one, None, Some(NOON)).await.expect("marked");
        database.add_notifications(&[one], &a_message(NOON)).await.expect("written");

        assert!(database
            .recall_maintenance(NOON + time::Duration::minutes(30), NOON)
            .await
            .expect("asked")
            .is_empty());
        let recalled = database
            .recall_maintenance(due + time::Duration::minutes(30), due - time::Duration::minutes(30))
            .await
            .expect("recalled");
        assert_eq!(recalled.len(), 2);
        assert!(recalled.iter().all(|one| one.read_at.is_none()), "unread again");
        assert!(database
            .recall_maintenance(due + time::Duration::minutes(40), due - time::Duration::minutes(20))
            .await
            .expect("asked")
            .is_empty(), "once");

        let mut gone = database.forget_past_maintenance(due).await.expect("forgotten");
        gone.sort();
        let mut expected = vec![(one, written[0].id), (other, written[1].id)];
        expected.sort();
        assert_eq!(gone, expected);
        assert_eq!(database.notifications_of(one, None, 10).await.expect("read").len(), 1);
    }

    #[tokio::test]
    async fn defaults_choices_and_settings_are_kept_and_replaced() {
        let (database, one, other) = two_accounts().await;
        let defaults = database.notification_defaults().await.expect("read");
        for kind in ["message", "new_content", "deletion"] {
            assert_eq!(defaults[kind], Channels { bell: true, screen: true }, "{kind}");
        }
        let quiet = Channels { bell: true, screen: false };
        database.set_notification_default("new_content", quiet).await.expect("set");
        assert_eq!(database.notification_defaults().await.expect("read")["new_content"], quiet);

        database.set_notification_choice(one, "message", quiet).await.expect("set");
        database
            .set_notification_choice(one, "message", Channels { bell: false, screen: false })
            .await
            .expect("set again");
        assert_eq!(
            database.notification_choices(one).await.expect("read")["message"],
            Channels { bell: false, screen: false }
        );
        assert!(database.notification_choices(other).await.expect("read").is_empty());
        let for_kind = database.notification_choices_for("message").await.expect("read");
        assert_eq!(for_kind.len(), 1);
        assert!(for_kind.contains_key(&one));

        assert_eq!(
            database.notification_settings(one).await.expect("read"),
            NotificationSettings::default()
        );
        let night = NotificationSettings {
            quiet_from: Some(22 * 60),
            quiet_until: Some(7 * 60),
            priority_while_playing: false,
            priority_while_quiet: true,
        };
        database.set_notification_settings(one, &night).await.expect("set");
        assert_eq!(database.notification_settings(one).await.expect("read"), night);
    }

    async fn a_library(database: &Database, name: &str, kind: LibraryKind) -> LibraryId {
        database
            .create_library(name, kind, "fr", &[(name.to_lowercase(), PathBuf::from("/mnt/one").join(name))])
            .await
            .expect("library created")
            .id
    }

    #[tokio::test]
    async fn a_library_announces_until_told_otherwise_and_keeps_how_far_it_got() {
        let (database, one, other) = two_accounts().await;
        let films = a_library(&database, "Films", LibraryKind::Movies).await;

        let seen = database.announcing_libraries().await.expect("read");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].kind, "movies");
        assert_eq!(seen[0].announces, None, "created after the migration: not looked at yet");
        assert_eq!(seen[0].announced_until, None);

        database.set_announced_until(films, NOON).await.expect("set");
        database
            .set_library_announces(films, false, NOON + time::Duration::days(1))
            .await
            .expect("set");
        let seen = database.announcing_libraries().await.expect("read");
        assert_eq!(seen[0].announces, Some(false));
        assert_eq!(seen[0].announced_until, Some(NOON), "the mark already there is kept");

        database.set_library_announcement_choice(one, films, false).await.expect("set");
        database.set_library_announcement_choice(other, films, true).await.expect("set");
        assert_eq!(database.deaf_to_library(films).await.expect("read"), vec![one]);
        assert!(database.library_announcement_choices(other).await.expect("read")[&films]);
    }

    #[tokio::test]
    async fn arrivals_are_read_with_the_work_that_names_them() {
        let database = Database::open_in_memory().await.expect("database opens");
        let films = a_library(&database, "Films", LibraryKind::Movies).await;
        let shows = a_library(&database, "Series", LibraryKind::Series).await;
        assert_eq!(database.latest_addition(films).await.expect("read"), None);

        let film = database
            .create_work(films, WorkKind::Movie, "Amber Field", "amber field", Some(2020))
            .await
            .expect("film");
        database.mark_work_unidentified(film.id).await.expect("marked");
        let series = database
            .create_work(shows, WorkKind::Series, "Salt Road", "salt road", None)
            .await
            .expect("series");
        let season = database
            .create_child_work(shows, series.id, 1, WorkKind::Season, "Season 1", "season 1")
            .await
            .expect("season");
        let episode = database
            .create_child_work(shows, season.id, 1, WorkKind::Episode, "Pilot", "pilot")
            .await
            .expect("episode");

        let long_ago = datetime!(2000-01-01 0:00 UTC);
        let in_films = database.arrivals_since(films, long_ago).await.expect("read");
        assert_eq!(in_films.len(), 1);
        assert_eq!(in_films[0].work_id, film.id);
        assert_eq!(in_films[0].kind, "movie");
        assert_eq!(in_films[0].series, None);
        assert_eq!(in_films[0].identification, "unidentified");
        assert!(!in_films[0].has_poster);
        assert_eq!(in_films[0].age_rating, None);

        let in_shows = database.arrivals_since(shows, long_ago).await.expect("read");
        assert_eq!(in_shows.len(), 1, "a series and its season are not arrivals");
        assert_eq!(in_shows[0].work_id, episode.id);
        assert_eq!(in_shows[0].series, Some((series.id, "Salt Road".to_string())));
        assert_eq!(in_shows[0].identification, "pending");

        assert!(database
            .arrivals_since(films, in_films[0].added_at)
            .await
            .expect("read")
            .is_empty());
        assert_eq!(
            database.latest_addition(shows).await.expect("read"),
            Some(in_shows[0].added_at.max(season.added_at).max(series.added_at))
        );
    }

    #[tokio::test]
    async fn what_an_administrator_saw_is_kept_for_them_alone_and_moves_on() {
        let (database, one, other) = two_accounts().await;

        database.mark_attention_seen(one, "unidentified", 12).await.expect("marked");
        database.mark_attention_seen(one, "unidentified", 15).await.expect("marked again");
        database.mark_attention_seen(one, "failed_tasks", 99).await.expect("marked");

        let mut seen = database.attention_seen(one).await.expect("read");
        seen.sort();
        assert_eq!(
            seen,
            vec![("failed_tasks".to_string(), 99), ("unidentified".to_string(), 15)]
        );
        assert!(database.attention_seen(other).await.expect("read").is_empty());

        database.forget_attention_seen(one, "failed_tasks").await.expect("forgotten");
        assert_eq!(
            database.attention_seen(one).await.expect("read"),
            vec![("unidentified".to_string(), 15)]
        );
    }
}
