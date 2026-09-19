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
use melyxar_core::job::{JobKind, JobPriority, JobState, JobStep};
use melyxar_core::library::{Library, LibraryKind};
use melyxar_core::media::{SubtitleDetails, Track, TrackKind};
use melyxar_core::media_log::{file_name_of, MediaPath};
use melyxar_core::refresh::RefreshMode;
use melyxar_core::work::WorkKind;
use melyxar_database::catalogue::{LocalExtraVideo, SourceAnalysis, StoredSource};
use melyxar_database::Database;
use melyxar_jobs::{JobHandle, StartedJob};
use melyxar_library::scan::{walk, FoundFile, KnownFile, ScanError};
use melyxar_library::{episode, naming, sidecar};

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
///
/// The priority says who is waiting: a person who pressed a button, or nobody
/// at all, which is what a scan taken up again after a restart is.
///
/// The mode says how much of the library is gone over: what turned up on the
/// disk, what is still missing, or everything again.
pub async fn start_scan(
    state: &AppState,
    library: Library,
    priority: JobPriority,
    mode: RefreshMode,
) -> Result<ScanJob> {
    let outcome: Arc<Mutex<Option<ScanReport>>> = Arc::new(Mutex::new(None));
    let recorded = Arc::clone(&outcome);
    let state = state.clone();
    let target = library.id.to_string();

    let started = state
        .jobs()
        .clone()
        .start(
            JobKind::ScanLibrary,
            priority,
            Some(target),
            move |handle| async move {
                match scan_library(&state, &library, &handle, mode).await {
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
pub async fn start_scan_and_identification(
    state: &AppState,
    library: Library,
    priority: JobPriority,
    mode: RefreshMode,
) -> Result<JobId> {
    let scan = start_scan(state, library.clone(), priority, mode).await?;
    let id = scan.id();

    // Without a provider key there is nothing to look anything up with. The
    // scan still runs, and the library still browses: that is a server
    // configured without a key, not a broken one. The readings below still
    // follow, since they are about the files and not about the pages.
    let provider = state.metadata_provider();

    let waiting = state.clone();
    tokio::spawn(async move {
        let (ended, _) = scan.wait().await;
        if ended != JobState::Succeeded {
            // A scan that did not finish leaves nothing dependable to look up,
            // and whoever reads the list of jobs can already see why.
            return;
        }

        // The pages and the pictures first, and waited for.
        if let Some(provider) = provider {
            match crate::identify::start_identification(&waiting, provider, library.clone(), mode)
                .await
            {
                Ok(job) => {
                    job.wait().await;
                }
                Err(error) => {
                    tracing::warn!(%error, "the scan finished but the look up would not start");
                }
            }
        }

        // And only then the two readings that go through every film. A look up
        // that went wrong does not hold them back: they are about the files.
        read_what_this_library_asked_for(&waiting, &library, priority).await;
    });

    Ok(id)
}

/// Sets going the readings a library asked its scan to do, once the pages are in.
///
/// **After the pages and the pictures, never before.** Each of these goes
/// through every film from end to end, hours of it on a collection of any
/// size, and a grid that sits empty for those hours while the posters wait
/// behind them is the thing everybody complains about. Emby and Jellyfin both
/// put the pages first for exactly this reason, and so does this.
///
/// Started as jobs of their own rather than carried inside the scan, which is
/// what lets them be watched and stopped one by one: somebody who wants their
/// thumbnails later can stop that alone and keep everything else.
///
/// One after the other, in the order `UpkeepTask::ALL` puts them: each reads
/// every file of the collection from end to end, and two of those at once on
/// one disk is two slow readings rather than two quick ones. Where a jump can
/// land comes first, because that is the one a film goes wrong without.
async fn read_what_this_library_asked_for(
    state: &AppState,
    library: &Library,
    priority: JobPriority,
) {
    for task in crate::upkeep::UpkeepTask::ALL {
        if !task.is_done_during_the_scan_of(library) {
            continue;
        }
        match crate::upkeep::start(state, task, library.clone(), priority).await {
            // Waited for, so the next one starts on a disk that is free.
            Ok(job) => {
                let _ = job.completion.await;
            }
            Err(error) => tracing::warn!(
                library = library.name,
                task = task.as_str(),
                %error,
                "the pages are in but this reading would not start; the upkeep has it"
            ),
        }
    }
}

/// Scans every root of a library.
///
/// The handle is what the job layer gives a running job: it carries progress
/// and the request to stop.
pub async fn scan_library(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
    mode: RefreshMode,
) -> Result<ScanReport> {
    let mut report = ScanReport::default();
    let database = state.database();
    // Read once, at the start, rather than once per root: it is one answer
    // about this run, and a setting changed halfway through would otherwise
    // read one disk one way and the next another.
    let work = database.library_work().await?;
    tracing::debug!(
        library = library.name,
        mode = mode.as_str(),
        key_frames_during_scan = library.options.key_frames_during_scan,
        thumbnails_during_scan = library.options.thumbnails_during_scan,
        read_companion_files = work.read_companion_files,
        "a scan is starting"
    );

    // Everything again means forgetting what was read out of every file first,
    // which is what puts them all back in front of the pass that reads them.
    // Nothing else is touched: the files, the pages, the pictures, where a
    // viewer had got to and the thumbnails of the bar all stay as they are.
    if mode.reads_every_file_again() {
        let forgotten = database.forget_analysis(library.id).await?;
        tracing::info!(
            library = library.name,
            files = forgotten,
            "every file of this library will be read again"
        );
    }
    // Read before anything is recorded, so a file added today is named by the
    // same rules as the ones already here.
    let signs = signs_of(state, library).await?;

    // Counted by root rather than by file: how many files there are is exactly
    // what this pass is finding out, and a disk is what it stops between.
    handle.at_step(JobStep::WalkingFolders).await;
    handle.set_total(library.roots.len() as i64).await;

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
                handle.advance(1).await;
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

        if work.read_companion_files {
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
        handle.advance(1).await;
    }

    handle.at_step(JobStep::ReadingNamesAgain).await;
    let reread = reread_names_of_nameless_works(state, library).await?;
    report.renamed = reread.renamed;
    report.merged = reread.merged;
    analyse_pending(state, library, handle, &mut report).await?;

    // The two readings that go through every film from end to end are not part
    // of a scan, whether or not this library asks for them: they follow the
    // pages and the pictures rather than standing in front of them, which is
    // what `start_scan_and_identification` arranges. A grid that fills with
    // posters while the long readings grind away behind it is the whole point.
    if library.options.leaves_something_to_the_upkeep() {
        say_what_is_left_to_the_upkeep(state, library).await;
    }

    if report.changed_anything() {
        database.bump_library_version(library.id).await?;
    }
    if handle.is_cancelled() {
        report.cancelled = true;
    }

    tracing::info!(
        library = library.name,
        mode = mode.as_str(),
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

/// Says what this scan is leaving to the upkeep, and how much of it there is.
///
/// The question this answers is the one the maintainer asks the evening after
/// a scan: the films are all there, and the playback bar has no pictures on
/// it. Without this line the only honest answer is that a scan is not what
/// makes them, which nobody could work out from anything on the screen.
///
/// Never a failure: this is a sentence, and a library that could not be
/// counted has already said so where it happened.
async fn say_what_is_left_to_the_upkeep(state: &AppState, library: &Library) {
    for task in crate::upkeep::UpkeepTask::ALL {
        if task.is_done_during_the_scan_of(library) {
            continue;
        }
        let Ok(waiting) = crate::upkeep::what_is_waiting_for(state, task, library.id).await else {
            return;
        };
        tracing::debug!(
            library = library.name,
            task = task.as_str(),
            waiting,
            "this scan does not do this reading; the upkeep has it"
        );
    }
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
                work = %parsed.title,
                "two copies of one film read as one film now"
            );
            done.merged += 1;
            continue;
        }

        database
            .rename_work(work.id, &parsed.title, &sort_title, parsed.year)
            .await?;
        tracing::info!(
            was = %work.title,
            now = %parsed.title,
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
/// Reads one file again for what it says about itself.
///
/// A scan opens only a file whose size or date has changed on disk, which is
/// what keeps a second scan of a large library cheap. The price is that a
/// server which has learnt to read something new out of a file can never reach
/// the ones it has already described: the file has not changed, so nothing
/// looks at it again. This is how somebody asks for one, and it costs one
/// reading of one file.
///
/// Only what the file says of itself is done again: the container, how long it
/// runs, its rate, and every track with its codec, its colours and the margins
/// it declares. All of it is replaced in one go, so a reading that fails
/// leaves the film described as it was rather than stripped of everything it
/// had. What is attached to the file and was not read out of it stays
/// untouched either way, so a film keeps its page, its pictures, where a
/// viewer had got to and the thumbnails of its playback bar.
///
/// Answers nothing when there is no such file, which is a page looking at
/// something that has since gone.
pub async fn read_copy_again(state: &AppState, source_id: MediaSourceId) -> Result<Option<JobId>> {
    let database = state.database();
    let Some(source) = database.source_by_id(source_id).await? else {
        return Ok(None);
    };
    let Some(tools) = state.tools() else {
        return Err(AppError::Domain(melyxar_core::Error::not_found(
            "media tools",
        )));
    };

    let file = PendingFile {
        root_label: source.root_label.clone(),
        root_path: source.root_path.clone(),
        source_id,
        relative_path: source.relative_path.clone(),
    };
    let name = file.name();
    let analyser = tools.ffprobe.clone();
    let owned = state.clone();

    let started = state
        .jobs()
        .clone()
        .start(
            JobKind::ReadCopyAgain,
            JobPriority::REQUESTED,
            Some(source_id.to_string()),
            move |handle| async move {
                let database = owned.database();
                handle.at_step(JobStep::AnalysingFiles).await;
                handle.set_total(1).await;
                handle.now_working_on(Some(&name)).await;

                // Nothing is forgotten first. One reading replaces the
                // container, the streams and the chapters in one go, so a
                // reading that fails leaves the film described as it was
                // rather than stripped of everything it had: a file read again
                // and unreadable would otherwise come out of this worse than
                // it went in.
                let outcome = analyse_one(database, &analyser, &file)
                    .await
                    .map_err(|error| error.to_string())?;
                handle.advance(1).await;

                match outcome {
                    Outcome::Analysed => Ok(()),
                    Outcome::Unreadable => Err("the analyser could not read that file".to_string()),
                    Outcome::Stopped => Ok(()),
                }
            },
        )
        .await?;

    Ok(Some(started.id))
}

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
    if library.kind.is_episodic() {
        if let Some(work) = episode_work_for(state, library, relative_path, signs).await? {
            return Ok(work);
        }
    }

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
            work = %existing.title,
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

/// Finds the episode a file stands for, with its season and its series above
/// it, or says the name never told which episode this is.
///
/// Saying nothing is a real answer and not a failure: the file goes on to the
/// ordinary path, which records it as an episode belonging to nothing, and
/// such an episode is met on its own in the grid. A file nobody could number
/// stays visible and gets corrected; a file filed under a season nobody wrote
/// is never even looked at.
async fn episode_work_for(
    state: &AppState,
    library: &Library,
    relative_path: &Path,
    signs: &naming::LibrarySigns,
) -> Result<Option<WorkId>> {
    let file_name = relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let year = melyxar_core::time::current_year();
    let folders = folders_above(relative_path);
    let from_a_season_folder = folders
        .iter()
        .find_map(|folder| episode::season_of_folder(folder));

    let read = match episode::parse_episode(file_name, year, signs) {
        Some(read) => Some(read),
        // Nothing in the name says which episode this is. Under a season
        // folder, where the season and the series are already settled, a name
        // opening on a number is that number: it is how a whole run taken off
        // a disc is usually named, and reading it nowhere else keeps a film
        // called by a number out of it.
        None if from_a_season_folder.is_some() => {
            episode::episode_of_a_leading_number(file_name, year, signs)
        }
        None => None,
    };
    let Some(read) = read else {
        return Ok(None);
    };

    let Some(named) = the_series(&read, &folders, year, signs) else {
        return Ok(None);
    };
    // The name first, the season folder to fill in wherever it sits above the
    // file, and the one season a series has when nobody ever wrote a season
    // anywhere: a show with a single season is written without one, and that
    // is what it means.
    let season = read
        .season
        .or(from_a_season_folder)
        .unwrap_or(THE_ONLY_SEASON);

    let database = state.database();
    let series_sort = naming::sort_title(&named.title);
    // Looked up by its name and its year, exactly as a film is, which two
    // series of the same name and two different years need. What used to split
    // one series in two was not the year being asked for but where it came
    // from: read off each file, one file carried it and the next did not.
    let series = match database
        .series_by_name(library.id, &series_sort, named.year)
        .await?
    {
        Some(found) => found,
        None => {
            database
                .create_work(
                    library.id,
                    WorkKind::Series,
                    &named.title,
                    &series_sort,
                    named.year,
                )
                .await?
        }
    };

    let season_work = child_at(state, library, series.id, season, WorkKind::Season, || {
        crate::episodes::name_of_season(season)
    })
    .await?;

    let episode_work = child_at(
        state,
        library,
        season_work.id,
        read.first,
        WorkKind::Episode,
        || {
            read.title
                .clone()
                .unwrap_or_else(|| crate::episodes::name_of_episode(read.first, read.last))
        },
    )
    .await?;

    Ok(Some(episode_work.id))
}

/// The child of a work sitting at that number, written down if it is not there.
///
/// The name is only worked out when one has to be written, because naming a
/// season costs nothing and naming it for a season already on the page costs a
/// string per file of the collection.
async fn child_at(
    state: &AppState,
    library: &Library,
    parent_id: WorkId,
    ordinal: i32,
    kind: WorkKind,
    name: impl FnOnce() -> String,
) -> Result<melyxar_core::work::Work> {
    let database = state.database();
    if let Some(found) = database.child_by_ordinal(parent_id, ordinal).await? {
        return Ok(found);
    }
    let title = name();
    let sort_title = naming::sort_title(&title);
    Ok(database
        .create_child_work(library.id, parent_id, ordinal, kind, &title, &sort_title)
        .await?)
}

/// The season a series has when nobody wrote a season anywhere.
const THE_ONLY_SEASON: i32 = 1;

/// The folders between a file and the root of its library, nearest first.
fn folders_above(relative_path: &Path) -> Vec<&str> {
    let Some(parent) = relative_path.parent() else {
        return Vec::new();
    };
    let mut folders: Vec<&str> = parent
        .components()
        .filter_map(|part| part.as_os_str().to_str())
        .collect();
    folders.reverse();
    folders
}

/// What names the series this file belongs to, and the year that name carries.
///
/// A season folder is the one mark that says without any doubt that what sits
/// above it is a series, so where there is one, the folder holding it is the
/// series and it is that folder which names it. Everything under it lands in
/// the same series however each file happens to be named: two seasons ripped
/// by two teams that write the title in two languages are still one series,
/// and reading the file names first made two.
///
/// Where there is no season folder nothing has changed: a folder holding files
/// in bulk groups nothing by itself, so the file name names the series, and
/// the folder answers only when the name did not.
fn the_series(
    read: &episode::ParsedEpisode,
    folders: &[&str],
    current_year: i32,
    signs: &naming::LibrarySigns,
) -> Option<episode::NamedSeries> {
    if let Some(named) = the_folder_of_the_series(folders)
        .and_then(|folder| episode::series_of_folder(folder, current_year, signs))
    {
        return Some(named);
    }
    if !read.series.is_empty() {
        return Some(episode::NamedSeries {
            title: read.series.clone(),
            year: read.year,
        });
    }
    folders
        .iter()
        .find_map(|folder| episode::series_of_folder(folder, current_year, signs))
}

/// The folder that names the series: the one holding the nearest season
/// folder, when the path goes through one at all.
fn the_folder_of_the_series<'a>(folders: &[&'a str]) -> Option<&'a str> {
    let season = folders
        .iter()
        .position(|folder| episode::season_of_folder(folder).is_some())?;
    folders.get(season + 1).copied()
}

/// The kind of work a file in this library stands for when nothing better is
/// known about it.
///
/// A film is one file to one work. In an episodic library this is only reached
/// by a file whose name never said which episode it is: it is an episode all
/// the same, belonging to no season, and it is met on its own in the grid
/// rather than disappearing behind a series it was never attached to.
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
                file = %file_name_of(&companion.relative_path),
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

    // Announced once there is something to announce: a pass with nothing to do
    // is over before anybody reads its name, and a name that flashes past is
    // worse than no name at all.
    handle.at_step(JobStep::AnalysingFiles).await;
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
            handle.now_working_on(Some(&file.name())).await;
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

impl PendingFile {
    /// What to put on the screen while this one is being read.
    fn name(&self) -> String {
        self.relative_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
            .to_string()
    }
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
    // follows is always about one film in particular, so its name travels
    // with it.
    let lining_up = melyxar_media_probe::HowTheStreamsLineUp::of(&report);
    if lining_up.is_worth_saying() {
        tracing::info!(
            file = %file_name_of(&file.relative_path),
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
        state_of_kind(directory, roots, read_companion_files, "movies").await
    }

    /// A state whose only library holds series rather than films.
    async fn series_state_with_roots(
        directory: &Path,
        roots: Vec<(&str, PathBuf)>,
    ) -> (AppState, Library) {
        state_of_kind(directory, roots, false, "series").await
    }

    /// The one that really builds it, whatever the library is meant to hold.
    async fn state_of_kind(
        directory: &Path,
        roots: Vec<(&str, PathBuf)>,
        read_companion_files: bool,
        kind: &str,
    ) -> (AppState, Library) {
        let config = Config {
            directories: Directories {
                data: directory.join("data"),
                cache: directory.join("cache"),
                transcodes: directory.join("cache/transcodes"),
            },
            libraries: vec![LibraryConfig {
                name: "Films".into(),
                kind: kind.into(),
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
        // Reading what sits next to a film is a setting of the server, written
        // the way a screen writes it.
        let work = database.library_work().await.expect("read");
        database
            .save_library_work(melyxar_database::settings::LibraryWork {
                read_companion_files,
                ..work
            })
            .await
            .expect("the settings are written");
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

    /// Tells a library to do the two heavy readings during its own scan.
    ///
    /// Off for every library to begin with, so a test about what a scan reads
    /// out of a film has to say so, exactly as somebody ticking the box on the
    /// screen does.
    async fn reading_everything_during_the_scan(state: &AppState, library: &Library) -> Library {
        state
            .database()
            .set_library_options(
                library.id,
                melyxar_core::library::LibraryOptions {
                    key_frames_during_scan: true,
                    thumbnails_during_scan: true,
                },
            )
            .await
            .expect("the switches are written down");
        state
            .database()
            .library_by_name(&library.name)
            .await
            .expect("read")
            .expect("the library is still there")
    }

    /// Somebody to answer for, since what a page shows depends on who is
    /// looking at it.
    ///
    /// The one account rather than a new one each time: a server has one until
    /// signing in arrives, and asking twice for a second would be asking for
    /// somebody who cannot exist.
    async fn a_viewer(state: &AppState) -> melyxar_core::id::UserId {
        let database = state.database();
        if let Some((already, _)) = database.user_by_name("Viewer").await.expect("read") {
            return already.id;
        }
        database
            .create_user("Viewer", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created")
            .id
    }

    /// Runs a scan the way the server does, and gives back what it did.
    async fn scan(state: &AppState, library: &Library) -> ScanReport {
        let (job_state, report) = start_scan(
            state,
            library.clone(),
            JobPriority::REQUESTED,
            RefreshMode::default(),
        )
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

    /// Every work of a library, whatever it hangs under, for reading an
    /// arrangement back.
    async fn arrangement(state: &AppState, library: &Library) -> Vec<melyxar_core::work::Work> {
        state
            .database()
            .recent_works(library.id, 100)
            .await
            .expect("read")
    }

    /// The works of one kind, in the order they are numbered.
    fn of_kind(
        works: &[melyxar_core::work::Work],
        kind: WorkKind,
    ) -> Vec<melyxar_core::work::Work> {
        let mut found: Vec<_> = works
            .iter()
            .filter(|work| work.kind == kind)
            .cloned()
            .collect();
        found.sort_by_key(|work| work.ordinal);
        found
    }

    #[tokio::test]
    async fn the_tidy_arrangement_becomes_a_series_a_season_and_its_episodes() {
        // A folder per series with a folder per season inside it: what the
        // maintainer does and what every other server asks for.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(
            &media,
            "Distant Signal/Saison 1/Distant Signal - S01E01 - The Long Night.mkv",
            b"x",
        );
        write(
            &media,
            "Distant Signal/Saison 1/Distant Signal - S01E02 - Cold Water.mkv",
            b"xx",
        );
        write(
            &media,
            "Distant Signal/Saison 2/Distant Signal - S02E01 - First Light.mkv",
            b"xxx",
        );

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let works = arrangement(&state, &library).await;

        let series = of_kind(&works, WorkKind::Series);
        assert_eq!(series.len(), 1, "one series, not one per file");
        assert_eq!(series[0].title, "Distant Signal");
        assert_eq!(series[0].parent_id, None);

        let seasons = of_kind(&works, WorkKind::Season);
        assert_eq!(
            seasons.iter().map(|s| s.ordinal).collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
        assert!(seasons.iter().all(|s| s.parent_id == Some(series[0].id)));

        let episodes = of_kind(&works, WorkKind::Episode);
        assert_eq!(episodes.len(), 3);
        // The first season holds two, numbered one and two; the second holds
        // one, numbered one again, because an episode is numbered inside its
        // own season and not across the series.
        let first = episodes
            .iter()
            .filter(|e| e.parent_id == Some(seasons[0].id))
            .map(|e| e.ordinal)
            .collect::<Vec<_>>();
        assert_eq!(first, vec![Some(1), Some(2)]);
        assert_eq!(
            episodes
                .iter()
                .filter(|e| e.parent_id == Some(seasons[1].id))
                .map(|e| e.ordinal)
                .collect::<Vec<_>>(),
            vec![Some(1)]
        );

        // The episode keeps the name its file gave it.
        assert!(
            episodes.iter().any(|e| e.title == "The Long Night"),
            "{:?}",
            episodes.iter().map(|e| &e.title).collect::<Vec<_>>()
        );
    }

    #[tokio::test]
    async fn episodes_laid_flat_in_a_series_folder_are_arranged_the_same() {
        // No season folder anywhere. The name carries the season, so nothing
        // is missing and the arrangement comes out identical.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(
            &media,
            "Distant Signal/Distant.Signal.S01E01.1080p.mkv",
            b"x",
        );
        write(
            &media,
            "Distant Signal/Distant.Signal.S01E02.1080p.mkv",
            b"xx",
        );

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let works = arrangement(&state, &library).await;

        assert_eq!(of_kind(&works, WorkKind::Series).len(), 1);
        assert_eq!(of_kind(&works, WorkKind::Season).len(), 1);
        assert_eq!(
            of_kind(&works, WorkKind::Episode)
                .iter()
                .map(|e| e.ordinal)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
    }

    #[tokio::test]
    async fn episodes_in_bulk_are_arranged_from_their_names_alone() {
        // Two series thrown into one folder with no folder of their own. The
        // names are the only thing there is, and they are enough.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(&media, "Distant.Signal.S01E01.mkv", b"x");
        write(&media, "Distant.Signal.S01E02.mkv", b"xx");
        write(&media, "Amber.Field.S03E07.mkv", b"xxx");

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let works = arrangement(&state, &library).await;

        let series = of_kind(&works, WorkKind::Series);
        let mut names: Vec<_> = series.iter().map(|s| s.title.clone()).collect();
        names.sort();
        assert_eq!(names, vec!["Amber Field", "Distant Signal"]);
        assert_eq!(of_kind(&works, WorkKind::Season).len(), 2);
        assert_eq!(of_kind(&works, WorkKind::Episode).len(), 3);
    }

    #[tokio::test]
    async fn a_folder_answers_for_what_a_name_never_said() {
        // The file says only its number. The folder it sits in says which
        // season, and the folder above that names the series.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(&media, "Distant Signal/Saison 2/Episode 03.mkv", b"x");
        write(&media, "Distant Signal/Specials/E01.mkv", b"xx");

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let works = arrangement(&state, &library).await;

        let series = of_kind(&works, WorkKind::Series);
        assert_eq!(series.len(), 1);
        assert_eq!(series[0].title, "Distant Signal");

        let seasons = of_kind(&works, WorkKind::Season);
        assert_eq!(
            seasons.iter().map(|s| s.ordinal).collect::<Vec<_>>(),
            vec![Some(0), Some(2)],
            "what belongs to no season is season zero"
        );

        let episodes = of_kind(&works, WorkKind::Episode);
        assert_eq!(
            episodes.iter().map(|e| e.ordinal).collect::<Vec<_>>(),
            vec![Some(1), Some(3)]
        );
    }

    #[tokio::test]
    async fn one_folder_of_seasons_holds_one_series_however_its_files_are_named() {
        // Seen on a real collection: one season ripped by a team that writes
        // the title of the series in one language, another by a team that
        // writes it in another. Read from the names alone this is two series,
        // one of which the provider recognises under no name at all.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(
            &media,
            "Distant Signal/Saison 1/Amber.Field.S01E01.mkv",
            b"x",
        );
        write(
            &media,
            "Distant Signal/Saison 3/Distant.Signal.S03E01.1080p.mkv",
            b"xx",
        );
        // And the shape a release takes when it wraps each episode in a folder
        // of its own, which the folder holding the season still answers for.
        write(
            &media,
            "Distant Signal/Saison 3/Distant.Signal.S03E02.WEB/Distant.Signal.S03E02.WEB.mkv",
            b"xxx",
        );

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let works = arrangement(&state, &library).await;

        let series = of_kind(&works, WorkKind::Series);
        assert_eq!(
            series.iter().map(|s| s.title.clone()).collect::<Vec<_>>(),
            vec!["Distant Signal"],
            "the folder holding the seasons names the series, once"
        );
        let seasons = of_kind(&works, WorkKind::Season);
        assert_eq!(
            seasons.iter().map(|s| s.ordinal).collect::<Vec<_>>(),
            vec![Some(1), Some(3)]
        );
        assert!(seasons.iter().all(|s| s.parent_id == Some(series[0].id)));
        assert_eq!(of_kind(&works, WorkKind::Episode).len(), 3);
    }

    #[tokio::test]
    async fn a_season_folder_numbers_the_files_that_say_nothing_but_a_number() {
        // Seen on a real collection: a series of fifty two episodes, filed
        // under a folder of its own and a folder per season, whose file names
        // carry the number of the episode and its title and nothing else.
        // Read from the names alone, every one of them was a work standing by
        // itself and the series existed nowhere.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(
            &media,
            "Distant Signal/Saison 1/01 - The Amber Field.mkv",
            b"x",
        );
        write(
            &media,
            "Distant Signal/Saison 1/02 - Silent Harbour.mkv",
            b"xx",
        );
        // Four digits are a year and never an episode, so this one is still
        // filed under nothing rather than read as the two thousand and
        // nineteenth episode.
        write(
            &media,
            "Distant Signal/Saison 1/2019 Lost Footage.mkv",
            b"xxx",
        );

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let works = arrangement(&state, &library).await;

        let series = of_kind(&works, WorkKind::Series);
        assert_eq!(
            series.iter().map(|s| s.title.clone()).collect::<Vec<_>>(),
            vec!["Distant Signal"]
        );
        let seasons = of_kind(&works, WorkKind::Season);
        assert_eq!(
            seasons.iter().map(|s| s.ordinal).collect::<Vec<_>>(),
            vec![Some(1)]
        );

        let episodes = of_kind(&works, WorkKind::Episode);
        let numbered: Vec<_> = episodes
            .iter()
            .filter(|episode| episode.parent_id == Some(seasons[0].id))
            .map(|episode| (episode.ordinal, episode.title.clone()))
            .collect();
        assert_eq!(
            numbered,
            vec![
                (Some(1), "The Amber Field".to_string()),
                (Some(2), "Silent Harbour".to_string()),
            ],
            "the number in front is the episode and the rest of the name is its title"
        );
        assert!(
            episodes
                .iter()
                .any(|episode| episode.parent_id.is_none() && episode.ordinal.is_none()),
            "a name opening on a year belongs to no season and stays visible on its own"
        );
    }

    #[tokio::test]
    async fn a_series_with_no_season_anywhere_gets_the_one_it_has() {
        // Nothing says a season: not the name, not a folder. A show written
        // this way has one season, and that is what it means.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(&media, "Distant Signal/Episode 1.mkv", b"x");
        write(&media, "Distant Signal/Episode 2.mkv", b"xx");

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let works = arrangement(&state, &library).await;

        let seasons = of_kind(&works, WorkKind::Season);
        assert_eq!(seasons.len(), 1);
        assert_eq!(seasons[0].ordinal, Some(1));
        assert_eq!(of_kind(&works, WorkKind::Episode).len(), 2);
    }

    #[tokio::test]
    async fn two_copies_of_one_episode_are_one_episode() {
        // Exactly what two copies of one film are: one work carrying two
        // files, which is what puts a chooser on the page instead of the same
        // episode twice.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(
            &media,
            "Distant Signal/Distant.Signal.S01E01.1080p.mkv",
            b"x",
        );
        write(
            &media,
            "Distant Signal/Distant.Signal.S01E01.2160p.mkv",
            b"xx",
        );

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let works = arrangement(&state, &library).await;

        let episodes = of_kind(&works, WorkKind::Episode);
        assert_eq!(episodes.len(), 1, "one episode, two files");
        assert_eq!(
            state
                .database()
                .sources_of_work(episodes[0].id)
                .await
                .expect("read")
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn a_file_nobody_could_number_is_still_somewhere_to_be_seen() {
        // The decision written down in the architecture notes: a file whose
        // name never said which episode it is belongs to no season, and an
        // episode belonging to nothing is met on its own rather than being
        // filed under a season nobody wrote.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(&media, "Distant Signal/Distant.Signal.S01E01.mkv", b"x");
        write(&media, "Distant Signal/A Night Nobody Numbered.mkv", b"xx");

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let works = arrangement(&state, &library).await;

        let stray: Vec<_> = works
            .iter()
            .filter(|work| work.kind == WorkKind::Episode && work.parent_id.is_none())
            .collect();
        assert_eq!(stray.len(), 1);
        assert_eq!(stray[0].title, "A Night Nobody Numbered");

        let shown = state
            .database()
            .browse_works(&melyxar_database::browse::BrowseRequest {
                library_id: Some(library.id),
                ..Default::default()
            })
            .await
            .expect("read");
        assert!(
            shown.cards.iter().any(|card| card.id == stray[0].id),
            "it has somewhere to be seen: {:?}",
            shown.cards.iter().map(|c| &c.title).collect::<Vec<_>>()
        );
        // And the series it sits beside is on the same grid, while its seasons
        // and its episodes are not.
        assert_eq!(shown.cards.len(), 2);
    }

    #[tokio::test]
    async fn only_what_the_provider_has_a_catalogue_for_is_asked_about() {
        // A series is asked about; its seasons and its episodes are described
        // by the one answer that describes the series, so asking about them
        // separately is several hundred questions for what one answer already
        // said.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(
            &media,
            "Distant Signal/Saison 1/Distant.Signal.S01E01.mkv",
            b"x",
        );
        write(
            &media,
            "Distant Signal/Saison 1/Distant.Signal.S01E02.mkv",
            b"xx",
        );

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;

        let waiting = state
            .database()
            .works_awaiting_identification(library.id)
            .await
            .expect("read");
        assert_eq!(
            waiting
                .iter()
                .map(|work| (work.title.clone(), work.kind))
                .collect::<Vec<_>>(),
            vec![("Distant Signal".to_string(), WorkKind::Series)],
            "the series, and neither its season nor its episodes"
        );
    }

    #[tokio::test]
    async fn a_series_page_carries_its_seasons_and_a_season_page_its_episodes() {
        // The whole page in one answer, ways back up included: a page that
        // opens with eight requests opens eight times slower than one that
        // opens with one, and the heading is drawn before anything else.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(
            &media,
            "Distant Signal/Saison 1/Distant Signal - S01E01 - The Long Night.mkv",
            b"x",
        );
        write(
            &media,
            "Distant Signal/Saison 1/Distant Signal - S01E02 - Cold Water.mkv",
            b"xx",
        );
        write(
            &media,
            "Distant Signal/Saison 2/Distant Signal - S02E01 - First Light.mkv",
            b"xxx",
        );

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let works = arrangement(&state, &library).await;
        let series = of_kind(&works, WorkKind::Series)[0].clone();

        let page = crate::detail::work_detail(&state, a_viewer(&state).await, series.id)
            .await
            .expect("read")
            .expect("the series has a page");
        assert!(page.ancestry.is_empty(), "a series stands on its own");
        assert_eq!(
            page.children
                .iter()
                .map(|child| (child.work.kind, child.work.ordinal, child.work.child_count))
                .collect::<Vec<_>>(),
            vec![
                (WorkKind::Season, Some(1), 2),
                (WorkKind::Season, Some(2), 1)
            ]
        );

        let first_season = page.children[0].work.id;
        let season_page = crate::detail::work_detail(&state, a_viewer(&state).await, first_season)
            .await
            .expect("read")
            .expect("the season has a page");
        assert_eq!(
            season_page
                .ancestry
                .iter()
                .map(|up| (up.kind, up.id))
                .collect::<Vec<_>>(),
            vec![(WorkKind::Series, series.id)],
            "the way back up travels with the page"
        );
        assert_eq!(
            season_page
                .children
                .iter()
                .map(|child| (
                    child.work.ordinal,
                    child.work.title.clone(),
                    child.work.playable
                ))
                .collect::<Vec<_>>(),
            vec![
                (Some(1), "The Long Night".to_string(), true),
                (Some(2), "Cold Water".to_string(), true)
            ]
        );

        let episode = season_page.children[0].work.id;
        let episode_page = crate::detail::work_detail(&state, a_viewer(&state).await, episode)
            .await
            .expect("read")
            .expect("the episode has a page");
        assert!(
            episode_page.children.is_empty(),
            "nothing hangs under an episode"
        );
        assert_eq!(
            episode_page
                .ancestry
                .iter()
                .map(|up| (up.kind, up.ordinal))
                .collect::<Vec<_>>(),
            vec![(WorkKind::Season, Some(1)), (WorkKind::Series, None)],
            "its season, then its series"
        );
        assert_eq!(
            episode_page.versions.len(),
            1,
            "and the file it is played from"
        );
    }

    #[tokio::test]
    async fn a_series_page_says_which_episode_to_carry_on_with() {
        // One press has to reach something playable, so the page works it out
        // rather than leaving a viewer to find their place in a list.
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        for episode in 1..=3 {
            write(
                &media,
                &format!("Distant Signal/Saison 1/Distant.Signal.S01E0{episode}.mkv"),
                b"x",
            );
        }

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;
        let viewer = a_viewer(&state).await;
        let works = arrangement(&state, &library).await;
        let series = of_kind(&works, WorkKind::Series)[0].clone();

        let page = crate::detail::work_detail(&state, viewer, series.id)
            .await
            .expect("read")
            .expect("the series has a page");
        let first = page.carry_on_with.expect("nothing watched, so the first");
        assert_eq!((first.season, first.episode), (Some(1), Some(1)));
        assert!(
            first.source_id.is_some(),
            "and a file to play it from, or the button has nothing to press"
        );

        // Watched out of order: the first and the last.
        for episode in [1, 3] {
            let of_that_number = of_kind(&works, WorkKind::Episode)
                .into_iter()
                .find(|work| work.ordinal == Some(episode))
                .expect("the episode is there");
            state
                .database()
                .record_playback_progress(
                    viewer,
                    of_that_number.id,
                    melyxar_core::time::Millis::new(0),
                    melyxar_core::work::PlaybackState::Watched,
                    melyxar_core::time::now(),
                )
                .await
                .expect("marked");
        }

        let page = crate::detail::work_detail(&state, viewer, series.id)
            .await
            .expect("read")
            .expect("still there");
        assert_eq!(
            page.carry_on_with.map(|next| next.episode),
            Some(Some(2)),
            "the hole, not the one after the last one played"
        );
        // And the season card says how many are left rather than how many
        // there are.
        assert_eq!(
            page.children
                .iter()
                .map(|season| (season.work.child_count, season.work.unwatched))
                .collect::<Vec<_>>(),
            vec![(3, 1)]
        );

        // The one after an episode is simply the one after it, watched or not.
        let second = of_kind(&works, WorkKind::Episode)
            .into_iter()
            .find(|work| work.ordinal == Some(2))
            .expect("there");
        assert_eq!(
            crate::detail::work_detail(&state, viewer, second.id)
                .await
                .expect("read")
                .expect("there")
                .carry_on_with
                .map(|next| next.episode),
            Some(Some(3))
        );
    }

    #[tokio::test]
    async fn a_grid_of_series_shows_the_series_and_not_what_hangs_under_them() {
        let directory = tempfile::tempdir().expect("temporary folder");
        let media = directory.path().join("media");
        write(
            &media,
            "Distant Signal/Saison 1/Distant.Signal.S01E01.mkv",
            b"x",
        );
        write(
            &media,
            "Distant Signal/Saison 1/Distant.Signal.S01E02.mkv",
            b"xx",
        );
        write(
            &media,
            "Amber Field/Saison 3/Amber.Field.S03E07.mkv",
            b"xxx",
        );

        let (state, library) =
            series_state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        scan(&state, &library).await;

        let shown = state
            .database()
            .browse_works(&melyxar_database::browse::BrowseRequest {
                library_id: Some(library.id),
                ..Default::default()
            })
            .await
            .expect("read");
        assert_eq!(
            shown
                .cards
                .iter()
                .map(|c| c.title.clone())
                .collect::<Vec<_>>(),
            vec!["Amber Field", "Distant Signal"],
            "two series, and none of their seasons or episodes"
        );
        assert!(shown.cards.iter().all(|c| c.kind == WorkKind::Series));
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

    /// The same film, carrying a subtitle made of words inside it.
    ///
    /// The words have to be inside the film rather than beside it: a subtitle
    /// in a file of its own is already the file it would be pulled out into,
    /// and the reading this exercises is exactly the one such a file spares.
    /// So the sidecar the tool is given is taken away again the moment it has
    /// been muxed, before anything walks the folder.
    fn write_real_video_carrying_words(path: &Path) -> bool {
        let beside = path.with_extension("srt");
        if std::fs::write(
            &beside,
            "1\n00:00:00,100 --> 00:00:00,900\nQuiet Harbour\n\n",
        )
        .is_err()
        {
            return false;
        }
        let made = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=1:size=320x240:rate=10",
            ])
            .arg("-i")
            .arg(&beside)
            .args([
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-c:s",
                "srt",
                "-shortest",
            ])
            .arg(path)
            .output();
        let _ = std::fs::remove_file(&beside);
        made.is_ok_and(|output| output.status.success())
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

    /// Runs every reading that is waiting and waits for it to end.
    ///
    /// What the chain after a scan does, and what the button on the upkeep
    /// screen does. Answers how many jobs it took, since the readings are jobs
    /// of their own now rather than steps of the scan.
    async fn read_everything_that_is_waiting(state: &AppState) -> usize {
        let started = crate::upkeep::start_what_is_waiting(state, JobPriority::REQUESTED).await;
        // The jobs are written down before they start, so waiting on the rows
        // is waiting on the work.
        while !state
            .database()
            .unfinished_jobs()
            .await
            .expect("read")
            .is_empty()
        {
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        started
    }

    #[tokio::test]
    async fn the_pages_come_before_the_two_long_readings_and_not_after_them() {
        // What somebody watches after adding a disk: the grid filling with
        // posters. Each of the two readings goes through every film from end
        // to end, hours of it on a real collection, so a library that asks for
        // them used to leave the grid blank for those hours and only then
        // fetch a single page. Emby and Jellyfin both put the pages first.
        //
        // Read off the jobs rather than off a clock: each is written down when
        // it starts, so the order they were written down in is the order they
        // ran in.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        std::fs::create_dir_all(&media).expect("the media folder");
        std::fs::write(media.join("Quiet.Harbour.2019.mkv"), b"not really a film")
            .expect("the file is written");

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let library = reading_everything_during_the_scan(&state, &library).await;

        start_scan_and_identification(
            &state,
            library.clone(),
            JobPriority::REQUESTED,
            RefreshMode::default(),
        )
        .await
        .expect("the chain started");

        // Every job of the chain, in the order each was written down.
        let mut ran: Vec<JobKind> = Vec::new();
        for _ in 0..600 {
            let all = state.database().recent_jobs(50).await.expect("read");
            ran = all.iter().rev().map(|job| job.kind).collect();
            let done = ran.contains(&JobKind::ReadKeyFrames)
                && ran.contains(&JobKind::GenerateThumbnails)
                && state
                    .database()
                    .unfinished_jobs()
                    .await
                    .expect("read")
                    .is_empty();
            if done {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }

        let place_of = |kind: JobKind| {
            ran.iter()
                .position(|ran| *ran == kind)
                .unwrap_or_else(|| panic!("{kind:?} never ran: {ran:?}"))
        };
        assert!(
            place_of(JobKind::ScanLibrary) < place_of(JobKind::IdentifyWork),
            "the scan comes first: {ran:?}"
        );
        assert!(
            place_of(JobKind::IdentifyWork) < place_of(JobKind::ReadKeyFrames),
            "the pages come before the film is read for its jumps: {ran:?}"
        );
        assert!(
            place_of(JobKind::IdentifyWork) < place_of(JobKind::GenerateThumbnails),
            "and before it is read for its bar: {ran:?}"
        );
    }

    #[tokio::test]
    async fn a_scan_leaves_the_heavy_readings_to_the_upkeep_unless_it_is_told_not_to() {
        // The defect this exists for: the readings went through every film of
        // the library inside the scan, so a scan of a real collection took
        // days and the first five thousand films were all anybody ever got.
        // They belong to the upkeep now, which runs of a night and can be set
        // going from a button, and a library that wants them in one sitting
        // says so.
        //
        // The film carries words inside it, so that all three readings have
        // something to do rather than two of them.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        std::fs::create_dir_all(&media).expect("the media folder");
        if !write_real_video_carrying_words(&media.join("Quiet.Harbour.2019.mkv")) {
            eprintln!("no media tool here, the readings were not exercised");
            return;
        }

        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        let report = scan(&state, &library).await;
        assert_eq!(report.added, 1);

        // And what it left is counted, which is what the upkeep screen shows
        // and what the button acts on.
        let left = crate::upkeep::what_is_left(&state).await.expect("counted");
        for task in crate::upkeep::UpkeepTask::ALL {
            let entry = left
                .iter()
                .find(|entry| entry.task == task && entry.library == library.id)
                .expect("every library answers for every reading");
            assert_eq!(entry.waiting, 1, "{}", task.as_str());
            assert!(!entry.during_the_scan);
            assert!(!entry.under_way);
        }

        assert_eq!(
            read_everything_that_is_waiting(&state).await,
            3,
            "one job for each reading, on the one library that has anything waiting"
        );

        let done = crate::upkeep::what_is_left(&state).await.expect("counted");
        for task in crate::upkeep::UpkeepTask::ALL {
            let entry = done
                .iter()
                .find(|entry| entry.task == task && entry.library == library.id)
                .expect("every library answers for every reading");
            assert_eq!(entry.waiting, 0, "{}", task.as_str());
            assert_eq!(entry.done, 1, "{}", task.as_str());
        }

        assert_eq!(
            crate::upkeep::start_what_is_waiting(&state, JobPriority::REQUESTED).await,
            0,
            "nothing is waiting, so no job that would end on the spot is started"
        );
    }

    #[tokio::test]
    async fn a_scan_leaves_every_film_with_the_thumbnails_of_its_bar() {
        // The whole chain on a real film: walked, described, read for where a
        // jump can land, then read for the little pictures of the bar.
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        std::fs::create_dir_all(&media).expect("the media folder");
        let film = media.join("Quiet.Harbour.2019.mkv");
        let made = tokio::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=12:duration=25",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-g",
                "24",
            ])
            .arg(&film)
            .output()
            .await;
        if !made.is_ok_and(|output| output.status.success()) {
            eprintln!("no media tool here, the thumbnails were not exercised");
            return;
        }

        let film_folder = media.clone();
        let (state, library) = state_with_roots(directory.path(), vec![("disk-one", media)]).await;
        // This library has been told to do both readings itself, which is what
        // the switch on its settings screen does.
        let library = reading_everything_during_the_scan(&state, &library).await;
        let report = scan(&state, &library).await;
        assert_eq!(report.added, 1);
        // The readings follow the scan rather than sitting inside it, so that
        // the pages and the pictures come first. This is what the chain runs
        // once they are in.
        assert_eq!(
            read_everything_that_is_waiting(&state).await,
            2,
            "one job for each reading"
        );

        let source_id = state
            .database()
            .sources_of_root(library.roots[0].id)
            .await
            .expect("read")
            .first()
            .map(|source| source.id)
            .expect("the film was recorded");
        let thumbnails = state
            .database()
            .thumbnails_of(source_id)
            .await
            .expect("read")
            .expect("written down");
        assert_eq!(thumbnails.counted, 3, "nought, ten and twenty seconds");
        assert_eq!(thumbnails.sheets, 1);
        assert!(thumbnails.width > 0 && thumbnails.height > 0);

        assert!(crate::thumbnails::sheet_of(&state, source_id, 0)
            .await
            .expect("the sheet is in the cache")
            .exists());
        assert!(
            crate::thumbnails::sheet_of(&state, source_id, 1)
                .await
                .is_err(),
            "there is no second sheet to ask for"
        );

        // Nothing half written is left behind in the cache.
        let folder = state.config().directories.thumbnails();
        let left = std::fs::read_dir(&folder)
            .expect("the cache folder")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().contains(".making"))
            .count();
        assert_eq!(left, 0);

        // A second scan reads nothing again: the film already has them, so
        // there is nothing left waiting for the readings to be started for.
        scan(&state, &library).await;
        assert_eq!(
            read_everything_that_is_waiting(&state).await,
            0,
            "nothing is waiting, so nothing is started"
        );

        // And a server whose table is gone takes up what is on the disk rather
        // than reading every film again. Three hundred films are a night of
        // reading, and a row lost must never cost that night twice.
        let sheet = crate::thumbnails::sheet_of(&state, source_id, 0)
            .await
            .expect("the sheet is there");
        let written_at = std::fs::metadata(&sheet)
            .expect("the sheet is there")
            .modified()
            .expect("a time");

        let (fresh, same_library) =
            state_with_roots(directory.path(), vec![("disk-one", film_folder)]).await;
        let same_library = reading_everything_during_the_scan(&fresh, &same_library).await;
        let from_nothing = scan(&fresh, &same_library).await;
        assert_eq!(from_nothing.added, 1, "the table knew nothing of this film");
        read_everything_that_is_waiting(&fresh).await;
        let found_again = fresh
            .database()
            .sources_of_root(same_library.roots[0].id)
            .await
            .expect("read")
            .first()
            .map(|source| source.id)
            .expect("the film was recorded again");
        assert!(
            fresh
                .database()
                .thumbnails_of(found_again)
                .await
                .expect("read")
                .is_some(),
            "and it has its thumbnails again"
        );
        assert_eq!(
            std::fs::metadata(&sheet)
                .expect("the sheet is still there")
                .modified()
                .expect("a time"),
            written_at,
            "the sheet was taken up as it stands, so the film was never read again"
        );
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
        let job = start_scan(
            &state,
            library.clone(),
            JobPriority::REQUESTED,
            RefreshMode::default(),
        )
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
        let first = start_scan(
            &state,
            library.clone(),
            JobPriority::REQUESTED,
            RefreshMode::default(),
        )
        .await
        .expect("job started");
        let second = start_scan(
            &state,
            library.clone(),
            JobPriority::REQUESTED,
            RefreshMode::default(),
        )
        .await;
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
