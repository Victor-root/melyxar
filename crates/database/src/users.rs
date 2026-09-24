//! Accounts, their rights and their preferences.
//!
//! Present from the first migration even though the first version ships with a
//! single account: every progress row, favourite and preference hangs off a
//! user, and retrofitting that would touch every table and every route.

use melyxar_core::id::{LibraryId, UserId};
use melyxar_core::library::LibraryKind;
use melyxar_core::time::{now, Timestamp};
use melyxar_core::user::{DownmixMethod, Permissions, Preferences, ThemeMode, User};
use sqlx::{AssertSqlSafe, Row};

use crate::convert::{bool_to_int, int_to_bool, parse_id, parse_timestamp, timestamp_to_text};
use crate::{Database, Result};

/// The languages a library holds, ready to fill a picker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AvailableLanguages {
    pub audio: Vec<String>,
    pub subtitle: Vec<String>,
}

/// A name offered on the sign in screen, with its picture when it has one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameAtTheDoor {
    pub name: String,
    /// Where its picture is, under the folder of pictures.
    pub avatar_path: Option<String>,
}

/// Everything an account is made of, rights and preferences together.
///
/// Written once: three reads load the same account, and a column added to one
/// of them and forgotten in the others is a field that silently goes missing
/// depending on which screen asked.
const WHAT_AN_ACCOUNT_IS: &str =
    "u.id, u.name, u.avatar_path, u.is_administrator, u.sees_every_library,
     u.max_age_rating,
     u.may_download, u.may_delete, u.may_delete_from_disk, u.max_sessions, u.created_at,
     p.interface_language, p.preferred_audio_language, p.preferred_subtitle_language,
     p.theme_mode, p.accent_color, p.custom_css, p.volume,
     p.downmix_method, p.downmix_gain,
     p.banner_height, p.banner_cut, p.banner_shown, p.banner_at_random,
     p.banner_fills_the_screen, p.header_hides_on_scroll,
     p.hidden_at_the_door, p.home_order";

/// The read of an account, with whatever else the caller needs alongside and
/// however it picks the rows.
///
/// Shared with the sessions next door, which loads the very same account from
/// the token somebody presented: one reading of an account, wherever the row
/// is picked from.
pub(crate) fn reading_accounts(also: &str, ending: &str) -> String {
    format!(
        "SELECT {WHAT_AN_ACCOUNT_IS}{also}
         FROM users u
         JOIN user_preferences p ON p.user_id = u.id
         {ending}"
    )
}

impl Database {
    /// The libraries one account has been granted.
    pub(crate) async fn libraries_allowed_to(&self, id: UserId) -> Result<Vec<(String,)>> {
        Ok(
            sqlx::query_as("SELECT library_id FROM user_library_access WHERE user_id = ?")
                .bind(id.to_db_string())
                .fetch_all(self.reader())
                .await?,
        )
    }

    /// Creates an account along with its preferences row.
    pub async fn create_user(
        &self,
        name: &str,
        password_hash: Option<&str>,
        permissions: &Permissions,
    ) -> Result<User> {
        let mut transaction = self.begin().await?;
        let made = write_an_account(&mut transaction, name, password_hash, permissions).await?;
        transaction.commit().await?;
        Ok(made)
    }

    /// Creates the very first account of a server, and nothing if there is one.
    ///
    /// The looking and the writing inside one transaction, on the one
    /// connection this database writes through. Two people reaching a brand
    /// new server in the same breath would otherwise both find it empty and
    /// both make themselves its administrator; the second now waits for the
    /// first, and then finds an account.
    pub async fn create_the_first_user(
        &self,
        name: &str,
        password_hash: &str,
    ) -> Result<Option<User>> {
        let mut transaction = self.begin().await?;

        let (already,): (i64,) = sqlx::query_as("SELECT count(*) FROM users")
            .fetch_one(&mut *transaction)
            .await?;
        if already > 0 {
            return Ok(None);
        }

        let made = write_an_account(
            &mut transaction,
            name,
            Some(password_hash),
            &Permissions::administrator(),
        )
        .await?;
        transaction.commit().await?;
        Ok(Some(made))
    }

    /// Number of accounts, used to decide whether the setup wizard still has
    /// to run.
    pub async fn user_count(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT count(*) FROM users")
            .fetch_one(self.reader())
            .await?;
        Ok(row.0)
    }

