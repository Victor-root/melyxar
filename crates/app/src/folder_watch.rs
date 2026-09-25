//! Watching the folders of a library, and scanning it again as soon as
//! something in them changes.
//!
//! Only for a library that asked for it: the switch is off by default. What is
//! watched is left to the kernel, which says when a file is created, written,
//! moved or removed. Nothing here decides what that change means: once the
//! folders have been quiet for a moment, the library's ordinary scan runs, and
//! it already knows a new file from a moved one or from one that went away. A
//! second path doing half of that would be a second truth to keep straight.
//!
//! Quiet means two things. No change in the library for a few seconds, which a
//! film being copied never gives, since a copy writes without a pause until it
//! is done. And every file touched keeping the same size between two looks, for
//! the copy that stalls for a moment halfway through. A scan already running
//! is never doubled: the change waits for it to end and sets another going.

use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::time::Duration;

use melyxar_core::id::LibraryId;
use melyxar_core::job::JobPriority;
use melyxar_core::refresh::RefreshMode;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::{mpsc, Mutex};
use tokio::time::Instant;

use crate::AppState;

/// How long a library's folders must go without a change before it is
/// scanned. Long enough to bridge the gaps between the files of one copy of a
/// whole season, short enough that a film dropped in shows up while its owner
/// is still looking.
const QUIET: Duration = Duration::from_secs(10);

/// How long apart the two looks at the sizes of the files touched are.
const SIZES_LOOKED_AT_AGAIN: Duration = Duration::from_secs(3);

/// How often the watched libraries are brought in line with the database on
/// their own, which is also how a watch that could not be set up is tried
/// again: a disk mounted late, a limit raised on the host.
const TRIED_AGAIN_EVERY: Duration = Duration::from_secs(300);

/// How often the libraries waiting to be scanned are looked at.
const LOOKED_AT_EVERY: Duration = Duration::from_secs(1);

/// Why a library that asked to be watched is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhyNot {
    /// The system watches no more folders than its limit, and this library
    /// holds more than were left. Raised on the host, not here.
    TooManyFolders,
    /// One of its folders is not there.
    FolderMissing,
    /// The system would not watch it, for a reason it did not name.
    Unavailable,
}

impl WhyNot {
    /// The word a screen turns into a sentence.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TooManyFolders => "too_many_folders",
            Self::FolderMissing => "folder_missing",
            Self::Unavailable => "unavailable",
        }
    }

    fn of(error: &notify::Error) -> Self {
        match error.kind {
            notify::ErrorKind::MaxFilesWatch => Self::TooManyFolders,
            notify::ErrorKind::PathNotFound => Self::FolderMissing,
            _ => Self::Unavailable,
        }
    }
}

/// Where the watching of one library stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatchState {
    Watching,
    Refused(WhyNot),
}

/// What the kernel said, and about which library.
type Seen = (LibraryId, notify::Result<notify::Event>);

/// One library being watched: the folders it was set up for, so a folder added
/// or taken away sets it up again, and the watch itself, which stops when it
/// is dropped.
struct Watched {
    roots: Vec<PathBuf>,
    state: WatchState,
    _watcher: Option<RecommendedWatcher>,
}

/// The watching of every library that asked for it.
pub struct FolderWatch {
    watched: Mutex<HashMap<LibraryId, Watched>>,
    /// Held while the watches are brought in line, so two changes made at
    /// once never set the same library up twice.
    following: Mutex<()>,
    sender: mpsc::UnboundedSender<Seen>,
    /// Taken once, by the task that follows the changes.
    receiver: std::sync::Mutex<Option<mpsc::UnboundedReceiver<Seen>>>,
}

impl Default for FolderWatch {
    fn default() -> Self {
        let (sender, receiver) = mpsc::unbounded_channel();
        Self {
            watched: Mutex::new(HashMap::new()),
            following: Mutex::new(()),
            sender,
            receiver: std::sync::Mutex::new(Some(receiver)),
        }
    }
}

impl FolderWatch {
    /// Where the watching of a library stands, and nothing for a library that
    /// is not being watched, or not yet.
    pub async fn state_of(&self, library: LibraryId) -> Option<WatchState> {
        self.watched.lock().await.get(&library).map(|watched| watched.state)
    }

