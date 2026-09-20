//! Being signed in: how a token that arrives becomes an account.
//!
//! A signed in browser is a device, so it is a row of the device table rather
//! than a table of its own. That was the decision before any of this was
//! written: one token per device, of long life, revocable on its own, with the
//! list of them and when each was last used. A browser on a machine and a
//! television in a living room are the same thing seen twice, and giving them
//! two tables would mean writing the revoking, the sweeping and the listing
//! twice as well.
//!
//! Nothing here says how long a token is or what it is made of: what is
//! written down is a fingerprint the caller worked out, and this crate never
//! sees the token itself. A copy of this database hands over nobody's session.
//!
//! **A session is not given an end, it is given a last use.** It lives for as
//! long as it goes on being used, which is what anybody means by staying
//! signed in, and it is the same fact the list of devices shows. The one thing
//! that has to be watched is how often that fact is written: there is a single
//! writer for the whole database, and a film being watched asks for a segment
//! every few seconds. So it is written again only once it has gone stale,
//! which costs at most one write an hour per device.

use melyxar_core::id::{DeviceId, UserId};
use melyxar_core::time::Timestamp;
use melyxar_core::user::User;
use sqlx::{AssertSqlSafe, Row};

use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, Result};

/// How long a session survives without being used.
///
/// Thirty days: long enough that nobody is asked to sign in again in ordinary
/// use, and that a television sitting unused over a holiday still works on the
/// way back; short enough that a token taken off a machine somebody stopped
/// using stops working on its own.
pub const A_SESSION_LASTS: time::Duration = time::Duration::days(30);

/// How stale the record of a session's last use may get before it is written
/// down again.
///
/// A session is kept alive by writing down that it was used, and there is one
/// writer for the whole database. Written on every request, one film being
/// watched would queue a write per segment of it behind every scan and every
/// resume position. An hour costs at most one write an hour per device, and
/// moves the end of a session by at most an hour out of thirty days, which
/// nobody can feel.
const WRITTEN_DOWN_AGAIN_AFTER: time::Duration = time::Duration::hours(1);

/// Whoever presented a token, and the device they presented it from.
#[derive(Debug, Clone, PartialEq)]
pub struct SignedIn {
    pub user: User,
    /// Which device this is, so one can be signed out without touching the
    /// others.
    pub device: DeviceId,
}

