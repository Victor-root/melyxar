//! Filing the songs of a library of music as its scan finds them.
//!
//! The walk, the comparison with what is written down, the files gone and the
//! files back are the scan's own and the same for every library. What a
//! library of music does apart is everything after: reading what each file
//! says, deciding a folder at a time where its songs go, and writing them
//! down with what the reading gave, so no file ever waits on the analyser of
//! the films.
//!
//! A folder is filed again whenever one of its files is new, changed, or not
//! read yet, and all of it is read again then: whether an album nobody claimed
//! is a compilation is a question about every song on it, and one song added
//! to it can change the answer for the others.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use melyxar_core::id::{MediaSourceId, TrackId, WorkId};
use melyxar_core::job::JobStep;
use melyxar_core::library::{Library, LibraryRoot};
use melyxar_core::media::{AudioDetails, Track, TrackKind};
use melyxar_core::media_log::MediaPath;
use melyxar_core::music::loudness_from_replay_gain;
use melyxar_core::time::{Millis, Timestamp};
use melyxar_database::catalogue::SourceAnalysis;
use melyxar_database::music::MusicFile;
use melyxar_jobs::JobHandle;
use melyxar_library::FoundFile;
use melyxar_library::music::{FolderSong, file_folder};
use melyxar_tags::{ReadError, Tags};

use crate::scan::ScanReport;
use crate::{AppState, Result};

/// Files the songs of one root that need it: those of every folder where a
/// file arrived, changed, or was never read.
///
/// `media` is everything the walk found on this root, `added` what it found
/// that is not written down yet and was not set aside.
pub(crate) async fn file_root(
    state: &AppState,
    library: &Library,
    root: &LibraryRoot,
    media: &[FoundFile],
    added: &[FoundFile],
    handle: &JobHandle,
    report: &mut ScanReport,
) -> Result<()> {
    let database = state.database();
    let stored: HashMap<PathBuf, (MediaSourceId, WorkId)> = database
        .sources_of_root(root.id)
        .await?
        .into_iter()
        .map(|source| (source.relative_path, (source.id, source.work_id)))
        .collect();
    let never_read = database.unanalysed_sources_of_root(root.id).await?;

    let folders: HashSet<PathBuf> = added
        .iter()
        .map(|file| folder_of(&file.relative_path))
        .chain(
            never_read
                .iter()
                .map(|source| folder_of(&source.relative_path)),
        )
        .collect();
    if folders.is_empty() {
        return Ok(());
    }

    // Every file of those folders the library holds or takes in. A file set
    // aside is neither, and stays out of the filing of its neighbours too.
    let arriving: HashSet<&Path> = added
        .iter()
        .map(|file| file.relative_path.as_path())
        .collect();
    let to_read: Vec<ToRead> = media
        .iter()
        .filter(|file| folders.contains(&folder_of(&file.relative_path)))
        .filter_map(|file| {
            let known = stored.get(&file.relative_path).copied();
            (known.is_some() || arriving.contains(file.relative_path.as_path())).then(|| ToRead {
                path: root.path.join(&file.relative_path),
                relative_path: file.relative_path.clone(),
                size_bytes: file.size_bytes,
                modified_at: file.modified_at,
                source_id: known.map_or_else(MediaSourceId::new, |(source, _)| source),
                song: known.map(|(_, song)| song),
            })
        })
        .collect();

    tracing::debug!(
        library = library.name,
        root = root.label,
        folders = folders.len(),
        files = to_read.len(),
        "the songs of these folders are read and filed"
    );

    let Some(read) = read_all(state, &root.label, to_read, handle).await else {
        report.cancelled = true;
        return Ok(());
    };

    // Folder by folder, in path order, so two scans file the same way.
    let mut by_folder: BTreeMap<PathBuf, Vec<Read>> = BTreeMap::new();
    for one in read {
        by_folder
            .entry(folder_of(&one.file.relative_path))
            .or_default()
            .push(one);
    }

    for (folder, songs) in by_folder {
        let listed: Vec<FolderSong<'_>> = songs
            .iter()
            .map(|one| FolderSong {
                file_name: one
                    .file
                    .relative_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or_default(),
                tags: &one.tags,
            })
            .collect();
        let filings = file_folder(&folder, &listed);

        let files: Vec<MusicFile> = songs
            .into_iter()
            .zip(filings)
            .map(|(one, filing)| {
                match &one.reading {
                    Ok(_) => report.analysed += 1,
                    Err(_) => report.unreadable_files += 1,
                }
                MusicFile {
                    source_id: one.file.source_id,
                    song: one.file.song,
                    relative_path: one.file.relative_path,
                    size_bytes: one.file.size_bytes,
                    modified_at: one.file.modified_at,
                    filing,
                    reading: one.reading,
                }
            })
            .collect();

        let filed = database.file_music(library.id, root.id, &files).await?;
        report.added += filed.added;
        report.refiled += filed.moved;
    }
    Ok(())
}