    /// Brings the watches in line with what the libraries ask for now: a
    /// library that asked is watched, one that no longer does is let go, one
    /// whose folders changed is set up again, and one whose watch was refused
    /// is tried again.
    ///
    /// Waited for by whoever changed a library, so a screen reading the
    /// libraries right after reads where the watching really stands.
    pub async fn follow_the_libraries(&self, state: &AppState) {
        let libraries = match state.database().list_libraries().await {
            Ok(libraries) => libraries,
            Err(error) => {
                tracing::warn!(%error, "the libraries could not be read to follow what they watch");
                return;
            }
        };
        let _one_at_a_time = self.following.lock().await;
        let wanted: Vec<(LibraryId, String, Vec<PathBuf>)> = libraries
            .iter()
            .filter(|library| library.options.watch_in_real_time)
            .map(|library| {
                let roots = library.roots.iter().map(|root| root.path.clone()).collect();
                (library.id, library.name.clone(), roots)
            })
            .collect();

        // What has to be set up, worked out and let go of quickly: setting a
        // watch up walks every folder of a disk, and the screens reading
        // where the watching stands must not wait on that.
        let to_set_up: Vec<(LibraryId, String, Vec<PathBuf>)> = {
            let mut watched = self.watched.lock().await;
            watched.retain(|id, _| wanted.iter().any(|(kept, _, _)| kept == id));
            wanted
                .into_iter()
                .filter(|(id, _, roots)| {
                    !watched
                        .get(id)
                        .is_some_and(|kept| kept.roots == *roots && kept.state == WatchState::Watching)
                })
                .collect()
        };

        for (id, name, roots) in to_set_up {
            // The old watch goes before the new one is made, rather than the
            // two holding a watch on every folder at once against the limit.
            self.watched.lock().await.remove(&id);
            let sender = self.sender.clone();
            let folders = roots.clone();
            let made =
                tokio::task::spawn_blocking(move || watch_the_folders(id, &folders, sender)).await;
            let (watcher, state) = match made {
                Ok(made) => made,
                Err(error) => {
                    tracing::warn!(%error, "setting up a watch did not finish");
                    (None, WatchState::Refused(WhyNot::Unavailable))
                }
            };
            match state {
                WatchState::Watching => tracing::info!(
                    library = name,
                    folders = roots.len(),
                    "the folders of a library are watched"
                ),
                WatchState::Refused(why) => tracing::warn!(
                    library = name,
                    why = why.as_str(),
                    "the folders of a library could not be watched; tried again in a few minutes"
                ),
            }
            self.watched.lock().await.insert(
                id,
                Watched {
                    roots,
                    state,
                    _watcher: watcher,
                },
            );
        }
    }

    /// Takes note that a watch stopped working after it was set up, which is
    /// how the kernel reports running out of folders it may watch halfway
    /// through a large library.
    async fn refused(&self, library: LibraryId, why: WhyNot) {
        if let Some(watched) = self.watched.lock().await.get_mut(&library) {
            watched.state = WatchState::Refused(why);
        }
    }

    async fn roots_of(&self, library: LibraryId) -> Option<Vec<PathBuf>> {
        self.watched
            .lock()
            .await
            .get(&library)
            .map(|watched| watched.roots.clone())
    }
}

/// Sets up the watch of a library's folders. Blocking: the kernel is handed
/// every folder under them one at a time.
fn watch_the_folders(
    library: LibraryId,
    roots: &[PathBuf],
    sender: mpsc::UnboundedSender<Seen>,
) -> (Option<RecommendedWatcher>, WatchState) {
    let made = notify::recommended_watcher(move |seen| {
        // Nobody left to tell only once the server is stopping.
        let _ = sender.send((library, seen));
    });
    let mut watcher = match made {
        Ok(watcher) => watcher,
        Err(error) => return (None, WatchState::Refused(WhyNot::of(&error))),
    };
    for root in roots {
        if let Err(error) = watcher.watch(root, RecursiveMode::Recursive) {
            return (Some(watcher), WatchState::Refused(WhyNot::of(&error)));
        }
    }
    (Some(watcher), WatchState::Watching)
}

