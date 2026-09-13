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

use melyxar_core::id::{JobId, LibraryRootId, MediaSourceId, TrackId, WorkId};
use melyxar_core::job::{JobKind, JobPriority, JobState};
use melyxar_core::library::{Library, LibraryKind};
use melyxar_core::media::{SubtitleDetails, Track, TrackKind};
use melyxar_core::privacy::{MediaName, MediaPath};
use melyxar_core::work::WorkKind;
use melyxar_database::catalogue::{LocalExtraVideo, SourceAnalysis, StoredSource};
use melyxar_database::Database;
use melyxar_jobs::{JobHandle, StartedJob};
use melyxar_library::scan::{walk, FoundFile, KnownFile, ScanError};
use melyxar_library::{naming, sidecar};

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
    /// Works whose title was read again from their file name and came out
    /// different, because the rules that read file names improved.
    pub renamed: usize,
    /// Works that turned out to be another copy of a film already here, and
    /// whose files joined it rather than standing as a film of their own.
    pub merged: usize,
    pub extras: usize,
    /// Subtitle files attached to the film they sit next to.
    pub external_subtitles: usize,
    /// Description files that gave up an identifier. Zero unless the server
    /// was asked to read them.
    pub companion_files_read: usize,
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
        self.added > 0
            || self.changed > 0
            || self.missing > 0
            || self.restored > 0
            || self.renamed > 0
            || self.merged > 0
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

/// Scans a library and looks up what the scan found.
///
/// Answers the scan's own job, which is what a client follows: the look up is
/// a second job that appears when the first ends. They stay apart because they
/// fail for different reasons, and a provider that is down must not make a
/// scan look failed.
///
/// Chained because a scan that finds forty films and leaves every one of them
/// unnamed has done half of what anybody wanted. Nothing is looked up when no
/// provider key is configured, which is a server that browses without one
/// rather than a server that is broken.
pub async fn start_scan_and_identification(state: &AppState, library: Library) -> Result<JobId> {
    let scan = start_scan(state, library.clone()).await?;
    let id = scan.id();

    // Without a provider key there is nothing to look anything up with. The
    // scan still runs, and the library still browses: that is a server
    // configured without a key, not a broken one.
    let Some(provider) = state.metadata_provider() else {
        return Ok(id);
    };

    let waiting = state.clone();
    tokio::spawn(async move {
        let (ended, _) = scan.wait().await;
        if ended != JobState::Succeeded {
            // A scan that did not finish leaves nothing dependable to look up,
            // and whoever reads the list of jobs can already see why.
            return;
        }
        if let Err(error) = crate::identify::start_identification(&waiting, provider, library).await
        {
            tracing::warn!(%error, "the scan finished but the look up would not start");
        }
    });

    Ok(id)
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
    // Read before anything is recorded, so a file added today is named by the
    // same rules as the ones already here.
    let signs = signs_of(state, library).await?;

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

        record_changes(state, library, root.id, &media, &signs, &mut report).await?;
        attach_companions(database, root.id, &companions, &media, &mut report).await?;
        attach_subtitles(database, root.id, &outcome.subtitles, &mut report).await?;

        if state.config().scan.read_companion_files {
            read_companion_files(
                database,
                &root.label,
                &root.path,
                root.id,
                &outcome.companion_files,
                &mut report,
            )
            .await?;
        }
    }

    let reread = reread_names_of_nameless_works(state, library).await?;
    report.renamed = reread.renamed;
    report.merged = reread.merged;
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
        subtitles = report.external_subtitles,
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
    signs: &naming::LibrarySigns,
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
        let work_id = work_for(state, library, &file.relative_path, signs).await?;
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

