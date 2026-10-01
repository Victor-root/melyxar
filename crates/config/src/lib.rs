//! Server configuration.
//!
//! Everything that depends on the machine lives here: where data goes, which
//! port to listen on, where the processing tool is, and how much work may run
//! at once. Real paths and keys live in this file on the server, never in the
//! repository.

#![forbid(unsafe_code)]

use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Default listening port.
///
/// Checked against the official registry: the number carries only a historic
/// assignment with no real use today. It stays configurable, and the installer
/// verifies it is free before continuing.
pub const DEFAULT_PORT: u16 = 2100;

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("configuration file not found at {0}")]
    NotFound(PathBuf),
    #[error("configuration file could not be read: {0}")]
    Unreadable(#[from] std::io::Error),
    #[error("configuration file is malformed: {0}")]
    Malformed(#[from] toml::de::Error),
    #[error("configuration is invalid: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, ConfigError>;

/// Where everything is written, and where the interface is read from.
///
/// Three separate directories are written on purpose: what must be backed
/// up, what can be regenerated, and what is throwaway. None of them ever sits
/// inside a media folder, because media disks may be mounted read only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Directories {
    /// Database, uploaded branding files. Backed up.
    pub data: PathBuf,
    /// Resized images. Regenerable, so never backed up.
    pub cache: PathBuf,
    /// Segments produced while streaming. Throwaway, quota applies, and a
    /// memory backed filesystem suits it well.
    pub transcodes: PathBuf,
    /// The built interface, which the server only reads. Put there by the
    /// installer, and served from the next request on: a new interface needs
    /// no restart. Filled in when absent, so a file written before it existed
    /// still starts the server.
    #[serde(default = "default_interface")]
    pub interface: PathBuf,
}

fn default_interface() -> PathBuf {
    PathBuf::from("/usr/local/share/melyxar/web")
}

impl Default for Directories {
    fn default() -> Self {
        Self {
            data: PathBuf::from("/var/lib/melyxar"),
            cache: PathBuf::from("/var/cache/melyxar"),
            transcodes: PathBuf::from("/var/cache/melyxar/transcodes"),
            interface: default_interface(),
        }
    }
}

impl Directories {
    /// Path of the database file.
    pub fn database_file(&self) -> PathBuf {
        self.data.join("melyxar.db")
    }

    /// Directory holding files uploaded by the administrator, such as the
    /// logo. Sits in the data directory because it cannot be regenerated.
    pub fn uploads(&self) -> PathBuf {
        self.data.join("uploads")
    }

    /// Directory holding the picture each account chose for itself. In the
    /// data directory: it is made from an image nobody kept, so it cannot be
    /// made again.
    pub fn avatars(&self) -> PathBuf {
        self.data.join("avatars")
    }

    /// Directory holding generated images.
    pub fn images(&self) -> PathBuf {
        self.cache.join("images")
    }

    /// Directory holding subtitles converted to the form a browser draws.
    ///
    /// In the cache rather than with the film: the media folders are read only
    /// as far as this server is concerned, and a converted subtitle is made
    /// again in a moment from the file it came from.
    pub fn subtitles(&self) -> PathBuf {
        self.cache.join("subtitles")
    }

    /// Directory holding the subtitles downloaded for a film. In the data
    /// directory: each one cost one of a day's few downloads, and it is not
    /// made again from anything the server holds.
    pub fn downloaded_subtitles(&self) -> PathBuf {
        self.data.join("subtitles")
    }

    /// Directory holding the sheets of thumbnails shown on the playback bar.
    ///
    /// In the cache with everything else read out of a film: they are made
    /// again from the film in a single reading.
    pub fn thumbnails(&self) -> PathBuf {
        self.cache.join("thumbnails")
    }

    /// Directory holding the reference film a client is calibrated against.
    ///
    /// In the cache, like the pictures made from a real film: nothing here
    /// belongs to a library, and all of it is made again from nothing the
    /// moment it is missing.
    pub fn calibration(&self) -> PathBuf {
        self.cache.join("calibration")
    }

    pub fn backups(&self) -> PathBuf {
        self.data.join("backups")
    }

    /// The certificate this server signs itself, and its key.
    pub fn tls(&self) -> PathBuf {
        self.data.join("tls")
    }

    /// The models that listen, downloaded by the administrator. In the data
    /// directory: the biggest is three gigabytes, and nothing here makes it
    /// again but a download.
    pub fn speech_models(&self) -> PathBuf {
        self.data.join("speech-models")
    }

    /// Where a video is listened to: the recording of its sound is made here
    /// for the length of the reading and removed with it.
    pub fn speech_scratch(&self) -> PathBuf {
        self.cache.join("speech")
    }
}

/// Where the external processing tools are and how they behave.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaToolsConfig {
    /// Path of the encoder binary. Left empty to search the usual locations.
    #[serde(default)]
    pub ffmpeg_path: Option<PathBuf>,
    /// Path of the analyser binary. Left empty to derive it from the encoder.
    #[serde(default)]
    pub ffprobe_path: Option<PathBuf>,
    /// Path of the speech recognition tool. Left empty to search the usual
    /// locations, which include where the installation script puts it.
    #[serde(default)]
    pub whisper_path: Option<PathBuf>,
}