/// The files a change is about that are worth a scan, or nothing when the
/// change is worth none.
///
/// A file opened or read, which is every film being played, changes nothing a
/// scan would find, and neither does a change of its dates or permissions.
/// Hidden files and folders are never scanned, so what happens in them is
/// never worth one either. A change the kernel could not follow closely
/// enough to name is worth a scan with nothing in particular to wait on.
fn worth_a_scan(event: &notify::Event, roots: &[PathBuf]) -> Option<Vec<PathBuf>> {
    if event.need_rescan() {
        return Some(Vec::new());
    }
    match event.kind {
        EventKind::Access(_) | EventKind::Modify(notify::event::ModifyKind::Metadata(_)) => {
            return None;
        }
        _ => {}
    }
    let paths: Vec<PathBuf> = event
        .paths
        .iter()
        .filter(|path| !is_hidden(path, roots))
        .cloned()
        .collect();
    (!paths.is_empty()).then_some(paths)
}

/// Whether a path lies in a hidden file or folder under the root it is in.
fn is_hidden(path: &Path, roots: &[PathBuf]) -> bool {
    let inside = roots
        .iter()
        .find_map(|root| path.strip_prefix(root).ok())
        .unwrap_or(path);
    inside
        .components()
        .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
}

/// What to do next about a library whose folders changed.
#[derive(Debug, PartialEq, Eq)]
enum Next {
    Wait,
    /// Look at the sizes of the files touched.
    LookAtTheSizes,
    Scan,
}

/// A library whose folders changed, on its way to being scanned.
#[derive(Debug)]
struct Settling {
    last_change: Instant,
    touched: BTreeSet<PathBuf>,
    /// The sizes at the last look, in the order of `touched`, and when.
    sizes: Option<(Vec<Option<u64>>, Instant)>,
    /// Whether a scan was due and another was still running.
    waiting_for_a_scan: bool,
}

impl Settling {
    fn new(now: Instant) -> Self {
        Self {
            last_change: now,
            touched: BTreeSet::new(),
            sizes: None,
            waiting_for_a_scan: false,
        }
    }

    /// Something changed again: the quiet starts over.
    fn changed(&mut self, paths: Vec<PathBuf>, now: Instant) {
        self.touched.extend(paths);
        self.last_change = now;
        self.sizes = None;
        self.waiting_for_a_scan = false;
    }

    fn next(&self, now: Instant) -> Next {
        if self.waiting_for_a_scan {
            return Next::Scan;
        }
        if now.duration_since(self.last_change) < QUIET {
            return Next::Wait;
        }
        match &self.sizes {
            Some((_, at)) if now.duration_since(*at) < SIZES_LOOKED_AT_AGAIN => Next::Wait,
            _ => Next::LookAtTheSizes,
        }
    }

    /// Takes the sizes just read, and says whether they held since the last
    /// look, which is the files having stopped growing.
    fn looked(&mut self, sizes: Vec<Option<u64>>, now: Instant) -> bool {
        let held = self
            .sizes
            .as_ref()
            .is_some_and(|(before, _)| *before == sizes);
        self.sizes = Some((sizes, now));
        held
    }
}

/// The size of each file, or nothing for one that is not there any more.
fn sizes_of(paths: Vec<PathBuf>) -> Vec<Option<u64>> {
    paths
        .iter()
        .map(|path| std::fs::metadata(path).ok().map(|found| found.len()))
        .collect()
}

/// Watches the libraries that asked for it for as long as the server runs, and
/// scans each one again once the changes in its folders have settled.
pub fn keep_watching(state: &AppState) -> tokio::task::JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move {
        let Some(mut changes) = state
            .folder_watch()
            .receiver
            .lock()
            .ok()
            .and_then(|mut receiver| receiver.take())
        else {
            return;
        };
        state.folder_watch().follow_the_libraries(&state).await;

        let mut settling: HashMap<LibraryId, Settling> = HashMap::new();
        let mut look = tokio::time::interval(LOOKED_AT_EVERY);
        let mut try_again = tokio::time::interval(TRIED_AGAIN_EVERY);
        // Both fire at once to begin with, and the libraries were just followed.
        try_again.tick().await;
        loop {
            tokio::select! {
                seen = changes.recv() => {
                    let Some((library, seen)) = seen else { return };
                    match seen {
                        Ok(event) => {
                            let roots = state.folder_watch().roots_of(library).await.unwrap_or_default();
                            if let Some(paths) = worth_a_scan(&event, &roots) {
                                let now = Instant::now();
                                settling
                                    .entry(library)
                                    .or_insert_with(|| Settling::new(now))
                                    .changed(paths, now);
                            }
                        }
                        Err(error) => {
                            let why = WhyNot::of(&error);
                            tracing::warn!(%error, why = why.as_str(), "a watched library stopped being watched");
                            state.folder_watch().refused(library, why).await;
                        }
                    }
                }
                _ = look.tick() => {
                    settle(&state, &mut settling).await;
                }
                _ = try_again.tick() => {
                    state.folder_watch().follow_the_libraries(&state).await;
                }
            }
        }
    })
}