    /// Loads one account with its rights and preferences.
    pub async fn user(&self, id: UserId) -> Result<Option<User>> {
        let Some(row) = sqlx::query(AssertSqlSafe(reading_accounts("", "WHERE u.id = ?")))
            .bind(id.to_db_string())
            .fetch_optional(self.reader())
            .await?
        else {
            return Ok(None);
        };

        let allowed = self.libraries_allowed_to(id).await?;

        Ok(Some(build_user(&row, &allowed)?))
    }

    /// Loads an account by name, for signing in. The comparison ignores case,
    /// because nobody remembers how they capitalised their own name.
    pub async fn user_by_name(&self, name: &str) -> Result<Option<(User, Option<String>)>> {
        let Some(row) = sqlx::query(AssertSqlSafe(reading_accounts(
            ", u.password_hash",
            "WHERE u.name = ? COLLATE NOCASE",
        )))
        .bind(name)
        .fetch_optional(self.reader())
        .await?
        else {
            return Ok(None);
        };

        let id = parse_id::<UserId>(&row.try_get::<String, _>("id")?)?;
        let allowed = self.libraries_allowed_to(id).await?;

        let password_hash: Option<String> = row.try_get("password_hash")?;
        Ok(Some((build_user(&row, &allowed)?, password_hash)))
    }

    /// Every account, ordered by name. Small by nature: this is a home server.
    pub async fn list_users(&self) -> Result<Vec<User>> {
        let rows = sqlx::query(AssertSqlSafe(reading_accounts(
            "",
            "ORDER BY u.name COLLATE NOCASE",
        )))
        .fetch_all(self.reader())
        .await?;

        let mut users = Vec::with_capacity(rows.len());
        for row in &rows {
            let id = parse_id::<UserId>(&row.try_get::<String, _>("id")?)?;
            let allowed = self.libraries_allowed_to(id).await?;
            users.push(build_user(row, &allowed)?);
        }
        Ok(users)
    }

