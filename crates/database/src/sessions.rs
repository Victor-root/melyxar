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
//! signed in, and it is the same fact the list of devices shows. Nothing here
//! ever refuses a token for its age: a row that is present is a session that
//! works. What ends one is somebody signing out, a password being changed, or
//! the sweep below taking away a row nobody has used in a year.
//!
//! The one thing that has to be watched is how often the last use is written:
//! there is a single writer for the whole database, and a film being watched
//! asks for a segment every few seconds. So it is written again only once it
//! has gone stale, which costs at most one write an hour per device.

use std::collections::HashMap;

use melyxar_core::id::{DeviceId, UserId};
use melyxar_core::time::Timestamp;
use melyxar_core::user::User;
use sqlx::{AssertSqlSafe, Row};

use crate::convert::{parse_id, parse_timestamp, timestamp_to_text};
use crate::{Database, Result};

/// How long a session nobody uses is kept before it is thrown away.
///
/// A year, and it is the only thing that ends a session on its own. Nothing
/// refuses a token for its age: a browser that still holds its cookie is
/// signed in however long it has been, which is what ticking the box at the
/// door was meant to promise. A household that watches something over the
/// holidays and nothing else is not asked to sign in again every spring.
///
/// A year rather than never, because a device nobody has touched in twelve
/// months is a device somebody has stopped using, and a token still good on
/// one is a token nobody would miss. It is swept rather than refused, so the
/// row goes with it: a server used for years should not carry a line for
/// every browser anybody ever opened it in.
pub const AN_UNUSED_SESSION_IS_KEPT_FOR: time::Duration = time::Duration::days(365);

/// How stale the record of a session's last use may get before it is written
/// down again.
///
/// A session is kept alive by writing down that it was used, and there is one
/// writer for the whole database. Written on every request, one film being
/// watched would queue a write per segment of it behind every scan and every
/// resume position. An hour costs at most one write an hour per device, and
/// moves the moment the sweep would reach a session by at most an hour out of
/// a year, which nobody can feel.
const WRITTEN_DOWN_AGAIN_AFTER: time::Duration = time::Duration::hours(1);

/// Whether the browser is to hold on to its session once it is closed.
///
/// Nothing to do with the sweep above, which is the server's to decide. This
/// is the one thing the server cannot know on its own: whether the machine
/// belongs to the person signing in. Somebody on their own television says
/// yes and stays signed in for as long as they go on using it; somebody on a
/// machine at work says no, and closing the browser is the end of it for
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Remembered {
    /// Kept by the browser, so the machine stays signed in. The ordinary one,
    /// and what every device signed in before this existed was.
    Yes,
    /// Dropped the moment the browser is closed.
    UntilTheBrowserCloses,
}

impl Remembered {
    /// As the column holds it.
    fn as_int(self) -> i64 {
        match self {
            Self::Yes => 1,
            Self::UntilTheBrowserCloses => 0,
        }
    }

    /// Back from the column. Anything but a plain no is a yes, since that is
    /// what a row written before the column existed was given.
    fn from_int(stored: i64) -> Self {
        match stored {
            0 => Self::UntilTheBrowserCloses,
            _ => Self::Yes,
        }
    }
}

/// Whoever presented a token, and the device they presented it from.
#[derive(Debug, Clone, PartialEq)]
pub struct SignedIn {
    pub user: User,
    /// Which device this is, so one can be signed out without touching the
    /// others.
    pub device: DeviceId,
    /// What that device was called when it signed in: what the browser said
    /// it was, which is what a page turns into "Chrome on Windows".
    pub device_name: String,
    /// The browser that device is, as its own page found, when it has said.
    pub device_browser: Option<String>,
    /// What this browser was told about keeping its session. Carried so that
    /// a token handed to the same browser again is kept exactly as long as
    /// the one it replaces, rather than quietly becoming a longer one.
    pub remembered: Remembered,
    /// The identifier the browser gave itself, when it gave one: a session
    /// handed to it again replaces this one rather than sitting beside it.
    pub client: Option<String>,
}