/// Moves every library whose folders changed one step closer to its scan.
async fn settle(state: &AppState, settling: &mut HashMap<LibraryId, Settling>) {
    let now = Instant::now();
    let mut done = Vec::new();
    for (library, waiting) in settling.iter_mut() {
        match waiting.next(now) {
            Next::Wait => {}
            Next::LookAtTheSizes => {
                let paths: Vec<PathBuf> = waiting.touched.iter().cloned().collect();
                let sizes = tokio::task::spawn_blocking(move || sizes_of(paths))
                    .await
                    .unwrap_or_default();
                if waiting.looked(sizes, now) {
                    waiting.waiting_for_a_scan = true;
                }
            }
            Next::Scan => {
                if scan_it(state, *library, &waiting.touched).await {
                    done.push(*library);
                }
            }
        }
    }
    for library in done {
        settling.remove(&library);
    }
}

/// Starts the scan of a library whose folders changed. Answers whether it is
/// done with: started, or not worth waiting on any more. A scan already
/// running is waited on, and the next look tries again.
async fn scan_it(state: &AppState, library: LibraryId, touched: &BTreeSet<PathBuf>) -> bool {
    let found = match state.database().list_libraries().await {
        Ok(libraries) => libraries.into_iter().find(|kept| kept.id == library),
        Err(error) => {
            tracing::warn!(%error, "the library whose folders changed could not be read");
            return false;
        }
    };
    let Some(library) = found.filter(|kept| kept.options.watch_in_real_time) else {
        // Taken away, or no longer watched, while its changes settled.
        return true;
    };
    let name = library.name.clone();
    match crate::scan::start_scan_and_identification(
        state,
        library,
        JobPriority::REQUESTED,
        RefreshMode::default(),
    )
    .await
    {
        Ok(_) => {
            tracing::info!(
                library = name,
                files = touched.len(),
                first = touched.iter().next().map(|path| path.display().to_string()),
                "something changed in the folders of a library, which is scanned again"
            );
            true
        }
        Err(crate::AppError::Jobs(melyxar_jobs::JobError::AlreadyUnderWay)) => false,
        Err(error) => {
            tracing::warn!(library = name, %error, "the scan of a library whose folders changed would not start");
            true
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, seconds: u64) -> Instant {
        start + Duration::from_secs(seconds)
    }

    #[test]
    fn a_library_is_scanned_only_once_quiet_and_its_files_stopped_growing() {
        let start = Instant::now();
        let mut settling = Settling::new(start);
        settling.changed(vec![PathBuf::from("/films/Quiet Harbour (2019).mkv")], start);

        assert_eq!(settling.next(at(start, 5)), Next::Wait, "not quiet for long enough");
        assert_eq!(settling.next(at(start, 11)), Next::LookAtTheSizes);
        assert!(!settling.looked(vec![Some(100)], at(start, 11)), "one look proves nothing");
        assert_eq!(settling.next(at(start, 12)), Next::Wait, "the second look is a moment later");
        assert_eq!(settling.next(at(start, 15)), Next::LookAtTheSizes);
        assert!(settling.looked(vec![Some(100)], at(start, 15)), "the same size twice");
    }

    #[test]
    fn a_file_still_growing_is_waited_for() {
        let start = Instant::now();
        let mut settling = Settling::new(start);
        settling.changed(vec![PathBuf::from("/films/Quiet Harbour (2019).mkv")], start);
        assert!(!settling.looked(vec![Some(100)], at(start, 11)));
        // A copy that paused long enough to look quiet, then went on.
        assert!(!settling.looked(vec![Some(900)], at(start, 15)));
        assert!(settling.looked(vec![Some(900)], at(start, 19)));
    }

    #[test]
    fn any_change_starts_the_quiet_over() {
        let start = Instant::now();
        let mut settling = Settling::new(start);
        settling.changed(vec![PathBuf::from("/a.mkv")], start);
        settling.looked(vec![Some(1)], at(start, 11));
        settling.changed(vec![PathBuf::from("/b.mkv")], at(start, 12));
        assert_eq!(settling.next(at(start, 20)), Next::Wait);
        assert_eq!(settling.next(at(start, 23)), Next::LookAtTheSizes);
        assert_eq!(settling.touched.len(), 2, "both files are waited on");
    }

    #[test]
    fn a_scan_that_could_not_start_is_asked_for_again_at_once() {
        let start = Instant::now();
        let mut settling = Settling::new(start);
        settling.waiting_for_a_scan = true;
        assert_eq!(settling.next(start), Next::Scan);
    }

    fn event(kind: EventKind, path: &str) -> notify::Event {
        notify::Event::new(kind).add_path(PathBuf::from(path))
    }

    #[test]
    fn a_film_being_played_is_not_a_change() {
        let roots = [PathBuf::from("/mnt/disk/Films")];
        use notify::event::{AccessKind, AccessMode, CreateKind, MetadataKind, ModifyKind};
        assert_eq!(
            worth_a_scan(
                &event(
                    EventKind::Access(AccessKind::Open(AccessMode::Read)),
                    "/mnt/disk/Films/Quiet Harbour (2019).mkv"
                ),
                &roots
            ),
            None,
            "every film played is opened"
        );
        assert_eq!(
            worth_a_scan(
                &event(
                    EventKind::Modify(ModifyKind::Metadata(MetadataKind::AccessTime)),
                    "/mnt/disk/Films/Quiet Harbour (2019).mkv"
                ),
                &roots
            ),
            None,
            "nor is the date it was last read"
        );
        assert_eq!(
            worth_a_scan(
                &event(
                    EventKind::Create(CreateKind::File),
                    "/mnt/disk/Films/Quiet Harbour (2019).mkv"
                ),
                &roots
            ),
            Some(vec![PathBuf::from("/mnt/disk/Films/Quiet Harbour (2019).mkv")])
        );
    }

    #[test]
    fn what_happens_in_hidden_files_is_not_a_change() {
        let roots = [PathBuf::from("/mnt/disk/Films")];
        use notify::event::CreateKind;
        assert_eq!(
            worth_a_scan(
                &event(EventKind::Create(CreateKind::File), "/mnt/disk/Films/.Trash-1000/files/a.mkv"),
                &roots
            ),
            None
        );
        assert_eq!(
            worth_a_scan(
                &event(EventKind::Create(CreateKind::File), "/mnt/disk/Films/.partial.mkv"),
                &roots
            ),
            None
        );
    }

    #[test]
    fn a_change_the_kernel_lost_track_of_is_worth_a_scan() {
        let lost = notify::Event::new(EventKind::Other).set_flag(notify::event::Flag::Rescan);
        assert_eq!(worth_a_scan(&lost, &[]), Some(Vec::new()));
    }

    #[tokio::test]
    async fn a_file_dropped_in_a_watched_folder_is_heard_of() {
        let folder = tempfile::tempdir().expect("a folder");
        let (sender, mut receiver) = mpsc::unbounded_channel();
        let library = LibraryId::new();
        let roots = vec![folder.path().to_path_buf()];
        let (watcher, state) = watch_the_folders(library, &roots, sender);
        assert_eq!(state, WatchState::Watching);
        let _kept = watcher;

        std::fs::write(folder.path().join("Quiet Harbour (2019).mkv"), b"film").expect("written");
        let heard = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let (from, seen) = receiver.recv().await.expect("still watching");
                assert_eq!(from, library);
                if let Some(paths) = worth_a_scan(&seen.expect("an event"), &roots) {
                    return paths;
                }
            }
        })
        .await
        .expect("the change was heard of");
        assert_eq!(heard, vec![folder.path().join("Quiet Harbour (2019).mkv")]);
    }

    #[test]
    fn a_folder_that_is_not_there_says_so() {
        let (sender, _receiver) = mpsc::unbounded_channel();
        let (_, state) = watch_the_folders(
            LibraryId::new(),
            &[PathBuf::from("/nowhere/at/all/Films")],
            sender,
        );
        assert_eq!(state, WatchState::Refused(WhyNot::FolderMissing));
    }
}