    /// The names the sign in screen may offer, in the order it shows them.
    ///
    /// Its own small query rather than the whole of every account: this is the
    /// one read on this server that answers somebody who has not signed in, so
    /// it reads the two columns that screen draws and not one more. Nothing
    /// here carries an identifier, a right or a preference, because none of
    /// that is anybody's business before they are through the door.
    ///
    /// Accounts that asked to be left off are left off. They still sign in:
    /// the name is typed rather than pressed, and the screen keeps the field
    /// for exactly that.
    pub async fn names_at_the_door(&self) -> Result<Vec<NameAtTheDoor>> {
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT u.name, u.avatar_path
             FROM users u
             JOIN user_preferences p ON p.user_id = u.id
             WHERE p.hidden_at_the_door = 0
             ORDER BY u.name COLLATE NOCASE",
        )
        .fetch_all(self.reader())
        .await?;
        Ok(rows
            .into_iter()
            .map(|(name, avatar_path)| NameAtTheDoor { name, avatar_path })
            .collect())
    }

    /// Gives an account its picture, or takes it away with nothing, and
    /// answers the one it had so the caller can delete its file.
    pub async fn set_avatar(&self, id: UserId, avatar_path: Option<&str>) -> Result<Option<String>> {
        let mut transaction = self.begin().await?;
        let before: Option<String> =
            sqlx::query_scalar("SELECT avatar_path FROM users WHERE id = ?")
                .bind(id.to_db_string())
                .fetch_optional(&mut *transaction)
                .await?
                .flatten();
        sqlx::query("UPDATE users SET avatar_path = ? WHERE id = ?")
            .bind(avatar_path)
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(before)
    }

    /// Every language the library actually holds, told apart by kind.
    ///
    /// A picker built from this offers what someone can really choose. A list
    /// of five hundred languages, or a fixed handful decided in advance, both
    /// end with someone picking one no film in the house carries.
    pub async fn languages_in_use(&self) -> Result<AvailableLanguages> {
        let rows = sqlx::query(
            "SELECT DISTINCT kind, language FROM tracks
             WHERE language IS NOT NULL AND language <> ''
               AND kind IN ('audio', 'subtitle')
             ORDER BY kind, language",
        )
        .fetch_all(self.reader())
        .await?;

        let mut available = AvailableLanguages::default();
        for row in rows {
            let language: String = row.try_get("language")?;
            match row.try_get::<String, _>("kind")?.as_str() {
                "audio" => available.audio.push(language),
                _ => available.subtitle.push(language),
            }
        }
        Ok(available)
    }

    /// Takes an account away, and says whether there was one to take.
    ///
    /// Everything of theirs goes with it: where they were in every film, what
    /// they marked as liked, what they were still meaning to watch, their
    /// preferences and every device they were signed in on. That is the
    /// schema's own doing rather than this statement's, and it is what
    /// removing an account means.
    pub async fn delete_user(&self, id: UserId) -> Result<bool> {
        let done = sqlx::query("DELETE FROM users WHERE id = ?")
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Gives an account another name, and says `false` when another account
    /// already has it.
    ///
    /// Left to the unique index rather than looked up first, so two accounts
    /// asking for the same name in the same breath cannot both have it. The
    /// index ignores case, as signing in does; the account's own name written
    /// otherwise is not another account's.
    pub async fn rename_user(&self, id: UserId, name: &str) -> Result<bool> {
        let renamed = sqlx::query("UPDATE users SET name = ? WHERE id = ?")
            .bind(name)
            .bind(id.to_db_string())
            .execute(self.writer())
            .await;
        match renamed {
            Ok(_) => Ok(true),
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    /// Says which libraries an account may see, replacing whatever it had.
    ///
    /// Whole rather than one at a time: what is being set is the answer to
    /// one question, and a grant added while another is being taken away
    /// would leave an account seeing a set nobody asked for.
    pub async fn set_library_access(
        &self,
        id: UserId,
        sees_every_library: bool,
        granted: &[LibraryId],
    ) -> Result<()> {
        let mut transaction = self.begin().await?;

        sqlx::query("UPDATE users SET sees_every_library = ? WHERE id = ?")
            .bind(bool_to_int(sees_every_library))
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;

        sqlx::query("DELETE FROM user_library_access WHERE user_id = ?")
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;

        for library in granted {
            sqlx::query("INSERT INTO user_library_access (user_id, library_id) VALUES (?, ?)")
                .bind(id.to_db_string())
                .bind(library.to_db_string())
                .execute(&mut *transaction)
                .await?;
        }

        transaction.commit().await?;
        Ok(())
    }

    /// How many administrators this server has.
    ///
    /// What stops the last one being taken away: a server with nobody who may
    /// manage it is a server that has to be opened from a terminal to be put
    /// right, and nothing in the interface would ever say so.
    pub async fn administrator_count(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT count(*) FROM users WHERE is_administrator = 1")
            .fetch_one(self.reader())
            .await?;
        Ok(row.0)
    }

    /// Saves the preferences of one account, after clamping the values a
    /// client may have sent out of range.
    pub async fn save_preferences(&self, id: UserId, preferences: &Preferences) -> Result<()> {
        let preferences = preferences.clone().normalised();
        sqlx::query(
            "UPDATE user_preferences SET
                interface_language = ?, preferred_audio_language = ?,
                preferred_subtitle_language = ?, theme_mode = ?, accent_color = ?,
                custom_css = ?, volume = ?, downmix_method = ?, downmix_gain = ?,
                banner_height = ?, banner_cut = ?, banner_shown = ?, banner_at_random = ?,
                banner_fills_the_screen = ?, header_hides_on_scroll = ?,
                hidden_at_the_door = ?, home_order = ?
             WHERE user_id = ?",
        )
        .bind(&preferences.interface_language)
        .bind(&preferences.preferred_audio_language)
        .bind(&preferences.preferred_subtitle_language)
        .bind(preferences.theme_mode.as_str())
        .bind(&preferences.accent_color)
        .bind(&preferences.custom_css)
        .bind(preferences.volume)
        .bind(preferences.downmix_method.as_str())
        .bind(preferences.downmix_gain)
        .bind(preferences.banner_height)
        .bind(preferences.banner_cut)
        .bind(preferences.banner_shown)
        .bind(preferences.banner_at_random)
        .bind(preferences.banner_fills_the_screen)
        .bind(preferences.header_hides_on_scroll)
        .bind(preferences.hidden_at_the_door)
        .bind(written_order(&preferences.home_order))
        .bind(id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }
}

/// The order of a home page as it is stored: the names of the kinds, joined by
/// commas.
fn written_order(order: &[LibraryKind]) -> String {
    order
        .iter()
        .map(|kind| kind.as_str())
        .collect::<Vec<_>>()
        .join(",")
}

/// The order of a home page read back. A name nobody knows is dropped rather
/// than locking the account out.
fn read_order(stored: &str) -> Vec<LibraryKind> {
    stored.split(',').filter_map(LibraryKind::parse).collect()
}

/// Writes an account, its preferences and its grants, inside one transaction.
///
/// Shared by the two that make one, which differ only in what they look at
/// before writing.
async fn write_an_account(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    name: &str,
    password_hash: Option<&str>,
    permissions: &Permissions,
) -> Result<User> {
    let id = UserId::new();
    let created_at = now();
    let preferences = Preferences::default();

    sqlx::query(
        "INSERT INTO users (id, name, password_hash, is_administrator, sees_every_library,
                            max_age_rating, may_download, may_delete, may_delete_from_disk,
                            max_sessions, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_db_string())
    .bind(name)
    .bind(password_hash)
    .bind(bool_to_int(permissions.is_administrator))
    .bind(bool_to_int(permissions.sees_every_library))
    .bind(permissions.max_age_rating)
    .bind(bool_to_int(permissions.may_download))
    .bind(bool_to_int(permissions.may_delete))
    .bind(bool_to_int(permissions.may_delete_from_disk))
    .bind(permissions.max_sessions)
    .bind(timestamp_to_text(created_at))
    .execute(&mut **transaction)
    .await?;

    sqlx::query(
        "INSERT INTO user_preferences (user_id, interface_language, theme_mode, accent_color,
                                       volume, downmix_method, downmix_gain,
                                       banner_height, banner_cut, banner_shown, banner_at_random,
                                       banner_fills_the_screen, header_hides_on_scroll,
                                       hidden_at_the_door, home_order)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_db_string())
    .bind(&preferences.interface_language)
    .bind(preferences.theme_mode.as_str())
    .bind(&preferences.accent_color)
    .bind(preferences.volume)
    .bind(preferences.downmix_method.as_str())
    .bind(preferences.downmix_gain)
    .bind(preferences.banner_height)
    .bind(preferences.banner_cut)
    .bind(preferences.banner_shown)
    .bind(preferences.banner_at_random)
    .bind(preferences.banner_fills_the_screen)
    .bind(preferences.header_hides_on_scroll)
    .bind(preferences.hidden_at_the_door)
    .bind(written_order(&preferences.home_order))
    .execute(&mut **transaction)
    .await?;

    for library in &permissions.allowed_libraries {
        sqlx::query("INSERT INTO user_library_access (user_id, library_id) VALUES (?, ?)")
            .bind(id.to_db_string())
            .bind(library.to_db_string())
            .execute(&mut **transaction)
            .await?;
    }

    Ok(User {
        id,
        name: name.to_string(),
        avatar_path: None,
        permissions: permissions.clone(),
        preferences,
        created_at,
    })
}

/// Builds a domain account out of a row and its granted libraries.
///
/// A stored value that no longer maps to a known variant falls back to the
/// default rather than failing the whole read: an account that cannot load
/// locks its owner out, which is a far worse outcome than a reset preference.
pub(crate) fn build_user(row: &sqlx::sqlite::SqliteRow, allowed: &[(String,)]) -> Result<User> {
    let mut allowed_libraries = Vec::with_capacity(allowed.len());
    for (value,) in allowed {
        allowed_libraries.push(parse_id::<LibraryId>(value)?);
    }

    let created_at: Timestamp = parse_timestamp(&row.try_get::<String, _>("created_at")?)?;

    Ok(User {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        name: row.try_get("name")?,
        avatar_path: row.try_get("avatar_path")?,
        permissions: Permissions {
            is_administrator: int_to_bool(row.try_get("is_administrator")?),
            sees_every_library: int_to_bool(row.try_get("sees_every_library")?),
            allowed_libraries,
            max_age_rating: row.try_get("max_age_rating")?,
            may_download: int_to_bool(row.try_get("may_download")?),
            may_delete: int_to_bool(row.try_get("may_delete")?),
            may_delete_from_disk: int_to_bool(row.try_get("may_delete_from_disk")?),
            max_sessions: row.try_get("max_sessions")?,
        },
        preferences: Preferences {
            interface_language: row.try_get("interface_language")?,
            preferred_audio_language: row.try_get("preferred_audio_language")?,
            preferred_subtitle_language: row.try_get("preferred_subtitle_language")?,
            theme_mode: ThemeMode::parse(&row.try_get::<String, _>("theme_mode")?)
                .unwrap_or_default(),
            accent_color: row.try_get("accent_color")?,
            custom_css: row.try_get("custom_css")?,
            volume: row.try_get("volume")?,
            downmix_method: DownmixMethod::parse(&row.try_get::<String, _>("downmix_method")?)
                .unwrap_or_default(),
            downmix_gain: row.try_get("downmix_gain")?,
            banner_height: row.try_get("banner_height")?,
            banner_cut: row.try_get("banner_cut")?,
            banner_shown: row.try_get("banner_shown")?,
            banner_at_random: row.try_get("banner_at_random")?,
            banner_fills_the_screen: row.try_get("banner_fills_the_screen")?,
            header_hides_on_scroll: row.try_get("header_hides_on_scroll")?,
            hidden_at_the_door: row.try_get("hidden_at_the_door")?,
            home_order: read_order(&row.try_get::<String, _>("home_order")?),
        }
        .normalised(),
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sessions::Remembered;

    async fn database() -> Database {
        Database::open_in_memory().await.expect("database opens")
    }

    /// What the one address that answers a stranger will say.
    ///
    /// Two things have to hold and both are about what is given away. The
    /// order is the one the screen draws, so the row does not shuffle itself
    /// between two visits. And an account that asked to be left off is left
    /// off, which is the only thing that switch does: it is still an account,
    /// it still signs in, and nothing here says it exists.
    #[tokio::test]
    async fn an_account_wears_the_picture_it_was_given_until_it_is_taken_away() {
        let database = database().await;
        let zoe = database
            .create_user("Zoe", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account created");
        let picture = format!("users/{}/avatar-one.webp", zoe.id);

        assert_eq!(
            database.set_avatar(zoe.id, Some(&picture)).await.expect("set"),
            None
        );
        let (read, _) = database
            .user_by_name("Zoe")
            .await
            .expect("lookup works")
            .expect("account found");
        assert_eq!(read.avatar_path.as_deref(), Some(picture.as_str()));
        assert_eq!(
            database.names_at_the_door().await.expect("read"),
            vec![NameAtTheDoor {
                name: "Zoe".to_string(),
                avatar_path: Some(picture.clone()),
            }],
            "and the door shows it beside the name"
        );

        assert_eq!(
            database.set_avatar(zoe.id, None).await.expect("taken away"),
            Some(picture),
            "the one it had is answered, for its file to be deleted"
        );
        assert_eq!(
            database.names_at_the_door().await.expect("read")[0].avatar_path,
            None
        );
    }

    #[tokio::test]
    async fn an_account_takes_a_new_name_unless_another_one_has_it() {
        let database = database().await;
        let zoe = database
            .create_user("Zoe", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account created");
        database
            .create_user("Marc", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account created");

        assert!(!database.rename_user(zoe.id, "marc").await.expect("asked"));
        assert_eq!(
            database.user(zoe.id).await.expect("read").expect("found").name,
            "Zoe",
            "a name another account has, whatever its case, leaves the account as it was"
        );

        assert!(database.rename_user(zoe.id, "ZOE").await.expect("asked"));
        assert!(database.rename_user(zoe.id, "Zoé").await.expect("asked"));
        let (renamed, _) = database
            .user_by_name("Zoé")
            .await
            .expect("lookup works")
            .expect("found under the new name");
        assert_eq!(renamed.id, zoe.id);
        assert!(database.user_by_name("Zoe").await.expect("lookup works").is_none());
    }

    #[tokio::test]
    async fn the_door_offers_the_names_that_did_not_ask_to_be_left_off() {
        let database = database().await;
        for name in ["Zoe", "alice", "Marc"] {
            database
                .create_user(name, Some("a stored form"), &Permissions::viewer())
                .await
                .expect("account created");
        }

        let names = || async {
            database
                .names_at_the_door()
                .await
                .expect("read")
                .into_iter()
                .map(|offered| offered.name)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            names().await,
            vec!["alice", "Marc", "Zoe"],
            "in the order the screen draws them, and without regard to case"
        );

        let (marc, _) = database
            .user_by_name("Marc")
            .await
            .expect("lookup works")
            .expect("account found");
        let mut keeping_off = marc.preferences.clone();
        keeping_off.hidden_at_the_door = true;
        database
            .save_preferences(marc.id, &keeping_off)
            .await
            .expect("saved");

        assert_eq!(
            names().await,
            vec!["alice", "Zoe"],
            "somebody who asked to be left off is left off"
        );
        assert!(
            database
                .user_by_name("Marc")
                .await
                .expect("lookup works")
                .is_some(),
            "and is still an account, which is how they still sign in"
        );
    }

    #[tokio::test]
    async fn a_fresh_server_has_no_account_yet() {
        // This count is what decides whether the setup wizard still has to
        // run: an answer that never moves either locks the server behind a
        // wizard for ever or hands it over without ever asking for a password.
        let database = database().await;
        assert_eq!(database.user_count().await.expect("counted"), 0);

        database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");
        assert_eq!(database.user_count().await.expect("counted"), 1);

        database
            .create_user("someone", None, &Permissions::viewer())
            .await
            .expect("account created");
        assert_eq!(database.user_count().await.expect("counted"), 2);
    }

    #[tokio::test]
    async fn only_one_first_account_is_ever_made() {
        let database = database().await;

        let first = database
            .create_the_first_user("victor", "a stored form")
            .await
            .expect("asked")
            .expect("a brand new server had none");
        assert!(first.permissions.is_administrator);

        // The looking and the writing happen together, so a second one asking
        // finds an account rather than an empty server.
        assert!(
            database
                .create_the_first_user("somebody else", "another stored form")
                .await
                .expect("asked")
                .is_none(),
            "a server is set up once"
        );
        assert_eq!(database.user_count().await.expect("counted"), 1);
    }

    #[tokio::test]
    async fn creating_an_account_also_creates_its_preferences() {
        let database = database().await;
        let created = database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");

        let loaded = database
            .user(created.id)
            .await
            .expect("account readable")
            .expect("account exists");

        assert_eq!(loaded.name, "victor");
        assert!(loaded.permissions.is_administrator);
        assert_eq!(loaded.preferences.accent_color, "#c81e1e");
        assert_eq!(loaded.preferences.theme_mode, ThemeMode::System);
        assert_eq!(
            loaded.preferences.downmix_method,
            DownmixMethod::BroadcastStandard
        );
    }

    #[tokio::test]
    async fn signing_in_ignores_how_the_name_was_capitalised() {
        let database = database().await;
        database
            .create_user("Victor", Some("hash"), &Permissions::administrator())
            .await
            .expect("account created");

        let (user, hash) = database
            .user_by_name("victor")
            .await
            .expect("lookup works")
            .expect("account found");
        assert_eq!(user.name, "Victor");
        assert_eq!(hash.as_deref(), Some("hash"));
    }

    #[tokio::test]
    async fn two_accounts_cannot_share_a_name_even_in_a_different_case() {
        let database = database().await;
        database
            .create_user("victor", None, &Permissions::viewer())
            .await
            .expect("first account created");
        let outcome = database
            .create_user("VICTOR", None, &Permissions::viewer())
            .await;
        assert!(outcome.is_err(), "the name must stay unique");
    }

    #[tokio::test]
    async fn granted_libraries_are_stored_and_read_back() {
        let database = database().await;
        let library = LibraryId::new();
        sqlx::query(
            "INSERT INTO libraries (id, name, kind, created_at, updated_at)
             VALUES (?, 'Films', 'movies', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
        )
        .bind(library.to_db_string())
        .execute(database.writer())
        .await
        .expect("library inserted");

        let created = database
            .create_user(
                "limited",
                None,
                &Permissions {
                    sees_every_library: false,
                    allowed_libraries: vec![library],
                    ..Permissions::viewer()
                },
            )
            .await
            .expect("account created");

        let loaded = database
            .user(created.id)
            .await
            .expect("readable")
            .expect("exists");
        assert_eq!(loaded.permissions.allowed_libraries, vec![library]);
        assert!(loaded.permissions.may_access_library(library));
        assert!(!loaded.permissions.may_access_library(LibraryId::new()));
    }

    #[tokio::test]
    async fn an_account_taken_away_takes_everything_of_its_own_with_it() {
        let database = database().await;
        let created = database
            .create_user("victor", Some("a stored form"), &Permissions::administrator())
            .await
            .expect("account created");
        database
            .open_session(created.id, "a browser", "a fingerprint", Remembered::Yes, now())
            .await
            .expect("session opened");

        assert!(database.delete_user(created.id).await.expect("removed"));
        assert!(database.user(created.id).await.expect("read").is_none());
        assert!(
            database
                .session_holder("a fingerprint", now())
                .await
                .expect("read")
                .is_none(),
            "a device of an account that is gone is a device nobody holds"
        );
        assert!(
            !database.delete_user(created.id).await.expect("asked"),
            "taking away what is already gone says so rather than failing"
        );
    }

    #[tokio::test]
    async fn what_an_account_may_see_is_replaced_whole() {
        let database = database().await;
        let mut libraries = Vec::new();
        for name in ["Films", "Series", "Anime"] {
            let id = LibraryId::new();
            sqlx::query(
                "INSERT INTO libraries (id, name, kind, created_at, updated_at)
                 VALUES (?, ?, 'movies', '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z')",
            )
            .bind(id.to_db_string())
            .bind(name)
            .execute(database.writer())
            .await
            .expect("library inserted");
            libraries.push(id);
        }

        let created = database
            .create_user("limited", None, &Permissions::viewer())
            .await
            .expect("account created");
        assert!(created.permissions.sees_the_whole_server());

        database
            .set_library_access(created.id, false, &libraries[..2])
            .await
            .expect("granted");
        let loaded = database
            .user(created.id)
            .await
            .expect("read")
            .expect("exists");
        assert!(!loaded.permissions.sees_the_whole_server());
        assert_eq!(loaded.permissions.allowed_libraries, libraries[..2].to_vec());

        // Replaced rather than added to: what is set is the answer to one
        // question, whole.
        database
            .set_library_access(created.id, false, &libraries[2..])
            .await
            .expect("granted");
        let loaded = database
            .user(created.id)
            .await
            .expect("read")
            .expect("exists");
        assert_eq!(loaded.permissions.allowed_libraries, libraries[2..].to_vec());

        database
            .set_library_access(created.id, true, &[])
            .await
            .expect("granted");
        let loaded = database
            .user(created.id)
            .await
            .expect("read")
            .expect("exists");
        assert!(loaded.permissions.sees_the_whole_server());
        assert!(loaded.permissions.allowed_libraries.is_empty());
    }

    #[tokio::test]
    async fn the_administrators_are_counted_so_the_last_one_can_be_kept() {
        let database = database().await;
        assert_eq!(database.administrator_count().await.expect("counted"), 0);

        database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");
        database
            .create_user("someone", None, &Permissions::viewer())
            .await
            .expect("account created");
        assert_eq!(database.administrator_count().await.expect("counted"), 1);

        database
            .create_user("another", None, &Permissions::administrator())
            .await
            .expect("account created");
        assert_eq!(database.administrator_count().await.expect("counted"), 2);
    }

    #[tokio::test]
    async fn preferences_are_clamped_on_the_way_in_and_on_the_way_out() {
        let database = database().await;
        let created = database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");

        let wild = Preferences {
            volume: 42.0,
            downmix_gain: 99.0,
            downmix_method: DownmixMethod::NightDialogue,
            theme_mode: ThemeMode::Dark,
            banner_height: 9.0,
            banner_cut: 0.6,
            banner_shown: false,
            banner_at_random: true,
            banner_fills_the_screen: true,
            header_hides_on_scroll: false,
            home_order: vec![LibraryKind::Anime, LibraryKind::Movies],
            ..Preferences::default()
        };
        database
            .save_preferences(created.id, &wild)
            .await
            .expect("preferences saved");

        let loaded = database
            .user(created.id)
            .await
            .expect("readable")
            .expect("exists");
        assert_eq!(loaded.preferences.volume, 1.0);
        assert_eq!(loaded.preferences.downmix_gain, 3.0);
        assert_eq!(
            loaded.preferences.banner_height,
            melyxar_core::user::MAX_BANNER_HEIGHT,
            "a banner taller than the ceiling comes back at the ceiling"
        );
        // And what was already inside its range travels untouched, which is
        // what says the three really made the round trip through the row.
        assert_eq!(loaded.preferences.banner_cut, 0.6);
        assert!(!loaded.preferences.banner_shown);
        assert!(loaded.preferences.banner_at_random);
        assert!(loaded.preferences.banner_fills_the_screen);
        assert!(!loaded.preferences.header_hides_on_scroll);
        assert_eq!(
            loaded.preferences.home_order,
            vec![
                LibraryKind::Anime,
                LibraryKind::Movies,
                LibraryKind::Series,
                LibraryKind::HomeMedia,
                LibraryKind::Shows,
                LibraryKind::Music,
            ]
        );
        assert_eq!(
            loaded.preferences.downmix_method,
            DownmixMethod::NightDialogue
        );
        assert_eq!(loaded.preferences.theme_mode, ThemeMode::Dark);
    }

    #[tokio::test]
    async fn an_unknown_stored_preference_falls_back_instead_of_locking_the_account_out() {
        let database = database().await;
        let created = database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");

        // Simulates a value written by a newer version, or a hand edit.
        sqlx::query("UPDATE user_preferences SET theme_mode = 'neon', downmix_method = 'quadraphonic' WHERE user_id = ?")
            .bind(created.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("value forced");

        let loaded = database
            .user(created.id)
            .await
            .expect("readable")
            .expect("the account must still load");
        assert_eq!(loaded.preferences.theme_mode, ThemeMode::System);
        assert_eq!(
            loaded.preferences.downmix_method,
            DownmixMethod::BroadcastStandard
        );
    }

    #[tokio::test]
    async fn accounts_are_listed_in_name_order() {
        let database = database().await;
        for name in ["zoe", "Alice", "marc"] {
            database
                .create_user(name, None, &Permissions::viewer())
                .await
                .expect("account created");
        }
        let names: Vec<String> = database
            .list_users()
            .await
            .expect("listed")
            .into_iter()
            .map(|user| user.name)
            .collect();
        assert_eq!(names, vec!["Alice", "marc", "zoe"]);
    }

    #[tokio::test]
    async fn the_languages_offered_are_the_ones_the_library_really_holds() {
        use melyxar_core::id::{MediaSourceId, TrackId};
        use melyxar_core::library::LibraryKind;
        use melyxar_core::media::{
            AudioDetails, Loudness, SubtitleDetails, SubtitleLayout, Track, TrackKind, VideoDetails,
        };
        use melyxar_core::work::WorkKind;
        use std::path::{Path, PathBuf};

        let database = database().await;
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let work = database
            .create_work(
                library.id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        let source_id = database
            .insert_source(
                work.id,
                library.roots[0].id,
                Path::new("Quiet.Harbour.2019.mkv"),
                12_000,
                now(),
            )
            .await
            .expect("source recorded");

        let sound = |source_id: MediaSourceId, index: i32, language: Option<&str>| Track {
            id: TrackId::new(),
            source_id,
            stream_index: index,
            language: language.map(str::to_string),
            title: None,
            is_default: false,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "aac".into(),
                profile: None,
                channels: 2,
                channel_layout: None,
                sample_rate: None,
                bit_depth: None,
                bitrate: None,
                loudness: Loudness::default(),
            }),
        };
        let words = |source_id: MediaSourceId, index: i32, language: &str| Track {
            id: TrackId::new(),
            source_id,
            stream_index: index,
            language: Some(language.to_string()),
            title: None,
            is_default: false,
            is_forced: false,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "subrip".into(),
                layout: SubtitleLayout::Text,
                is_hearing_impaired: false,
                is_external: false,
                external_relative_path: None,
            }),
        };

        database
            .store_analysis(
                source_id,
                &crate::catalogue::SourceAnalysis {
                    container: Some("matroska,webm".into()),
                    duration: None,
                    overall_bitrate: None,
                },
                &[
                    // The picture carries a language in some files, and it is
                    // never something anyone picks.
                    Track {
                        id: TrackId::new(),
                        source_id,
                        stream_index: 0,
                        language: Some("jpn".into()),
                        title: None,
                        is_default: true,
                        is_forced: false,
                        kind: TrackKind::Video(VideoDetails {
                            codec: "h264".into(),
                            profile: None,
                            level: None,
                            width: 1920,
                            height: 1080,
                            margins: None,
                            aspect_ratio: None,
                            is_interlaced: false,
                            frame_rate: None,
                            bitrate: None,
                            pixel_format: None,
                            reference_frames: None,
                            color: Default::default(),
                            hdr: None,
                        }),
                    },
                    sound(source_id, 1, Some("fre")),
                    sound(source_id, 2, Some("eng")),
                    // Said twice on purpose: two soundtracks in one language
                    // are one choice, not two identical lines in a picker.
                    sound(source_id, 3, Some("fre")),
                    // A track whose language nobody wrote down.
                    sound(source_id, 4, None),
                    words(source_id, 5, "fre"),
                ],
                &[],
            )
            .await
            .expect("analysis stored");

        let available = database.languages_in_use().await.expect("read");
        assert_eq!(available.audio, vec!["eng", "fre"]);
        assert_eq!(
            available.subtitle,
            vec!["fre"],
            "a subtitle language is not a soundtrack language: a film can be \
             subtitled in a language nobody speaks in it"
        );
    }

    #[tokio::test]
    async fn an_absent_account_reads_as_absent_rather_than_as_an_error() {
        let database = database().await;
        assert!(database
            .user(UserId::new())
            .await
            .expect("lookup works")
            .is_none());
        assert!(database
            .user_by_name("nobody")
            .await
            .expect("lookup works")
            .is_none());
    }
}
