//! Server settings: one row, read often, written rarely.

use melyxar_core::time::{now, Timestamp};
use sqlx::Row;

use crate::convert::{
    bool_to_int, int_to_bool, parse_optional_timestamp, parse_timestamp, timestamp_to_text,
};
use crate::{Database, Result};

/// What is drawn behind the sign in screen when no picture was put there.
///
/// A picture an administrator uploaded wins over either of these, which is why
/// this says nothing about one: there is nothing left to choose once somebody
/// has said what they want behind their own door.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoginBackground {
    /// Light and dust in the accent colour, moving slowly. What a server
    /// wears out of the box.
    #[default]
    Abstract,
    /// The shelf of drawn things a media server holds: a poster, a sleeve, an
    /// episode, a reel of film, a player bar, a note, a waveform, a
    /// clapperboard.
    Library,
}

impl LoginBackground {
    /// As the column holds it, and as the sign in screen is told it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Abstract => "abstract",
            Self::Library => "library",
        }
    }

    /// Back from the word. Anything this server does not know is the default:
    /// a background is not worth refusing to start over.
    pub fn from_word(stored: &str) -> Self {
        match stored {
            "library" => Self::Library,
            _ => Self::Abstract,
        }
    }
}

/// Everything the administrator can change about this server.
#[derive(Debug, Clone, PartialEq)]
pub struct ServerSettings {
    pub server_name: String,
    pub logo_path: Option<String>,
    pub splash_path: Option<String>,
    /// A picture behind the sign in screen, which wins over the drawn one.
    pub login_background_path: Option<String>,
    /// Which drawn background is worn when there is no picture.
    pub login_background: LoginBackground,
    pub global_custom_css: Option<String>,
    /// Shows the account list before a password is typed. A deliberate
    /// disclosure, so it can be turned off.
    pub show_user_picker: bool,
    pub maintenance_enabled: bool,
    pub maintenance_message: Option<String>,
    pub maintenance_until: Option<Timestamp>,
    /// Write companion metadata files next to a media file. Needs a writable
    /// root, which is checked before anything is attempted rather than failing
    /// file by file. Reading them is a setting of the work below.
    pub write_companion_files: bool,
    pub watched_threshold: f64,
    pub activity_retention_days: i64,
    pub check_for_updates: bool,
    /// Never convert wide gamut colour for a viewer who cannot show it,
    /// everywhere on this server. Off by default: the conversion is the right
    /// answer on its own, and this exists for a processor too slow to keep up
    /// with what it costs. Dolby Vision without a compatible base layer is
    /// converted regardless, since left alone it looks broken rather than
    /// merely washed out.
    pub tone_mapping_disabled: bool,
    /// What the server does to a library on its own, and in what shape.
    pub work: LibraryWork,
    pub updated_at: Timestamp,
}

/// What the server does with the films of a library, and when.
///
/// Kept together because they are one screen and one answer: how deeply a scan
/// reads what sits next to a film, what the thumbnails of the playback bar look
/// like, and when the upkeep that makes them runs. All three used to live in
/// the configuration file, which meant a terminal and a restart to change one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryWork {
    /// Read the description files some collections keep next to a film.
    ///
    /// Off by default: such a file may hold anything, and a server that
    /// believes it without being asked is a server that takes a stranger's
    /// word over a provider's. Only identifiers are ever taken from one.
    pub read_companion_files: bool,
    /// Whether the little pictures of the playback bar are made at all.
    pub thumbnails_enabled: bool,
    /// How far apart in the film two of them stand.
    pub thumbnails_every_seconds: i64,
    /// Height of one, in pixels. The width follows the film's shape.
    pub thumbnails_height: i64,
    /// How many stand on one sheet.
    pub thumbnails_columns: i64,
    pub thumbnails_rows: i64,
    /// Whether the upkeep runs on its own of a night.
    pub upkeep_nightly: bool,
    /// When it does, in minutes since midnight, UTC.
    ///
    /// UTC because it is the only clock a server can read with certainty, and
    /// minutes because an offset is not always a whole hour. Whatever shows it
    /// turns it into the time of whoever is looking.
    pub upkeep_at_utc_minutes: i64,
}

