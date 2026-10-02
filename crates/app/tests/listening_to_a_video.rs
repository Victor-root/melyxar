//! A personal video with no subtitle, on a real file, from the folder to the
//! track the player offers.
//!
//! The speech tool itself is stood in for by a small script that writes what
//! the real one writes, since the model it needs weighs hundreds of megabytes.
//! Everything around it is the real thing: a video made here with a real
//! sound track, scanned, found waiting, handed to the tool, and what comes
//! back kept as a subtitle marked as generated. The tool's own part is tried
//! by hand against a real model, and its arguments by the tests of its crate.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use melyxar_app::upkeep::{self, UpkeepTask};
use melyxar_app::AppState;
use melyxar_config::{Config, Directories, LibraryConfig, MediaToolsConfig, RootConfig};
use melyxar_core::job::{JobPriority, JobState};
use melyxar_core::library::{Library, LibraryOptions};
use melyxar_core::media::{SubtitleDetails, TrackKind};
use melyxar_core::refresh::RefreshMode;
use melyxar_database::Database;

/// What the stand-in tool says it heard: a report of two words, as the real one writes it.
const REPORT: &str = r#"{"result":{"language":"en"},"transcription":[{"text":" Hello","offsets":{"from":0,"to":1000}},{"text":" there.","offsets":{"from":1000,"to":2000}}]}"#;

/// The subtitle file those two words become.
const HEARD: &str = "1\n00:00:00,000 --> 00:00:02,000\nHello there.\n\n";

/// A stand-in for the speech tool: it finds where it is told to write, and
/// writes what the real one writes there.
fn a_tool_that_hears(folder: &Path) -> PathBuf {
    let tool = folder.join("whisper-cli");
    std::fs::write(
        &tool,
        format!(
            "#!/bin/sh\n\
             while [ $# -gt 0 ]; do\n\
               case \"$1\" in -of) out=\"$2\"; shift;; esac\n\
               shift\n\
             done\n\
             printf '%s' '{}' > \"$out.json\"\n",
            REPORT
        ),
    )
    .expect("the stand-in is written");
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).expect("made runnable");
    tool
}

/// One that always fails, the way a tool that meets a file it cannot read does.
fn a_tool_that_fails(folder: &Path) -> PathBuf {
    let tool = folder.join("whisper-cli");
    std::fs::write(&tool, "#!/bin/sh\necho 'cannot read this' >&2\nexit 1\n").expect("written");
    std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).expect("made runnable");
    tool
}

/// A video with a sound track, and one without.
async fn some_videos(ffmpeg: &Path, folder: &Path) {
    std::fs::create_dir_all(folder).expect("the folder");
    let make = |name: &str, with_sound: bool| {
        let mut making = tokio::process::Command::new(ffmpeg);
        making.args(["-hide_banner", "-loglevel", "error", "-y"]);
        making.args(["-f", "lavfi", "-i", "testsrc2=size=128x72:rate=5:duration=3"]);
        if with_sound {
            making.args(["-f", "lavfi", "-i", "sine=frequency=440:duration=3"]);
        }
        making.args(["-c:v", "libx264", "-preset", "ultrafast"]);
        if with_sound {
            making.args(["-c:a", "aac", "-shortest"]);
        }
        making.arg(folder.join(name));
        making
    };
    for (name, with_sound) in [("Holiday.mkv", true), ("Silent.mkv", false)] {
        assert!(
            make(name, with_sound).status().await.expect("the tool runs").success(),
            "{name} is made"
        );
    }
}

/// A server whose only library holds personal videos and asks to have them
/// listened to.
async fn a_server(directory: &Path, root: PathBuf, tool: PathBuf) -> (AppState, Library) {
    let config = Config {
        directories: Directories {
            data: directory.join("data"),
            cache: directory.join("cache"),
            transcodes: directory.join("cache/transcodes"),
            ..Default::default()
        },
        media_tools: MediaToolsConfig {
            whisper_path: Some(tool),
            ..Default::default()
        },
        libraries: vec![LibraryConfig {
            name: "Home".into(),
            kind: "home_media".into(),
            metadata_language: "en".into(),
            roots: vec![RootConfig { label: "disk".into(), path: root }],
        }],
        ..Config::default()
    };
    melyxar_app::startup::prepare_directories(&config).expect("directories prepared");

    let database = Database::open_in_memory().await.expect("database opens");
    melyxar_app::startup::reconcile_libraries(&database, &config)
        .await
        .expect("libraries reconciled");
    let library = database.library_by_name("Home").await.expect("read").expect("declared");
    database
        .set_library_options(
            library.id,
            LibraryOptions { generate_subtitles: true, ..LibraryOptions::default() },
        )
        .await
        .expect("the library asks to have its videos listened to");
    let library = database.library_by_name("Home").await.expect("read").expect("declared");

    let (tools, capabilities) = melyxar_app::startup::detect_media_tools(&config).await;
    (AppState::new(config, database, tools, capabilities), library)
}

async fn scan(state: &AppState, library: &Library) {
    let (state_of_scan, _) = melyxar_app::scan::start_scan(
        state,
        library.clone(),
        JobPriority::REQUESTED,
        RefreshMode::WhatIsMissing,
    )
    .await
    .expect("the scan starts")
    .wait()
    .await;
    assert_eq!(state_of_scan, JobState::Succeeded, "the scan ran to the end");
}

