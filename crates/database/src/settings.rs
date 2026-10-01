//! Server settings: one row, read often, written rarely.

use melyxar_core::time::{now, Timestamp};
use melyxar_core::user::ThemeMode;
use sqlx::Row;

use crate::convert::{
    bool_to_int, int_to_bool, parse_optional_timestamp, parse_timestamp, timestamp_to_text,
};
use crate::{Database, Result};

/// What stands behind the sign in screen: one of the drawn backgrounds, or a
/// picture the administrator sent.
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
    /// The picture the administrator sent. The picture is kept apart, so a
    /// drawn background chosen for a while does not throw it away; until one
    /// is sent, the default is drawn in its place.
    Picture,
}

impl LoginBackground {
    /// As the column holds it, and as the sign in screen is told it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Abstract => "abstract",
            Self::Library => "library",
            Self::Picture => "picture",
        }
    }

    /// Back from the word. Anything this server does not know is the default:
    /// a background is not worth refusing to start over.
    pub fn from_word(stored: &str) -> Self {
        Self::parse(stored).unwrap_or_default()
    }

    /// The background a word names, and nothing for a word that names none,
    /// for what a screen sends rather than what the row holds.
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "abstract" => Some(Self::Abstract),
            "library" => Some(Self::Library),
            "picture" => Some(Self::Picture),
            _ => None,
        }
    }
}

/// The theme a word names as the server's own, the automatic one for anything
/// else: "follow the server" would be the server following itself.
fn default_theme_of(stored: &str) -> ThemeMode {
    match ThemeMode::parse(stored) {
        Some(theme @ (ThemeMode::Light | ThemeMode::Dark)) => theme,
        _ => ThemeMode::System,
    }
}

/// A file the administrator sent, kept by name in the settings row.
#[derive(Debug, Clone, Copy)]
enum Upload {
    Logo,
    DoorPicture,
}

impl Upload {
    fn read(self) -> &'static str {
        match self {
            Self::Logo => "SELECT logo_path FROM server_settings WHERE id = 1",
            Self::DoorPicture => "SELECT login_background_path FROM server_settings WHERE id = 1",
        }
    }

    fn write(self) -> &'static str {
        match self {
            Self::Logo => "UPDATE server_settings SET logo_path = ?, updated_at = ? WHERE id = 1",
            Self::DoorPicture => {
                "UPDATE server_settings SET login_background_path = ?, updated_at = ? WHERE id = 1"
            }
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
    /// The line under the server's name on the sign in screen, as the
    /// administrator wrote it. Nothing for Melyxar's own, which is worded in
    /// the language of whoever is looking.
    pub door_slogan: Option<String>,
    pub global_custom_css: Option<String>,
    /// The theme of whoever has not chosen one: light, dark or the device's.
    pub default_theme: ThemeMode,
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

impl ServerSettings {
    /// The picture the sign in screen shows: the one sent, while it is the
    /// chosen background. Kept but not chosen, it shows nothing.
    pub fn door_picture_shown(&self) -> Option<&str> {
        (self.login_background == LoginBackground::Picture)
            .then_some(self.login_background_path.as_deref())
            .flatten()
    }
}

/// How far behind each viewer segments stay when nobody chose, in seconds.
pub const USUAL_KEPT_BEHIND: u32 = 300;
/// The least that may be kept: twice the longest step back an account may
/// set, so even two presses in a row land on what is already there.
pub const LEAST_KEPT_BEHIND: u32 = 2 * melyxar_core::user::LONGEST_STEP as u32;
/// The most: past half an hour, keeping is no longer making room at all.
pub const MOST_KEPT_BEHIND: u32 = 1800;
/// The smallest room a cache may be given, in megabytes: a single 4K film
/// being converted fills several hundred megabytes within minutes.
pub const LEAST_CACHE_MEGABYTES: u32 = 1024;

/// Every codec a converted film may come out in, best first.
pub const EVERY_VIDEO_CODEC: [&str; 3] = ["av1", "hevc", "h264"];

/// What the server allows the films it converts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TranscodingLimits {
    /// How many at once; none for no ceiling.
    pub most_at_once: Option<u32>,
    /// How much of the disk their segments may fill, in megabytes; none for
    /// no ceiling.
    pub cache_megabytes: Option<u32>,
    /// How far behind each viewer the segments stay whatever the ceiling says.
    pub kept_behind_seconds: u32,
    /// The codecs a converted film may come out in, best first. Never empty.
    pub video_codecs: Vec<String>,
}

