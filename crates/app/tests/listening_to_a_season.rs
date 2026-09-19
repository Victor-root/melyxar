//! A whole season, on real files, from the folder to the skip button.
//!
//! Everything else about this is tested on its own: the arithmetic on numbers
//! handed straight to it, the questions against a real database, the reading
//! of a stretch of sound against real files. What none of that proves is the
//! thing somebody actually gets, which is that a folder of episodes dropped on
//! a disk ends up with a button over its opening titles and nothing anywhere
//! else.
//!
//! So a season is built here the way one really arrives: three episodes, each
//! its own sound, all three carrying the same opening at three different
//! distances from their own beginning, and the same closing titles at the end.
//! They are scanned, listened to, and asked what they came away with.

use std::path::{Path, PathBuf};

use melyxar_app::upkeep::{self, UpkeepTask};
use melyxar_app::AppState;
use melyxar_config::{Config, Directories, LibraryConfig, RootConfig};
use melyxar_core::job::{JobPriority, JobState};
use melyxar_core::library::Library;
use melyxar_core::refresh::RefreshMode;
use melyxar_core::segments::{MediaSegment, SegmentKind, SegmentOrigin};
use melyxar_core::time::Millis;
use melyxar_database::Database;

/// What the made up sound is written at before it is encoded.
const MADE_AT: u32 = 48_000;

/// How long every made up episode runs for.
const AN_EPISODE: f32 = 40.0;

/// How long the opening and the closing titles run for.
///
/// Comfortably over the ten seconds below which nothing is offered a button:
/// a stretch shorter than that is as likely to be two episodes happening to
/// agree as anything anybody wrote.
const THE_TITLES: f32 = 12.0;

/// A run of made up sound that moves the way music moves.
///
/// The note lasts a different time for every seed, on purpose. Two runs that
/// changed note in step would agree about when things happen even while
/// disagreeing about what happens, which is not a thing real sound does and
/// would flatter the comparison into finding an opening where there is none.
fn a_tune(seconds: f32, seed: u32) -> Vec<i16> {
    let mut next = seed | 1;
    let count = (seconds * MADE_AT as f32) as usize;
    let note = MADE_AT as usize * (13 + (seed % 11) as usize) / 100;
    let mut samples = Vec::with_capacity(count);
    let mut pitch = 440.0f32;
    for at in 0..count {
        if at % note == 0 {
            next = next.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            pitch = 400.0 + (next as f32 / u32::MAX as f32) * 2_000.0;
        }
        let moment = at as f32 / MADE_AT as f32;
        let turns = 2.0 * std::f32::consts::PI * pitch * moment;
        let value = 0.7 * turns.sin() + 0.3 * (2.5 * turns).sin();
        samples.push((value * 0.4 * f32::from(i16::MAX)) as i16);
    }
    samples
}

/// Writes one episode out as a real file: a picture, and the sound given.
async fn an_episode(tool: &Path, folder: &Path, number: u32, sound: &[i16]) {
    let raw = folder.join(format!("{number}.raw"));
    std::fs::write(
        &raw,
        sound
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect::<Vec<u8>>(),
    )
    .expect("the made up sound is written down");

    // The picture runs exactly as long as the sound it is given. Written any
    // other way the episode lies about its own length, `-shortest` cuts the
    // sound to the picture, and an opening put four minutes in is simply not
    // in the file.
    let seconds = sound.len() as f32 / MADE_AT as f32;
    let episode = folder.join(format!("Distant.Signal.S01E{number:02}.mkv"));
    let mut making = tokio::process::Command::new(tool);
    making.args(["-hide_banner", "-loglevel", "error", "-y"]);
    making.args([
        "-f",
        "lavfi",
        "-i",
        &format!("testsrc2=size=128x72:rate=5:duration={seconds}"),
    ]);
    making.args(["-f", "s16le", "-ar", &MADE_AT.to_string(), "-ac", "1", "-i"]);
    making.arg(&raw);
    making.args([
        "-map",
        "0:v",
        "-map",
        "1:a",
        "-c:v",
        "libx264",
        "-preset",
        "ultrafast",
        "-c:a",
        "aac",
        "-shortest",
    ]);
    making.arg(&episode);
    assert!(
        making.status().await.expect("the tool runs").success(),
        "an episode of made up sound"
    );
    std::fs::remove_file(&raw).expect("the raw sound is not part of the library");
}

