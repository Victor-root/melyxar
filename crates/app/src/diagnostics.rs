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
    /// The films still without a name, and what stopped each of them.
    ///
    /// Named rather than counted. A count is a question; these are the answer,
    /// and the title shown is the very thing the provider was asked about, so
    /// reading it is usually enough to see what went wrong.
    ///
    /// Titles and the names on disk they were read from, never paths: what is
    /// guarded elsewhere is where somebody's files live, and the name of a
    /// film is not that.
    pub nameless: Vec<NamelessReport>,
    /// Films that were named and are still missing something a page shows.
    ///
    /// A film nobody could name says so; a film with no poster says nothing,
    /// and the hole is only ever seen by whoever scrolls past it.
    pub incomplete: Vec<IncompleteReport>,
    /// Films held in more than one copy, with the name of each copy.
    ///
    /// Several copies of one film are wanted, and are also what a wrong
    /// grouping leaves behind. Reading the names side by side is what settles
    /// which of the two it is.
    pub copies: Vec<CopiesReport>,
    /// The last pieces of work and what became of them.
    ///
    /// In the report rather than only in a log, because a run that failed says
    /// why it failed here, and that is the first question worth asking when
    /// something did not happen.
    pub recent_work: Vec<WorkReport>,
}

/// One film nobody has been able to name.
#[derive(Debug, Clone, Serialize)]
pub struct NamelessReport {
    /// What it is called now, which is what was searched for.
    pub title: String,
    pub year: Option<i32>,
    /// Why the last look up failed, when one has run.
    pub reason: Option<&'static str>,
    /// The name on disk the title was read from.
    ///
    /// A title that reads oddly leaves exactly one question, and this is the
    /// answer to it. Shown only when it says something the title does not, so
    /// the list stays readable.
    pub file_name: Option<String>,
}

/// One film the library holds more than one copy of.
#[derive(Debug, Clone, Serialize)]
pub struct CopiesReport {
    pub title: String,
    pub year: Option<i32>,
    pub file_names: Vec<String>,
}

/// One named film and the holes left in it.
#[derive(Debug, Clone, Serialize)]
pub struct IncompleteReport {
    pub title: String,
    pub year: Option<i32>,
    /// poster, backdrop, overview, cast.
    pub missing: Vec<&'static str>,
}

/// One piece of background work, as the diagnostic shows it.
#[derive(Debug, Clone, Serialize)]
pub struct WorkReport {
    pub kind: &'static str,
    pub state: &'static str,
    pub done: i64,
    pub total: Option<i64>,
    /// Why it failed, when it did. This is the line that is worth reading.
    pub failure_reason: Option<String>,
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
    /// How many files of the library sit on this root.
    pub files: i64,
    /// A few of the names the folder holds, when it gave up no file at all.
    ///
    /// A disk that is there, readable, and yields nothing leaves one question:
    /// is it the wrong folder, or is it full of files this server does not
    /// recognise? Reading a handful of names answers it without a terminal.
    pub holds: Vec<String>,
}

/// Path of the graphics device an unprivileged container has to be given
/// before hardware acceleration can work.
const GRAPHICS_DEVICE: &str = "/dev/dri";

/// How many names are read out of a folder that gave up no file.
///
/// Enough to tell a folder holding other folders from one holding files of a
/// kind this server does not read, few enough that the report stays a report.
const NAMES_SHOWN: usize = 6;

/// A few of the names a folder holds, for a root that yielded nothing.
///
/// One listing of one folder, never a walk: this runs while somebody waits for
/// a page. It is not trying to find the films, only to say what is there, and
/// a folder name or an unfamiliar extension is the whole answer.
fn what_the_folder_holds(path: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(path) else {
        return Vec::new();
    };

    let mut names: Vec<String> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            match entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                true => format!("{name}/"),
                false => name,
            }
        })
        .take(NAMES_SHOWN)
        .collect();
    names.sort();
    names
}