impl TranscodingLimits {
    /// Brought back into range rather than refused: a screen with a defect
    /// must not leave a server refusing every film, or keeping so little
    /// behind a viewer that stepping back waits every time.
    pub fn normalised(self) -> Self {
        Self {
            most_at_once: self.most_at_once.map(|most| most.max(1)),
            cache_megabytes: self
                .cache_megabytes
                .map(|megabytes| megabytes.max(LEAST_CACHE_MEGABYTES)),
            kept_behind_seconds: self
                .kept_behind_seconds
                .clamp(LEAST_KEPT_BEHIND, MOST_KEPT_BEHIND),
            video_codecs: codecs_in_order(&self.video_codecs),
        }
    }
}

/// The known codecs among these, each once and best first, or every one of
/// them when none is left: a server allowed no codec could convert nothing.
fn codecs_in_order(asked: &[String]) -> Vec<String> {
    let known: Vec<String> = EVERY_VIDEO_CODEC
        .iter()
        .filter(|codec| asked.iter().any(|one| one.eq_ignore_ascii_case(codec)))
        .map(|codec| codec.to_string())
        .collect();
    match known.is_empty() {
        true => EVERY_VIDEO_CODEC.iter().map(|codec| codec.to_string()).collect(),
        false => known,
    }
}

/// What the server does with the films of a library, and when.
///
/// Kept together because they are one screen and one answer: how deeply a scan
/// reads what sits next to a film, and what the thumbnails of the playback bar
/// look like. Both used to live in the configuration file, which meant a
/// terminal and a restart to change one. When each task runs is kept with the
/// scheduled tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LibraryWork {
    /// Read the description files some collections keep next to a film.
    ///
    /// Off by default: such a file may hold anything, and a server that
    /// believes it without being asked is a server that takes a stranger's
    /// word over a provider's. Only identifiers are ever taken from one.
    pub read_companion_files: bool,
    /// How far apart in the film two of them stand.
    pub thumbnails_every_seconds: i64,
    /// Height of one, in pixels. The width follows the film's shape.
    pub thumbnails_height: i64,
    /// How many stand on one sheet.
    pub thumbnails_columns: i64,
    pub thumbnails_rows: i64,
}

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
            ..self
        }
    }
}

/// What the server asks OpenSubtitles with: the key of the administrator's
/// application, and their account, a name and a password, when given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenSubtitlesAccount {
    pub key: String,
    pub login: Option<(String, String)>,
}

/// How the server is reached, as it is stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Access {
    pub mode: String,
    pub certificate_path: Option<String>,
    pub private_key_path: Option<String>,
    /// Whether a request in the clear is sent to the encrypted address.
    pub redirect_to_https: bool,
    /// The names the certificate signed by the server carries, separated by
    /// commas.
    pub public_names: Option<String>,
}

