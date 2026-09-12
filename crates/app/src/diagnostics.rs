//! The diagnostic report.
//!
//! Written for someone who does not read code. It answers, in one place, the
//! questions that actually come up: are the media tools there and what can
//! they do, can the server see its folders, is the database in the mode that
//! keeps browsing responsive, and how much space is being used.
//!
//! Produced as data rather than as text, so the same report backs the command
//! line, the administration screen and the export that gets pasted into a
//! conversation.

use serde::Serialize;

use crate::{AppState, Result};

/// Everything worth knowing about a running server.
#[derive(Debug, Clone, Serialize)]
pub struct Diagnostics {
    pub version: &'static str,
    pub media_tools: MediaToolsReport,
    pub database: DatabaseReport,
    pub directories: Vec<DirectoryReport>,
    pub roots: Vec<RootReport>,
    pub accounts: i64,
    pub libraries: usize,
    pub catalogue: CatalogueReport,
}

/// What the library actually holds, which is the first thing anyone asks
/// after a scan.
#[derive(Debug, Clone, Serialize)]
pub struct CatalogueReport {
    pub works: i64,
    pub files: i64,
    /// Files no longer on disk. Kept rather than removed, so a number here is
    /// a question to look into and not a loss.
    pub missing_files: i64,
    pub identified: i64,
    /// Works still waiting for a provider to recognise them.
    pub awaiting_identification: i64,
    /// Whether films can be looked up at all. Saying so plainly beats leaving
    /// someone to wonder why every film is untitled.
    pub metadata_available: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaToolsReport {
    pub found: bool,
    pub encoder_path: Option<String>,
    pub analyser_path: Option<String>,
    pub version: Option<String>,
    /// Hardware paths the build carries. Empty means software only, which
    /// works but costs a great deal more processor time.
    pub hardware: Vec<String>,
    /// Whether the tool can produce what a browser needs at all.
    pub can_serve_browsers: bool,
    /// Whether wide gamut colour can be converted without a card. Without it,
    /// such films cannot be shown with correct colours at all.
    pub can_convert_wide_gamut: bool,
    /// Whether the graphics device is visible, which is what an unprivileged
    /// container most often gets wrong.
    pub graphics_device_present: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DatabaseReport {
    /// Journal mode in force. The write-ahead mode is what keeps browsing
    /// responsive while a scan writes.
    pub journal_mode: String,
    pub readers_never_wait_on_the_writer: bool,
    pub size_bytes: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DirectoryReport {
    pub purpose: &'static str,
    pub path: String,
    pub exists: bool,
    pub writable: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RootReport {
    pub library: String,
    pub label: String,
    pub access: &'static str,
    /// Stable code the interface turns into a sentence in the reader's own
    /// language.
    pub explanation_code: &'static str,
    pub checked: bool,
}

/// Path of the graphics device an unprivileged container has to be given
/// before hardware acceleration can work.
const GRAPHICS_DEVICE: &str = "/dev/dri";

/// Builds the report.
pub async fn collect(state: &AppState) -> Result<Diagnostics> {
    let config = state.config();
    let database = state.database();

    let journal_mode = database.journal_mode().await?;
    let libraries = database.list_libraries().await?;

    let mut roots = Vec::new();
    for entry in database.roots_with_access().await? {
        let library = libraries
            .iter()
            .find(|library| library.id == entry.root.library_id)
            .map(|library| library.name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        roots.push(RootReport {
            library,
            label: entry.root.label.clone(),
            access: entry.access.as_str(),
            explanation_code: melyxar_library::access::explanation_code(entry.access),
            checked: entry.checked_at.is_some(),
        });
    }

    let directories = vec![
        directory_report("data", &config.directories.data),
        directory_report("cache", &config.directories.cache),
        directory_report("transcodes", &config.directories.transcodes),
    ];

    let capabilities = state.capabilities();

    Ok(Diagnostics {
        version: env!("CARGO_PKG_VERSION"),
        media_tools: MediaToolsReport {
            found: state.tools().is_some(),
            encoder_path: state
                .tools()
                .map(|tools| tools.ffmpeg.to_string_lossy().into_owned()),
            analyser_path: state
                .tools()
                .map(|tools| tools.ffprobe.to_string_lossy().into_owned()),
            version: capabilities.map(|capabilities| capabilities.version.clone()),
            hardware: capabilities
                .map(|capabilities| {
                    capabilities
                        .hardware
                        .iter()
                        .map(|value| value.as_str().to_string())
                        .collect()
                })
                .unwrap_or_default(),
            can_serve_browsers: capabilities
                .is_some_and(melyxar_ffmpeg::Capabilities::supports_minimum_targets),
            can_convert_wide_gamut: capabilities
                .is_some_and(melyxar_ffmpeg::Capabilities::can_tone_map_in_software),
            graphics_device_present: std::path::Path::new(GRAPHICS_DEVICE).exists(),
        },
        database: DatabaseReport {
            readers_never_wait_on_the_writer: journal_mode.eq_ignore_ascii_case("wal"),
            journal_mode,
            size_bytes: database.size_bytes().await?,
        },
        directories,
        roots,
        accounts: database.user_count().await?,
        libraries: libraries.len(),
        catalogue: database
            .catalogue_summary()
            .await
            .map(|summary| CatalogueReport {
                works: summary.works,
                files: summary.files,
                missing_files: summary.missing_files,
                identified: summary.identified,
                awaiting_identification: summary.awaiting_identification,
                metadata_available: state.metadata_provider().is_some(),
            })?,
    })
}

fn directory_report(purpose: &'static str, path: &std::path::Path) -> DirectoryReport {
    let access = melyxar_library::check_root_access(path);
    DirectoryReport {
        purpose,
        path: path.to_string_lossy().into_owned(),
        exists: access != melyxar_core::library::RootAccess::Missing,
        writable: access.allows_writing(),
    }
}

/// Renders the report the way the command line shows it.
///
/// Plain lines with a marker, no colour codes: the output gets pasted into a
/// conversation, so it has to stay readable once the formatting is gone.
pub fn render_text(report: &Diagnostics) -> String {
    let mut out = String::new();
    // A macro rather than a closure: blank lines are pushed directly between
    // sections, and a closure holding a mutable borrow would forbid that.
    macro_rules! line {
        ($marker:expr, $text:expr $(,)?) => {{
            out.push_str($marker);
            out.push(' ');
            out.push_str(::std::convert::AsRef::<str>::as_ref(&$text));
            out.push('\n');
        }};
    }

    line!("*", format!("Melyxar {}", report.version));
    out.push('\n');

    line!("#", "Media tools");
    if report.media_tools.found {
        line!(
            "+",
            report
                .media_tools
                .version
                .clone()
                .unwrap_or_else(|| "found, version unknown".to_string()),
        );
        line!(
            if report.media_tools.can_serve_browsers {
                "+"
            } else {
                "x"
            },
            format!(
                "can produce what a browser reads: {}",
                yes_no(report.media_tools.can_serve_browsers)
            ),
        );
        line!(
            if report.media_tools.can_convert_wide_gamut {
                "+"
            } else {
                "!"
            },
            format!(
                "can convert wide gamut colour without a card: {}",
                yes_no(report.media_tools.can_convert_wide_gamut)
            ),
        );
        line!(
            if report.media_tools.hardware.is_empty() {
                "!"
            } else {
                "+"
            },
            if report.media_tools.hardware.is_empty() {
                "no hardware acceleration in this build, everything runs on the processor"
                    .to_string()
            } else {
                format!(
                    "hardware acceleration available: {}",
                    report.media_tools.hardware.join(", ")
                )
            },
        );
        line!(
            if report.media_tools.graphics_device_present {
                "+"
            } else {
                "!"
            },
            format!(
                "graphics device visible to this container: {}",
                yes_no(report.media_tools.graphics_device_present)
            ),
        );
    } else {
        line!("x", "not found; browsing works, playback does not");
    }
    out.push('\n');

    line!("#", "Database");
    line!(
        if report.database.readers_never_wait_on_the_writer {
            "+"
        } else {
            "x"
        },
        format!(
            "journal mode {} (readers never wait on the writer: {})",
            report.database.journal_mode,
            yes_no(report.database.readers_never_wait_on_the_writer)
        ),
    );
    line!(
        "+",
        format!("size {}", human_size(report.database.size_bytes)),
    );
    line!("+", format!("accounts {}", report.accounts));
    line!("+", format!("libraries {}", report.libraries));
    out.push('\n');

    line!("#", "Library");
    line!("+", format!("works {}", report.catalogue.works));
    line!("+", format!("files {}", report.catalogue.files));
    line!(
        if report.catalogue.missing_files == 0 {
            "+"
        } else {
            "!"
        },
        format!(
            "files no longer on disk {} (kept, never removed)",
            report.catalogue.missing_files
        ),
    );
    line!(
        "+",
        format!(
            "identified {} of {}",
            report.catalogue.identified, report.catalogue.works
        ),
    );
    line!(
        if report.catalogue.metadata_available {
            "+"
        } else {
            "!"
        },
        format!(
            "films can be looked up: {}",
            yes_no(report.catalogue.metadata_available)
        ),
    );
    if report.catalogue.awaiting_identification > 0 {
        line!(
            "!",
            format!(
                "waiting to be looked up {}",
                report.catalogue.awaiting_identification
            ),
        );
    }
    out.push('\n');

    line!("#", "Directories");
    for directory in &report.directories {
        line!(
            if directory.exists && directory.writable {
                "+"
            } else {
                "x"
            },
            format!(
                "{:<11} {} (exists: {}, writable: {})",
                directory.purpose,
                directory.path,
                yes_no(directory.exists),
                yes_no(directory.writable)
            ),
        );
    }
    out.push('\n');

    line!("#", "Library roots");
    if report.roots.is_empty() {
        line!("!", "none declared yet");
    }
    for root in &report.roots {
        line!(
            match root.access {
                "read_write" | "read_only" => "+",
                _ => "x",
            },
            format!(
                "{:<12} {:<12} {}",
                root.library, root.label, root.explanation_code
            ),
        );
    }

    out
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

fn human_size(bytes: i64) -> String {
    const UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories, LibraryConfig, RootConfig};
    use melyxar_database::Database;

    async fn state_with_root(directory: &std::path::Path, root: std::path::PathBuf) -> AppState {
        let config = Config {
            directories: Directories {
                data: directory.join("data"),
                cache: directory.join("cache"),
                transcodes: directory.join("cache/transcodes"),
            },
            libraries: vec![LibraryConfig {
                name: "Films".into(),
                kind: "movies".into(),
                metadata_language: "fr".into(),
                roots: vec![RootConfig {
                    label: "disk-one".into(),
                    path: root,
                }],
            }],
            ..Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");

        let database = Database::open_in_memory().await.expect("database opens");
        crate::startup::ensure_default_account(&database)
            .await
            .expect("account created");
        crate::startup::reconcile_libraries(&database, &config)
            .await
            .expect("libraries reconciled");
        crate::startup::refresh_root_access(&database)
            .await
            .expect("access refreshed");

        let (tools, capabilities) = crate::startup::detect_media_tools(&config).await;
        AppState::new(config, database, tools, capabilities)
    }

    #[tokio::test]
    async fn the_report_answers_the_questions_that_actually_come_up() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let report = collect(&state).await.expect("report collected");

        assert!(report.media_tools.found);
        assert!(report.media_tools.can_serve_browsers);
        assert_eq!(report.accounts, 1);
        assert_eq!(report.libraries, 1);
        assert_eq!(report.catalogue.works, 0);
        assert!(
            report.catalogue.metadata_available,
            "a film that cannot be looked up is a film that stays untitled, and the report has to say so"
        );
        assert_eq!(report.roots.len(), 1);
        assert_eq!(report.roots[0].library, "Films");
        assert!(report.roots[0].checked);
        assert_eq!(report.directories.len(), 3);
        assert!(report.directories.iter().all(|entry| entry.exists));
    }

    #[tokio::test]
    async fn a_root_that_is_not_mounted_shows_up_as_such_in_the_report() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_with_root(
            directory.path(),
            std::path::PathBuf::from("/nowhere/at/all"),
        )
        .await;

        let report = collect(&state).await.expect("report collected");
        assert_eq!(report.roots[0].access, "missing");
        assert_eq!(
            report.roots[0].explanation_code,
            "root_missing_or_not_mounted"
        );
    }

    #[tokio::test]
    async fn the_rendered_report_stays_readable_once_formatting_is_gone() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let text = render_text(&collect(&state).await.expect("report collected"));

        assert!(text.contains("Melyxar"));
        assert!(text.contains("Media tools"));
        assert!(text.contains("Database"));
        assert!(text.contains("Library roots"));
        assert!(
            !text.contains('\u{1b}'),
            "no escape codes: the output gets pasted into a conversation"
        );
    }

    #[test]
    fn sizes_are_rendered_in_units_a_person_reads() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 kB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }
}