async fn listen(state: &AppState, library: &Library) -> JobState {
    upkeep::start(state, UpkeepTask::Speech, library.clone(), JobPriority::REQUESTED)
        .await
        .expect("the listening starts")
        .completion
        .await
        .expect("the listening ends")
}

/// The model chosen, as a file of exactly the right size that holds nothing:
/// the stand-in tool never opens it.
async fn a_model_chosen(state: &AppState) {
    let folder = state.config().directories.speech_models();
    std::fs::create_dir_all(&folder).expect("the folder of models");
    let file = std::fs::File::create(folder.join("ggml-small.bin")).expect("created");
    file.set_len(487_601_967).expect("a sparse file of the right size");
    melyxar_app::speech::choose(state, Some("small")).await.expect("chosen");
}

/// The subtitles of every file of the library, by file name.
async fn subtitles_by_file(state: &AppState, library: &Library) -> Vec<(String, Vec<(melyxar_core::media::Track, SubtitleDetails)>)> {
    let database = state.database();
    let mut found = Vec::new();
    for root in &library.roots {
        for source in database.sources_of_root(root.id).await.expect("read") {
            let subtitles = database
                .tracks_of_source(source.id)
                .await
                .expect("read")
                .into_iter()
                .filter_map(|track| match track.kind.clone() {
                    TrackKind::Subtitle(details) => Some((track, details)),
                    _ => None,
                })
                .collect();
            found.push((
                source.relative_path.file_name().expect("a name").to_string_lossy().into_owned(),
                subtitles,
            ));
        }
    }
    found.sort_by(|one, other| one.0.cmp(&other.0));
    found
}

#[tokio::test]
async fn a_video_with_a_voice_and_no_subtitle_gets_one_marked_as_generated() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path().join("media");
    let ffmpeg = melyxar_ffmpeg::ToolPaths::discover(None, None).expect("the tools are here").ffmpeg;
    some_videos(&ffmpeg, &root).await;
    let tool = a_tool_that_hears(directory.path());

    let (state, library) = a_server(directory.path(), root, tool).await;
    scan(&state, &library).await;

    // Nothing waits for a model that has not been chosen: nought, not a queue
    // that nothing will ever empty.
    assert_eq!(
        upkeep::what_is_waiting_for(&state, UpkeepTask::Speech, library.id).await.expect("read"),
        0
    );
    a_model_chosen(&state).await;
    assert_eq!(
        upkeep::what_is_waiting_for(&state, UpkeepTask::Speech, library.id).await.expect("read"),
        1,
        "the video with a voice, and not the one with no sound at all"
    );

    assert_eq!(listen(&state, &library).await, JobState::Succeeded);

    let found = subtitles_by_file(&state, &library).await;
    let holiday = &found.iter().find(|(name, _)| name == "Holiday.mkv").expect("scanned").1;
    assert_eq!(holiday.len(), 1, "one subtitle was written: {found:?}");
    let (track, details) = &holiday[0];
    assert!(details.is_generated, "it says it was not written by a person");
    assert!(details.is_external && !details.is_hearing_impaired);
    assert_eq!(details.codec, "subrip");
    assert_eq!(track.language.as_deref(), Some("eng"), "the language the model heard");
    let file = state
        .config()
        .directories
        .downloaded_subtitles()
        .join(details.downloaded_file.as_deref().expect("kept as a file of the server's own"));
    assert_eq!(std::fs::read_to_string(file).expect("the file is there"), HEARD);

    let silent = &found.iter().find(|(name, _)| name == "Silent.mkv").expect("scanned").1;
    assert!(silent.is_empty(), "a video with no sound has nothing to listen to");

    // Done is done: a second pass finds nothing waiting and adds nothing.
    assert_eq!(
        upkeep::what_is_waiting_for(&state, UpkeepTask::Speech, library.id).await.expect("read"),
        0
    );
    assert_eq!(listen(&state, &library).await, JobState::Succeeded);
    let again = subtitles_by_file(&state, &library).await;
    assert_eq!(
        again.iter().find(|(name, _)| name == "Holiday.mkv").expect("scanned").1.len(),
        1
    );

    // And the scratch folder that held the recording of its sound is gone.
    let left = std::fs::read_dir(state.config().directories.speech_scratch())
        .expect("the folder is there")
        .count();
    assert_eq!(left, 0, "nothing of the recording is left behind");
}

#[tokio::test]
async fn a_tool_that_cannot_listen_leaves_the_video_waiting_and_writes_nothing() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path().join("media");
    let ffmpeg = melyxar_ffmpeg::ToolPaths::discover(None, None).expect("the tools are here").ffmpeg;
    some_videos(&ffmpeg, &root).await;
    let tool = a_tool_that_fails(directory.path());

    let (state, library) = a_server(directory.path(), root, tool).await;
    scan(&state, &library).await;
    a_model_chosen(&state).await;

    assert_eq!(listen(&state, &library).await, JobState::Succeeded, "the pass ends, it does not hang");
    let found = subtitles_by_file(&state, &library).await;
    assert!(found.iter().all(|(_, subtitles)| subtitles.is_empty()), "{found:?}");
    assert_eq!(
        upkeep::what_is_waiting_for(&state, UpkeepTask::Speech, library.id).await.expect("read"),
        1,
        "a reading that could not happen is not an answer about the file"
    );
    let downloaded = std::fs::read_dir(state.config().directories.downloaded_subtitles())
        .expect("the folder is there")
        .count();
    assert_eq!(downloaded, 0, "no file is written for a track that was not");
}