/// One device signed in, as a list of them shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SignedInDevice {
    pub id: DeviceId,
    pub user_id: UserId,
    pub user_name: String,
    /// The picture of the account, under the folder of pictures.
    pub user_avatar: Option<String>,
    /// What the browser said it was when it signed in.
    pub name: String,
    /// The browser its own page found, when it has said.
    pub browser: Option<String>,
    pub remembered: Remembered,
    pub signed_in_at: Timestamp,
    /// When it was last used, as written down: up to an hour behind.
    pub last_seen_at: Timestamp,
}

/// The devices one account is signed in on, as the list of accounts shows
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DevicesOfAnAccount {
    pub signed_in: i64,
    /// When the most recent of them was last used, as written down.
    pub last_seen_at: Timestamp,
}

/// The devices signed in to this server, counted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeviceCounts {
    pub signed_in: i64,
    /// Those used since the moment asked about.
    pub used_lately: i64,
}

impl Database {
    /// Writes down that somebody signed in, and from what.
    ///
    /// The name is whatever the layer above made of the browser that asked: it
    /// is shown in the list of devices and nothing is ever decided from it.
    ///
    /// A browser that says which one it is has the session this account held
    /// on it replaced, in the same transaction: signing in ten times from one
    /// browser leaves one session, not ten of which nobody holds nine.
    pub async fn open_session(
        &self,
        user_id: UserId,
        device_name: &str,
        token_fingerprint: &str,
        remembered: Remembered,
        at: Timestamp,
        client: Option<&str>,
    ) -> Result<DeviceId> {
        let id = DeviceId::new();
        let mut transaction = self.begin().await?;
        if let Some(client) = client {
            sqlx::query("DELETE FROM devices WHERE user_id = ? AND client_id = ?")
                .bind(user_id.to_db_string())
                .bind(client)
                .execute(&mut *transaction)
                .await?;
        }
        sqlx::query(
            "INSERT INTO devices
                 (id, user_id, name, token_hash, remembered, created_at, last_seen_at, client_id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(user_id.to_db_string())
        .bind(device_name)
        .bind(token_fingerprint)
        .bind(remembered.as_int())
        .bind(timestamp_to_text(at))
        .bind(timestamp_to_text(at))
        .bind(client)
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(id)
    }

    /// The account behind a token, and nothing when there is none.
    ///
    /// Answers nothing rather than failing for every way a token can fail to
    /// name anybody: never issued, signed out since, or swept for having gone
    /// a year unused. From outside they are one answer, which is the answer
    /// somebody gets when they are not signed in.
    ///
    /// Age is not one of those ways. A row that is here is a session that
    /// works, whenever it was opened.
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
            ", d.id AS device_id, d.name AS device_name, d.browser AS device_browser, \
             d.last_seen_at, d.remembered, d.client_id",
            "JOIN devices d ON d.user_id = u.id
             WHERE d.token_hash = ?",
        )))
        .bind(token_fingerprint)
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
            device_name: row.try_get("device_name")?,
            device_browser: row.try_get("device_browser")?,
            remembered: Remembered::from_int(row.try_get("remembered")?),
            client: row.try_get("client_id")?,
        }))
    }

    /// Writes down which browser a device is, as its own page found.
    pub async fn name_the_browser(&self, device: DeviceId, browser: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE devices SET browser = ? WHERE id = ?")
            .bind(browser)
            .bind(device.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Signs one device out, and says whether there was one to sign out.
    pub async fn close_session(&self, token_fingerprint: &str) -> Result<bool> {
        let done = sqlx::query("DELETE FROM devices WHERE token_hash = ?")
            .bind(token_fingerprint)
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// The devices signed in, the most recently used first: every one, or
    /// those of one account.
    pub async fn signed_in_devices(&self, of: Option<UserId>) -> Result<Vec<SignedInDevice>> {
        let rows = sqlx::query(
            "SELECT d.id, d.user_id, u.name AS user_name, u.avatar_path, d.name, d.browser,
                    d.remembered, d.created_at, d.last_seen_at
             FROM devices d
             JOIN users u ON u.id = d.user_id
             WHERE ?1 IS NULL OR d.user_id = ?1
             ORDER BY d.last_seen_at DESC, d.created_at DESC",
        )
        .bind(of.map(|user| user.to_db_string()))
        .fetch_all(self.reader())
        .await?;
        rows.iter()
            .map(|row| {
                Ok(SignedInDevice {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    user_id: parse_id(&row.try_get::<String, _>("user_id")?)?,
                    user_name: row.try_get("user_name")?,
                    user_avatar: row.try_get("avatar_path")?,
                    name: row.try_get("name")?,
                    browser: row.try_get("browser")?,
                    remembered: Remembered::from_int(row.try_get("remembered")?),
                    signed_in_at: parse_timestamp(&row.try_get::<String, _>("created_at")?)?,
                    last_seen_at: parse_timestamp(&row.try_get::<String, _>("last_seen_at")?)?,
                })
            })
            .collect()
    }

    /// Signs one device out by its identifier, and answers whose it was when
    /// there was one.
    pub async fn close_device(&self, device: DeviceId) -> Result<Option<UserId>> {
        let row: Option<(String,)> =
            sqlx::query_as("DELETE FROM devices WHERE id = ? RETURNING user_id")
                .bind(device.to_db_string())
                .fetch_optional(self.writer())
                .await?;
        row.map(|(user,)| parse_id(&user)).transpose()
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

    /// Throws away the sessions nobody has used in a year.
    ///
    /// This is what ends a session that nobody ended, so it is not tidying:
    /// until it runs, every one of these rows still signs somebody in. It has
    /// to be run, and the upkeep runs it once a day.
    ///
    /// Says how many it took, which is the only way anybody finds out that it
    /// is doing its job.
    pub async fn forget_stale_sessions(&self, at: Timestamp) -> Result<u64> {
        let done = sqlx::query("DELETE FROM devices WHERE last_seen_at < ?")
            .bind(timestamp_to_text(at - AN_UNUSED_SESSION_IS_KEPT_FOR))
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected())
    }

    /// How many devices are signed in, and how many of those were used since
    /// the moment given.
    ///
    /// Read from the last use as written down, which trails the real one by up
    /// to an hour (see the note at the top of this file): near enough to say
    /// who was about today.
    pub async fn device_counts(&self, used_since: Timestamp) -> Result<DeviceCounts> {
        let (signed_in, used_lately): (i64, i64) = sqlx::query_as(
            "SELECT count(*), count(CASE WHEN last_seen_at >= ? THEN 1 END) FROM devices",
        )
        .bind(timestamp_to_text(used_since))
        .fetch_one(self.reader())
        .await?;
        Ok(DeviceCounts {
            signed_in,
            used_lately,
        })
    }

    /// How many devices each account is signed in on, and when it was last
    /// about. An account signed in nowhere is not in the answer.
    ///
    /// Read from the last use as written down, which trails the real one by
    /// up to an hour (see the note at the top of this file).
    pub async fn devices_of_every_account(&self) -> Result<HashMap<UserId, DevicesOfAnAccount>> {
        let rows: Vec<(String, i64, String)> = sqlx::query_as(
            "SELECT user_id, count(*), max(last_seen_at) FROM devices GROUP BY user_id",
        )
        .fetch_all(self.reader())
        .await?;
        rows.into_iter()
            .map(|(user, signed_in, last_seen_at)| {
                Ok((
                    parse_id(&user)?,
                    DevicesOfAnAccount {
                        signed_in,
                        last_seen_at: parse_timestamp(&last_seen_at)?,
                    },
                ))
            })
            .collect()
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
    async fn signing_in_again_from_the_same_browser_replaces_the_session_there() {
        let (database, user_id) = a_server_with_one_account().await;
        let first = database
            .open_session(user_id, "a browser", "first", Remembered::Yes, A_MOMENT, Some("browser-one"))
            .await
            .expect("opened");
        let second = database
            .open_session(user_id, "a browser", "second", Remembered::Yes, A_MOMENT, Some("browser-one"))
            .await
            .expect("opened");
        assert_ne!(first, second);
        assert!(
            database
                .session_holder("first", A_MOMENT)
                .await
                .expect("read")
                .is_none(),
            "the session it replaced holds nobody"
        );
        let holder = database
            .session_holder("second", A_MOMENT)
            .await
            .expect("read")
            .expect("signed in");
        assert_eq!(holder.client.as_deref(), Some("browser-one"));

        // Another browser, and one that could keep nothing, sit beside it.
        database
            .open_session(user_id, "a phone", "third", Remembered::Yes, A_MOMENT, Some("browser-two"))
            .await
            .expect("opened");
        database
            .open_session(user_id, "a phone", "fourth", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("opened");
        database
            .open_session(user_id, "a phone", "fifth", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("opened");
        assert_eq!(
            database.signed_in_devices(Some(user_id)).await.expect("read").len(),
            4
        );
    }

    #[tokio::test]
    async fn two_accounts_on_one_browser_keep_a_session_each() {
        let (database, victor) = a_server_with_one_account().await;
        let zoe = database
            .create_user("zoe", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account created")
            .id;
        database
            .open_session(victor, "a browser", "his", Remembered::Yes, A_MOMENT, Some("shared"))
            .await
            .expect("opened");
        database
            .open_session(zoe, "a browser", "hers", Remembered::Yes, A_MOMENT, Some("shared"))
            .await
            .expect("opened");
        assert!(database.session_holder("his", A_MOMENT).await.expect("read").is_some());
        assert!(database.session_holder("hers", A_MOMENT).await.expect("read").is_some());
    }

    #[tokio::test]
    async fn the_devices_are_listed_most_recent_first_and_one_closes_alone() {
        let (database, victor) = a_server_with_one_account().await;
        let zoe = database
            .create_user("zoe", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account created")
            .id;
        let earlier = datetime!(2026-01-01 08:00 UTC);
        let later = datetime!(2026-01-02 09:30 UTC);
        let his = database
            .open_session(victor, "his laptop", "his", Remembered::Yes, earlier, None)
            .await
            .expect("opened");
        let hers = database
            .open_session(zoe, "her phone", "hers", Remembered::UntilTheBrowserCloses, later, None)
            .await
            .expect("opened");

        let every = database.signed_in_devices(None).await.expect("read");
        assert_eq!(
            every
                .iter()
                .map(|device| (device.id, device.user_name.as_str(), device.remembered))
                .collect::<Vec<_>>(),
            vec![
                (hers, "zoe", Remembered::UntilTheBrowserCloses),
                (his, "victor", Remembered::Yes)
            ]
        );
        assert_eq!(every[0].signed_in_at, later);
        assert_eq!(
            database
                .signed_in_devices(Some(victor))
                .await
                .expect("read")
                .iter()
                .map(|device| device.id)
                .collect::<Vec<_>>(),
            vec![his]
        );

        assert_eq!(database.close_device(hers).await.expect("closed"), Some(zoe));
        assert!(database.session_holder("hers", later).await.expect("read").is_none());
        assert!(database.session_holder("his", later).await.expect("read").is_some());
        assert_eq!(database.close_device(hers).await.expect("asked"), None);
    }

    #[tokio::test]
    async fn a_token_brings_back_the_account_it_was_opened_for() {
        let (database, user_id) = a_server_with_one_account().await;
        let device = database
            .open_session(user_id, "a browser", "a fingerprint", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("session opened");

        let signed_in = database
            .session_holder("a fingerprint", A_MOMENT)
            .await
            .expect("read")
            .expect("somebody is behind that token");

        assert_eq!(signed_in.device, device);
        assert_eq!(signed_in.device_name, "a browser");
        assert_eq!(signed_in.device_browser, None, "until its page says");
        assert_eq!(signed_in.user.id, user_id);
        assert_eq!(signed_in.user.name, "victor");
        // The rights and the preferences come with it, because every request
        // that asks who is there asks in order to decide something.
        assert!(signed_in.user.permissions.is_administrator);
        assert_eq!(signed_in.user.preferences.accent_color, "#c81e1e");
    }

    #[tokio::test]
    async fn the_browser_a_page_found_comes_back_with_its_device_and_no_other() {
        let (database, user_id) = a_server_with_one_account().await;
        let device = database
            .open_session(user_id, "a browser", "one fingerprint", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("session opened");
        database
            .open_session(user_id, "a browser", "another fingerprint", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("second session opened");

        database
            .name_the_browser(device, Some("Brave"))
            .await
            .expect("written");
        let named = database
            .session_holder("one fingerprint", A_MOMENT)
            .await
            .expect("read")
            .expect("signed in");
        assert_eq!(named.device_browser.as_deref(), Some("Brave"));
        let other = database
            .session_holder("another fingerprint", A_MOMENT)
            .await
            .expect("read")
            .expect("signed in");
        assert_eq!(other.device_browser, None);

        database.name_the_browser(device, None).await.expect("taken back");
        let unnamed = database
            .session_holder("one fingerprint", A_MOMENT)
            .await
            .expect("read")
            .expect("signed in");
        assert_eq!(unnamed.device_browser, None);
    }

    /// What somebody said about the machine they are on comes back with them.
    ///
    /// It comes back because the one thing that reads it is the handing over
    /// of a fresh token to the same browser, which has to be kept for as long
    /// as the one it replaces rather than quietly becoming a longer one.
    #[tokio::test]
    async fn a_session_remembers_whether_it_was_to_be_remembered() {
        let (database, user_id) = a_server_with_one_account().await;
        database
            .open_session(user_id, "a browser", "one fingerprint", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("session opened");
        database
            .open_session(
                user_id,
                "a machine at work",
                "another fingerprint",
                Remembered::UntilTheBrowserCloses,
                A_MOMENT,
                None,
            )
            .await
            .expect("session opened");

        let kept = database
            .session_holder("one fingerprint", A_MOMENT)
            .await
            .expect("read")
            .expect("somebody is behind that token");
        assert_eq!(kept.remembered, Remembered::Yes);

        let for_now = database
            .session_holder("another fingerprint", A_MOMENT)
            .await
            .expect("read")
            .expect("somebody is behind that token");
        assert_eq!(for_now.remembered, Remembered::UntilTheBrowserCloses);
    }

    /// A device signed in before the column existed was given a yes by the
    /// migration, and so is anything else that is not a plain no.
    #[test]
    fn a_device_that_never_said_is_one_that_is_remembered() {
        assert_eq!(Remembered::from_int(1), Remembered::Yes);
        assert_eq!(Remembered::from_int(0), Remembered::UntilTheBrowserCloses);
        assert_eq!(Remembered::from_int(7), Remembered::Yes);
        assert_eq!(Remembered::Yes.as_int(), 1);
        assert_eq!(Remembered::UntilTheBrowserCloses.as_int(), 0);
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
            .open_session(user.id, "a browser", "a fingerprint", Remembered::Yes, A_MOMENT, None)
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
            .open_session(user_id, "a browser", "a fingerprint", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("session opened");

        assert!(database
            .session_holder("another fingerprint", A_MOMENT)
            .await
            .expect("read")
            .is_none());
    }

    /// Age alone never refuses a token.
    ///
    /// This is the promise the box at the door makes, and it is the one thing
    /// here somebody would break by adding a sensible looking bound back into
    /// the query. A browser that kept its cookie is signed in, whenever it was
    /// last here; what ends a session is a sweep, a sign out or a password
    /// changed.
    #[tokio::test]
    async fn a_session_nobody_used_for_years_still_names_whoever_opened_it() {
        let (database, user_id) = a_server_with_one_account().await;
        database
            .open_session(user_id, "a browser", "a fingerprint", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("session opened");

        let years_later = A_MOMENT + time::Duration::days(3 * 365);
        let signed_in = database
            .session_holder("a fingerprint", years_later)
            .await
            .expect("read")
            .expect("nothing refuses a token for its age");
        assert_eq!(signed_in.user.id, user_id);
    }

    #[tokio::test]
    async fn using_a_session_keeps_it_alive_without_writing_on_every_request() {
        let (database, user_id) = a_server_with_one_account().await;
        database
            .open_session(user_id, "a browser", "a fingerprint", Remembered::Yes, A_MOMENT, None)
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

        // Past the point where it has gone stale, it is written down again.
        // That is what carries a session out of the sweep's reach: the sweep
        // reads this date and nothing else.
        let much_later = A_MOMENT + time::Duration::days(20);
        database
            .session_holder("a fingerprint", much_later)
            .await
            .expect("read")
            .expect("still there");
        assert_eq!(last_used(&database).await, much_later);
    }

    #[tokio::test]
    async fn signing_one_device_out_leaves_the_others_signed_in() {
        let (database, user_id) = a_server_with_one_account().await;
        database
            .open_session(user_id, "a browser", "one fingerprint", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("session opened");
        database
            .open_session(user_id, "a television", "another fingerprint", Remembered::Yes, A_MOMENT, None)
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
            .open_session(user_id, "a browser", "one fingerprint", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("session opened");
        database
            .open_session(user_id, "a television", "another fingerprint", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("session opened");
        database
            .open_session(
                somebody_else.id,
                "a browser",
                "a third fingerprint",
                Remembered::Yes,
                A_MOMENT,
                None,
            )
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
            .open_session(
                user_id,
                "an old browser",
                "an old fingerprint",
                Remembered::Yes,
                A_MOMENT,
                None,
            )
            .await
            .expect("session opened");

        let much_later = A_MOMENT + AN_UNUSED_SESSION_IS_KEPT_FOR + time::Duration::days(1);
        database
            .open_session(user_id, "a browser", "a fresh fingerprint", Remembered::Yes, much_later, None)
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

        // And the swept one is gone for good rather than merely refused: this
        // is now the only thing that ends a session nobody ended.
        assert!(
            database
                .session_holder("an old fingerprint", much_later)
                .await
                .expect("read")
                .is_none(),
            "a session the sweep took is a session that names nobody"
        );
    }

    /// A day short of the year, it stays. That bound is the whole setting, and
    /// a sweep that took a session somebody still uses would sign them out of
    /// their own television for no reason they could see.
    #[tokio::test]
    async fn a_session_used_within_the_year_is_left_alone() {
        let (database, user_id) = a_server_with_one_account().await;
        database
            .open_session(user_id, "a browser", "a fingerprint", Remembered::Yes, A_MOMENT, None)
            .await
            .expect("session opened");

        let a_day_short = A_MOMENT + AN_UNUSED_SESSION_IS_KEPT_FOR - time::Duration::days(1);
        assert_eq!(
            database
                .forget_stale_sessions(a_day_short)
                .await
                .expect("swept"),
            0
        );
        assert!(database
            .session_holder("a fingerprint", a_day_short)
            .await
            .expect("read")
            .is_some());
    }

    /// A device used the day before is still signed in, and is not counted as
    /// used today.
    #[tokio::test]
    async fn devices_are_counted_with_those_used_lately_apart() {
        let (database, user_id) = a_server_with_one_account().await;
        let yesterday = A_MOMENT - time::Duration::days(1);
        for (name, fingerprint, at) in [
            ("a television", "one", yesterday),
            ("a browser", "two", A_MOMENT),
            ("a phone", "three", A_MOMENT),
        ] {
            database
                .open_session(user_id, name, fingerprint, Remembered::Yes, at, None)
                .await
                .expect("session opened");
        }

        let counted = database
            .device_counts(A_MOMENT - time::Duration::hours(12))
            .await
            .expect("counted");
        assert_eq!(
            counted,
            DeviceCounts {
                signed_in: 3,
                used_lately: 2
            }
        );
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