/// Builds the report.
pub async fn collect(state: &AppState) -> Result<Diagnostics> {
    let config = state.config();
    let database = state.database();

    let journal_mode = database.journal_mode().await?;
    let libraries = database.list_libraries().await?;

    let counts: std::collections::HashMap<_, _> =
        database.file_counts_by_root().await?.into_iter().collect();

    let mut roots = Vec::new();
    for entry in database.roots_with_access().await? {
        let library = libraries
            .iter()
            .find(|library| library.id == entry.root.library_id)
            .map(|library| library.name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let files = counts.get(&entry.root.id).copied().unwrap_or_default();
        roots.push(RootReport {
            library,
            label: entry.root.label.clone(),
            access: entry.access.as_str(),
            explanation_code: entry.access.explanation_code(),
            checked: entry.checked_at.is_some(),
            files,
            holds: match files {
                0 => what_the_folder_holds(&entry.root.path),
                _ => Vec::new(),
            },
        });
    }

    // Every folder the server writes to, not only the three it is configured
    // with: a folder created once by another account is readable, looks fine
    // from above, and refuses every write made inside it. A picture that never
    // arrives is then a warning in a log nobody reads.
    let directories = vec![
        directory_report("data", &config.directories.data),
        directory_report("cache", &config.directories.cache),
        directory_report("transcodes", &config.directories.transcodes),
        directory_report("images", &config.directories.images()),
        directory_report("subtitles", &config.directories.subtitles()),
        directory_report("uploads", &config.directories.uploads()),
        directory_report("backups", &config.directories.backups()),
    ];

    let capabilities = state.capabilities();

    Ok(Diagnostics {
        version: melyxar_core::BUILD,
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
        nameless: database
            .works_still_nameless(NAMELESS_SHOWN)
            .await?
            .iter()
            .map(|nameless| NamelessReport {
                title: nameless.work.title.clone(),
                year: nameless.work.release_year,
                reason: nameless
                    .work
                    .identification_note
                    .map(melyxar_core::work::IdentificationNote::as_str),
                file_name: nameless
                    .file_name
                    .clone()
                    .filter(|name| name_says_more_than(name, &nameless.work.title)),
            })
            .collect(),
        copies: database
            .works_held_in_several_copies()
            .await?
            .into_iter()
            .map(|film| CopiesReport {
                title: film.title,
                year: film.release_year,
                file_names: film.file_names,
            })
            .collect(),
        incomplete: database
            .works_missing_something()
            .await?
            .into_iter()
            .map(|work| IncompleteReport {
                title: work.title,
                year: work.release_year,
                missing: work.missing,
            })
            .collect(),
        recent_work: database
            .recent_jobs(WORK_SHOWN)
            .await?
            .iter()
            .map(work_report)
            .collect(),
    })
}

/// Whether the name on disk says anything the title does not.
///
/// A file called exactly after the title would only repeat the line it sits
/// on, and the list is meant to be read in one go.
fn name_says_more_than(file_name: &str, title: &str) -> bool {
    let stem = file_name
        .rsplit_once('.')
        .map_or(file_name, |(stem, _)| stem);
    stem != title
}

/// How many nameless films the report names.
///
/// Enough to see a pattern in them, few enough that the report stays one
/// block somebody reads.
const NAMELESS_SHOWN: i64 = 25;

/// How many films held in several copies the report names before saying how
/// many more. The list behind it is not cut.
const COPIES_SHOWN: usize = 15;

/// How many incomplete films the report names before saying how many more.
///
/// The report is one block somebody reads; the list behind it is not cut.
const INCOMPLETE_SHOWN: usize = 25;

/// How many finished pieces of work the report carries.
///
/// Enough to show a failure and the runs around it, few enough that the report
/// stays something a person reads in one go.
const WORK_SHOWN: i64 = 10;

fn work_report(job: &melyxar_core::job::Job) -> WorkReport {
    WorkReport {
        kind: job.kind.as_str(),
        state: job.state.as_str(),
        done: job.progress_done,
        total: job.progress_total,
        failure_reason: job.failure_reason.clone(),
    }
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

    if !report.nameless.is_empty() {
        line!("#", "Films still without a name");
        for film in &report.nameless {
            let year = match film.year {
                Some(year) => format!(" ({year})"),
                None => String::new(),
            };
            let read_from = match &film.file_name {
                Some(name) => format!("  read from {name}"),
                None => String::new(),
            };
            line!(
                "!",
                format!(
                    "{}{year}  {}{read_from}",
                    film.title,
                    film.reason.unwrap_or("never looked up")
                ),
            );
        }
        if report.catalogue.awaiting_identification > report.nameless.len() as i64 {
            line!(
                " ",
                format!(
                    "  and {} more",
                    report.catalogue.awaiting_identification - report.nameless.len() as i64
                ),
            );
        }
        out.push('\n');
    }

    if !report.incomplete.is_empty() {
        line!("#", "Films missing something");
        for film in report.incomplete.iter().take(INCOMPLETE_SHOWN) {
            let year = match film.year {
                Some(year) => format!(" ({year})"),
                None => String::new(),
            };
            line!(
                "!",
                format!("{}{year}  no {}", film.title, film.missing.join(", no ")),
            );
        }
        if report.incomplete.len() > INCOMPLETE_SHOWN {
            line!(
                " ",
                format!("  and {} more", report.incomplete.len() - INCOMPLETE_SHOWN),
            );
        }
        out.push('\n');
    }

    if !report.copies.is_empty() {
        line!("#", "Films held in more than one copy");
        for film in report.copies.iter().take(COPIES_SHOWN) {
            let year = match film.year {
                Some(year) => format!(" ({year})"),
                None => String::new(),
            };
            line!("+", format!("{}{year}", film.title));
            for name in &film.file_names {
                line!(" ", format!("    {name}"));
            }
        }
        if report.copies.len() > COPIES_SHOWN {
            line!(
                " ",
                format!("  and {} more", report.copies.len() - COPIES_SHOWN),
            );
        }
        out.push('\n');
    }

    line!("#", "Recent work");
    if report.recent_work.is_empty() {
        line!("+", "nothing has run yet");
    }
    for work in &report.recent_work {
        let progress = match work.total {
            Some(total) => format!("{} of {total}", work.done),
            None => work.done.to_string(),
        };
        line!(
            if work.failure_reason.is_some() {
                "x"
            } else {
                "+"
            },
            format!("{:<15} {:<10} {progress}", work.kind, work.state),
        );
        // On a line of its own and never shortened: this is the sentence that
        // says what went wrong, and it is the whole reason the section exists.
        if let Some(reason) = &work.failure_reason {
            line!(" ", format!("  {reason}"));
        }
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
            match (root.access, root.files) {
                // A disk that is there and gave up nothing is not a disk that
                // is working, whatever its permissions say.
                ("read_write" | "read_only", 1..) => "+",
                ("read_write" | "read_only", _) => "!",
                _ => "x",
            },
            format!(
                "{:<12} {:<16} {:<20} {} files",
                root.library, root.label, root.explanation_code, root.files
            ),
        );
        // A root that gave up nothing is the one case where the line above
        // says nothing useful, so this one says what is actually there.
        if root.files == 0 {
            line!(
                " ",
                match root.holds.is_empty() {
                    true => "    this folder is empty".to_string(),
                    false => format!("    it holds: {}", root.holds.join(", ")),
                },
            );
        }
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
        assert_eq!(report.directories.len(), 7);
        assert!(report.directories.iter().all(|entry| entry.exists));
        assert!(
            report
                .directories
                .iter()
                .any(|entry| entry.purpose == "images"),
            "the folder the pictures are written in is one a person has to be told about"
        );
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

    #[tokio::test]
    async fn the_films_still_without_a_name_are_named_rather_than_counted() {
        // A count is a question. Reading the title that was searched, next to
        // the reason it failed, is usually the whole answer.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let library = state
            .database()
            .list_libraries()
            .await
            .expect("read")
            .pop()
            .expect("one library");
        let work = state
            .database()
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour Extended",
                "quiet harbour extended",
                Some(2019),
            )
            .await
            .expect("work created");
        state
            .database()
            .set_identification_note(work.id, melyxar_core::work::IdentificationNote::NoMatch)
            .await
            .expect("note written");

        let report = collect(&state).await.expect("report collected");
        assert_eq!(report.nameless.len(), 1);
        assert_eq!(report.nameless[0].title, "Quiet Harbour Extended");
        assert_eq!(report.nameless[0].year, Some(2019));
        assert_eq!(report.nameless[0].reason, Some("no_match"));

        let text = render_text(&report);
        assert!(text.contains("Quiet Harbour Extended (2019)"), "{text}");
        assert!(text.contains("no_match"), "{text}");
    }

    #[tokio::test]
    async fn a_film_nobody_could_name_shows_the_name_on_disk_it_was_read_from() {
        // The title is only ever as good as the name it was read from, so a
        // title that reads oddly leaves one question. Answering it used to
        // mean opening the database by hand, and the whole point of this
        // report is that nothing needs a terminal.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let library = state
            .database()
            .list_libraries()
            .await
            .expect("read")
            .pop()
            .expect("one library");
        let work = state
            .database()
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                None,
            )
            .await
            .expect("work created");
        state
            .database()
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new("Anciens/xyQuiet Harbour BD Rip.avi"),
                1_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");

        let text = render_text(&collect(&state).await.expect("report collected"));
        assert!(
            text.contains("read from xyQuiet Harbour BD Rip.avi"),
            "the name behind the title is the one thing that explains it: {text}"
        );
        assert!(
            !text.contains("Anciens"),
            "the name of the file, and never the folders leading to it: {text}"
        );
    }

    #[tokio::test]
    async fn a_root_that_gave_up_nothing_says_so_and_says_what_is_there() {
        // A disk that is plugged in, readable, and holds no film the server
        // recognised used to look exactly like a disk that worked. The numbers
        // simply did not move, and nothing anywhere said why.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(media.join("Films")).expect("media folder");
        std::fs::write(media.join("readme.txt"), b"x").expect("file written");
        let state = state_with_root(directory.path(), media).await;

        let text = render_text(&collect(&state).await.expect("report collected"));
        assert!(text.contains("0 files"), "{text}");
        assert!(text.contains("it holds: Films/, readme.txt"), "{text}");
    }

    #[tokio::test]
    async fn a_root_whose_folder_is_empty_says_that_plainly() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let text = render_text(&collect(&state).await.expect("report collected"));
        assert!(text.contains("this folder is empty"), "{text}");
    }

    #[tokio::test]
    async fn a_root_holding_films_is_not_asked_what_is_in_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let library = state
            .database()
            .list_libraries()
            .await
            .expect("read")
            .pop()
            .expect("one library");
        let work = state
            .database()
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        state
            .database()
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new("Quiet Harbour 1080p.mkv"),
                1_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");

        let text = render_text(&collect(&state).await.expect("report collected"));
        assert!(text.contains("1 files"), "{text}");
        assert!(!text.contains("it holds:"), "{text}");
        assert!(!text.contains("this folder is empty"), "{text}");
    }

    #[tokio::test]
    async fn a_film_held_in_several_copies_is_shown_with_each_of_their_names() {
        // A grouping nobody can check is a grouping nobody should trust. The
        // names side by side are the check.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let library = state
            .database()
            .list_libraries()
            .await
            .expect("read")
            .pop()
            .expect("one library");
        let work = state
            .database()
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        for name in ["Quiet Harbour 1080p.mkv", "zz12Quiet Harbour 1080p.mkv"] {
            state
                .database()
                .insert_source(
                    work.id,
                    library.roots[0].id,
                    std::path::Path::new(name),
                    1_000,
                    melyxar_core::time::now(),
                )
                .await
                .expect("source recorded");
        }

        let text = render_text(&collect(&state).await.expect("report collected"));
        assert!(text.contains("Films held in more than one copy"), "{text}");
        assert!(text.contains("zz12Quiet Harbour 1080p.mkv"), "{text}");
    }

    #[tokio::test]
    async fn a_film_whose_file_is_named_exactly_after_it_says_so_once() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let library = state
            .database()
            .list_libraries()
            .await
            .expect("read")
            .pop()
            .expect("one library");
        let work = state
            .database()
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                None,
            )
            .await
            .expect("work created");
        state
            .database()
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new("Quiet Harbour.mkv"),
                1_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");

        let text = render_text(&collect(&state).await.expect("report collected"));
        assert!(
            !text.contains("read from"),
            "repeating the title as a file name adds nothing: {text}"
        );
    }

    #[tokio::test]
    async fn a_film_that_was_named_and_still_has_holes_in_it_is_named_too() {
        // Nothing anywhere says this out loud: the film has its title, the run
        // succeeded, and the only sign is a grey rectangle somebody scrolls
        // past. Without naming them there is no way to tell a film the
        // provider has no picture of from one whose picture never arrived.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let library = state
            .database()
            .list_libraries()
            .await
            .expect("read")
            .pop()
            .expect("one library");
        let work = state
            .database()
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        state
            .database()
            .apply_identification(work.id, &named("Quiet Harbour"), false)
            .await
            .expect("identification applied");

        let report = collect(&state).await.expect("report collected");
        assert_eq!(report.incomplete.len(), 1);
        assert_eq!(report.incomplete[0].title, "Quiet Harbour");
        assert!(
            report.incomplete[0].missing.contains(&"poster"),
            "{:?}",
            report.incomplete[0]
        );

        let text = render_text(&report);
        assert!(text.contains("Films missing something"), "{text}");
        assert!(text.contains("Quiet Harbour (2019)"), "{text}");
        assert!(text.contains("no poster"), "{text}");
    }

    /// The little a provider has to say for a film to count as named.
    fn named(title: &str) -> melyxar_database::metadata::IdentifiedWork {
        melyxar_database::metadata::IdentifiedWork {
            provider: "tmdb".to_string(),
            external_id: "111".to_string(),
            imdb_id: None,
            language: "fr".to_string(),
            sort_title: title.to_lowercase(),
            title: title.to_string(),
            tagline: None,
            overview: None,
            release_year: Some(2019),
            runtime: None,
            community_rating: None,
            age_rating_label: None,
            genres: Vec::new(),
            studios: Vec::new(),
            credits: Vec::new(),
            collection: None,
            trailers: Vec::new(),
        }
    }

    #[tokio::test]
    async fn a_run_that_failed_says_why_in_the_report_itself() {
        // The one sentence worth having, and the reason the report exists: a
        // person who cannot read a log copies this and it names the fault.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let runner = melyxar_jobs::JobRunner::new(state.database().clone());
        runner
            .start(
                melyxar_core::job::JobKind::IdentifyWork,
                melyxar_core::job::JobPriority::BACKGROUND,
                None,
                |_| async move { Err("the provider refused the key".to_string()) },
            )
            .await
            .expect("job started")
            .completion
            .await
            .expect("the job ran");

        let report = collect(&state).await.expect("report collected");
        assert_eq!(report.recent_work[0].state, "failed");
        assert_eq!(
            report.recent_work[0].failure_reason.as_deref(),
            Some("the provider refused the key")
        );

        let text = render_text(&report);
        assert!(text.contains("Recent work"), "{text}");
        assert!(
            text.contains("the provider refused the key"),
            "the reason is never shortened away: {text}"
        );
    }

    #[test]
    fn sizes_are_rendered_in_units_a_person_reads() {
        assert_eq!(human_size(512), "512 B");
        assert_eq!(human_size(2048), "2.0 kB");
        assert_eq!(human_size(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 GB");
    }

    #[test]
    fn a_size_bigger_than_the_largest_unit_still_has_a_name() {
        // Four disks of several terabytes is the ordinary case here, and a
        // report that stops at the last unit it knows would not print at all.
        let four_terabytes = 4 * 1024_i64.pow(4);
        assert_eq!(human_size(four_terabytes), "4.0 TB");
        assert_eq!(human_size(four_terabytes * 500), "2000.0 TB");
    }

    #[tokio::test]
    async fn the_report_answers_yes_or_no_rather_than_leaving_a_blank() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let text = render_text(&collect(&state).await.expect("report collected"));
        assert!(
            text.contains("(exists: yes, writable: yes)"),
            "the folders were just prepared, so the report says so in as many words: {text}"
        );
        assert!(
            text.contains("readers never wait on the writer: no"),
            "and the other answer too: this database is in memory, where the \
             journal the real one uses does not apply: {text}"
        );
        assert!(
            !text.contains("waiting to be looked up"),
            "nothing is waiting, so the report says nothing about it: {text}"
        );
    }

    #[tokio::test]
    async fn the_report_says_when_films_are_still_waiting_to_be_looked_up() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("media");
        std::fs::create_dir_all(&media).expect("media folder");
        let state = state_with_root(directory.path(), media).await;

        let library = state
            .database()
            .library_by_name("Films")
            .await
            .expect("read")
            .expect("declared");
        state
            .database()
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");

        let text = render_text(&collect(&state).await.expect("report collected"));
        assert!(
            text.contains("waiting to be looked up"),
            "a film with no title yet is the first thing to explain: {text}"
        );
    }
}
