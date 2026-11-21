//! What the tests of music set up before asking anything: a library of
//! music on a disk of its own, songs copied into it, a scan of it, and the
//! works it made.

use std::path::{Path, PathBuf};

use melyxar_core::job::{JobPriority, JobState};
use melyxar_core::library::Library;
use melyxar_core::refresh::RefreshMode;
use melyxar_core::work::Work;

use crate::AppState;
use crate::scan::ScanReport;

/// The one second files the tag reader is tested on, shared rather than made
/// again.
pub(crate) fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tags/tests/fixtures")
        .join(name)
}

/// A copy of a fixture, where a song of the library would be.
pub(crate) fn put(root: &Path, relative: &str, fixture_name: &str) -> PathBuf {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("folder made");
    std::fs::copy(fixture(fixture_name), &path).expect("song copied");
    path
}

/// A server holding one library of music, on one disk.
pub(crate) async fn music_library(directory: &Path, root: &Path) -> (AppState, Library) {
    let (config, database, library) =
        crate::a_test_server(directory, "music", vec![("disk-one", root.to_path_buf())]).await;
    let (tools, capabilities) =
        crate::startup::media_tools(Ok(melyxar_ffmpeg::testing::tools())).await;
    (
        AppState::new(config, database, tools, capabilities),
        library,
    )
}

/// A scan of the library, waited for to its end.
pub(crate) async fn scan(state: &AppState, library: &Library) -> ScanReport {
    let (ended, report) = crate::scan::start_scan(
        state,
        library.clone(),
        JobPriority::REQUESTED,
        RefreshMode::default(),
    )
    .await
    .expect("scan started")
    .wait()
    .await;
    assert_eq!(ended, JobState::Succeeded);
    report.expect("a finished scan says what it did")
}

/// The song a file of the library became.
pub(crate) async fn song_of(state: &AppState, library: &Library, relative: &str) -> Work {
    let source = state
        .database()
        .sources_of_root(library.roots[0].id)
        .await
        .expect("read")
        .into_iter()
        .find(|source| source.relative_path == Path::new(relative))
        .expect("the file is written down");
    state
        .database()
        .work(source.work_id)
        .await
        .expect("read")
        .expect("its song")
}

/// The album a song is on.
pub(crate) async fn album_of(state: &AppState, song: &Work) -> Work {
    state
        .database()
        .work(song.parent_id.expect("on an album"))
        .await
        .expect("read")
        .expect("the album")
}
