//! Server settings: one row, read often, written rarely.

use melyxar_core::time::{now, Timestamp};
use sqlx::Row;

use crate::convert::{
    bool_to_int, int_to_bool, parse_optional_timestamp, parse_timestamp, timestamp_to_text,
};
use crate::{Database, Result};

/// Everything the administrator can change about this server.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerSettings {
    pub server_name: String,
    pub logo_path: Option<String>,
    pub splash_path: Option<String>,
    pub login_background_path: Option<String>,
    pub global_custom_css: Option<String>,
    /// Shows the account list before a password is typed. A deliberate
    /// disclosure, so it can be turned off.
    pub show_user_picker: bool,
    pub maintenance_enabled: bool,
    pub maintenance_message: Option<String>,
    pub maintenance_until: Option<Timestamp>,
    /// Read companion metadata files sitting next to a media file.
    pub read_companion_files: bool,
    /// Write them. Needs a writable root, which is checked before anything is
    /// attempted rather than failing file by file.
    pub write_companion_files: bool,
    pub watched_threshold: f64,
    pub activity_retention_days: i64,
    pub check_for_updates: bool,
    pub updated_at: Timestamp,
}

impl Database {
    /// Reads the settings row, which the first migration guarantees exists.
    pub async fn server_settings(&self) -> Result<ServerSettings> {
        let row = sqlx::query(
            "SELECT server_name, logo_path, splash_path, login_background_path,
                    global_custom_css, show_user_picker, maintenance_enabled,
                    maintenance_message, maintenance_until, read_companion_files,
                    write_companion_files, watched_threshold, activity_retention_days,
                    check_for_updates, updated_at
             FROM server_settings WHERE id = 1",
        )
        .fetch_one(self.reader())
        .await?;

        Ok(ServerSettings {
            server_name: row.try_get("server_name")?,
            logo_path: row.try_get("logo_path")?,
            splash_path: row.try_get("splash_path")?,
            login_background_path: row.try_get("login_background_path")?,
            global_custom_css: row.try_get("global_custom_css")?,
            show_user_picker: int_to_bool(row.try_get("show_user_picker")?),
            maintenance_enabled: int_to_bool(row.try_get("maintenance_enabled")?),
            maintenance_message: row.try_get("maintenance_message")?,
            maintenance_until: parse_optional_timestamp(
                row.try_get::<Option<String>, _>("maintenance_until")?
                    .as_deref(),
            )?,
            read_companion_files: int_to_bool(row.try_get("read_companion_files")?),
            write_companion_files: int_to_bool(row.try_get("write_companion_files")?),
            watched_threshold: row.try_get("watched_threshold")?,
            activity_retention_days: row.try_get("activity_retention_days")?,
            check_for_updates: int_to_bool(row.try_get("check_for_updates")?),
            updated_at: parse_timestamp(&row.try_get::<String, _>("updated_at")?)?,
        })
    }

    /// Replaces the settings row.
    pub async fn save_server_settings(&self, settings: &ServerSettings) -> Result<()> {
        sqlx::query(
            "UPDATE server_settings SET
                server_name = ?, logo_path = ?, splash_path = ?, login_background_path = ?,
                global_custom_css = ?, show_user_picker = ?, maintenance_enabled = ?,
                maintenance_message = ?, maintenance_until = ?, read_companion_files = ?,
                write_companion_files = ?, watched_threshold = ?, activity_retention_days = ?,
                check_for_updates = ?, updated_at = ?
             WHERE id = 1",
        )
        .bind(&settings.server_name)
        .bind(&settings.logo_path)
        .bind(&settings.splash_path)
        .bind(&settings.login_background_path)
        .bind(&settings.global_custom_css)
        .bind(bool_to_int(settings.show_user_picker))
        .bind(bool_to_int(settings.maintenance_enabled))
        .bind(&settings.maintenance_message)
        .bind(settings.maintenance_until.map(timestamp_to_text))
        .bind(bool_to_int(settings.read_companion_files))
        .bind(bool_to_int(settings.write_companion_files))
        .bind(settings.watched_threshold)
        .bind(settings.activity_retention_days)
        .bind(bool_to_int(settings.check_for_updates))
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Turns maintenance on or off, along with its message.
    ///
    /// A dedicated call rather than a full save, because it is the one setting
    /// changed in a hurry and it must not carry stale neighbours with it.
    pub async fn set_maintenance(
        &self,
        enabled: bool,
        message: Option<&str>,
        until: Option<Timestamp>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE server_settings
             SET maintenance_enabled = ?, maintenance_message = ?, maintenance_until = ?, updated_at = ?
             WHERE id = 1",
        )
        .bind(bool_to_int(enabled))
        .bind(message)
        .bind(until.map(timestamp_to_text))
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::user::DEFAULT_ACCENT_COLOR;

    #[tokio::test]
    async fn a_fresh_server_starts_with_usable_defaults() {
        let database = Database::open_in_memory().await.expect("database opens");
        let settings = database.server_settings().await.expect("settings readable");

        assert_eq!(settings.server_name, "Melyxar");
        assert!(!settings.maintenance_enabled);
        assert!(
            !settings.write_companion_files,
            "nothing is ever written next to media unless asked"
        );
        assert!(
            !settings.read_companion_files,
            "reading companion files is opt in too"
        );
        assert!(settings.show_user_picker);
        assert_eq!(settings.watched_threshold, 0.9);
        // The accent colour lives in per-user preferences, not here, but the
        // default must match the one the domain declares.
        assert_eq!(DEFAULT_ACCENT_COLOR, "#c81e1e");
    }

    #[tokio::test]
    async fn settings_survive_a_round_trip() {
        let database = Database::open_in_memory().await.expect("database opens");
        let mut settings = database.server_settings().await.expect("settings readable");

        settings.server_name = "Salon".into();
        settings.logo_path = Some("uploads/logo.png".into());
        settings.show_user_picker = false;
        settings.read_companion_files = true;
        settings.activity_retention_days = 90;

        database
            .save_server_settings(&settings)
            .await
            .expect("settings saved");

        let reloaded = database.server_settings().await.expect("settings readable");
        assert_eq!(reloaded.server_name, "Salon");
        assert_eq!(reloaded.logo_path.as_deref(), Some("uploads/logo.png"));
        assert!(!reloaded.show_user_picker);
        assert!(reloaded.read_companion_files);
        assert_eq!(reloaded.activity_retention_days, 90);
    }

    #[tokio::test]
    async fn maintenance_is_switched_on_its_own_without_touching_neighbours() {
        let database = Database::open_in_memory().await.expect("database opens");
        let mut settings = database.server_settings().await.expect("settings readable");
        settings.server_name = "Salon".into();
        database
            .save_server_settings(&settings)
            .await
            .expect("settings saved");

        database
            .set_maintenance(true, Some("Back in ten minutes"), None)
            .await
            .expect("maintenance set");

        let reloaded = database.server_settings().await.expect("settings readable");
        assert!(reloaded.maintenance_enabled);
        assert_eq!(
            reloaded.maintenance_message.as_deref(),
            Some("Back in ten minutes")
        );
        assert_eq!(
            reloaded.server_name, "Salon",
            "switching maintenance must not carry stale neighbours"
        );

        database
            .set_maintenance(false, None, None)
            .await
            .expect("maintenance cleared");
        let cleared = database.server_settings().await.expect("settings readable");
        assert!(!cleared.maintenance_enabled);
        assert!(cleared.maintenance_message.is_none());
    }
}