/// Bounds on background work, so that a scan never makes browsing sluggish.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Every setting on its own: a section written with one line in it keeps the
// usual value for everything else, rather than refusing to start over the
// lines that were not written.
#[serde(default)]
pub struct LimitsConfig {
    /// Concurrent media analyses.
    pub concurrent_probes: usize,
    /// Concurrent image generations.
    pub concurrent_image_jobs: usize,
    /// Concurrent calls to a metadata provider.
    pub concurrent_metadata_requests: usize,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            concurrent_probes: 2,
            concurrent_image_jobs: 2,
            concurrent_metadata_requests: 4,
        }
    }
}

/// Logging behaviour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// Every setting on its own: a section written with one line in it keeps the
// usual value for everything else, rather than refusing to start over the
// lines that were not written.
#[serde(default)]
pub struct LoggingConfig {
    /// Verbosity, using the usual filter syntax.
    pub level: String,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".to_string(),
        }
    }
}

/// One root folder declared in the configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RootConfig {
    /// Short label used in logs and in the interface instead of the path.
    pub label: String,
    pub path: PathBuf,
}

/// One library declared in the configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LibraryConfig {
    pub name: String,
    /// One of the stored library kinds.
    pub kind: String,
    /// The language films are described in, for this library alone. A library
    /// of Japanese animation and one of French films want different answers on
    /// the same server, which is why it sits here rather than on the server.
    #[serde(default = "default_metadata_language")]
    pub metadata_language: String,
    /// Several roots per library is the ordinary case: a collection spread
    /// across four disks is one library, not four.
    pub roots: Vec<RootConfig>,
}

/// The language used when the configuration does not say.
///
/// English, and not the language the author of this server happens to speak.
/// A default is what everybody who installs this without reading gets, so it
/// has to be the neutral answer rather than one person's preference. Whoever
/// wants another one writes it down, and later chooses it on a screen.
fn default_metadata_language() -> String {
    "en".to_string()
}

/// The whole configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_bind_address")]
    pub bind_address: IpAddr,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub directories: Directories,
    #[serde(default)]
    pub media_tools: MediaToolsConfig,
    #[serde(default)]
    pub limits: LimitsConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    /// Left out entirely when empty, so that a starting file printed by the
    /// installer can have a library appended to it as it stands. An empty list
    /// written out would make the appended block a duplicate key.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libraries: Vec<LibraryConfig>,
}

fn default_bind_address() -> IpAddr {
    IpAddr::V4(Ipv4Addr::UNSPECIFIED)
}

fn default_port() -> u16 {
    DEFAULT_PORT
}

impl Default for Config {
    fn default() -> Self {
        Self {
            bind_address: default_bind_address(),
            port: default_port(),
            directories: Directories::default(),
            media_tools: MediaToolsConfig::default(),
            limits: LimitsConfig::default(),
            logging: LoggingConfig::default(),
            libraries: Vec::new(),
        }
    }
}

impl Config {
    /// Usual location of the configuration file.
    pub fn default_path() -> PathBuf {
        PathBuf::from("/etc/melyxar/melyxar.toml")
    }

