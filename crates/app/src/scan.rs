//! Scanning a library: what is on disk, what is recorded, and the difference.
//!
//! The scan is where the careful parts of the other crates are put to work, so
//! the rules it obeys are worth stating plainly:
//!
//! * a root that cannot be read stops the work on that root and nothing else,
//!   because one unplugged disk must not empty the three others;
//! * a file that is gone is marked absent, never deleted;
//! * two copies of one film are two files of one work, which is what puts a
//!   version chooser on the page rather than the same title twice in a grid;
//! * identifying a work is a separate job, so a scan never waits on a provider.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use melyxar_core::id::JobId;
use melyxar_core::id::{LibraryRootId, MediaSourceId, WorkId};
use melyxar_core::job::{JobKind, JobPriority, JobState};
use melyxar_core::library::{Library, LibraryKind};
use melyxar_core::privacy::{MediaName, MediaPath};
use melyxar_core::work::WorkKind;
use melyxar_database::catalogue::{LocalExtraVideo, SourceAnalysis, StoredSource};
use melyxar_database::Database;
use melyxar_jobs::{JobHandle, StartedJob};
use melyxar_library::naming;
use melyxar_library::scan::{walk, FoundFile, KnownFile, ScanError};

use crate::{AppError, AppState, Result};

/// What a scan did, in the terms the maintainer reads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    pub added: usize,
    pub changed: usize,
    /// Files no longer on disk, marked absent rather than removed.
    pub missing: usize,
    /// Files that were absent and turned up again.
    pub restored: usize,
    pub unchanged: usize,
    pub analysed: usize,
    /// Files the analyser could not read. Recorded rather than hidden: a file
    /// nobody can analyse is a file nobody will be able to play either.
    pub unreadable_files: usize,
    pub extras: usize,
    /// Roots that could not be walked, by label. Never by path: a label is
    /// what logs and screens are allowed to show.
    pub unusable_roots: Vec<String>,
    pub unreadable_folders: usize,
    /// Set when the work was stopped part way through.
    pub cancelled: bool,
}

impl ScanReport {
    /// Whether anything at all moved, which is what decides if the version
    /// counter of the library has to be bumped.
    pub fn changed_anything(&self) -> bool {
        self.added > 0 || self.changed > 0 || self.missing > 0 || self.restored > 0
    }
}

/// A scan running in the background.
///
/// Dropping it lets the scan run on, which is what a request handler wants.
/// A caller that means to wait, such as the command line, calls `wait`.
pub struct ScanJob {
    started: StartedJob,
    outcome: Arc<Mutex<Option<ScanReport>>>,
}

impl ScanJob {
    /// The job identifier, which is what a client follows the scan by.
    pub fn id(&self) -> JobId {
        self.started.id
    }

    /// Waits for the scan to end, and says what it did.
    ///
    /// The report is absent when the scan did not get to the end, which is
    /// exactly the case where there is nothing to report.
    pub async fn wait(self) -> (JobState, Option<ScanReport>) {
        let state = self.started.completion.await.unwrap_or_else(|error| {
            tracing::error!(error = %error, "the scan task ended unexpectedly");
            JobState::Failed
        });
        let report = self
            .outcome
            .lock()
            .expect("the report lock is never held across an await")
            .clone();
        (state, report)
    }
}

/// Starts a scan as a background job.
///
/// This is the one way a scan begins, whether the request came from a person
/// at a terminal or from a button, so both get the same guarantees: it shows
/// up in the list of what is running, it can be stopped, and a second scan of
/// the same library is refused rather than run alongside the first.
pub async fn start_scan(state: &AppState, library: Library) -> Result<ScanJob> {
    let outcome: Arc<Mutex<Option<ScanReport>>> = Arc::new(Mutex::new(None));
    let recorded = Arc::clone(&outcome);
    let state = state.clone();
    let target = library.id.to_string();

    let started = state
        .jobs()
        .clone()
        .start(
            JobKind::ScanLibrary,
            JobPriority::REQUESTED,
            Some(target),
            move |handle| async move {
                match scan_library(&state, &library, &handle).await {
                    Ok(report) => {
                        *recorded
                            .lock()
                            .expect("the report lock is never held across an await") =
                            Some(report.clone());
                        if !report.unusable_roots.is_empty() {
                            // A scan that skipped a disk did its job, and the
                            // skipped disk is still worth saying out loud.
                            tracing::warn!(
                                roots = report.unusable_roots.join(", "),
                                "some roots could not be walked"
                            );
                        }
                        Ok(())
                    }
                    Err(error) => Err(error.to_string()),
                }
            },
        )
        .await?;

    Ok(ScanJob { started, outcome })
}