/// Minutes in a day, which is the one thing a time of day has to stay inside.
const MINUTES_IN_A_DAY: i64 = 24 * 60;

impl LibraryWork {
    /// The same settings with every value brought back into a range that can
    /// work.
    ///
    /// Brought back rather than refused: a screen sending nonsense is a screen
    /// with a defect, and answering it with the nearest thing that works keeps
    /// a server running while somebody fixes the screen. A shape with a nought
    /// in it would mean films read for ever and no picture to show for it.
    pub fn brought_into_range(self) -> Self {
        Self {
            thumbnails_every_seconds: self.thumbnails_every_seconds.clamp(1, 600),
            thumbnails_height: self.thumbnails_height.clamp(1, 1080),
            thumbnails_columns: self.thumbnails_columns.clamp(1, 20),
            thumbnails_rows: self.thumbnails_rows.clamp(1, 20),
            upkeep_at_utc_minutes: self.upkeep_at_utc_minutes.rem_euclid(MINUTES_IN_A_DAY),
            ..self
        }
    }
}

impl Database {
    /// Reads the settings row, which the first migration guarantees exists.
    pub async fn server_settings(&self) -> Result<ServerSettings> {
        let row = sqlx::query(
            "SELECT server_name, logo_path, splash_path, login_background_path,
                    login_background_style, global_custom_css, show_user_picker,
                    maintenance_enabled, maintenance_message, maintenance_until,
                    read_companion_files, write_companion_files, watched_threshold,
                    activity_retention_days, check_for_updates, tone_mapping_disabled,
                    thumbnails_enabled,
                    thumbnails_every_seconds, thumbnails_height, thumbnails_columns,
                    thumbnails_rows, upkeep_nightly, upkeep_at_utc_minutes, updated_at
             FROM server_settings WHERE id = 1",
        )
        .fetch_one(self.reader())
        .await?;

        Ok(ServerSettings {
            server_name: row.try_get("server_name")?,
            logo_path: row.try_get("logo_path")?,
            splash_path: row.try_get("splash_path")?,
            login_background_path: row.try_get("login_background_path")?,
            login_background: LoginBackground::from_word(
                &row.try_get::<String, _>("login_background_style")?,
            ),
            global_custom_css: row.try_get("global_custom_css")?,
            show_user_picker: int_to_bool(row.try_get("show_user_picker")?),
            maintenance_enabled: int_to_bool(row.try_get("maintenance_enabled")?),
            maintenance_message: row.try_get("maintenance_message")?,
            maintenance_until: parse_optional_timestamp(
                row.try_get::<Option<String>, _>("maintenance_until")?
                    .as_deref(),
            )?,
            write_companion_files: int_to_bool(row.try_get("write_companion_files")?),
            watched_threshold: row.try_get("watched_threshold")?,
            activity_retention_days: row.try_get("activity_retention_days")?,
            check_for_updates: int_to_bool(row.try_get("check_for_updates")?),
            tone_mapping_disabled: int_to_bool(row.try_get("tone_mapping_disabled")?),
            work: LibraryWork {
                read_companion_files: int_to_bool(row.try_get("read_companion_files")?),
                thumbnails_enabled: int_to_bool(row.try_get("thumbnails_enabled")?),
                thumbnails_every_seconds: row.try_get("thumbnails_every_seconds")?,
                thumbnails_height: row.try_get("thumbnails_height")?,
                thumbnails_columns: row.try_get("thumbnails_columns")?,
                thumbnails_rows: row.try_get("thumbnails_rows")?,
                upkeep_nightly: int_to_bool(row.try_get("upkeep_nightly")?),
                upkeep_at_utc_minutes: row.try_get("upkeep_at_utc_minutes")?,
            },
            updated_at: parse_timestamp(&row.try_get::<String, _>("updated_at")?)?,
        })
    }