/// Takes away what the filing left behind: albums no song is on any more and
/// artists nobody is credited as, with their pictures. Asked once per scan,
/// after every root is filed, since a song can leave an album on one disk for
/// one on another.
pub(crate) async fn prune(state: &AppState, library: &Library) -> Result<()> {
    let swept = state.database().prune_music(library.id).await?;
    crate::images::forget_the_pictures(state, &swept.picture_paths).await;
    Ok(())
}

/// The folder a file sits in, relative to its root.
fn folder_of(relative_path: &Path) -> PathBuf {
    relative_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default()
}

/// One file to read, with everything needed to write it down afterwards.
struct ToRead {
    path: PathBuf,
    relative_path: PathBuf,
    size_bytes: i64,
    modified_at: Timestamp,
    source_id: MediaSourceId,
    song: Option<WorkId>,
}

/// What reading one file gave.
struct Read {
    file: ToRead,
    tags: Tags,
    reading: std::result::Result<(SourceAnalysis, Track), String>,
}

/// Reads every file, a few at a time. Answers nothing when the scan was
/// stopped part way, so that nothing half read is filed.
async fn read_all(
    state: &AppState,
    root_label: &str,
    files: Vec<ToRead>,
    handle: &JobHandle,
) -> Option<Vec<Read>> {
    if files.is_empty() {
        return Some(Vec::new());
    }
    handle.at_step(JobStep::AnalysingFiles).await;
    handle.set_total(files.len() as i64).await;

    // Bounded like the analysis of the films, and for the same reason: the
    // machine is there for somebody watching or listening first.
    let limit = state.config().limits.concurrent_probes;
    let analyser = state.tools().map(|tools| tools.ffprobe.clone());
    let owned_handle = handle.clone();
    let label = root_label.to_string();

    let read = melyxar_jobs::for_each_bounded(files, limit, move |file| {
        let handle = owned_handle.clone();
        let analyser = analyser.clone();
        let label = label.clone();
        async move {
            if handle.is_cancelled() {
                return None;
            }
            let name = file
                .relative_path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default()
                .to_string();
            handle.now_working_on(Some(&name)).await;
            let one = read_one(file, analyser.as_deref(), &label).await;
            handle.advance(1).await;
            Some(one)
        }
    })
    .await;
    read.into_iter().collect()
}

/// Reads one file: in place first, and failing that with the analyser of the
/// films, which knows the few forms the reader here does not.
async fn read_one(file: ToRead, analyser: Option<&Path>, root_label: &str) -> Read {
    let path = file.path.clone();
    let in_place = tokio::task::spawn_blocking(move || melyxar_tags::read(&path))
        .await
        .unwrap_or_else(|failure| std::panic::resume_unwind(failure.into_panic()));

    let (tags, reading) = match in_place {
        Ok(read) => (read.tags, Ok(described(&read.sound, file.source_id))),
        Err(ReadError::Unsupported) => match analyser {
            Some(analyser) => probed(analyser, &file).await,
            None => (
                Tags::default(),
                Err("no media tool is available to read this kind of file".to_string()),
            ),
        },
        Err(ReadError::Unreadable(reason)) => (Tags::default(), Err(reason)),
    };

    if let Err(reason) = &reading {
        tracing::warn!(
            file = %MediaPath::new(root_label, &file.path),
            error = %reason,
            "this song could not be read; it is filed by its name and its folders"
        );
    }
    Read {
        file,
        tags,
        reading,
    }
}