    /// Reads and validates a configuration file.
    pub fn load(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(ConfigError::NotFound(path.to_path_buf()));
        }
        let text = std::fs::read_to_string(path)?;
        let config: Self = toml::from_str(&text)?;
        config.validate()?;
        Ok(config)
    }

    /// Parses a configuration from text, used by tests and by the installer.
    pub fn parse(text: &str) -> Result<Self> {
        let config: Self = toml::from_str(text)?;
        config.validate()?;
        Ok(config)
    }

    /// Rejects a configuration that cannot work, with a message naming what to
    /// fix rather than a generic failure.
    pub fn validate(&self) -> Result<()> {
        if self.port == 0 {
            return Err(ConfigError::Invalid("the port must not be zero".into()));
        }
        if self.limits.concurrent_probes == 0 {
            return Err(ConfigError::Invalid(
                "concurrent_probes must be at least one, otherwise no file is ever analysed".into(),
            ));
        }
        for library in &self.libraries {
            if melyxar_core::library::LibraryKind::parse(&library.kind).is_none() {
                return Err(ConfigError::Invalid(format!(
                    "library '{}' declares the unknown kind '{}'",
                    library.name, library.kind
                )));
            }
            if library.roots.is_empty() {
                return Err(ConfigError::Invalid(format!(
                    "library '{}' declares no root folder",
                    library.name
                )));
            }
            for root in &library.roots {
                if root.label.trim().is_empty() {
                    return Err(ConfigError::Invalid(format!(
                        "a root of library '{}' has an empty label, and the label is what appears in logs instead of the path",
                        library.name
                    )));
                }
                if !root.path.is_absolute() {
                    return Err(ConfigError::Invalid(format!(
                        "root '{}' of library '{}' must be an absolute path",
                        root.label, library.name
                    )));
                }
            }
        }
        Ok(())
    }

    /// Renders the configuration back to text, used by the installer to write
    /// a starting file.
    pub fn to_toml(&self) -> String {
        toml::to_string_pretty(self).expect("the configuration is always serialisable")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"
        port = 2100

        [[libraries]]
        name = "Films"
        kind = "movies"
        metadata_language = "fr"

        [[libraries.roots]]
        label = "disk-one"
        path = "/mnt/one/Films"

        [[libraries.roots]]
        label = "disk-two"
        path = "/mnt/two/Films"
    "#;

    #[test]
    fn one_line_in_a_section_keeps_the_usual_value_for_the_rest() {
        // What somebody writes when they want one thing changed. Refusing to
        // start over the lines they did not write is a server that will not
        // come up because of a setting.
        let config = Config::parse(
            r#"
            port = 2100

            [logging]
            level = "debug"

            [limits]
            concurrent_probes = 4
        "#,
        )
        .expect("valid configuration");

        assert_eq!(config.logging.level, "debug");
        assert_eq!(config.limits.concurrent_probes, 4);
    }

    #[test]
    fn a_file_still_carrying_a_setting_that_has_moved_still_starts_the_server() {
        // The thumbnails, the upkeep, the description files, the limits on
        // transcodes and the codecs they come out in are settings of the
        // server now, changed on a screen. A file written before that
        // still holds their old sections, and a server that refused to start
        // over lines nobody can see any more would be a server somebody
        // has to fix from a terminal to reach the screen that replaced them.
        let text = format!(
            "{MINIMAL}\n[thumbnails]\nevery_seconds = 5\n\n[tasks]\nnightly_upkeep = false\n\n\
             [scan]\nread_companion_files = true\n\n[limits]\nmax_transcoding_sessions = 2\ntranscode_quota_megabytes = 8192\n\n\
             [transcode]\nenabled_video_codecs = [\"h264\"]\n"
        );
        assert!(
            Config::parse(&text).is_ok(),
            "a section nobody reads any more is a section to pass over"
        );
    }

    #[test]
    fn a_minimal_file_loads_and_fills_in_the_rest() {
        let config = Config::parse(MINIMAL).expect("valid configuration");
        assert_eq!(config.port, 2100);
        assert_eq!(config.libraries.len(), 1);
        assert_eq!(config.libraries[0].roots.len(), 2);
        // Defaults applied without being spelled out.
        assert_eq!(config.directories.data, PathBuf::from("/var/lib/melyxar"));
        assert_eq!(config.limits.concurrent_probes, 2);
    }

    #[test]
    fn several_roots_in_one_library_is_the_ordinary_case() {
        let config = Config::parse(MINIMAL).expect("valid configuration");
        let labels: Vec<_> = config.libraries[0]
            .roots
            .iter()
            .map(|root| root.label.as_str())
            .collect();
        assert_eq!(labels, vec!["disk-one", "disk-two"]);
    }

    #[test]
    fn an_unknown_library_kind_is_refused_by_name() {
        let text = MINIMAL.replace(r#"kind = "movies""#, r#"kind = "photos""#);
        let error = Config::parse(&text).expect_err("an unknown kind must be refused");
        assert!(error.to_string().contains("photos"));
    }

    #[test]
    fn a_library_without_a_root_is_refused() {
        let text = r#"
            [[libraries]]
            name = "Films"
            kind = "movies"
            roots = []
        "#;
        assert!(Config::parse(text).is_err());
    }

    #[test]
    fn a_relative_root_path_is_refused() {
        let text = MINIMAL.replace(r#"path = "/mnt/one/Films""#, r#"path = "Films""#);
        let error = Config::parse(&text).expect_err("a relative path must be refused");
        assert!(error.to_string().contains("absolute"));
    }

    #[test]
    fn an_empty_root_label_is_refused_because_logs_rely_on_it() {
        let text = MINIMAL.replace(r#"label = "disk-one""#, r#"label = "  ""#);
        assert!(Config::parse(&text).is_err());
    }

    #[test]
    fn a_zero_limit_is_refused_rather_than_silently_doing_nothing() {
        let text = format!("{MINIMAL}\n[limits]\nconcurrent_probes = 0\nconcurrent_image_jobs = 1\nconcurrent_metadata_requests = 1\n");
        assert!(Config::parse(&text).is_err());
    }

    #[test]
    fn the_configuration_survives_a_round_trip_through_text() {
        let config = Config::parse(MINIMAL).expect("valid configuration");
        let rendered = config.to_toml();
        let reparsed = Config::parse(&rendered).expect("rendered configuration stays valid");
        assert_eq!(config, reparsed);
    }

    #[test]
    fn a_starting_file_can_have_a_library_appended_to_it_as_it_stands() {
        // The installer prints a starting file and then appends a library
        // block. An empty list written out would make that a duplicate key.
        let starting = Config::default().to_toml();
        assert!(
            !starting.contains("libraries"),
            "an empty list must be left out entirely: {starting}"
        );

        let extended = format!(
            "{starting}\n[[libraries]]\nname = \"Films\"\nkind = \"movies\"\n\n[[libraries.roots]]\nlabel = \"disk-one\"\npath = \"/mnt/one/Films\"\n"
        );
        let config = Config::parse(&extended).expect("the extended file is valid");
        assert_eq!(config.libraries.len(), 1);
    }

    #[test]
    fn directories_derive_their_files_from_the_three_roots() {
        let directories = Directories::default();
        assert!(directories.database_file().starts_with(&directories.data));
        assert!(directories.uploads().starts_with(&directories.data));
        assert!(directories.images().starts_with(&directories.cache));
    }

    #[test]
    fn a_file_written_before_the_interface_had_a_folder_still_starts_the_server() {
        let config = Config::parse(
            r#"
            port = 2100

            [directories]
            data = "/srv/melyxar/data"
            cache = "/srv/melyxar/cache"
            transcodes = "/srv/melyxar/cache/transcodes"
        "#,
        )
        .expect("valid configuration");
        assert_eq!(config.directories.interface, default_interface());
    }

    #[test]
    fn a_missing_file_is_reported_with_its_path() {
        let error = Config::load(Path::new("/nowhere/melyxar.toml"))
            .expect_err("a missing file must be reported");
        assert!(matches!(error, ConfigError::NotFound(_)));
    }

    #[test]
    fn a_file_on_disk_loads() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("melyxar.toml");
        std::fs::write(&path, MINIMAL).expect("write configuration");
        let config = Config::load(&path).expect("valid configuration");
        assert_eq!(config.libraries.len(), 1);
    }
}