    /// Replaces the settings row.
    pub async fn save_server_settings(&self, settings: &ServerSettings) -> Result<()> {
        sqlx::query(
            "UPDATE server_settings SET
                server_name = ?, logo_path = ?, splash_path = ?, login_background_path = ?,
                login_background_style = ?, global_custom_css = ?, show_user_picker = ?,
                maintenance_enabled = ?,
                maintenance_message = ?, maintenance_until = ?, read_companion_files = ?,
                write_companion_files = ?, watched_threshold = ?, activity_retention_days = ?,
                check_for_updates = ?, tone_mapping_disabled = ?, updated_at = ?
             WHERE id = 1",
        )
        .bind(&settings.server_name)
        .bind(&settings.logo_path)
        .bind(&settings.splash_path)
        .bind(&settings.login_background_path)
        .bind(settings.login_background.as_str())
        .bind(&settings.global_custom_css)
        .bind(bool_to_int(settings.show_user_picker))
        .bind(bool_to_int(settings.maintenance_enabled))
        .bind(&settings.maintenance_message)
        .bind(settings.maintenance_until.map(timestamp_to_text))
        .bind(bool_to_int(settings.work.read_companion_files))
        .bind(bool_to_int(settings.write_companion_files))
        .bind(settings.watched_threshold)
        .bind(settings.activity_retention_days)
        .bind(bool_to_int(settings.check_for_updates))
        .bind(bool_to_int(settings.tone_mapping_disabled))
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Whether wide gamut colour is ever converted for a viewer who cannot
    /// show it, everywhere on this server.
    ///
    /// Read on every single film played, so its own small query rather than
    /// the whole settings row: the row also carries branding and maintenance
    /// text nobody needs to answer this.
    pub async fn tone_mapping_disabled(&self) -> Result<bool> {
        let value: i64 =
            sqlx::query_scalar("SELECT tone_mapping_disabled FROM server_settings WHERE id = 1")
                .fetch_one(self.reader())
                .await?;
        Ok(int_to_bool(value))
    }

    /// Turns the conversion of wide gamut colour on or off for the whole
    /// server.
    ///
    /// A dedicated call rather than a full save, for the same reason as
    /// maintenance below: this is one switch on one screen, and a screen that
    /// wrote the whole row back would carry with it whatever somebody else had
    /// changed since it opened.
    pub async fn set_tone_mapping_disabled(&self, disabled: bool) -> Result<()> {
        sqlx::query(
            "UPDATE server_settings SET tone_mapping_disabled = ?, updated_at = ? WHERE id = 1",
        )
        .bind(bool_to_int(disabled))
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// How many days the activity journal keeps, on its own for the same
    /// reason as the switch above.
    pub async fn set_activity_retention_days(&self, days: i64) -> Result<()> {
        sqlx::query(
            "UPDATE server_settings SET activity_retention_days = ?, updated_at = ? WHERE id = 1",
        )
        .bind(days)
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Replaces what the server does with a library, and nothing else.
    ///
    /// A call of its own rather than a full save, for the same reason as
    /// maintenance below: this is one screen with one button, and a screen
    /// that wrote the whole row back would carry with it whatever somebody
    /// else had changed since it was opened.
    ///
    /// Every value is brought into a range that can work on the way in, so a
    /// screen with a defect cannot leave a server making thumbnails every
    /// nought seconds.
    pub async fn save_library_work(&self, work: LibraryWork) -> Result<LibraryWork> {
        let work = work.brought_into_range();
        sqlx::query(
            "UPDATE server_settings SET
                read_companion_files = ?, thumbnails_enabled = ?,
                thumbnails_every_seconds = ?, thumbnails_height = ?,
                thumbnails_columns = ?, thumbnails_rows = ?,
                upkeep_nightly = ?, upkeep_at_utc_minutes = ?, updated_at = ?
             WHERE id = 1",
        )
        .bind(bool_to_int(work.read_companion_files))
        .bind(bool_to_int(work.thumbnails_enabled))
        .bind(work.thumbnails_every_seconds)
        .bind(work.thumbnails_height)
        .bind(work.thumbnails_columns)
        .bind(work.thumbnails_rows)
        .bind(bool_to_int(work.upkeep_nightly))
        .bind(work.upkeep_at_utc_minutes)
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(work)
    }

    /// What the server does with a library, on its own.
    ///
    /// Read far more often than the rest of the settings, since every reading
    /// of a film asks it what shape to make things in.
    pub async fn library_work(&self) -> Result<LibraryWork> {
        Ok(self.server_settings().await?.work)
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
    async fn how_long_the_activity_journal_keeps_is_written_on_its_own() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(
            database.server_settings().await.expect("read").activity_retention_days,
            180
        );
        database
            .set_activity_retention_days(30)
            .await
            .expect("written");
        assert_eq!(
            database.server_settings().await.expect("read").activity_retention_days,
            30
        );
    }

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
            !settings.work.read_companion_files,
            "reading companion files is opt in too"
        );
        // What the configuration file used to carry, so a server coming up on
        // this migration behaves exactly as it did the moment before.
        assert!(settings.work.thumbnails_enabled);
        assert_eq!(settings.work.thumbnails_every_seconds, 10);
        assert_eq!(settings.work.thumbnails_height, 180);
        assert_eq!(settings.work.thumbnails_columns, 10);
        assert_eq!(settings.work.thumbnails_rows, 10);
        assert!(settings.work.upkeep_nightly);
        assert_eq!(
            settings.work.upkeep_at_utc_minutes,
            3 * 60,
            "three in the morning, the hour the other servers settle on for the same work"
        );
        assert!(settings.show_user_picker);
        // Nothing of the branding is set until an administrator sets it: the
        // screen falls back on what this server ships with, and what it ships
        // with lives with the screen rather than in a row of the database.
        assert_eq!(settings.logo_path, None);
        assert_eq!(settings.login_background_path, None);
        assert_eq!(settings.login_background, LoginBackground::Abstract);
        assert_eq!(settings.watched_threshold, 0.9);
        assert!(
            !settings.tone_mapping_disabled,
            "the automatic rule is the right answer on its own"
        );
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
        settings.login_background = LoginBackground::Library;
        settings.show_user_picker = false;
        settings.work.read_companion_files = true;
        settings.activity_retention_days = 90;
        settings.tone_mapping_disabled = true;

        database
            .save_server_settings(&settings)
            .await
            .expect("settings saved");

        let reloaded = database.server_settings().await.expect("settings readable");
        assert_eq!(reloaded.server_name, "Salon");
        assert_eq!(reloaded.logo_path.as_deref(), Some("uploads/logo.png"));
        assert_eq!(reloaded.login_background, LoginBackground::Library);
        assert!(!reloaded.show_user_picker);
        assert!(reloaded.work.read_companion_files);
        assert_eq!(reloaded.activity_retention_days, 90);
        assert!(reloaded.tone_mapping_disabled);
    }

    /// A word nobody here knows is read as the one a fresh server wears.
    ///
    /// Reached by a database edited by hand, or by one written by a newer
    /// version of this server and opened by an older one. Neither is worth
    /// refusing to start over: what is at stake is which picture moves behind
    /// a sign in screen.
    #[test]
    fn a_background_this_server_does_not_know_is_the_one_it_ships_with() {
        use LoginBackground::{Abstract, Library};
        assert_eq!(LoginBackground::from_word("abstract"), Abstract);
        assert_eq!(LoginBackground::from_word("library"), Library);
        assert_eq!(LoginBackground::from_word("aurora"), Abstract);
        assert_eq!(LoginBackground::from_word(""), Abstract);

        // And back out again as the same word, or a screen would be told one
        // thing and the database would hold another.
        assert_eq!(LoginBackground::Abstract.as_str(), "abstract");
        assert_eq!(LoginBackground::Library.as_str(), "library");
    }

    #[tokio::test]
    async fn what_the_server_does_with_a_library_is_written_on_its_own() {
        // One screen, one button. A screen that wrote the whole row back would
        // carry with it whatever somebody else had changed since it opened.
        let database = Database::open_in_memory().await.expect("database opens");
        let mut settings = database.server_settings().await.expect("settings readable");
        settings.server_name = "Salon".into();
        database
            .save_server_settings(&settings)
            .await
            .expect("settings saved");

        let kept = database
            .save_library_work(LibraryWork {
                read_companion_files: true,
                thumbnails_enabled: true,
                thumbnails_every_seconds: 5,
                thumbnails_height: 240,
                thumbnails_columns: 8,
                thumbnails_rows: 8,
                upkeep_nightly: false,
                upkeep_at_utc_minutes: 90,
            })
            .await
            .expect("work saved");

        assert_eq!(kept, database.library_work().await.expect("read back"));
        assert_eq!(kept.thumbnails_every_seconds, 5);
        assert!(!kept.upkeep_nightly);
        assert_eq!(kept.upkeep_at_utc_minutes, 90);
        assert_eq!(
            database
                .server_settings()
                .await
                .expect("settings readable")
                .server_name,
            "Salon",
            "writing what the server does must not carry stale neighbours"
        );
    }

    #[tokio::test]
    async fn a_shape_that_cannot_work_is_brought_back_rather_than_refused() {
        // A nought anywhere in the shape means every film read for ever with
        // no picture to show for it, and a time of day outside a day means an
        // upkeep that never comes round. A screen with a defect must not be
        // able to leave a server in either state.
        let database = Database::open_in_memory().await.expect("database opens");
        let kept = database
            .save_library_work(LibraryWork {
                thumbnails_every_seconds: 0,
                thumbnails_height: 0,
                thumbnails_columns: 0,
                thumbnails_rows: 0,
                upkeep_at_utc_minutes: -30,
                ..database.library_work().await.expect("read")
            })
            .await
            .expect("work saved");

        assert_eq!(kept.thumbnails_every_seconds, 1);
        assert_eq!(kept.thumbnails_height, 1);
        assert_eq!(kept.thumbnails_columns, 1);
        assert_eq!(kept.thumbnails_rows, 1);
        assert_eq!(
            kept.upkeep_at_utc_minutes,
            23 * 60 + 30,
            "half an hour before midnight, which is what half an hour before              midnight is"
        );
        assert_eq!(kept, database.library_work().await.expect("read back"));
    }

    #[tokio::test]
    async fn never_converting_wide_gamut_colour_is_switched_on_its_own() {
        // One switch on one screen. A save of the whole row would carry with
        // it whatever somebody else had changed since it opened.
        let database = Database::open_in_memory().await.expect("database opens");
        let mut settings = database.server_settings().await.expect("settings readable");
        settings.server_name = "Salon".into();
        database
            .save_server_settings(&settings)
            .await
            .expect("settings saved");

        assert!(!database.tone_mapping_disabled().await.expect("read"));

        database
            .set_tone_mapping_disabled(true)
            .await
            .expect("switched");
        assert!(database.tone_mapping_disabled().await.expect("read"));
        assert_eq!(
            database
                .server_settings()
                .await
                .expect("settings readable")
                .server_name,
            "Salon",
            "switching it must not carry stale neighbours"
        );

        database
            .set_tone_mapping_disabled(false)
            .await
            .expect("switched back");
        assert!(!database.tone_mapping_disabled().await.expect("read"));
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