/// What the analyser of the films says of a file the reader here does not
/// know: its tags as pairs, and its first stream of sound.
async fn probed(
    analyser: &Path,
    file: &ToRead,
) -> (Tags, std::result::Result<(SourceAnalysis, Track), String>) {
    let report = match melyxar_ffmpeg::probe::probe(analyser, &file.path).await {
        Ok(report) => report,
        Err(error) => return (Tags::default(), Err(error.to_string())),
    };
    let tags = melyxar_tags::from_pairs(
        report
            .format
            .tags
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str())),
    );
    let analysed = melyxar_media_probe::AnalysedFile::from_report(&report, file.source_id);
    let sound = analysed
        .tracks
        .into_iter()
        .find(|track| matches!(track.kind, TrackKind::Audio(_)));
    let reading = match sound {
        Some(track) => Ok((
            SourceAnalysis {
                container: analysed.container,
                duration: analysed.duration,
                overall_bitrate: analysed.overall_bitrate,
            },
            track,
        )),
        None => Err("the file holds no sound".to_string()),
    };
    (tags, reading)
}

/// The file and its one stream of sound, as the rest of the server describes
/// a file, from what the reader found.
fn described(sound: &melyxar_tags::Sound, source_id: MediaSourceId) -> (SourceAnalysis, Track) {
    let bits = |kilobits: u32| i64::from(kilobits) * 1_000;
    (
        SourceAnalysis {
            container: Some(sound.container.to_string()),
            duration: Some(Millis::new(
                i64::try_from(sound.duration_ms).unwrap_or(i64::MAX),
            )),
            overall_bitrate: sound.overall_bitrate.map(bits),
        },
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: sound.codec.to_string(),
                profile: None,
                // A file that does not say is taken for the stereo nearly
                // every song is.
                channels: sound.channels.map_or(2, i32::from),
                channel_layout: None,
                sample_rate: sound.sample_rate.and_then(|rate| i32::try_from(rate).ok()),
                bit_depth: sound.bit_depth.map(i32::from),
                bitrate: sound.audio_bitrate.map(bits),
                loudness: loudness_from_replay_gain(sound.replay_gain_db, sound.replay_gain_peak),
            }),
        },
    )
}

#[cfg(test)]
mod tests {
    use std::process::Command;

    use melyxar_core::job::{JobPriority, JobState};
    use melyxar_core::refresh::RefreshMode;
    use melyxar_core::work::{Work, WorkKind};

    use super::*;