impl Database {
    /// Reads the settings row, which the first migration guarantees exists.
    pub async fn server_settings(&self) -> Result<ServerSettings> {
        let row = sqlx::query(
            "SELECT server_name, logo_path, splash_path, login_background_path,
                    login_background_style, door_slogan, global_custom_css, default_theme,
                    show_user_picker, maintenance_enabled, maintenance_message,
                    maintenance_until, read_companion_files, write_companion_files, watched_threshold,
                    activity_retention_days, check_for_updates, tone_mapping_disabled,
                    thumbnails_every_seconds, thumbnails_height, thumbnails_columns,
                    thumbnails_rows, updated_at
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
            door_slogan: row.try_get("door_slogan")?,
            global_custom_css: row.try_get("global_custom_css")?,
            default_theme: default_theme_of(&row.try_get::<String, _>("default_theme")?),
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
                thumbnails_every_seconds: row.try_get("thumbnails_every_seconds")?,
                thumbnails_height: row.try_get("thumbnails_height")?,
                thumbnails_columns: row.try_get("thumbnails_columns")?,
                thumbnails_rows: row.try_get("thumbnails_rows")?,
            },
            updated_at: parse_timestamp(&row.try_get::<String, _>("updated_at")?)?,
        })
    }

    /// Replaces the settings row.
    pub async fn save_server_settings(&self, settings: &ServerSettings) -> Result<()> {
        sqlx::query(
            "UPDATE server_settings SET
                server_name = ?, logo_path = ?, splash_path = ?, login_background_path = ?,
                login_background_style = ?, global_custom_css = ?, default_theme = ?,
                show_user_picker = ?, maintenance_enabled = ?,
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
        .bind(settings.default_theme.as_str())
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

    /// Whether the first steps of a brand new server, taken in the browser
    /// after its first account, are still to be gone through.
    pub async fn first_steps_pending(&self) -> Result<bool> {
        let done: bool =
            sqlx::query_scalar("SELECT first_steps_done FROM server_settings WHERE id = 1")
                .fetch_one(self.reader())
                .await?;
        Ok(!done)
    }

    /// Writes down that the first steps are behind this server, for good.
    pub async fn finish_first_steps(&self) -> Result<()> {
        sqlx::query("UPDATE server_settings SET first_steps_done = 1, updated_at = ? WHERE id = 1")
            .bind(timestamp_to_text(now()))
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// The key OMDb gave the administrator, when one was given. Its own small
    /// query for the same reason as the switch above.
    pub async fn omdb_key(&self) -> Result<Option<String>> {
        Ok(sqlx::query_scalar("SELECT omdb_key FROM server_settings WHERE id = 1")
            .fetch_one(self.reader())
            .await?)
    }

    /// Keeps the key OMDb gave the administrator, or forgets it with nothing.
    pub async fn set_omdb_key(&self, key: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE server_settings SET omdb_key = ?, updated_at = ? WHERE id = 1")
            .bind(key)
            .bind(timestamp_to_text(now()))
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// What the server asks OpenSubtitles with, when the administrator gave
    /// it: the key of their application, and their account when they gave
    /// one.
    pub async fn opensubtitles_account(&self) -> Result<Option<OpenSubtitlesAccount>> {
        let row: (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT opensubtitles_key, opensubtitles_username, opensubtitles_password
               FROM server_settings WHERE id = 1",
        )
        .fetch_one(self.reader())
        .await?;
        Ok(match row {
            (Some(key), username, password) => Some(OpenSubtitlesAccount {
                key,
                login: username.zip(password),
            }),
            (None, _, _) => None,
        })
    }

    /// Keeps what the server asks OpenSubtitles with, or forgets it with
    /// nothing.
    pub async fn set_opensubtitles_account(&self, account: Option<&OpenSubtitlesAccount>) -> Result<()> {
        let (username, password) = account
            .and_then(|account| account.login.as_ref())
            .map(|(username, password)| (Some(username.as_str()), Some(password.as_str())))
            .unwrap_or((None, None));
        sqlx::query(
            "UPDATE server_settings SET opensubtitles_key = ?, opensubtitles_username = ?,
                                        opensubtitles_password = ?, updated_at = ?
              WHERE id = 1",
        )
        .bind(account.map(|account| account.key.as_str()))
        .bind(username)
        .bind(password)
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// What the server allows the films it converts: how many at once, and
    /// how much of the disk their segments may fill.
    ///
    /// Read each time a film is opened, so its own small query for the same
    /// reason as the switch above.
    pub async fn transcoding_limits(&self) -> Result<TranscodingLimits> {
        let row = sqlx::query(
            "SELECT max_transcoding_sessions, transcode_cache_megabytes,
                    transcode_kept_behind_seconds, transcode_video_codecs
             FROM server_settings WHERE id = 1",
        )
        .fetch_one(self.reader())
        .await?;
        let wanted = |column: &str| -> Result<Option<u32>> {
            Ok(row
                .try_get::<Option<i64>, _>(column)?
                .and_then(|value| u32::try_from(value).ok()))
        };
        Ok(TranscodingLimits {
            most_at_once: wanted("max_transcoding_sessions")?,
            cache_megabytes: wanted("transcode_cache_megabytes")?,
            kept_behind_seconds: wanted("transcode_kept_behind_seconds")?
                .unwrap_or(USUAL_KEPT_BEHIND),
            video_codecs: row
                .try_get::<String, _>("transcode_video_codecs")?
                .split(',')
                .map(str::to_string)
                .collect(),
        }
        .normalised())
    }

    /// Writes what the server allows the films it converts, brought back into
    /// range first.
    pub async fn set_transcoding_limits(&self, limits: TranscodingLimits) -> Result<TranscodingLimits> {
        let limits = limits.normalised();
        sqlx::query(
            "UPDATE server_settings SET max_transcoding_sessions = ?,
                    transcode_cache_megabytes = ?, transcode_kept_behind_seconds = ?,
                    transcode_video_codecs = ?, updated_at = ?
             WHERE id = 1",
        )
        .bind(limits.most_at_once.map(i64::from))
        .bind(limits.cache_megabytes.map(i64::from))
        .bind(i64::from(limits.kept_behind_seconds))
        .bind(limits.video_codecs.join(","))
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(limits)
    }

    /// What the server is called, on its own for the same reason as the
    /// switch above.
    pub async fn set_server_name(&self, name: &str) -> Result<()> {
        sqlx::query("UPDATE server_settings SET server_name = ?, updated_at = ? WHERE id = 1")
            .bind(name)
            .bind(timestamp_to_text(now()))
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Gives the server its logo, or takes it away with nothing, and answers
    /// the one it had so the caller can delete its file.
    pub async fn set_logo(&self, logo_path: Option<&str>) -> Result<Option<String>> {
        self.swap_upload(Upload::Logo, logo_path).await
    }

    /// Puts this picture behind the sign in screen, or takes it away, and
    /// answers the one it replaces so its file can be deleted.
    pub async fn set_door_picture(&self, path: Option<&str>) -> Result<Option<String>> {
        self.swap_upload(Upload::DoorPicture, path).await
    }

    /// Which drawn background the sign in screen wears when no picture was
    /// put there.
    pub async fn set_door_background(&self, background: LoginBackground) -> Result<()> {
        sqlx::query(
            "UPDATE server_settings SET login_background_style = ?, updated_at = ? WHERE id = 1",
        )
        .bind(background.as_str())
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// The theme of whoever has not chosen one.
    pub async fn set_default_theme(&self, theme: ThemeMode) -> Result<()> {
        sqlx::query("UPDATE server_settings SET default_theme = ?, updated_at = ? WHERE id = 1")
            .bind(theme.as_str())
            .bind(timestamp_to_text(now()))
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// The line under the server's name on the sign in screen, or nothing for
    /// Melyxar's own.
    pub async fn set_door_slogan(&self, slogan: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE server_settings SET door_slogan = ?, updated_at = ? WHERE id = 1")
            .bind(slogan)
            .bind(timestamp_to_text(now()))
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Writes one file the administrator sent in place of the one before, and
    /// answers that one, read in the same transaction.
    async fn swap_upload(&self, upload: Upload, path: Option<&str>) -> Result<Option<String>> {
        let mut transaction = self.begin().await?;
        let before: Option<String> = sqlx::query_scalar(upload.read())
            .fetch_one(&mut *transaction)
            .await?;
        sqlx::query(upload.write())
            .bind(path)
            .bind(timestamp_to_text(now()))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(before)
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

    /// How the server is reached: its mode, as a word, the paths of a
    /// provided certificate and its key, and its two options.
    pub async fn access(&self) -> Result<Access> {
        let row = sqlx::query(
            "SELECT access_mode, certificate_path, private_key_path, redirect_to_https, public_names
               FROM server_settings WHERE id = 1",
        )
        .fetch_one(self.reader())
        .await?;
        Ok(Access {
            mode: row.try_get("access_mode")?,
            certificate_path: row.try_get("certificate_path")?,
            private_key_path: row.try_get("private_key_path")?,
            redirect_to_https: row.try_get("redirect_to_https")?,
            public_names: row.try_get("public_names")?,
        })
    }

    /// Sets it, on its own for the same reason as the journal's days.
    pub async fn set_access(&self, access: &Access) -> Result<()> {
        sqlx::query(
            "UPDATE server_settings
                SET access_mode = ?, certificate_path = ?, private_key_path = ?,
                    redirect_to_https = ?, public_names = ?, updated_at = ?
              WHERE id = 1",
        )
        .bind(&access.mode)
        .bind(&access.certificate_path)
        .bind(&access.private_key_path)
        .bind(access.redirect_to_https)
        .bind(&access.public_names)
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// The model that listens, by the word the layer above knows it by.
    /// Nothing until the administrator has chosen one.
    pub async fn speech_model(&self) -> Result<Option<String>> {
        Ok(
            sqlx::query_scalar("SELECT speech_model FROM server_settings WHERE id = 1")
                .fetch_one(self.reader())
                .await?,
        )
    }

    /// Chooses the model that listens, or none.
    pub async fn set_speech_model(&self, model: Option<&str>) -> Result<()> {
        sqlx::query("UPDATE server_settings SET speech_model = ?, updated_at = ? WHERE id = 1")
            .bind(model)
            .bind(timestamp_to_text(now()))
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// How much of the processor listening takes, by the word the layer
    /// above knows it by.
    pub async fn speech_effort(&self) -> Result<String> {
        Ok(
            sqlx::query_scalar("SELECT speech_effort FROM server_settings WHERE id = 1")
                .fetch_one(self.reader())
                .await?,
        )
    }

    /// Chooses how much of the processor listening takes.
    pub async fn set_speech_effort(&self, effort: &str) -> Result<()> {
        sqlx::query("UPDATE server_settings SET speech_effort = ?, updated_at = ? WHERE id = 1")
            .bind(effort)
            .bind(timestamp_to_text(now()))
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// How many wrong passwords in a row an account takes before it is
    /// held back.
    pub async fn sign_in_tries(&self) -> Result<i64> {
        Ok(
            sqlx::query_scalar("SELECT sign_in_tries FROM server_settings WHERE id = 1")
                .fetch_one(self.reader())
                .await?,
        )
    }

    /// Sets it, on its own for the same reason as the journal's days.
    pub async fn set_sign_in_tries(&self, tries: i64) -> Result<()> {
        sqlx::query("UPDATE server_settings SET sign_in_tries = ?, updated_at = ? WHERE id = 1")
            .bind(tries)
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
                read_companion_files = ?,
                thumbnails_every_seconds = ?, thumbnails_height = ?,
                thumbnails_columns = ?, thumbnails_rows = ?, updated_at = ?
             WHERE id = 1",
        )
        .bind(bool_to_int(work.read_companion_files))
        .bind(work.thumbnails_every_seconds)
        .bind(work.thumbnails_height)
        .bind(work.thumbnails_columns)
        .bind(work.thumbnails_rows)
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
    async fn the_server_is_renamed_without_touching_its_neighbours() {
        let database = Database::open_in_memory().await.expect("database opens");
        database
            .set_activity_retention_days(30)
            .await
            .expect("written");
        database
            .set_server_name("Home Cinema")
            .await
            .expect("written");
        let settings = database.server_settings().await.expect("read");
        assert_eq!(settings.server_name, "Home Cinema");
        assert_eq!(settings.activity_retention_days, 30);
    }

    #[tokio::test]
    async fn a_brand_new_server_has_its_first_steps_ahead_until_they_are_finished() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert!(database.first_steps_pending().await.expect("read"));
        database.finish_first_steps().await.expect("written");
        assert!(!database.first_steps_pending().await.expect("read"));
    }

    #[tokio::test]
    async fn the_omdb_key_is_kept_and_forgotten() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(database.omdb_key().await.expect("read"), None);
        database.set_omdb_key(Some("abcd1234")).await.expect("kept");
        assert_eq!(database.omdb_key().await.expect("read").as_deref(), Some("abcd1234"));
        database.set_omdb_key(None).await.expect("forgotten");
        assert_eq!(database.omdb_key().await.expect("read"), None);
    }

    #[tokio::test]
    async fn the_opensubtitles_account_is_kept_and_forgotten() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(database.opensubtitles_account().await.expect("read"), None);
        let account = OpenSubtitlesAccount {
            key: "key".to_string(),
            login: Some(("me".to_string(), "secret".to_string())),
        };
        database.set_opensubtitles_account(Some(&account)).await.expect("kept");
        assert_eq!(database.opensubtitles_account().await.expect("read"), Some(account));
        database.set_opensubtitles_account(None).await.expect("forgotten");
        assert_eq!(database.opensubtitles_account().await.expect("read"), None);
    }

    #[tokio::test]
    async fn a_logo_given_answers_the_one_it_replaces() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(database.set_logo(Some("logo-a.webp")).await.expect("given"), None);
        assert_eq!(
            database.set_logo(Some("logo-b.webp")).await.expect("given"),
            Some("logo-a.webp".to_string())
        );
        assert_eq!(
            database.server_settings().await.expect("read").logo_path.as_deref(),
            Some("logo-b.webp")
        );
        assert_eq!(
            database.set_logo(None).await.expect("taken away"),
            Some("logo-b.webp".to_string())
        );
        assert_eq!(database.server_settings().await.expect("read").logo_path, None);
    }

    #[tokio::test]
    async fn the_door_picture_answers_the_one_it_replaces_and_the_background_stays() {
        let database = Database::open_in_memory().await.expect("database opens");
        database
            .set_door_background(LoginBackground::Library)
            .await
            .expect("chosen");
        assert_eq!(
            database.set_door_picture(Some("door-a.webp")).await.expect("given"),
            None
        );
        assert_eq!(
            database.set_door_picture(Some("door-b.webp")).await.expect("given"),
            Some("door-a.webp".to_string())
        );
        let settings = database.server_settings().await.expect("read");
        assert_eq!(settings.login_background_path.as_deref(), Some("door-b.webp"));
        assert_eq!(settings.login_background, LoginBackground::Library);
        assert_eq!(settings.logo_path, None, "the logo is another column");
        assert_eq!(
            database.set_door_picture(None).await.expect("taken away"),
            Some("door-b.webp".to_string())
        );
        let settings = database.server_settings().await.expect("read");
        assert_eq!(settings.login_background_path, None);
        assert_eq!(settings.login_background, LoginBackground::Library);
    }

    #[tokio::test]
    async fn the_door_slogan_is_written_and_given_back() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(database.server_settings().await.expect("read").door_slogan, None);
        database
            .set_door_slogan(Some("Films for the whole house"))
            .await
            .expect("written");
        assert_eq!(
            database.server_settings().await.expect("read").door_slogan.as_deref(),
            Some("Films for the whole house")
        );
        database.set_door_slogan(None).await.expect("given back");
        assert_eq!(database.server_settings().await.expect("read").door_slogan, None);
    }

    #[test]
    fn only_a_known_word_names_a_background() {
        assert_eq!(LoginBackground::parse("library"), Some(LoginBackground::Library));
        assert_eq!(LoginBackground::parse("abstract"), Some(LoginBackground::Abstract));
        assert_eq!(LoginBackground::parse("stars"), None);
        assert_eq!(LoginBackground::from_word("stars"), LoginBackground::Abstract);
    }

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
        assert_eq!(settings.work.thumbnails_every_seconds, 10);
        assert_eq!(settings.work.thumbnails_height, 180);
        assert_eq!(settings.work.thumbnails_columns, 10);
        assert_eq!(settings.work.thumbnails_rows, 10);
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
        settings.default_theme = ThemeMode::Dark;
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
        assert_eq!(reloaded.default_theme, ThemeMode::Dark);
        assert!(reloaded.work.read_companion_files);
        assert_eq!(reloaded.activity_retention_days, 90);
        assert!(reloaded.tone_mapping_disabled);
    }

    #[tokio::test]
    async fn the_default_theme_is_the_devices_until_one_is_chosen_for_the_server() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(
            database.server_settings().await.expect("read").default_theme,
            ThemeMode::System
        );

        database
            .set_default_theme(ThemeMode::Light)
            .await
            .expect("chosen");
        assert_eq!(
            database.server_settings().await.expect("read").default_theme,
            ThemeMode::Light
        );
    }

    #[test]
    fn the_server_never_defaults_to_following_itself() {
        assert_eq!(default_theme_of("dark"), ThemeMode::Dark);
        assert_eq!(default_theme_of("light"), ThemeMode::Light);
        assert_eq!(default_theme_of("system"), ThemeMode::System);
        assert_eq!(default_theme_of("server"), ThemeMode::System);
        assert_eq!(default_theme_of("neon"), ThemeMode::System);
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
        assert_eq!(LoginBackground::Picture.as_str(), "picture");
        assert_eq!(LoginBackground::from_word("picture"), LoginBackground::Picture);
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
                thumbnails_every_seconds: 5,
                thumbnails_height: 240,
                thumbnails_columns: 8,
                thumbnails_rows: 8,
            })
            .await
            .expect("work saved");

        assert_eq!(kept, database.library_work().await.expect("read back"));
        assert_eq!(kept.thumbnails_every_seconds, 5);
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
        // no picture to show for it. A screen with a defect must not be able
        // to leave a server in that state.
        let database = Database::open_in_memory().await.expect("database opens");
        let kept = database
            .save_library_work(LibraryWork {
                thumbnails_every_seconds: 0,
                thumbnails_height: 0,
                thumbnails_columns: 0,
                thumbnails_rows: 0,
                ..database.library_work().await.expect("read")
            })
            .await
            .expect("work saved");

        assert_eq!(kept.thumbnails_every_seconds, 1);
        assert_eq!(kept.thumbnails_height, 1);
        assert_eq!(kept.thumbnails_columns, 1);
        assert_eq!(kept.thumbnails_rows, 1);
        assert_eq!(kept, database.library_work().await.expect("read back"));
    }

    #[tokio::test]
    async fn a_server_converts_without_any_ceiling_until_one_is_set() {
        let database = Database::open_in_memory().await.expect("database opens");
        let usual = database.transcoding_limits().await.expect("read");
        assert_eq!(
            usual,
            TranscodingLimits {
                most_at_once: None,
                cache_megabytes: None,
                kept_behind_seconds: USUAL_KEPT_BEHIND,
                video_codecs: vec!["av1".into(), "hevc".into(), "h264".into()],
            }
        );

        let chosen = TranscodingLimits {
            most_at_once: Some(3),
            cache_megabytes: Some(8192),
            kept_behind_seconds: 600,
            video_codecs: vec!["hevc".into(), "h264".into()],
        };
        assert_eq!(
            database
                .set_transcoding_limits(chosen.clone())
                .await
                .expect("set"),
            chosen
        );
        assert_eq!(database.transcoding_limits().await.expect("read"), chosen);

        database
            .set_transcoding_limits(usual.clone())
            .await
            .expect("taken away");
        assert_eq!(database.transcoding_limits().await.expect("read"), usual);
    }

    #[tokio::test]
    async fn the_door_shows_its_picture_only_while_it_is_the_chosen_background() {
        let database = Database::open_in_memory().await.expect("database opens");
        database.set_door_picture(Some("door.webp")).await.expect("sent");
        database
            .set_door_background(LoginBackground::Library)
            .await
            .expect("chosen");
        let settings = database.server_settings().await.expect("read");
        assert_eq!(settings.door_picture_shown(), None, "kept, not chosen");
        assert_eq!(settings.login_background_path.as_deref(), Some("door.webp"));

        database
            .set_door_background(LoginBackground::Picture)
            .await
            .expect("chosen");
        let settings = database.server_settings().await.expect("read");
        assert_eq!(settings.door_picture_shown(), Some("door.webp"));
    }

    #[test]
    fn codecs_are_kept_once_each_best_first_and_unknown_ones_dropped() {
        let asked: Vec<String> = ["H264", "vp9", "av1", "h264"]
            .iter()
            .map(|codec| codec.to_string())
            .collect();
        assert_eq!(codecs_in_order(&asked), ["av1", "h264"]);
    }

    #[tokio::test]
    async fn limits_that_cannot_work_are_brought_back_into_range() {
        let database = Database::open_in_memory().await.expect("database opens");
        let kept = database
            .set_transcoding_limits(TranscodingLimits {
                most_at_once: Some(0),
                cache_megabytes: Some(1),
                kept_behind_seconds: 5,
                video_codecs: Vec::new(),
            })
            .await
            .expect("set");
        assert_eq!(
            kept,
            TranscodingLimits {
                most_at_once: Some(1),
                cache_megabytes: Some(LEAST_CACHE_MEGABYTES),
                kept_behind_seconds: LEAST_KEPT_BEHIND,
                video_codecs: vec!["av1".into(), "hevc".into(), "h264".into()],
            },
            "a server allowed no codec could convert nothing"
        );
        assert_eq!(database.transcoding_limits().await.expect("read"), kept);
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

    #[tokio::test]
    async fn the_tries_before_an_account_is_held_back_start_at_ten_and_are_kept() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(database.sign_in_tries().await.expect("read"), 10);
        database.set_sign_in_tries(5).await.expect("saved");
        assert_eq!(database.sign_in_tries().await.expect("read"), 5);
    }

    #[tokio::test]
    async fn listening_is_quiet_until_another_effort_is_chosen_and_it_is_kept() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(database.speech_effort().await.expect("read"), "quiet");
        database.set_speech_effort("maximum").await.expect("saved");
        assert_eq!(database.speech_effort().await.expect("read"), "maximum");
        assert!(
            database.set_speech_effort("frantic").await.is_err(),
            "a word the table does not know is refused by the table itself"
        );
    }

    #[tokio::test]
    async fn the_model_that_listens_is_none_until_chosen_and_is_kept() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(database.speech_model().await.expect("read"), None);
        database.set_speech_model(Some("small")).await.expect("saved");
        assert_eq!(database.speech_model().await.expect("read").as_deref(), Some("small"));
        database.set_speech_model(None).await.expect("saved");
        assert_eq!(database.speech_model().await.expect("read"), None);
    }

    #[tokio::test]
    async fn the_way_in_starts_behind_a_proxy_and_is_kept() {
        let database = Database::open_in_memory().await.expect("database opens");
        let access = database.access().await.expect("read");
        assert_eq!(access.mode, "proxy");
        assert_eq!(access.certificate_path, None);
        assert!(access.redirect_to_https);
        assert_eq!(access.public_names, None);

        let provided = Access {
            mode: "provided".to_string(),
            certificate_path: Some("/etc/ssl/a.pem".to_string()),
            private_key_path: Some("/etc/ssl/a.key".to_string()),
            redirect_to_https: false,
            public_names: Some("media.example.org,203.0.113.7".to_string()),
        };
        database.set_access(&provided).await.expect("saved");
        assert_eq!(database.access().await.expect("read"), provided);
    }
}