/// A season folder holding three episodes, each opening after a reminder of
/// its own length.
///
/// `sharing` says whether the three really hold the same opening and closing
/// titles. When they do not, each one is entirely its own, which is the season
/// that must come away with no button at all.
async fn a_season_on_disk(tool: &Path, root: &Path, sharing: bool) {
    let folder = root.join("Distant Signal").join("Season 01");
    std::fs::create_dir_all(&folder).expect("the season folder");

    let opening = a_tune(THE_TITLES, 7);
    let closing = a_tune(THE_TITLES, 13);
    for (number, before) in [(1u32, 1.4f32), (2, 4.9), (3, 3.1)] {
        let after = AN_EPISODE - before - 2.0 * THE_TITLES;
        let mut sound = a_tune(before, 100 + number);
        if sharing {
            sound.extend_from_slice(&opening);
        } else {
            sound.extend(a_tune(THE_TITLES, 300 + number));
        }
        sound.extend(a_tune(after, 200 + number));
        if sharing {
            sound.extend_from_slice(&closing);
        } else {
            sound.extend(a_tune(THE_TITLES, 400 + number));
        }
        an_episode(tool, &folder, number, &sound).await;
    }
}

/// How long an episode that opens late runs for.
///
/// Long enough that a quarter of it reaches past the four minutes the first
/// listening reads, which is what buys it a second, longer look. Shorter than
/// sixteen minutes and there is nothing deeper to look at, because the first
/// listening already read half of it.
const A_LONG_EPISODE: f32 = 20.0 * 60.0;

/// How far into a long episode its titles sit.
///
/// Past the four minutes read the first time, and inside the five that a
/// quarter of twenty minutes comes to. This is the first episode of a season
/// on a real disk: a cold scene setting the year up, and only then the titles.
const LATE_TITLES_AT: f32 = 4.0 * 60.0 + 18.0;

/// And how long they run for once they arrive.
const LATE_TITLES: f32 = 20.0;

/// A season of three long episodes whose titles all sit past four minutes.
///
/// Each one opens on a cold scene of its own length, so the titles sit
/// somewhere different in each of the three and cannot be found by their
/// position.
async fn a_season_that_opens_late_on_disk(tool: &Path, root: &Path) {
    let folder = root.join("Distant Signal").join("Season 01");
    std::fs::create_dir_all(&folder).expect("the season folder");

    let opening = a_tune(LATE_TITLES, 21);
    for (number, before) in [
        (1u32, LATE_TITLES_AT),
        (2, LATE_TITLES_AT + 9.0),
        (3, LATE_TITLES_AT + 4.0),
    ] {
        let mut sound = a_tune(before, 500 + number);
        sound.extend_from_slice(&opening);
        sound.extend(a_tune(A_LONG_EPISODE - before - LATE_TITLES, 600 + number));
        an_episode(tool, &folder, number, &sound).await;
    }
}

/// A server whose only library holds series and points at this folder.
async fn a_server(directory: &Path, root: PathBuf) -> (AppState, Library) {
    let config = Config {
        directories: Directories {
            data: directory.join("data"),
            cache: directory.join("cache"),
            transcodes: directory.join("cache/transcodes"),
        },
        libraries: vec![LibraryConfig {
            name: "Series".into(),
            kind: "series".into(),
            metadata_language: "fr".into(),
            roots: vec![RootConfig {
                label: "disk".into(),
                path: root,
            }],
        }],
        ..Config::default()
    };
    melyxar_app::startup::prepare_directories(&config).expect("directories prepared");

    let database = Database::open_in_memory().await.expect("database opens");
    melyxar_app::startup::reconcile_libraries(&database, &config)
        .await
        .expect("libraries reconciled");
    let library = database
        .library_by_name("Series")
        .await
        .expect("read")
        .expect("the library was declared");

    let (tools, capabilities) = melyxar_app::startup::detect_media_tools(&config).await;
    (
        AppState::new(config, database, tools, capabilities),
        library,
    )
}

