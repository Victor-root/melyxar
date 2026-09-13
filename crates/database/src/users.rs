//! Accounts, their rights and their preferences.
//!
//! Present from the first migration even though the first version ships with a
//! single account: every progress row, favourite and preference hangs off a
//! user, and retrofitting that would touch every table and every route.

use melyxar_core::id::{LibraryId, UserId};
use melyxar_core::time::{now, Timestamp};
use melyxar_core::user::{DownmixMethod, Permissions, Preferences, ThemeMode, User};
use sqlx::Row;
use std::str::FromStr;

use crate::convert::{bool_to_int, int_to_bool, parse_timestamp, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

/// The languages a library holds, ready to fill a picker.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AvailableLanguages {
    pub audio: Vec<String>,
    pub subtitle: Vec<String>,
}

impl Database {
    /// Creates an account along with its preferences row.
    pub async fn create_user(
        &self,
        name: &str,
        password_hash: Option<&str>,
        permissions: &Permissions,
    ) -> Result<User> {
        let id = UserId::new();
        let created_at = now();
        let preferences = Preferences::default();

        let mut transaction = self.begin().await?;

        sqlx::query(
            "INSERT INTO users (id, name, password_hash, is_administrator, max_age_rating,
                                may_download, may_delete, may_delete_from_disk, max_sessions, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(name)
        .bind(password_hash)
        .bind(bool_to_int(permissions.is_administrator))
        .bind(permissions.max_age_rating)
        .bind(bool_to_int(permissions.may_download))
        .bind(bool_to_int(permissions.may_delete))
        .bind(bool_to_int(permissions.may_delete_from_disk))
        .bind(permissions.max_sessions)
        .bind(timestamp_to_text(created_at))
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "INSERT INTO user_preferences (user_id, interface_language, theme_mode, accent_color,
                                           volume, downmix_method, downmix_gain)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(&preferences.interface_language)
        .bind(preferences.theme_mode.as_str())
        .bind(&preferences.accent_color)
        .bind(preferences.volume)
        .bind(preferences.downmix_method.as_str())
        .bind(preferences.downmix_gain)
        .execute(&mut *transaction)
        .await?;

        for library in &permissions.allowed_libraries {
            sqlx::query("INSERT INTO user_library_access (user_id, library_id) VALUES (?, ?)")
                .bind(id.to_db_string())
                .bind(library.to_db_string())
                .execute(&mut *transaction)
                .await?;
        }

        transaction.commit().await?;

        Ok(User {
            id,
            name: name.to_string(),
            avatar_path: None,
            permissions: permissions.clone(),
            preferences,
            created_at,
        })
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
        let Some(row) = sqlx::query(
            "SELECT u.id, u.name, u.avatar_path, u.is_administrator, u.max_age_rating,
                    u.may_download, u.may_delete, u.may_delete_from_disk, u.max_sessions,
                    u.created_at,
                    p.interface_language, p.preferred_audio_language, p.preferred_subtitle_language,
                    p.theme_mode, p.accent_color, p.custom_css, p.volume,
                    p.downmix_method, p.downmix_gain
             FROM users u
             JOIN user_preferences p ON p.user_id = u.id
             WHERE u.id = ?",
        )
        .bind(id.to_db_string())
        .fetch_optional(self.reader())
        .await?
        else {
            return Ok(None);
        };

        let allowed: Vec<(String,)> =
            sqlx::query_as("SELECT library_id FROM user_library_access WHERE user_id = ?")
                .bind(id.to_db_string())
                .fetch_all(self.reader())
                .await?;

        Ok(Some(build_user(&row, &allowed)?))
    }

    /// Loads an account by name, for signing in. The comparison ignores case,
    /// because nobody remembers how they capitalised their own name.
    pub async fn user_by_name(&self, name: &str) -> Result<Option<(User, Option<String>)>> {
        let Some(row) = sqlx::query(
            "SELECT u.id, u.name, u.avatar_path, u.is_administrator, u.max_age_rating,
                    u.may_download, u.may_delete, u.may_delete_from_disk, u.max_sessions,
                    u.created_at, u.password_hash,
                    p.interface_language, p.preferred_audio_language, p.preferred_subtitle_language,
                    p.theme_mode, p.accent_color, p.custom_css, p.volume,
                    p.downmix_method, p.downmix_gain
             FROM users u
             JOIN user_preferences p ON p.user_id = u.id
             WHERE u.name = ? COLLATE NOCASE",
        )
        .bind(name)
        .fetch_optional(self.reader())
        .await?
        else {
            return Ok(None);
        };

        let id = parse_id::<UserId>(&row.try_get::<String, _>("id")?)?;
        let allowed: Vec<(String,)> =
            sqlx::query_as("SELECT library_id FROM user_library_access WHERE user_id = ?")
                .bind(id.to_db_string())
                .fetch_all(self.reader())
                .await?;

        let password_hash: Option<String> = row.try_get("password_hash")?;
        Ok(Some((build_user(&row, &allowed)?, password_hash)))
    }

    /// Every account, ordered by name. Small by nature: this is a home server.
    pub async fn list_users(&self) -> Result<Vec<User>> {
        let rows = sqlx::query(
            "SELECT u.id, u.name, u.avatar_path, u.is_administrator, u.max_age_rating,
                    u.may_download, u.may_delete, u.may_delete_from_disk, u.max_sessions,
                    u.created_at,
                    p.interface_language, p.preferred_audio_language, p.preferred_subtitle_language,
                    p.theme_mode, p.accent_color, p.custom_css, p.volume,
                    p.downmix_method, p.downmix_gain
             FROM users u
             JOIN user_preferences p ON p.user_id = u.id
             ORDER BY u.name COLLATE NOCASE",
        )
        .fetch_all(self.reader())
        .await?;

        let mut users = Vec::with_capacity(rows.len());
        for row in &rows {
            let id = parse_id::<UserId>(&row.try_get::<String, _>("id")?)?;
            let allowed: Vec<(String,)> =
                sqlx::query_as("SELECT library_id FROM user_library_access WHERE user_id = ?")
                    .bind(id.to_db_string())
                    .fetch_all(self.reader())
                    .await?;
            users.push(build_user(row, &allowed)?);
        }
        Ok(users)
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

    /// Saves the preferences of one account, after clamping the values a
    /// client may have sent out of range.
    pub async fn save_preferences(&self, id: UserId, preferences: &Preferences) -> Result<()> {
        let preferences = preferences.clone().normalised();
        sqlx::query(
            "UPDATE user_preferences SET
                interface_language = ?, preferred_audio_language = ?,
                preferred_subtitle_language = ?, theme_mode = ?, accent_color = ?,
                custom_css = ?, volume = ?, downmix_method = ?, downmix_gain = ?
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
        .bind(id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }
}

fn parse_id<T: FromStr>(value: &str) -> Result<T> {
    value
        .parse()
        .map_err(|_| DatabaseError::Corrupt(format!("identifier '{value}' is malformed")))
}

/// Builds a domain account out of a row and its granted libraries.
///
/// A stored value that no longer maps to a known variant falls back to the
/// default rather than failing the whole read: an account that cannot load
/// locks its owner out, which is a far worse outcome than a reset preference.
fn build_user(row: &sqlx::sqlite::SqliteRow, allowed: &[(String,)]) -> Result<User> {
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
        }
        .normalised(),
        created_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn database() -> Database {
        Database::open_in_memory().await.expect("database opens")
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
