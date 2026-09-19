//! The whole chain, on files a tool really wrote and really read back.
//!
//! Everything else about this crate is tested on numbers handed straight to
//! it, which proves the arithmetic and nothing else. What actually happens to
//! an episode is that its sound was encoded once by whoever made the file,
//! decoded again here, mixed down from however many channels it had, and
//! resampled down to eight thousand a second. Every one of those steps changes
//! the numbers, and the question this crate exists to answer is whether what
//! is written down survives them.
//!
//! So two episodes are made here: each its own sound, both carrying the same
//! opening, at two distances from their own beginning that are nothing alike
//! and land nowhere near a window boundary. They go through a real encoder,
//! come back through the real reader, and the opening has to be found.

use std::path::{Path, PathBuf};

use melyxar_core::time::Millis;
use melyxar_ffmpeg::process::AskedToStop;
use melyxar_sound::{what_they_have_in_common, Listened};

/// What the made up sound is written at before it is encoded, which is what a
/// real soundtrack is written at.
const MADE_AT: u32 = 48_000;

/// A run of made up sound that moves the way music moves.
///
/// A sound holding still describes nothing on purpose, so a test built on a
/// held note would prove only that. The note also lasts a different time for
/// every seed: two runs that changed note in step would agree about when
/// things happen even while disagreeing about what happens, which is not a
/// thing real sound does and would flatter the comparison.
fn a_tune(seconds: f32, seed: u32) -> Vec<i16> {
    let mut next = seed | 1;
    let mut roll = move || {
        next = next.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        next as f32 / u32::MAX as f32
    };

    let count = (seconds * MADE_AT as f32) as usize;
    let note = MADE_AT as usize * (13 + (seed % 11) as usize) / 100;
    let mut samples = Vec::with_capacity(count);
    let mut pitch = 440.0f32;
    for at in 0..count {
        if at % note == 0 {
            pitch = 400.0 + roll() * 2_000.0;
        }
        let moment = at as f32 / MADE_AT as f32;
        let turns = 2.0 * std::f32::consts::PI * pitch * moment;
        let value = 0.7 * turns.sin() + 0.3 * (2.5 * turns).sin();
        samples.push((value * 0.4 * f32::from(i16::MAX)) as i16);
    }
    samples
}

fn as_bytes(samples: &[i16]) -> Vec<u8> {
    samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect()
}

/// Writes one episode out as a real file, encoded the way a real one is.
async fn an_episode(
    tool: &Path,
    directory: &Path,
    name: &str,
    before: f32,
    opening: &[i16],
    after: f32,
    seed: u32,
) -> PathBuf {
    let mut sound = a_tune(before, seed);
    sound.extend_from_slice(opening);
    sound.extend(a_tune(after, seed.wrapping_add(1_000)));

    let raw = directory.join(format!("{name}.raw"));
    std::fs::write(&raw, as_bytes(&sound)).expect("the made up sound is written down");

    let episode = directory.join(format!("{name}.mka"));
    let mut making = tokio::process::Command::new(tool);
    making.args(["-hide_banner", "-loglevel", "error", "-y"]);
    making.args(["-f", "s16le", "-ar", &MADE_AT.to_string(), "-ac", "1", "-i"]);
    making.arg(&raw);
    making.args(["-c:a", "aac"]);
    making.arg(&episode);
    assert!(
        making.status().await.expect("the tool runs").success(),
        "an episode of made up sound"
    );
    episode
}

fn about(found: Millis, expected: f32) -> bool {
    (found.as_seconds_f64() - f64::from(expected)).abs() <= 0.7
}

#[tokio::test]
async fn the_opening_survives_being_encoded_and_read_back() {
    let tools = melyxar_ffmpeg::ToolPaths::discover(None, None).expect("the tools are here");
    let directory = tempfile::tempdir().expect("temporary directory");

    // Six seconds of opening, held by both episodes, at two distances that
    // are nothing alike: one episode opens almost at once, the other after a
    // reminder of last week.
    let opening = a_tune(6.0, 7);
    let (early, late) = (1.37f32, 11.09f32);
    let first = an_episode(
        &tools.ffmpeg,
        directory.path(),
        "first",
        early,
        &opening,
        14.0,
        11,
    )
    .await;
    let second = an_episode(
        &tools.ffmpeg,
        directory.path(),
        "second",
        late,
        &opening,
        14.0,
        29,
    )
    .await;

    let mut listened = Vec::new();
    for episode in [&first, &second] {
        let samples = melyxar_ffmpeg::sound::samples_of(
            &tools.ffmpeg,
            episode,
            0,
            Millis::ZERO,
            Millis::new(40_000),
            AskedToStop::never(),
        )
        .await
        .expect("the sound of an episode comes back");
        assert!(
            !samples.is_empty(),
            "a file that was just written holds sound"
        );
        listened.push(Listened::of(&samples));
    }

    let found = what_they_have_in_common(&listened[0], &listened[1])
        .expect("two episodes sharing an opening share something");

    assert!(
        about(found.length(), 6.0),
        "the whole of the opening and little else: {:?}",
        found.length()
    );
    assert!(
        about(found.in_the_first().start, early),
        "found where it really sits in the first: {:?}",
        found.in_the_first()
    );
    assert!(
        about(found.in_the_second().start, late),
        "and where it really sits in the second: {:?}",
        found.in_the_second()
    );
}

#[tokio::test]
async fn two_real_episodes_sharing_no_opening_share_nothing() {
    // The answer that has to be right even more than the other one: a season
    // whose episodes open straight into the story must get no skip button at
    // all, rather than one over its first scene.
    let tools = melyxar_ffmpeg::ToolPaths::discover(None, None).expect("the tools are here");
    let directory = tempfile::tempdir().expect("temporary directory");

    let first = an_episode(&tools.ffmpeg, directory.path(), "first", 20.0, &[], 0.0, 41).await;
    let second = an_episode(
        &tools.ffmpeg,
        directory.path(),
        "second",
        20.0,
        &[],
        0.0,
        83,
    )
    .await;

    let mut listened = Vec::new();
    for episode in [&first, &second] {
        let samples = melyxar_ffmpeg::sound::samples_of(
            &tools.ffmpeg,
            episode,
            0,
            Millis::ZERO,
            Millis::new(40_000),
            AskedToStop::never(),
        )
        .await
        .expect("the sound of an episode comes back");
        listened.push(Listened::of(&samples));
    }

    let shared = what_they_have_in_common(&listened[0], &listened[1])
        .map_or(Millis::ZERO, |found| found.length());
    assert!(
        shared.as_seconds_f64() < 2.0,
        "two episodes that share nothing agree here and there by chance, never \
         for long: {shared:?}"
    );
}