/// Scans the library, then listens to every season of it.
async fn scan_then_listen(state: &AppState, library: &Library) {
    let (scanned, _) = melyxar_app::scan::start_scan(
        state,
        library.clone(),
        JobPriority::REQUESTED,
        RefreshMode::WhatIsMissing,
    )
    .await
    .expect("the scan starts")
    .wait()
    .await;
    assert_eq!(scanned, JobState::Succeeded, "the scan ran to the end");

    let listened = upkeep::start(
        state,
        UpkeepTask::Openings,
        library.clone(),
        JobPriority::REQUESTED,
    )
    .await
    .expect("the listening starts")
    .completion
    .await
    .expect("the listening task ends");
    assert_eq!(
        listened,
        JobState::Succeeded,
        "the listening ran to the end"
    );
}

/// What every file of the library came away with, in the order they were
/// scanned.
async fn what_each_file_came_away_with(state: &AppState) -> Vec<(String, Vec<MediaSegment>)> {
    let database = state.database();
    let library = database
        .library_by_name("Series")
        .await
        .expect("read")
        .expect("the library is still there");

    let mut found = Vec::new();
    for root in &library.roots {
        for source in database.sources_of_root(root.id).await.expect("read") {
            found.push((
                source
                    .relative_path
                    .file_name()
                    .expect("a file has a name")
                    .to_string_lossy()
                    .into_owned(),
                database.segments_of_source(source.id).await.expect("read"),
            ));
        }
    }
    found.sort_by(|one, other| one.0.cmp(&other.0));
    found
}

fn about(found: Millis, expected: f32) -> bool {
    (found.as_seconds_f64() - f64::from(expected)).abs() <= 1.0
}

#[tokio::test]
async fn a_season_whose_episodes_share_an_opening_gets_a_button_over_it() {
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path().join("media");
    let tools = melyxar_ffmpeg::ToolPaths::discover(None, None).expect("the tools are here");
    a_season_on_disk(&tools.ffmpeg, &root, true).await;

    let (state, library) = a_server(directory.path(), root).await;
    scan_then_listen(&state, &library).await;

    let found = what_each_file_came_away_with(&state).await;
    assert_eq!(found.len(), 3, "three episodes were scanned: {found:?}");

    // The reminder of last week is a different length in each of the three, so
    // the opening sits somewhere different in each of the three.
    for (file, opens_at) in found.iter().zip([1.4f32, 4.9, 3.1]) {
        let opening = file
            .1
            .iter()
            .find(|segment| segment.kind == SegmentKind::Intro)
            .unwrap_or_else(|| panic!("{} has an opening: {:?}", file.0, file.1));
        assert_eq!(opening.origin, SegmentOrigin::Detected);
        assert!(
            about(opening.start, opens_at),
            "{} opens at {opens_at} s: {opening:?}",
            file.0
        );
        assert!(
            about(opening.end.saturating_sub(opening.start), THE_TITLES),
            "{} opens for {THE_TITLES} s: {opening:?}",
            file.0
        );

        let closing = file
            .1
            .iter()
            .find(|segment| segment.kind == SegmentKind::Outro)
            .unwrap_or_else(|| panic!("{} has closing titles: {:?}", file.0, file.1));
        assert!(
            about(closing.start, AN_EPISODE - THE_TITLES),
            "{} closes at the end: {closing:?}",
            file.0
        );
    }

    assert_eq!(
        state
            .database()
            .count_seasons_to_listen_to(library.id)
            .await
            .expect("read"),
        0,
        "and the season is settled rather than read again every night"
    );
}