/// Scans every root of a library.
///
/// The handle is what the job layer gives a running job: it carries progress
/// and the request to stop.
pub async fn scan_library(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<ScanReport> {
    let mut report = ScanReport::default();
    let database = state.database();

    for root in &library.roots {
        if handle.is_cancelled() {
            report.cancelled = true;
            break;
        }

        // Every scan tests the root for real rather than trusting what was
        // recorded earlier: a disk can go away between two scans.
        let access = melyxar_library::check_root_access(&root.path);
        database.set_root_access(root.id, access).await?;

        let outcome = match walk(&root.label, &root.path) {
            Ok(outcome) => outcome,
            Err(ScanError::RootUnusable { state }) => {
                tracing::warn!(
                    root = root.label,
                    state = state.as_str(),
                    "root skipped; the other roots of this library are scanned as usual"
                );
                report.unusable_roots.push(root.label.clone());
                continue;
            }
        };
        report.unreadable_folders += outcome.unreadable_folders.len();

        let (media, companions): (Vec<FoundFile>, Vec<FoundFile>) = outcome
            .files
            .into_iter()
            .partition(|file| file.companion_kind.is_none());

        record_changes(state, library, root.id, &media, &mut report).await?;
        attach_companions(database, root.id, &companions, &media, &mut report).await?;
    }

    analyse_pending(state, library, handle, &mut report).await?;

    if report.changed_anything() {
        database.bump_library_version(library.id).await?;
    }
    if handle.is_cancelled() {
        report.cancelled = true;
    }

    tracing::info!(
        library = library.name,
        added = report.added,
        changed = report.changed,
        missing = report.missing,
        restored = report.restored,
        unchanged = report.unchanged,
        analysed = report.analysed,
        extras = report.extras,
        cancelled = report.cancelled,
        "scan finished"
    );
    Ok(report)
}

/// Writes down what the walk found for one root.
async fn record_changes(
    state: &AppState,
    library: &Library,
    root_id: LibraryRootId,
    media: &[FoundFile],
    report: &mut ScanReport,
) -> Result<()> {
    let database = state.database();
    let stored = database.sources_of_root(root_id).await?;
    let known: Vec<KnownFile> = stored
        .iter()
        .map(|source| KnownFile {
            relative_path: source.relative_path.clone(),
            size_bytes: source.size_bytes,
            modified_at: source.modified_at,
        })
        .collect();
    let changes = melyxar_library::diff(media, &known);
    report.unchanged += changes.unchanged;

    let by_path: HashMap<&Path, &StoredSource> = stored
        .iter()
        .map(|source| (source.relative_path.as_path(), source))
        .collect();

    for file in &changes.added {
        let work_id = work_for(state, library, &file.relative_path).await?;
        database
            .insert_source(
                work_id,
                root_id,
                &file.relative_path,
                file.size_bytes,
                file.modified_at,
            )
            .await?;
        report.added += 1;
    }

    for file in &changes.changed {
        if let Some(source) = by_path.get(file.relative_path.as_path()) {
            database
                .refresh_source_identity(source.id, file.size_bytes, file.modified_at)
                .await?;
            report.changed += 1;
        }
    }

    for path in &changes.missing {
        if let Some(source) = by_path.get(path.as_path()) {
            if source.missing_since.is_none() {
                database.mark_source_missing(source.id).await?;
                report.missing += 1;
            }
        }
    }

    // A file that was absent and is back keeps the identifier it had, and with
    // it every position, favourite and count attached to it.
    for file in media {
        if let Some(source) = by_path.get(file.relative_path.as_path()) {
            if source.missing_since.is_some() {
                database.mark_source_present(source.id).await?;
                report.restored += 1;
            }
        }
    }

    Ok(())
}

/// Finds the work a file belongs to, or creates it.
async fn work_for(state: &AppState, library: &Library, relative_path: &Path) -> Result<WorkId> {
    let database = state.database();
    let file_name = relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let parsed = naming::parse(file_name, melyxar_core::time::current_year());
    let sort_title = naming::sort_title(&parsed.title);

    if let Some(existing) = database
        .work_by_identity(library.id, &sort_title, parsed.year)
        .await?
    {
        tracing::debug!(
            work = %MediaName::new(&existing.title),
            "another copy of a film already known"
        );
        return Ok(existing.id);
    }

    let work = database
        .create_work(
            library.id,
            work_kind_for(library.kind),
            &parsed.title,
            &sort_title,
            parsed.year,
        )
        .await?;
    Ok(work.id)
}

/// The kind of work a file in this library stands for.
///
/// Only films are laid out one file to one work today. The episodic kinds get
/// their own arrangement when series arrive, and until then a file found in
/// such a library is recorded as an episode, which is what it is.
fn work_kind_for(kind: LibraryKind) -> WorkKind {
    match kind {
        LibraryKind::Movies => WorkKind::Movie,
        LibraryKind::Series | LibraryKind::Anime | LibraryKind::Shows => WorkKind::Episode,
        LibraryKind::Music => WorkKind::Song,
    }
}

/// Attaches trailers and other clips to the film they sit next to.
async fn attach_companions(
    database: &Database,
    root_id: LibraryRootId,
    companions: &[FoundFile],
    media: &[FoundFile],
    report: &mut ScanReport,
) -> Result<()> {
    if companions.is_empty() {
        return Ok(());
    }
    let stored = database.sources_of_root(root_id).await?;
    let work_by_path: HashMap<&Path, WorkId> = stored
        .iter()
        .map(|source| (source.relative_path.as_path(), source.work_id))
        .collect();

    for companion in companions {
        let Some(kind) = companion.companion_kind else {
            continue;
        };
        // A sample clip belongs to nothing a viewer would open. It is found
        // and then left alone, which is the point of recognising it at all.
        if kind == "sample" {
            continue;
        }
        let Some(owner) = film_of(companion, media) else {
            tracing::debug!(
                file = %MediaName::new(
                    companion
                        .relative_path
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or_default()
                ),
                "a companion clip matches no film next to it and was left alone"
            );
            continue;
        };
        let Some(work_id) = work_by_path.get(owner.as_path()) else {
            continue;
        };

        database
            .store_local_extra_video(
                *work_id,
                &LocalExtraVideo {
                    kind: kind.to_string(),
                    name: None,
                    root_id,
                    relative_path: companion.relative_path.clone(),
                },
            )
            .await?;
        report.extras += 1;
    }
    Ok(())
}

/// The film a companion clip belongs to, among the files sitting beside it.
///
/// The name of the clip is the name of the film plus a marker, so taking the
/// marker off gives the film back. Two readings are tried: the exact name,
/// which is the convention, and failing that the title and year, which catches
/// a clip that dropped the technical tags.
fn film_of(companion: &FoundFile, media: &[FoundFile]) -> Option<PathBuf> {
    let file_name = companion.relative_path.file_name()?.to_str()?;
    let base = naming::without_companion_marker(file_name)?;
    let folder = companion.relative_path.parent();

    let neighbours = media
        .iter()
        .filter(|file| file.relative_path.parent() == folder);

    let exact = neighbours.clone().find(|file| {
        file.relative_path.file_name().and_then(|n| n.to_str()) == Some(base.as_str())
    });
    if let Some(file) = exact {
        return Some(file.relative_path.clone());
    }

    let year = melyxar_core::time::current_year();
    let wanted = naming::parse(&base, year);
    neighbours
        .filter(|file| {
            file.relative_path
                .file_name()
                .and_then(|name| name.to_str())
                .map(|name| naming::parse(name, year))
                .is_some_and(|parsed| {
                    parsed.year == wanted.year
                        && naming::sort_title(&parsed.title) == naming::sort_title(&wanted.title)
                })
        })
        .map(|file| file.relative_path.clone())
        .next()
}

/// Analyses every file of the library that carries no analysis yet.
///
/// Kept apart from the walk on purpose: analysing is the slow half, and it has
/// to be able to run after a scan that was stopped, or on files that were added
/// long ago and never got looked at.
async fn analyse_pending(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
    report: &mut ScanReport,
) -> Result<()> {
    let Some(tools) = state.tools() else {
        tracing::warn!(
            library = library.name,
            "no media tool available, the files were recorded but not analysed"
        );
        return Ok(());
    };

    let database = state.database();
    let mut pending: Vec<PendingFile> = Vec::new();
    for root in &library.roots {
        for source in database.unanalysed_sources_of_root(root.id).await? {
            pending.push(PendingFile {
                root_label: root.label.clone(),
                root_path: root.path.clone(),
                source_id: source.id,
                relative_path: source.relative_path.clone(),
            });
        }
    }
    if pending.is_empty() {
        return Ok(());
    }

    handle.set_total(pending.len() as i64).await;
    let analyser = tools.ffprobe.clone();
    let limit = state.config().limits.concurrent_probes;
    // Each file is analysed on its own task, so what they work with is owned
    // rather than borrowed from a scan that may well finish first.
    let owned_database = database.clone();
    let owned_handle = handle.clone();

    // Bounded on purpose: an unbounded scan would take the machine away from
    // the playback it is supposed to leave untouched.
    let outcomes = melyxar_jobs::for_each_bounded(pending, limit, move |file| {
        let analyser = analyser.clone();
        let handle = owned_handle.clone();
        let database = owned_database.clone();
        async move {
            if handle.is_cancelled() {
                return Outcome::Stopped;
            }
            let outcome = analyse_one(&database, &analyser, &file)
                .await
                .unwrap_or_else(|error| {
                    tracing::warn!(error = %error, "a file could not be recorded after analysis");
                    Outcome::Unreadable
                });
            handle.advance(1).await;
            outcome
        }
    })
    .await;

    for outcome in outcomes {
        match outcome {
            Outcome::Analysed => report.analysed += 1,
            Outcome::Unreadable => report.unreadable_files += 1,
            Outcome::Stopped => report.cancelled = true,
        }
    }
    Ok(())
}

/// A file waiting to be analysed, with everything needed to reach it.
struct PendingFile {
    root_label: String,
    root_path: PathBuf,
    source_id: MediaSourceId,
    relative_path: PathBuf,
}

enum Outcome {
    Analysed,
    Unreadable,
    Stopped,
}

async fn analyse_one(database: &Database, analyser: &Path, file: &PendingFile) -> Result<Outcome> {
    let full_path = file.root_path.join(&file.relative_path);
    let report = match melyxar_ffmpeg::probe::probe(analyser, &full_path).await {
        Ok(report) => report,
        Err(error) => {
            tracing::warn!(
                file = %MediaPath::new(&file.root_label, &full_path),
                error = %error,
                "the analyser could not read this file"
            );
            return Ok(Outcome::Unreadable);
        }
    };

    let analysed = melyxar_media_probe::AnalysedFile::from_report(&report, file.source_id);
    database
        .store_analysis(
            file.source_id,
            &SourceAnalysis {
                container: analysed.container.clone(),
                duration: analysed.duration,
                overall_bitrate: analysed.overall_bitrate,
            },
            &analysed.tracks,
            &analysed.chapters,
        )
        .await
        .map_err(AppError::from)?;
    Ok(Outcome::Analysed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories, LibraryConfig, RootConfig};
    use melyxar_database::Database;
    use std::process::Command;

    /// A state whose only library points at the given folders.
    async fn state_with_roots(
        directory: &Path,
        roots: Vec<(&str, PathBuf)>,
    ) -> (AppState, Library) {
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
                roots: roots
                    .into_iter()
                    .map(|(label, path)| RootConfig {
                        label: label.to_string(),
                        path,
                    })
                    .collect(),
            }],
            ..Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");

        let database = Database::open_in_memory().await.expect("database opens");
        crate::startup::reconcile_libraries(&database, &config)
            .await
            .expect("libraries reconciled");
        let library = database
            .library_by_name("Films")
            .await
            .expect("read")
            .expect("the library was declared");

        let (tools, capabilities) = crate::startup::detect_media_tools(&config).await;
        (
            AppState::new(config, database, tools, capabilities),
            library,
        )
    }

    /// Runs a scan the way the server does, and gives back what it did.
    async fn scan(state: &AppState, library: &Library) -> ScanReport {
        let (job_state, report) = start_scan(state, library.clone())
            .await
            .expect("job started")
            .wait()
            .await;
        assert_eq!(job_state, JobState::Succeeded, "the scan ran to the end");
        report.expect("a finished scan has a report")
    }

    fn write(root: &Path, relative: &str, contents: &[u8]) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("folder created");
        }
        std::fs::write(path, contents).expect("file written");
    }

    /// Writes a real, tiny film. Returns false when the tools are missing, so
    /// the test says so rather than pretending to have checked something.
    fn write_real_video(path: &Path) -> bool {
        let Ok(output) = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=320x240:rate=10",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=1",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(path)
            .output()
        else {
            return false;
        };
        output.status.success()
    }

    #[tokio::test]
    async fn a_first_scan_records_every_film_it_finds() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        write(&media, "Amber.Field.2020.MULTi.2160p.mkv", b"xx");
        write(&media, "cover.jpg", b"x");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let report = scan(&state, &library).await;

        assert_eq!(report.added, 2);
        assert_eq!(report.unchanged, 0);
        assert_eq!(
            state
                .database()
                .count_works(library.id)
                .await
                .expect("read"),
            2
        );
        let works = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read");
        assert!(works.iter().any(|work| work.title == "Quiet Harbour"
            && work.release_year == Some(2019)
            && work.kind == WorkKind::Movie));
    }

    #[tokio::test]
    async fn scanning_twice_changes_nothing_at_all() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let second = scan(&state, &library).await;

        assert_eq!(second.added, 0);
        assert_eq!(second.unchanged, 1);
        assert_eq!(
            state
                .database()
                .count_works(library.id)
                .await
                .expect("read"),
            1,
            "a second scan must not double the library"
        );
    }

    #[tokio::test]
    async fn two_copies_of_one_film_are_two_files_of_one_work() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        write(&media, "Quiet.Harbour.2019.MULTi.2160p.HDR.mkv", b"xx");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let report = scan(&state, &library).await;

        assert_eq!(report.added, 2);
        assert_eq!(
            state
                .database()
                .count_works(library.id)
                .await
                .expect("read"),
            1,
            "the same title twice in a grid is the defect a version chooser exists to avoid"
        );
    }

    #[tokio::test]
    async fn a_film_that_is_gone_is_marked_absent_and_comes_back_with_its_identifier() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");

        let (state, library) =
            state_with_roots(directory.path(), vec![("disk-one", media.clone())]).await;
        scan(&state, &library).await;
        let source_id = state
            .database()
            .sources_of_root(library.roots[0].id)
            .await
            .expect("read")[0]
            .id;

        std::fs::remove_file(media.join("Quiet.Harbour.2019.MULTi.1080p.mkv")).expect("removed");
        let gone = scan(&state, &library).await;
        assert_eq!(gone.missing, 1);
        let stored = state
            .database()
            .sources_of_root(library.roots[0].id)
            .await
            .expect("read");
        assert_eq!(stored.len(), 1, "a scan never deletes");
        assert!(stored[0].missing_since.is_some());

        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        let back = scan(&state, &library).await;
        assert_eq!(back.restored, 1);
        let stored = state
            .database()
            .sources_of_root(library.roots[0].id)
            .await
            .expect("read");
        assert_eq!(
            stored[0].id, source_id,
            "keeping the identifier is what keeps the watch history"
        );
        assert!(stored[0].missing_since.is_none());
    }

    #[tokio::test]
    async fn one_unusable_root_never_empties_the_others() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let good = directory.path().join("films");
        write(&good, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");

        let (state, library) = state_with_roots(
            directory.path(),
            vec![
                ("disk-one", good),
                ("disk-two", PathBuf::from("/nowhere/at/all")),
            ],
        )
        .await;

        let report = scan(&state, &library).await;
        assert_eq!(report.unusable_roots, vec!["disk-two".to_string()]);
        assert_eq!(
            report.added, 1,
            "an unplugged disk must not stop the ones that are there"
        );

        // And a second scan with the disk still gone must not mark the films
        // of the other disk absent.
        let second = scan(&state, &library).await;
        assert_eq!(second.missing, 0);
        assert_eq!(second.unchanged, 1);
    }

    #[tokio::test]
    async fn a_replaced_file_is_noticed_and_analysed_again() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");

        let (state, library) =
            state_with_roots(directory.path(), vec![("disk-one", media.clone())]).await;
        scan(&state, &library).await;

        write(
            &media,
            "Quiet.Harbour.2019.MULTi.1080p.mkv",
            b"a much bigger file",
        );
        let report = scan(&state, &library).await;
        assert_eq!(report.changed, 1);
        assert_eq!(report.added, 0);
    }

    #[tokio::test]
    async fn a_trailer_is_attached_to_its_film_rather_than_becoming_one() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p-trailer.mkv", b"x");
        write(&media, "sample.mkv", b"x");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let report = scan(&state, &library).await;

        assert_eq!(report.added, 1, "a trailer is not a film");
        assert_eq!(report.extras, 1);
        assert_eq!(
            state
                .database()
                .count_works(library.id)
                .await
                .expect("read"),
            1
        );

        let works = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read");
        let extras = state
            .database()
            .extra_videos_of_work(works[0].id)
            .await
            .expect("read");
        assert_eq!(extras.len(), 1);
        assert_eq!(extras[0].kind, "trailer");
    }

    #[tokio::test]
    async fn a_trailer_that_dropped_the_technical_tags_still_finds_its_film() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.BluRay.mkv", b"x");
        write(&media, "Quiet.Harbour.2019.1080p-trailer.mkv", b"x");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let report = scan(&state, &library).await;

        assert_eq!(report.added, 1);
        assert_eq!(report.extras, 1);
    }

    #[tokio::test]
    async fn a_file_the_analyser_cannot_read_is_counted_and_the_scan_carries_on() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(
            &media,
            "Quiet.Harbour.2019.MULTi.1080p.mkv",
            b"not a film at all",
        );
        write(
            &media,
            "Amber.Field.2020.MULTi.1080p.mkv",
            b"neither is this",
        );

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let report = scan(&state, &library).await;

        assert_eq!(report.added, 2);
        if state.tools().is_some() {
            assert_eq!(report.unreadable_files, 2);
            assert_eq!(report.analysed, 0);
        }
    }

    #[tokio::test]
    async fn a_real_film_gives_up_its_streams_and_they_are_stored() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        std::fs::create_dir_all(&media).expect("folder created");
        if !write_real_video(&media.join("Quiet.Harbour.2019.MULTi.1080p.mkv")) {
            eprintln!("no media tool here, the analysis of a real file was not exercised");
            return;
        }

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let report = scan(&state, &library).await;

        assert_eq!(report.added, 1);
        assert_eq!(report.analysed, 1);

        let source = state
            .database()
            .sources_of_root(library.roots[0].id)
            .await
            .expect("read")[0]
            .clone();
        let tracks = state
            .database()
            .tracks_of_source(source.id)
            .await
            .expect("read");
        assert_eq!(tracks.len(), 2, "one picture and one sound");
        assert!(tracks.iter().any(
            |track| matches!(&track.kind, melyxar_core::media::TrackKind::Video(details)
                if details.width == 320 && details.height == 240)
        ));

        // And the file is not analysed a second time for nothing.
        let second = scan(&state, &library).await;
        assert_eq!(second.analysed, 0);
        assert_eq!(second.unchanged, 1);
    }

    #[tokio::test]
    async fn a_scan_started_as_a_job_reports_what_it_did() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let job = start_scan(&state, library.clone())
            .await
            .expect("job started");
        let (state_at_end, report) = job.wait().await;

        assert_eq!(state_at_end, JobState::Succeeded);
        assert_eq!(report.expect("a finished scan has a report").added, 1);
    }

    #[tokio::test]
    async fn two_scans_of_one_library_never_run_at_once() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let first = start_scan(&state, library.clone())
            .await
            .expect("job started");
        let second = start_scan(&state, library.clone()).await;
        assert!(
            second.is_err(),
            "two scans of one library would walk over each other"
        );
        first.wait().await;
    }

    #[tokio::test]
    async fn a_scan_moves_the_version_counter_only_when_something_moved() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let before = state
            .database()
            .library_version(library.id)
            .await
            .expect("read");
        scan(&state, &library).await;
        let after_first = state
            .database()
            .library_version(library.id)
            .await
            .expect("read");
        assert!(after_first > before);

        scan(&state, &library).await;
        assert_eq!(
            state
                .database()
                .library_version(library.id)
                .await
                .expect("read"),
            after_first,
            "a scan that found nothing new must not make every client refetch"
        );
    }
}