impl Database {
    /// Writes down that somebody signed in, and from what.
    ///
    /// The name is whatever the layer above made of the browser that asked: it
    /// is shown in the list of devices and nothing is ever decided from it.
    pub async fn open_session(
        &self,
        user_id: UserId,
        device_name: &str,
        token_fingerprint: &str,
        at: Timestamp,
    ) -> Result<DeviceId> {
        let id = DeviceId::new();
        sqlx::query(
            "INSERT INTO devices (id, user_id, name, token_hash, created_at, last_seen_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(user_id.to_db_string())
        .bind(device_name)
        .bind(token_fingerprint)
        .bind(timestamp_to_text(at))
        .bind(timestamp_to_text(at))
        .execute(self.writer())
        .await?;
        Ok(id)
    }

    /// The account behind a token, and nothing when there is none.
    ///
    /// Answers nothing rather than failing for every way a token can fail to
    /// name anybody: never issued, signed out since, or simply not used for
    /// longer than a session lasts. From outside they are one answer, which is
    /// the answer somebody gets when they are not signed in.
    ///
    /// Writes down that the session was used, but only once it has gone stale:
    /// see the note at the top of this file.
    pub async fn session_holder(
        &self,
        token_fingerprint: &str,
        at: Timestamp,
    ) -> Result<Option<SignedIn>> {
        // The whole of an account and the device it was reached through, in
        // one read: this runs on every request, pictures and segments of film
        // included, so it is the one place where a second round trip would be
        // paid for over and over.
        let Some(row) = sqlx::query(AssertSqlSafe(crate::users::reading_accounts(
            ", d.id AS device_id, d.last_seen_at",
            "JOIN devices d ON d.user_id = u.id
             WHERE d.token_hash = ? AND d.last_seen_at >= ?",
        )))
        .bind(token_fingerprint)
        .bind(timestamp_to_text(at - A_SESSION_LASTS))
        .fetch_optional(self.reader())
        .await?
        else {
            return Ok(None);
        };

        let device: DeviceId = parse_id(&row.try_get::<String, _>("device_id")?)?;
        let last_seen_at: String = row.try_get("last_seen_at")?;
        if crate::convert::parse_timestamp(&last_seen_at)? <= at - WRITTEN_DOWN_AGAIN_AFTER {
            sqlx::query("UPDATE devices SET last_seen_at = ? WHERE id = ?")
                .bind(timestamp_to_text(at))
                .bind(device.to_db_string())
                .execute(self.writer())
                .await?;
        }

        let user_id: UserId = parse_id(&row.try_get::<String, _>("id")?)?;
        let allowed = self.libraries_allowed_to(user_id).await?;
        Ok(Some(SignedIn {
            user: crate::users::build_user(&row, &allowed)?,
            device,
        }))
    }

    /// Signs one device out, and says whether there was one to sign out.
    pub async fn close_session(&self, token_fingerprint: &str) -> Result<bool> {
        let done = sqlx::query("DELETE FROM devices WHERE token_hash = ?")
            .bind(token_fingerprint)
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Signs every device of one account out, and says how many that was.
    ///
    /// What a changed password means: somebody changes it because they think
    /// somebody else knows it, and a password changed while the other one
    /// stays signed in has changed nothing at all.
    pub async fn close_every_session_of(&self, user_id: UserId) -> Result<u64> {
        let done = sqlx::query("DELETE FROM devices WHERE user_id = ?")
            .bind(user_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected())
    }

    /// Throws away the sessions nobody has used for longer than one lasts.
    ///
    /// They already answer nothing, so this changes no answer: it is there so
    /// that a server used for years does not carry a row per browser anybody
    /// ever opened it in, and so that the list of devices shows the ones that
    /// are real.
    pub async fn forget_stale_sessions(&self, at: Timestamp) -> Result<u64> {
        let done = sqlx::query("DELETE FROM devices WHERE last_seen_at < ?")
            .bind(timestamp_to_text(at - A_SESSION_LASTS))
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected())
    }

    /// Sets, changes or takes away the stored form of an account's password.
    ///
    /// Nothing about hashing lives here: what is written is what the layer
    /// above made of it, and nothing in this crate can tell one from a
    /// password typed in.
    pub async fn set_password(&self, user_id: UserId, stored: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(stored)
            .bind(user_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::LibraryId;
    use melyxar_core::user::Permissions;
    use time::macros::datetime;

    const A_MOMENT: Timestamp = datetime!(2026-01-01 12:00 UTC);

    async fn a_server_with_one_account() -> (Database, UserId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let user = database
            .create_user("victor", Some("a stored form"), &Permissions::administrator())
            .await
            .expect("account created");
        (database, user.id)
    }

    #[tokio::test]
    async fn a_token_brings_back_the_account_it_was_opened_for() {
        let (database, user_id) = a_server_with_one_account().await;
        let device = database
            .open_session(user_id, "a browser", "a fingerprint", A_MOMENT)
            .await
            .expect("session opened");

        let signed_in = database
            .session_holder("a fingerprint", A_MOMENT)
            .await
            .expect("read")
            .expect("somebody is behind that token");

        assert_eq!(signed_in.device, device);
        assert_eq!(signed_in.user.id, user_id);
        assert_eq!(signed_in.user.name, "victor");
        // The rights and the preferences come with it, because every request
        // that asks who is there asks in order to decide something.
        assert!(signed_in.user.permissions.is_administrator);
        assert_eq!(signed_in.user.preferences.accent_color, "#c81e1e");
    }

    #[tokio::test]
    async fn the_libraries_an_account_was_granted_come_with_it() {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = LibraryId::new();
        sqlx::query(
            "INSERT INTO libraries (id, name, kind, created_at, updated_at)
             VALUES (?, 'Films', 'movies', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(library.to_db_string())
        .execute(database.writer())
        .await
        .expect("library inserted");

        let user = database
            .create_user(
                "limited",
                Some("a stored form"),
                &Permissions {
                    sees_every_library: false,
                    allowed_libraries: vec![library],
                    ..Permissions::viewer()
                },
            )
            .await
            .expect("account created");
        database
            .open_session(user.id, "a browser", "a fingerprint", A_MOMENT)
            .await
            .expect("session opened");

        let signed_in = database
            .session_holder("a fingerprint", A_MOMENT)
            .await
            .expect("read")
            .expect("somebody is behind that token");
        assert_eq!(
            signed_in.user.permissions.allowed_libraries,
            vec![library],
            "a request has to know what this account may see before it answers"
        );
    }

    #[tokio::test]
    async fn a_token_nobody_opened_names_nobody() {
        let (database, user_id) = a_server_with_one_account().await;
        database
            .open_session(user_id, "a browser", "a fingerprint", A_MOMENT)
            .await
            .expect("session opened");

        assert!(database
            .session_holder("another fingerprint", A_MOMENT)
            .await
            .expect("read")
            .is_none());
    }

    #[tokio::test]
    async fn a_session_nobody_used_for_longer_than_it_lasts_names_nobody() {
        let (database, user_id) = a_server_with_one_account().await;
        // Two of them, asked about once each: asking about one keeps it alive,
        // which is the whole point of it, and would answer the other question.
        database
            .open_session(user_id, "a browser", "one fingerprint", A_MOMENT)
            .await
            .expect("session opened");
        database
            .open_session(user_id, "a television", "another fingerprint", A_MOMENT)
            .await
            .expect("session opened");

        let an_hour_short = A_MOMENT + A_SESSION_LASTS - time::Duration::hours(1);
        assert!(
            database
                .session_holder("one fingerprint", an_hour_short)
                .await
                .expect("read")
                .is_some(),
            "still inside its life"
        );

        let well_past = A_MOMENT + A_SESSION_LASTS + time::Duration::days(1);
        assert!(
            database
                .session_holder("another fingerprint", well_past)
                .await
                .expect("read")
                .is_none(),
            "a token nobody has used in a month is a token nobody holds"
        );
    }

    #[tokio::test]
    async fn using_a_session_keeps_it_alive_without_writing_on_every_request() {
        let (database, user_id) = a_server_with_one_account().await;
        database
            .open_session(user_id, "a browser", "a fingerprint", A_MOMENT)
            .await
            .expect("session opened");

        // A second request a minute later must not write: one film being
        // watched asks for a segment every few seconds, and there is a single
        // writer for the whole database.
        let a_minute_later = A_MOMENT + time::Duration::minutes(1);
        database
            .session_holder("a fingerprint", a_minute_later)
            .await
            .expect("read")
            .expect("still there");
        assert_eq!(
            last_used(&database).await,
            A_MOMENT,
            "nothing was written for a request a minute after the last one"
        );

        // Past the point where it has gone stale, it is written down again,
        // which is what carries the end of the session forward.
        let much_later = A_MOMENT + time::Duration::days(20);
        database
            .session_holder("a fingerprint", much_later)
            .await
            .expect("read")
            .expect("still there");
        assert_eq!(last_used(&database).await, much_later);

        let past_the_first_month = A_MOMENT + A_SESSION_LASTS + time::Duration::days(1);
        assert!(
            database
                .session_holder("a fingerprint", past_the_first_month)
                .await
                .expect("read")
                .is_some(),
            "used twenty days in, it lasts thirty days from there and not from the start"
        );
    }

    #[tokio::test]
    async fn signing_one_device_out_leaves_the_others_signed_in() {
        let (database, user_id) = a_server_with_one_account().await;
        database
            .open_session(user_id, "a browser", "one fingerprint", A_MOMENT)
            .await
            .expect("session opened");
        database
            .open_session(user_id, "a television", "another fingerprint", A_MOMENT)
            .await
            .expect("session opened");

        assert!(database
            .close_session("one fingerprint")
            .await
            .expect("closed"));
        assert!(database
            .session_holder("one fingerprint", A_MOMENT)
            .await
            .expect("read")
            .is_none());
        assert!(
            database
                .session_holder("another fingerprint", A_MOMENT)
                .await
                .expect("read")
                .is_some(),
            "signing out of one machine must not sign the television out"
        );

        assert!(
            !database
                .close_session("one fingerprint")
                .await
                .expect("closed"),
            "closing what is already closed says so rather than failing"
        );
    }

    #[tokio::test]
    async fn changing_a_password_can_sign_every_device_of_that_account_out() {
        let (database, user_id) = a_server_with_one_account().await;
        let somebody_else = database
            .create_user("someone", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account created");
        database
            .open_session(user_id, "a browser", "one fingerprint", A_MOMENT)
            .await
            .expect("session opened");
        database
            .open_session(user_id, "a television", "another fingerprint", A_MOMENT)
            .await
            .expect("session opened");
        database
            .open_session(somebody_else.id, "a browser", "a third fingerprint", A_MOMENT)
            .await
            .expect("session opened");

        assert_eq!(
            database
                .close_every_session_of(user_id)
                .await
                .expect("closed"),
            2
        );
        assert!(database
            .session_holder("one fingerprint", A_MOMENT)
            .await
            .expect("read")
            .is_none());
        assert!(database
            .session_holder("another fingerprint", A_MOMENT)
            .await
            .expect("read")
            .is_none());
        assert!(
            database
                .session_holder("a third fingerprint", A_MOMENT)
                .await
                .expect("read")
                .is_some(),
            "one person changing their password signs nobody else out"
        );
    }

    #[tokio::test]
    async fn the_sessions_nobody_uses_any_more_are_swept_and_the_live_ones_are_not() {
        let (database, user_id) = a_server_with_one_account().await;
        database
            .open_session(user_id, "an old browser", "an old fingerprint", A_MOMENT)
            .await
            .expect("session opened");

        let much_later = A_MOMENT + A_SESSION_LASTS + time::Duration::days(1);
        database
            .open_session(user_id, "a browser", "a fresh fingerprint", much_later)
            .await
            .expect("session opened");

        assert_eq!(
            database
                .forget_stale_sessions(much_later)
                .await
                .expect("swept"),
            1
        );
        assert!(database
            .session_holder("a fresh fingerprint", much_later)
            .await
            .expect("read")
            .is_some());
    }

    #[tokio::test]
    async fn a_password_can_be_set_changed_and_taken_away() {
        let (database, user_id) = a_server_with_one_account().await;

        database
            .set_password(user_id, Some("another stored form"))
            .await
            .expect("password set");
        let (_, stored) = database
            .user_by_name("victor")
            .await
            .expect("lookup works")
            .expect("account found");
        assert_eq!(stored.as_deref(), Some("another stored form"));

        // Taken away rather than set to nothing readable: an account without
        // one is the state the setup wizard answers, not an account whose
        // password is the empty word.
        database.set_password(user_id, None).await.expect("cleared");
        let (_, stored) = database
            .user_by_name("victor")
            .await
            .expect("lookup works")
            .expect("account found");
        assert_eq!(stored, None);
    }

    /// When the one session in the database was last used.
    async fn last_used(database: &Database) -> Timestamp {
        let row: (String,) = sqlx::query_as("SELECT last_seen_at FROM devices")
            .fetch_one(database.reader())
            .await
            .expect("one device");
        crate::convert::parse_timestamp(&row.0).expect("an instant")
    }
}