#[tokio::test]
async fn a_season_already_settled_is_read_again_when_it_is_asked_for_by_name() {
    // What the terminal command does, whole: a season the upkeep has already
    // settled would be passed over for ever, so being asked for by name has
    // to undo that answer before looking for a new one. This is how a rule
    // about what an opening is gets tried on one series in a minute rather
    // than on a whole collection in an evening.
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path().join("media");
    let tools = melyxar_ffmpeg::ToolPaths::discover(None, None).expect("the tools are here");
    a_season_on_disk(&tools.ffmpeg, &root, true).await;

    let (state, library) = a_server(directory.path(), root).await;
    scan_then_listen(&state, &library).await;
    assert_eq!(
        state
            .database()
            .count_seasons_to_listen_to(library.id)
            .await
            .expect("read"),
        0,
        "the season is settled, which is what would stop it being read again"
    );

    // Part of the name, in the wrong case, the way somebody types it.
    let seasons = state
        .database()
        .seasons_of_series(Some("DISTANT"), None)
        .await
        .expect("read");
    assert_eq!(seasons.len(), 1, "the one season of the one series");

    let (ran, went) =
        melyxar_app::openings::start_listening_again(&state, Some("distant"), seasons)
            .await
            .expect("the listening starts")
            .wait()
            .await;
    assert_eq!(ran, JobState::Succeeded, "the listening ran to the end");
    assert_eq!(went.len(), 1);
    assert_eq!(went[0].episodes, 3);
    assert_eq!(
        (went[0].with_an_opening, went[0].with_a_closing),
        (3, 3),
        "reading it again finds what reading it the first time found"
    );

    for (file, segments) in what_each_file_came_away_with(&state).await {
        assert_eq!(
            segments
                .iter()
                .map(|segment| segment.kind)
                .collect::<Vec<_>>(),
            vec![SegmentKind::Intro, SegmentKind::Outro],
            "{file} kept exactly one of each rather than two copies of both"
        );
    }
}

#[tokio::test]
async fn a_season_whose_episodes_share_nothing_gets_no_button_at_all() {
    // The answer that matters more than the other one. A season that opens
    // straight into the story must come away with nothing, rather than with a
    // button over its first scene.
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path().join("media");
    let tools = melyxar_ffmpeg::ToolPaths::discover(None, None).expect("the tools are here");
    a_season_on_disk(&tools.ffmpeg, &root, false).await;

    let (state, library) = a_server(directory.path(), root).await;
    scan_then_listen(&state, &library).await;

    let found = what_each_file_came_away_with(&state).await;
    assert_eq!(found.len(), 3);
    for (file, segments) in &found {
        assert!(segments.is_empty(), "{file} came away with {segments:?}");
    }

    assert_eq!(
        state
            .database()
            .count_seasons_to_listen_to(library.id)
            .await
            .expect("read"),
        0,
        "nothing found is an answer, and it settles the season"
    );
}

#[tokio::test]
async fn a_season_whose_titles_sit_past_the_first_four_minutes_is_listened_to_further_in() {
    // The defect this exists for: some thirty files of a real collection came
    // away with an opening that stopped dead at three minutes fifty nine,
    // which is not where any of those openings ended but exactly where the
    // listening stopped. They were mostly first episodes of a season, which
    // open on a long cold scene before their titles. Four minutes is right
    // for almost every episode ever made and wrong for those, so the ones
    // that come away with nothing, or with an opening hard against that edge,
    // are listened to a second time and further in.
    let directory = tempfile::tempdir().expect("temporary directory");
    let root = directory.path().join("media");
    let tools = melyxar_ffmpeg::ToolPaths::discover(None, None).expect("the tools are here");
    a_season_that_opens_late_on_disk(&tools.ffmpeg, &root).await;

    let (state, library) = a_server(directory.path(), root).await;
    scan_then_listen(&state, &library).await;

    let found = what_each_file_came_away_with(&state).await;
    assert_eq!(found.len(), 3, "three episodes were scanned: {found:?}");

    for (file, opens_at) in
        found
            .iter()
            .zip([LATE_TITLES_AT, LATE_TITLES_AT + 9.0, LATE_TITLES_AT + 4.0])
    {
        let opening = file
            .1
            .iter()
            .find(|segment| segment.kind == SegmentKind::Intro)
            .unwrap_or_else(|| {
                panic!(
                    "{} opens at {opens_at} s, which only a second look reaches: {:?}",
                    file.0, file.1
                )
            });
        assert!(
            opening.start.as_seconds_f64() > 4.0 * 60.0,
            "{} really does open past the first four minutes: {opening:?}",
            file.0
        );
        assert!(
            (opening.start.as_seconds_f64() - f64::from(opens_at)).abs() < 2.0,
            "{} opens where it really opens: {opening:?}",
            file.0
        );
        assert!(
            (opening.end.saturating_sub(opening.start).as_seconds_f64() - f64::from(LATE_TITLES))
                .abs()
                < 3.0,
            "{} opens for as long as it really opens: {opening:?}",
            file.0
        );
    }
}