/// Reads the file names of the works nobody has named, and keeps what changed.
///
/// A work waiting to be identified has never been given anything but the name
/// of its file, and the rules that read those names get better. Without this,
/// a collection scanned before an improvement keeps for ever the mangled
/// titles that are exactly why a provider recognised none of it, and the only
/// way out would be to throw the database away.
///
/// A title a provider gave, or a person chose by hand, is never touched.
///
/// A name that now reads as a film already in the library is not a second
/// film: its file joins that one. Two copies whose names differed only by
/// something a tool stuck on the front are one film with two copies, and
/// leaving them apart puts the same title twice in a grid.
pub(crate) async fn reread_names_of_nameless_works(
    state: &AppState,
    library: &Library,
) -> Result<Reread> {
    let database = state.database();
    let year = melyxar_core::time::current_year();
    let signs = signs_of(state, library).await?;
    let mut done = Reread::default();

    for work in database.works_named_after_their_file(library.id).await? {
        let file_name = work
            .relative_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        let parsed = naming::parse_signed(file_name, year, &signs);
        if parsed.title == work.title && parsed.year == work.release_year {
            continue;
        }
        let sort_title = naming::sort_title(&parsed.title);

        if let Some(twin) = database
            .work_by_identity(library.id, &sort_title, parsed.year)
            .await?
            .filter(|twin| twin.id != work.id)
        {
            join_work_into(state, work.id, twin.id).await?;
            tracing::info!(
                work = %MediaName::new(&parsed.title),
                "two copies of one film read as one film now"
            );
            done.merged += 1;
            continue;
        }

        database
            .rename_work(work.id, &parsed.title, &sort_title, parsed.year)
            .await?;
        tracing::info!(
            was = %MediaName::new(&work.title),
            now = %MediaName::new(&parsed.title),
            "a film still waiting to be named reads differently now"
        );
        done.renamed += 1;
    }

    Ok(done)
}

/// Joins one work to another, and clears what the one that went had cached.
///
/// The pictures of a work are found by owner rather than by a key the engine
/// knows about, so dropping the work leaves their files behind. They are
/// removed here, where the folder they live in is known.
pub(crate) async fn join_work_into(state: &AppState, from: WorkId, into: WorkId) -> Result<()> {
    let no_longer_used = state.database().merge_work_into(from, into).await?;
    let images = state.config().directories.images();
    for path in no_longer_used {
        tokio::fs::remove_file(images.join(path)).await.ok();
    }
    Ok(())
}

/// Takes one copy away from the film it sits on, as a film of its own.
///
/// Copies are put together on their own, by the rules that read names and by
/// what the provider answers, and both can be wrong about a file. Whoever is
/// looking at the page can see that two copies are not the same film at all,
/// and this is how they say so: the copy leaves, named after its own file, and
/// waits to be looked up like any film a scan has just found.
///
/// Answers nothing when the film holds this one copy and no other, which is a
/// film to identify again rather than one to take apart.
pub async fn detach_copy(state: &AppState, source_id: MediaSourceId) -> Result<Option<WorkId>> {
    let database = state.database();
    let Some((relative_path, library_id)) = database.where_a_source_lives(source_id).await? else {
        return Ok(None);
    };
    let library = database
        .list_libraries()
        .await?
        .into_iter()
        .find(|library| library.id == library_id)
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("library")))?;

    let signs = signs_of(state, &library).await?;
    let file_name = relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let parsed = naming::parse_signed(file_name, melyxar_core::time::current_year(), &signs);

    let detached = database
        .detach_source(
            source_id,
            work_kind_for(library.kind),
            &parsed.title,
            &naming::sort_title(&parsed.title),
            parsed.year,
        )
        .await?;

    if detached.is_some() {
        database.bump_library_version(library.id).await?;
    }
    Ok(detached.map(|work| work.id))
}

/// What reading the file names again changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Reread {
    /// Works whose title came out different.
    pub renamed: usize,
    /// Works that turned out to be a copy of a film already in the library.
    pub merged: usize,
}

/// What the names of this library carry, read off the library itself.
///
/// Asked for once per scan rather than per file: such a sign is only visible
/// across the whole set of names, and reading them again for every file would
/// be a query per film.
async fn signs_of(state: &AppState, library: &Library) -> Result<naming::LibrarySigns> {
    let names = state.database().source_names_of_library(library.id).await?;
    Ok(naming::signs_in(&names))
}