    /// The one second files the tag reader is tested on, shared rather than
    /// made again.
    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tags/tests/fixtures")
            .join(name)
    }

    fn put(root: &Path, relative: &str, fixture_name: &str) -> PathBuf {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folder made");
        std::fs::copy(fixture(fixture_name), &path).expect("song copied");
        path
    }

    /// Writes tags into a copy of a fixture the way a tagging program would,
    /// with the tool the server runs on. Answers false where there is none.
    fn tagged(root: &Path, relative: &str, fixture_name: &str, tags: &[(&str, &str)]) -> bool {
        let Ok(tools) = melyxar_ffmpeg::ToolPaths::discover(None, None) else {
            return false;
        };
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folder made");
        let mut command = Command::new(&tools.ffmpeg);
        command.args(["-hide_banner", "-loglevel", "error", "-y", "-i"]);
        command.arg(fixture(fixture_name));
        command.args(["-c", "copy", "-map_metadata", "-1"]);
        for (name, value) in tags {
            command.arg("-metadata").arg(format!("{name}={value}"));
        }
        command.arg(&path);
        assert!(
            command.status().expect("the tool runs").success(),
            "{relative} tagged"
        );
        true
    }

    async fn music_library(directory: &Path, root: &Path) -> (AppState, Library) {
        let (config, database, library) =
            crate::a_test_server(directory, "music", vec![("disk-one", root.to_path_buf())]).await;
        let (tools, capabilities) = crate::startup::detect_media_tools(&config).await;
        (
            AppState::new(config, database, tools, capabilities),
            library,
        )
    }

    async fn scan(state: &AppState, library: &Library) -> ScanReport {
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

    async fn works(state: &AppState, library: &Library, kind: WorkKind) -> Vec<Work> {
        let mut found: Vec<Work> = state
            .database()
            .recent_works(library.id, 1_000)
            .await
            .expect("read")
            .into_iter()
            .filter(|work| work.kind == kind)
            .collect();
        found.sort_by(|one, two| one.sort_title.cmp(&two.sort_title));
        found
    }

    async fn song_of(state: &AppState, library: &Library, relative: &str) -> Work {
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

    async fn album_of(state: &AppState, song: &Work) -> Work {
        state
            .database()
            .work(song.parent_id.expect("on an album"))
            .await
            .expect("read")
            .expect("the album")
    }

    #[tokio::test]
    async fn a_scan_has_every_song_read_for_how_loud_it_is_and_how_its_sound_is_spread() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Music");
        put(
            &root,
            "Amber Field/Northern Lights/01 - Quiet Harbour.flac",
            "one-second.flac",
        );
        let (state, library) = music_library(directory.path(), &root).await;
        if state.tools().is_none() {
            eprintln!("no media tool here, nothing was read");
            return;
        }
        crate::scan::scan_and_what_follows(
            &state,
            library.clone(),
            JobPriority::REQUESTED,
            RefreshMode::default(),
        )
        .await
        .expect("scanned");

        // The reading is a job of its own that the scan starts and does not wait for.
        let mut left = i64::MAX;
        for _ in 0..100 {
            left = state
                .database()
                .count_songs_to_analyse(library.id)
                .await
                .expect("read");
            if left == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
        assert_eq!(left, 0, "nothing was left to read");

        let song = song_of(
            &state,
            &library,
            "Amber Field/Northern Lights/01 - Quiet Harbour.flac",
        )
        .await;
        let spectrum = state
            .database()
            .song_spectrum(song.id)
            .await
            .expect("read")
            .expect("its sound was written down");
        assert_eq!(usize::from(spectrum.bands), melyxar_sound::SPECTRUM_BANDS);
        assert_eq!(
            spectrum.levels.len(),
            usize::from(spectrum.bands) * usize::from(spectrum.frames_a_second),
            "a second of sound, one reading a quarter of a second"
        );
        let songs = state
            .database()
            .music_songs(
                library.id,
                melyxar_database::music_browse::SongOrder::Title,
                false,
                0,
                10,
            )
            .await
            .expect("read")
            .items;
        assert!(songs[0].lufs.is_some(), "and how loud it is");
    }

    #[tokio::test]
    async fn songs_that_say_nothing_are_filed_by_their_folders_and_read_in_place() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Music");
        put(
            &root,
            "Amber Field/Northern Lights/01 - Quiet Harbour.flac",
            "one-second.flac",
        );
        put(
            &root,
            "Amber Field/Northern Lights/02 - Tides.mp3",
            "one-second.mp3",
        );
        put(
            &root,
            "Amber Field/Northern Lights/cover.jpg",
            "one-second.mp3",
        );
        let (state, library) = music_library(directory.path(), &root).await;

        let report = scan(&state, &library).await;
        assert_eq!(report.added, 2, "two songs and no picture");
        assert_eq!(report.analysed, 2);
        assert_eq!(report.unreadable_files, 0);

        let first = song_of(
            &state,
            &library,
            "Amber Field/Northern Lights/01 - Quiet Harbour.flac",
        )
        .await;
        assert_eq!(first.kind, WorkKind::Song);
        assert_eq!(first.title, "Quiet Harbour");
        assert_eq!(first.ordinal, Some(1));
        let second = song_of(
            &state,
            &library,
            "Amber Field/Northern Lights/02 - Tides.mp3",
        )
        .await;
        assert_eq!(second.ordinal, Some(2));

        let album = album_of(&state, &first).await;
        assert_eq!(album.title, "Northern Lights");
        assert_eq!(second.parent_id, Some(album.id), "one album");
        let artists = works(&state, &library, WorkKind::Artist).await;
        assert_eq!(
            artists.iter().map(|a| a.title.as_str()).collect::<Vec<_>>(),
            vec!["Amber Field"]
        );

        let source = state
            .database()
            .sources_of_work(first.id)
            .await
            .expect("read")
            .remove(0);
        let tracks = state
            .database()
            .tracks_of_source(source.id)
            .await
            .expect("read");
        match &tracks[0].kind {
            TrackKind::Audio(sound) => assert_eq!(sound.codec, "flac"),
            other => panic!("a song is sound, not {other:?}"),
        }

        // Nothing changed: nothing is read or filed again.
        let again = scan(&state, &library).await;
        assert_eq!((again.added, again.analysed, again.refiled), (0, 0, 0));
        assert_eq!(works(&state, &library, WorkKind::Song).await.len(), 2);
    }

    #[tokio::test]
    async fn a_library_of_music_never_takes_a_video_for_a_song() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Music");
        put(&root, "Amber Field/Live/concert.mkv", "one-second.mp3");
        put(&root, "Amber Field/Live/01 - Opening.ogg", "one-second.ogg");
        let (state, library) = music_library(directory.path(), &root).await;

        let report = scan(&state, &library).await;
        assert_eq!(report.added, 1);
        assert!(works(&state, &library, WorkKind::Movie).await.is_empty());
    }

    #[tokio::test]
    async fn an_album_nobody_claimed_with_songs_by_different_people_is_a_compilation() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Music");
        let one = [
            ("title", "One"),
            ("artist", "Amber Field"),
            ("album", "Summer Hits"),
            ("track", "1"),
        ];
        let two = [
            ("title", "Two"),
            ("artist", "The Lanterns"),
            ("album", "Summer Hits"),
            ("track", "2"),
        ];
        if !tagged(&root, "Hits/1.flac", "one-second.flac", &one) {
            eprintln!("no media tool here, the tags could not be written");
            return;
        }
        tagged(&root, "Hits/2.flac", "one-second.flac", &two);
        let (state, library) = music_library(directory.path(), &root).await;
        scan(&state, &library).await;

        let first = song_of(&state, &library, "Hits/1.flac").await;
        assert_eq!(first.title, "One");
        let album = album_of(&state, &first).await;
        assert_eq!(album.title, "Summer Hits");
        assert_eq!(
            song_of(&state, &library, "Hits/2.flac").await.parent_id,
            Some(album.id)
        );
        let artists: Vec<String> = works(&state, &library, WorkKind::Artist)
            .await
            .into_iter()
            .map(|artist| artist.title)
            .collect();
        assert_eq!(
            artists,
            vec!["Amber Field", "The Lanterns", "Various Artists"]
        );
    }

    #[tokio::test]
    async fn a_song_deleted_takes_its_emptied_album_and_stays_out_of_the_scans() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Music");
        let tags = [
            ("title", "Quiet Harbour"),
            ("artist", "Amber Field"),
            ("album", "Demo"),
        ];
        if !tagged(&root, "A/1.flac", "one-second.flac", &tags) {
            eprintln!("no media tool here, the tags could not be written");
            return;
        }
        let (state, library) = music_library(directory.path(), &root).await;
        scan(&state, &library).await;
        let song = song_of(&state, &library, "A/1.flac").await;
        let owner = state
            .database()
            .create_user(
                "Owner",
                None,
                &melyxar_core::user::Permissions::administrator(),
            )
            .await
            .expect("account created");

        crate::deletion::delete(&state, &owner, &[song.id], false)
            .await
            .expect("taken out");
        assert!(root.join("A/1.flac").exists(), "the file stays on the disk");
        assert!(works(&state, &library, WorkKind::Song).await.is_empty());
        assert!(
            works(&state, &library, WorkKind::Album).await.is_empty(),
            "the album left empty is gone"
        );
        assert!(
            works(&state, &library, WorkKind::Artist).await.is_empty(),
            "so is the artist nobody is credited as"
        );
        assert_eq!(scan(&state, &library).await.added, 0, "it stays out");
    }

    #[tokio::test]
    async fn a_song_tagged_again_moves_to_its_new_album_and_the_old_one_goes() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Music");
        let before = [
            ("title", "Quiet Harbour"),
            ("artist", "Amber Field"),
            ("album", "Demo"),
        ];
        if !tagged(&root, "A/1.flac", "one-second.flac", &before) {
            eprintln!("no media tool here, the tags could not be written");
            return;
        }
        let (state, library) = music_library(directory.path(), &root).await;
        scan(&state, &library).await;
        let song = song_of(&state, &library, "A/1.flac").await;

        // Tagged again: another album. The size moves with it, which is how
        // a scan sees a file has changed.
        let after = [
            ("title", "Quiet Harbour"),
            ("artist", "Amber Field"),
            ("album", "Northern Lights"),
            ("comment", "tagged again, and a little longer for it"),
        ];
        tagged(&root, "A/1.flac", "one-second.flac", &after);
        let report = scan(&state, &library).await;
        assert_eq!(report.changed, 1);
        assert_eq!(report.refiled, 1);

        let moved = song_of(&state, &library, "A/1.flac").await;
        assert_eq!(
            moved.id, song.id,
            "the same song, with whatever was done with it"
        );
        assert_eq!(album_of(&state, &moved).await.title, "Northern Lights");
        let albums: Vec<String> = works(&state, &library, WorkKind::Album)
            .await
            .into_iter()
            .map(|album| album.title)
            .collect();
        assert_eq!(
            albums,
            vec!["Northern Lights"],
            "the album left empty is gone"
        );
    }

    #[tokio::test]
    async fn a_form_the_reader_does_not_know_is_read_by_the_analyser_of_the_films() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Music");
        let tags = [
            ("title", "Quiet Harbour"),
            ("artist", "Amber Field"),
            ("album", "Northern Lights"),
            ("track", "3/12"),
        ];
        if !tagged(&root, "Old/03.wma", "one-second.wma", &tags) {
            eprintln!("no media tool here, the file could not be written");
            return;
        }
        let (state, library) = music_library(directory.path(), &root).await;
        let report = scan(&state, &library).await;
        assert_eq!(
            (report.added, report.analysed, report.unreadable_files),
            (1, 1, 0)
        );

        let song = song_of(&state, &library, "Old/03.wma").await;
        assert_eq!(song.title, "Quiet Harbour");
        assert_eq!(song.ordinal, Some(3));
        assert_eq!(album_of(&state, &song).await.title, "Northern Lights");
    }
}