/// Finds the work a file belongs to, or creates it.
async fn work_for(
    state: &AppState,
    library: &Library,
    relative_path: &Path,
    signs: &naming::LibrarySigns,
) -> Result<WorkId> {
    let database = state.database();
    let file_name = relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let parsed = naming::parse_signed(file_name, melyxar_core::time::current_year(), signs);
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

/// Reads the description files sitting next to the media, when the server was
/// asked to.
///
/// Only the identifiers at the providers are kept. A title or a synopsis taken
/// from such a file would be believed outright and would outrank what the
/// provider says, whereas an identifier is something a provider can be asked
/// about and can disagree with.
async fn read_companion_files(
    database: &Database,
    root_label: &str,
    root_path: &Path,
    root_id: LibraryRootId,
    companion_files: &[PathBuf],
    report: &mut ScanReport,
) -> Result<()> {
    if companion_files.is_empty() {
        return Ok(());
    }
    let stored = database.sources_of_root(root_id).await?;

    for source in &stored {
        let Some(stem) = source.relative_path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let folder = source.relative_path.parent();

        // Either a file carrying the film's own name, or the one some tools
        // write under a fixed name in a folder holding a single film.
        let Some(path) = companion_files.iter().find(|path| {
            path.parent() == folder
                && path
                    .file_stem()
                    .and_then(|value| value.to_str())
                    .is_some_and(|value| value == stem || value.eq_ignore_ascii_case("movie"))
        }) else {
            continue;
        };

        let full_path = root_path.join(path);
        let contents = match tokio::fs::read_to_string(&full_path).await {
            Ok(contents) => contents,
            Err(error) => {
                tracing::debug!(
                    file = %MediaPath::new(root_label, &full_path),
                    error = %error,
                    "a description file could not be read"
                );
                continue;
            }
        };

        let ids = melyxar_library::companion::read_ids(&contents);
        if ids.is_empty() {
            continue;
        }
        if let Some(tmdb) = &ids.tmdb {
            database
                .set_work_external_id(source.work_id, "tmdb", tmdb)
                .await?;
        }
        if let Some(imdb) = &ids.imdb {
            database
                .set_work_external_id(source.work_id, "imdb", imdb)
                .await?;
        }
        report.companion_files_read += 1;
    }
    Ok(())
}

/// Records the subtitle files sitting next to the media of one root.
///
/// A subtitle in its own file is a track of the film, so it is attached to the
/// file it belongs to rather than kept in a corner of its own. The whole set
/// is rewritten on every scan, which is what makes adding or removing one of
/// them show up without any bookkeeping.
async fn attach_subtitles(
    database: &Database,
    root_id: LibraryRootId,
    subtitles: &[PathBuf],
    report: &mut ScanReport,
) -> Result<()> {
    let stored = database.sources_of_root(root_id).await?;
    if stored.is_empty() {
        return Ok(());
    }
    let had_subtitles = database.sources_with_external_subtitles(root_id).await?;

    for source in &stored {
        let Some(stem) = source.relative_path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let folder = source.relative_path.parent();

        let mut tracks = Vec::new();
        for path in subtitles {
            if path.parent() != folder {
                continue;
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            // The convention is the film's own name followed by what the
            // subtitle is: language, and whether it is forced or for viewers
            // who are hard of hearing.
            let Some(remainder) = name.strip_prefix(stem) else {
                continue;
            };
            let remainder = remainder
                .strip_suffix(&format!(
                    ".{}",
                    path.extension()
                        .and_then(|value| value.to_str())
                        .unwrap_or_default()
                ))
                .unwrap_or(remainder);
            let Some(found) = sidecar::read(name, remainder) else {
                continue;
            };

            tracks.push(Track {
                id: TrackId::new(),
                source_id: source.id,
                // A file of its own holds one stream, and it is addressed by
                // its path rather than by a position in a container.
                stream_index: 0,
                language: found.language.clone(),
                title: None,
                is_default: false,
                is_forced: found.is_forced,
                kind: TrackKind::Subtitle(SubtitleDetails {
                    codec: found.codec.to_string(),
                    layout: found.layout,
                    is_hearing_impaired: found.is_hearing_impaired,
                    is_external: true,
                    external_relative_path: Some(path.clone()),
                }),
            });
        }

        // Nothing found and nothing stored means nothing to do. Nothing found
        // where something was stored means a subtitle was taken away, and that
        // has to be written down like any other change.
        if tracks.is_empty() && !had_subtitles.contains(&source.id) {
            continue;
        }
        report.external_subtitles += tracks.len();
        database
            .store_external_subtitles(source.id, &tracks)
            .await?;
    }
    Ok(())
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
            // Written down rather than only said: a reason in a log line is
            // gone by the time anybody asks, and this is the one thing that
            // says what to do about a film that fails when it is played.
            database
                .record_analysis_failure(file.source_id, &error.to_string())
                .await?;
            return Ok(Outcome::Unreadable);
        }
    };

    // Said out loud for every file that has anything to say, because nothing
    // else ever will: the container declares where each stream starts and how
    // long it runs, and until now nobody read either. The complaint that
    // follows is always about one film in particular, so the name travels with
    // it, censored like every other name in a log.
    let lining_up = melyxar_media_probe::HowTheStreamsLineUp::of(&report);
    if lining_up.is_worth_saying() {
        tracing::info!(
            file = %MediaName::new(
                file.relative_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default()
            ),
            video_starts_at_ms = lining_up.video_starts_at,
            audio_starts_at_ms = lining_up.audio_starts_at,
            sound_after_picture_ms = lining_up.offset(),
            video_runs_for_ms = lining_up.video_runs_for,
            audio_runs_for_ms = lining_up.audio_runs_for,
            sound_longer_by_ms = lining_up.drift(),
            "the picture and the sound of this file do not line up"
        );
    }

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
        state_with(directory, roots, false).await
    }

    /// The same, saying whether description files may be read.
    async fn state_with(
        directory: &Path,
        roots: Vec<(&str, PathBuf)>,
        read_companion_files: bool,
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
            scan: melyxar_config::ScanConfig {
                read_companion_files,
            },
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

    /// The little a provider has to say for a work to stop being described by
    /// its file name.
    fn named(title: &str) -> melyxar_database::metadata::IdentifiedWork {
        melyxar_database::metadata::IdentifiedWork {
            provider: "tmdb".to_string(),
            external_id: "111".to_string(),
            imdb_id: None,
            language: "fr".to_string(),
            sort_title: naming::sort_title(title),
            title: title.to_string(),
            tagline: None,
            overview: None,
            release_year: Some(1999),
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
    async fn a_film_still_waiting_to_be_named_has_its_file_name_read_again() {
        // What a collection scanned before the rules improved looks like: the
        // work carries the whole file name, which is exactly why no provider
        // recognised it. The file has not moved, so nothing else would ever
        // look at its name again.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet Harbour 2160p SOMEGROUP.mkv", b"x");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;

        let work = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read")
            .pop()
            .expect("one film");
        state
            .database()
            .rename_work(
                work.id,
                "Quiet Harbour 2160p SOMEGROUP",
                "quiet harbour 2160p somegroup",
                None,
            )
            .await
            .expect("renamed to what the old rules read");

        let report = scan(&state, &library).await;
        assert_eq!(report.renamed, 1);
        assert_eq!(
            state
                .database()
                .work(work.id)
                .await
                .expect("read")
                .expect("present")
                .title,
            "Quiet Harbour",
            "the film keeps its identifier, its file and its history, and gains a name"
        );

        let settled = scan(&state, &library).await;
        assert_eq!(
            settled.renamed, 0,
            "a name that already reads correctly is not written again"
        );
    }

    #[tokio::test]
    async fn a_library_learns_the_word_its_owner_signs_files_with() {
        // The case no rule of shape can reach: a title written wholly in
        // capitals, signed with a word that is also in capitals. What tells
        // them apart is that the signature is on the other films too.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "QUIET HARBOUR CONTRE ATTAQUE SOMEGROUP.mkv", b"x");
        write(&media, "Amber Field (2020) 1080p SOMEGROUP.mkv", b"x");
        write(&media, "Winter Signal 2160p SOMEGROUP.mkv", b"x");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        // The first scan has nothing to read the signature off yet, and the
        // pass that reads names again, which runs at the end of that same
        // scan, is what puts it right.
        scan(&state, &library).await;

        let titles: Vec<String> = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read")
            .into_iter()
            .map(|work| work.title)
            .collect();
        assert!(
            titles.contains(&"QUIET HARBOUR CONTRE ATTAQUE".to_string()),
            "{titles:?}"
        );
        assert!(
            !titles.iter().any(|title| title.contains("SOMEGROUP")),
            "the signature belongs to no title: {titles:?}"
        );
    }

    #[tokio::test]
    async fn copies_named_with_something_stuck_on_the_front_are_one_film() {
        // Copies of one film whose names differ only by a few characters at
        // the very front. Nothing about those characters says what they are;
        // what says it is that the same name is there without them.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet Harbour BD.Rip 1080 x264.mkv", b"x");
        write(&media, "zz12Quiet Harbour BD.Rip 1080 x264.mkv", b"xx");
        write(&media, "wxyzQuiet Harbour BD.Rip 1080 x264.mkv", b"xxx");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let report = scan(&state, &library).await;
        assert_eq!(report.added, 3, "three files");

        let works = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read");
        assert_eq!(
            works.len(),
            1,
            "one film, whatever was stuck to the front of its copies: {:?}",
            works.iter().map(|work| &work.title).collect::<Vec<_>>()
        );
        assert_eq!(works[0].title, "Quiet Harbour");
        assert_eq!(
            state
                .database()
                .sources_of_root(library.roots[0].id)
                .await
                .expect("read")
                .len(),
            3,
            "three copies of it, and not one file lost"
        );

        let settled = scan(&state, &library).await;
        assert_eq!(settled.merged, 0, "there is nothing left to merge");
        assert_eq!(settled.renamed, 0);
    }

    #[tokio::test]
    async fn a_copy_taken_away_becomes_a_film_named_after_its_own_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet Harbour (2019) 1080p.mkv", b"x");
        write(&media, "Amber Field (2020) 1080p.mkv", b"xx");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;

        // The state a grouping that got it wrong leaves behind: two files that
        // are not the same film at all, on one film.
        let works = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read");
        let (kept, wrong) = (works[0].id, works[1].id);
        let leaving = state.database().sources_of_work(wrong).await.expect("read")[0].id;
        state
            .database()
            .merge_work_into(wrong, kept)
            .await
            .expect("put together, wrongly");

        let detached = detach_copy(&state, leaving)
            .await
            .expect("the copy leaves")
            .expect("a film held twice can give one of them up");

        let film = state
            .database()
            .work(detached)
            .await
            .expect("read")
            .expect("present");
        assert!(
            film.title == "Quiet Harbour" || film.title == "Amber Field",
            "named after its own file and nothing else: {}",
            film.title
        );
        assert_eq!(
            state
                .database()
                .count_works(library.id)
                .await
                .expect("read"),
            2,
            "two films again"
        );
    }

    #[tokio::test]
    async fn a_run_of_films_with_names_built_on_one_another_stays_a_run_of_films() {
        // The shape a series has: one title, its plural, its number, and a
        // word added. Nothing here may ever be put together, whatever is stuck
        // to the front of the copies.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        for name in [
            "Harbour BD.Rip 1080 x264.mkv",
            "Harbours BD.Rip 1080 x264.mkv",
            "Harbour II BD.Rip 1080 x264.mkv",
            "Harbour Rising BD.Rip 1080 x264.mkv",
            "zz12Harbour BD.Rip 1080 x264.mkv",
        ] {
            write(&media, name, name.as_bytes());
        }

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;

        let mut titles: Vec<String> = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read")
            .into_iter()
            .map(|work| work.title)
            .collect();
        titles.sort();
        assert_eq!(
            titles,
            vec![
                "Harbour".to_string(),
                "Harbour II".to_string(),
                "Harbour Rising".to_string(),
                "Harbours".to_string(),
            ],
            "four films, and the fifth file is a second copy of the first"
        );
    }

    #[tokio::test]
    async fn a_second_copy_of_a_film_joins_the_first_rather_than_doubling_it() {
        // The reason the signature has to be known while a file is recorded
        // and not only afterwards: a copy added later must read as the same
        // title, or it becomes a second film in the grid.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "QUIET HARBOUR SOMEGROUP.mkv", b"x");
        write(&media, "Amber Field (2020) 1080p SOMEGROUP.mkv", b"x");
        write(&media, "Winter Signal 2160p SOMEGROUP.mkv", b"x");

        let (state, library) =
            state_with_roots(directory.path(), vec![("disk-one", media.clone())]).await;
        scan(&state, &library).await;
        let before = state
            .database()
            .count_works(library.id)
            .await
            .expect("read");

        write(&media, "QUIET HARBOUR SOMEGROUP 1080p.mkv", b"xx");
        scan(&state, &library).await;

        assert_eq!(
            state
                .database()
                .count_works(library.id)
                .await
                .expect("read"),
            before,
            "one film, two files"
        );
    }

    #[tokio::test]
    async fn a_film_a_provider_named_is_never_renamed_after_its_file() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet Harbour 2160p SOMEGROUP.mkv", b"x");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let work = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read")
            .pop()
            .expect("one film");
        state
            .database()
            .apply_identification(work.id, &named("Un Autre Titre"), true)
            .await
            .expect("chosen by hand");

        assert_eq!(scan(&state, &library).await.renamed, 0);
        assert_eq!(
            state
                .database()
                .work(work.id)
                .await
                .expect("read")
                .expect("present")
                .title,
            "Un Autre Titre",
            "a title somebody chose is never undone by a file name"
        );
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
    async fn a_file_can_be_asked_for_again_without_being_touched() {
        // What the analyser is asked to read grows, and a collection analysed
        // by an older build keeps the gaps that build left. Nothing would ever
        // look at those files again, since an analysis is kept once it is done.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        std::fs::create_dir_all(&media).expect("folder created");
        if !write_real_video(&media.join("Quiet.Harbour.2019.MULTi.1080p.mkv")) {
            eprintln!("no media tool here, reading a real file again was not exercised");
            return;
        }

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        assert_eq!(scan(&state, &library).await.analysed, 1);
        assert_eq!(scan(&state, &library).await.analysed, 0, "kept once done");

        let forgotten = state
            .database()
            .forget_analysis(library.id)
            .await
            .expect("the analysis is forgotten");
        assert_eq!(forgotten, 1);

        let again = scan(&state, &library).await;
        assert_eq!(again.analysed, 1, "and read again when asked");
        assert_eq!(again.added, 0, "the file itself was never touched");

        let source = state
            .database()
            .sources_of_root(library.roots[0].id)
            .await
            .expect("read")[0]
            .clone();
        assert_eq!(
            state
                .database()
                .tracks_of_source(source.id)
                .await
                .expect("read")
                .len(),
            2,
            "reading a file again replaces what it said, and never doubles it"
        );
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
    async fn a_trailer_goes_to_its_own_film_and_not_to_the_one_next_to_it() {
        // Films are laid flat in one folder here, so a trailer is surrounded by
        // other films. Two of them came out the same year, which is the case
        // where a rule that leans on the year alone attaches it to the wrong
        // one: a viewer then opens a film and is offered somebody else's
        // trailer.
        // The film the trailer belongs to is deliberately not the first of the
        // folder: a rule that settles for the first neighbour that matches
        // anything would pick the other one.
        for trailer in [
            // Named exactly after its film, tags and all.
            "Quiet.Harbour.2019.MULTi.1080p.BluRay-trailer.mkv",
            // The same trailer with the technical tags dropped, which is how
            // most of them are actually named.
            "Quiet.Harbour.2019.1080p-trailer.mkv",
        ] {
            let directory = tempfile::tempdir().expect("temporary directory");
            let media = directory.path().join("films");
            write(&media, "Amber.Field.2019.MULTi.1080p.BluRay.mkv", b"x");
            write(&media, "Quiet.Harbour.2019.MULTi.1080p.BluRay.mkv", b"x");
            write(&media, trailer, b"x");

            let (state, library) =
                state_with_roots(directory.path(), vec![("disk-one", media)]).await;
            let report = scan(&state, &library).await;
            assert_eq!(report.added, 2, "{trailer}");
            assert_eq!(report.extras, 1, "{trailer}");

            let page = state
                .database()
                .browse_works(&melyxar_database::browse::BrowseRequest {
                    library_id: Some(library.id),
                    ..Default::default()
                })
                .await
                .expect("read");
            for card in &page.cards {
                let extras = state
                    .database()
                    .extra_videos_of_work(card.id)
                    .await
                    .expect("read");
                match card.title.starts_with("Quiet") {
                    true => assert_eq!(extras.len(), 1, "the trailer belongs to this one: {trailer}"),
                    false => assert!(
                        extras.is_empty(),
                        "this film has no trailer, and being next to one is not having one: {trailer}"
                    ),
                }
            }
        }
    }

    #[tokio::test]
    async fn a_subtitle_next_to_a_film_becomes_one_of_its_tracks() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.fr.srt", b"subtitle");
        write(
            &media,
            "Quiet.Harbour.2019.MULTi.1080p.en.forced.srt",
            b"subtitle",
        );

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let report = scan(&state, &library).await;

        assert_eq!(report.added, 1, "a subtitle is not a film");
        assert_eq!(report.external_subtitles, 2);

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
        let subtitles: Vec<_> = tracks
            .iter()
            .filter_map(|track| match &track.kind {
                melyxar_core::media::TrackKind::Subtitle(details) => Some((track, details)),
                _ => None,
            })
            .collect();
        assert_eq!(subtitles.len(), 2);
        assert!(subtitles.iter().all(|(_, details)| details.is_external));
        assert!(subtitles
            .iter()
            .any(|(track, _)| track.language.as_deref() == Some("fre") && !track.is_forced));
        assert!(subtitles
            .iter()
            .any(|(track, _)| track.language.as_deref() == Some("eng") && track.is_forced));
    }

    #[tokio::test]
    async fn a_subtitle_taken_away_stops_being_a_track() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.fr.srt", b"subtitle");

        let (state, library) =
            state_with_roots(directory.path(), vec![("disk-one", media.clone())]).await;
        scan(&state, &library).await;

        std::fs::remove_file(media.join("Quiet.Harbour.2019.MULTi.1080p.fr.srt")).expect("removed");
        let report = scan(&state, &library).await;
        assert_eq!(report.external_subtitles, 0);

        let source = state
            .database()
            .sources_of_root(library.roots[0].id)
            .await
            .expect("read")[0]
            .clone();
        assert!(state
            .database()
            .tracks_of_source(source.id)
            .await
            .expect("read")
            .iter()
            .all(
                |track| !matches!(&track.kind, melyxar_core::media::TrackKind::Subtitle(details)
                if details.is_external)
            ));
    }

    #[tokio::test]
    async fn a_description_file_is_left_alone_unless_the_server_was_asked_to_read_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        write(
            &media,
            "Quiet.Harbour.2019.MULTi.1080p.nfo",
            b"<movie><tmdbid>12345</tmdbid></movie>",
        );

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let report = scan(&state, &library).await;

        assert_eq!(report.added, 1, "a description file is not a film");
        assert_eq!(report.companion_files_read, 0);
        let works = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read");
        assert!(state
            .database()
            .work_external_ids(works[0].id)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn a_description_file_gives_up_its_identifiers_when_the_server_is_asked_to_read_it() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        write(
            &media,
            "Quiet.Harbour.2019.MULTi.1080p.nfo",
            b"<movie><title>Something Else</title><tmdbid>12345</tmdbid>\
              <imdbid>tt7654321</imdbid></movie>",
        );

        let (state, library) = state_with(directory.path(), vec![("disk-one", media)], true).await;
        let report = scan(&state, &library).await;
        assert_eq!(report.companion_files_read, 1);

        let works = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read");
        assert_eq!(
            state
                .database()
                .work_external_ids(works[0].id)
                .await
                .expect("read"),
            vec![
                ("imdb".to_string(), "tt7654321".to_string()),
                ("tmdb".to_string(), "12345".to_string()),
            ]
        );
        assert_eq!(
            works[0].title, "Quiet Harbour",
            "the title comes from the file name, never from a description file"
        );
    }

    #[tokio::test]
    async fn a_description_file_belongs_to_the_folder_it_sits_in() {
        // The one some tools write under a fixed name in a folder holding a
        // single film. A rule that takes it from anywhere would give one
        // film's identifiers to another.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        write(
            &media.join("elsewhere"),
            "movie.nfo",
            b"<movie><tmdbid>99999</tmdbid></movie>",
        );
        write(&media.join("elsewhere"), "Amber.Field.2020.mkv", b"x");

        let (state, library) = state_with(directory.path(), vec![("disk-one", media)], true).await;
        scan(&state, &library).await;

        let works = state
            .database()
            .recent_works(library.id, 10)
            .await
            .expect("read");
        for work in &works {
            let ids = state
                .database()
                .work_external_ids(work.id)
                .await
                .expect("read");
            match work.title.starts_with("Amber") {
                true => assert_eq!(
                    ids,
                    vec![("tmdb".to_string(), "99999".to_string())],
                    "the film the description file sits beside"
                ),
                false => assert!(
                    ids.is_empty(),
                    "a film in another folder has nothing to do with it"
                ),
            }
        }
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
        // What a client follows the scan by, so it has to name a real row.
        let followed = job.id();
        let (state_at_end, report) = job.wait().await;

        assert_eq!(state_at_end, JobState::Succeeded);
        assert_eq!(report.expect("a finished scan has a report").added, 1);
        assert_eq!(
            state
                .database()
                .job(followed)
                .await
                .expect("read")
                .expect("the scan was written down before it started")
                .state,
            JobState::Succeeded
        );
    }

    // A folder the server cannot open is counted in the report, and that count
    // has no test here: making a folder unreadable needs a user who is not its
    // owner, and these tests run as one. The walk itself is covered in the
    // library crate, which keeps going and never loses the files it can reach.

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

    #[tokio::test]
    async fn any_kind_of_movement_makes_a_grid_stale_not_only_an_arrival() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        write(&media, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;

        let version = |state: AppState, id| async move {
            state.database().library_version(id).await.expect("read")
        };

        scan(&state, &library).await;
        let after_arrival = version(state.clone(), library.id).await;

        // Replaced by a better copy: the same name, other contents.
        write(
            &directory.path().join("films"),
            "Quiet.Harbour.2019.MULTi.1080p.mkv",
            b"a larger file altogether",
        );
        let report = scan(&state, &library).await;
        assert_eq!(report.changed, 1);
        let after_change = version(state.clone(), library.id).await;
        assert!(
            after_change > after_arrival,
            "a copy that was replaced changes what the page shows"
        );

        // A disk unplugged and plugged back in: the file leaves and comes back
        // exactly as it was, which is the case the restored count is for.
        let path = directory
            .path()
            .join("films")
            .join("Quiet.Harbour.2019.MULTi.1080p.mkv");
        let as_it_was = std::fs::metadata(&path)
            .expect("the file is there")
            .modified()
            .expect("a modification time");
        std::fs::remove_file(&path).expect("file removed");

        let report = scan(&state, &library).await;
        assert_eq!(report.missing, 1);
        let after_loss = version(state.clone(), library.id).await;
        assert!(
            after_loss > after_change,
            "a film that cannot be played any more changes what the page shows"
        );

        write(
            &directory.path().join("films"),
            "Quiet.Harbour.2019.MULTi.1080p.mkv",
            b"a larger file altogether",
        );
        std::fs::File::options()
            .write(true)
            .open(&path)
            .expect("the file is there")
            .set_modified(as_it_was)
            .expect("the time it had is put back");

        let report = scan(&state, &library).await;
        assert_eq!(report.restored, 1);
        assert_eq!(
            report.changed, 0,
            "the file came back exactly as it left, so nothing about it changed"
        );
        assert!(
            version(state.clone(), library.id).await > after_loss,
            "a film that came back changes what the page shows"
        );
    }
}