#[cfg(test)]
mod picture_tests {
    use std::process::Command;

    use melyxar_core::job::{JobPriority, JobState};
    use melyxar_core::refresh::RefreshMode;
    use melyxar_core::work::WorkKind;

    use super::*;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tags/tests/fixtures")
            .join(name)
    }

    /// Runs the tool the server runs on, or answers false where there is
    /// none: pictures are made with it, so without it there is nothing to
    /// test.
    fn run(arguments: &[&std::ffi::OsStr]) -> bool {
        let Ok(tools) = melyxar_ffmpeg::ToolPaths::discover(None, None) else {
            return false;
        };
        let status = Command::new(&tools.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-y"])
            .args(arguments)
            .status()
            .expect("the tool runs");
        assert!(status.success());
        true
    }

    /// A small picture of one colour.
    fn a_picture(path: &Path, colour: &str) -> bool {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folder made");
        let source = format!("color=c={colour}:s=64x64:d=1");
        run(&[
            "-f".as_ref(),
            "lavfi".as_ref(),
            "-i".as_ref(),
            source.as_ref(),
            "-frames:v".as_ref(),
            "1".as_ref(),
            path.as_os_str(),
        ])
    }

    fn a_song(root: &Path, relative: &str) -> PathBuf {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("folder made");
        std::fs::copy(fixture("one-second.mp3"), &path).expect("copied");
        path
    }

    async fn library(directory: &Path, root: &Path) -> (AppState, Library) {
        let (config, database, library) =
            crate::a_test_server(directory, "music", vec![("disk-one", root.to_path_buf())]).await;
        let (tools, capabilities) = crate::startup::detect_media_tools(&config).await;
        (
            AppState::new(config, database, tools, capabilities),
            library,
        )
    }

    async fn scan(state: &AppState, library: &Library) -> ScanReport {
        let (ended, report) = crate::scan::start_scan(
            state,
            library.clone(),
            JobPriority::REQUESTED,
            RefreshMode::default(),
        )
        .await
        .expect("started")
        .wait()
        .await;
        assert_eq!(ended, JobState::Succeeded);
        report.expect("a report")
    }

    async fn wears_a_picture(
        state: &AppState,
        library: &Library,
        kind: WorkKind,
        title: &str,
    ) -> bool {
        let work = state
            .database()
            .recent_works(library.id, 100)
            .await
            .expect("read")
            .into_iter()
            .find(|work| work.kind == kind && work.title == title)
            .expect("the work is there");
        state
            .database()
            .image_fingerprint("work", &work.id.to_db_string(), "poster")
            .await
            .expect("read")
            .is_some()
    }

    #[tokio::test]
    async fn an_album_wears_the_cover_beside_it_and_an_artist_the_picture_above() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Music");
        a_song(&root, "Amber Field/Northern Lights/01 - Quiet Harbour.mp3");
        a_song(&root, "Amber Field/Southern Nights/01 - Tides.mp3");
        if !a_picture(&root.join("Amber Field/Northern Lights/cover.jpg"), "red") {
            eprintln!("no media tool here, no picture could be made");
            return;
        }
        a_picture(&root.join("Amber Field/artist.jpg"), "blue");
        let (state, library) = library(directory.path(), &root).await;

        let report = scan(&state, &library).await;
        assert_eq!(report.pictured, 2, "the album with a cover, and its artist");
        assert!(wears_a_picture(&state, &library, WorkKind::Album, "Northern Lights").await);
        assert!(
            !wears_a_picture(&state, &library, WorkKind::Album, "Southern Nights").await,
            "nothing beside it and nothing inside its song"
        );
        assert!(wears_a_picture(&state, &library, WorkKind::Artist, "Amber Field").await);

        let again = scan(&state, &library).await;
        assert_eq!(
            again.pictured, 0,
            "a picture already made is not made again"
        );
    }

    #[tokio::test]
    async fn an_album_with_nothing_beside_it_wears_the_cover_inside_its_song() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Music");
        let cover = directory.path().join("cover.png");
        if !a_picture(&cover, "green") {
            eprintln!("no media tool here, no picture could be made");
            return;
        }
        let song = root.join("Amber Field/Northern Lights/01 - Quiet Harbour.mp3");
        std::fs::create_dir_all(song.parent().expect("a folder")).expect("folder made");
        let original = fixture("one-second.mp3");
        run(&[
            "-i".as_ref(),
            original.as_os_str(),
            "-i".as_ref(),
            cover.as_os_str(),
            "-map".as_ref(),
            "0".as_ref(),
            "-map".as_ref(),
            "1".as_ref(),
            "-c".as_ref(),
            "copy".as_ref(),
            "-disposition:v".as_ref(),
            "attached_pic".as_ref(),
            song.as_os_str(),
        ]);
        let (state, library) = library(directory.path(), &root).await;

        let report = scan(&state, &library).await;
        assert_eq!(
            report.added, 1,
            "the picture inside is not a video of its own"
        );
        assert_eq!(report.pictured, 1);
        assert!(wears_a_picture(&state, &library, WorkKind::Album, "Northern Lights").await);
    }
}
