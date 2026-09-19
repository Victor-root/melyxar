//! Reading a stretch of a film's sound, and nothing else of it.
//!
//! Finding the opening titles of an episode means comparing its sound against
//! another episode's, and comparing sound means having it as plain numbers
//! rather than as whatever an encoder made of it. That is what this does: one
//! stretch of one track, mixed down to a single channel, at the one rate the
//! comparison works in.
//!
//! Two things keep this cheap. The picture is never touched, which is where
//! almost all of a film's weight sits, so the tool reads the file through
//! without decoding a single frame of it. And only the two ends of an episode
//! are ever asked for, because an opening does not hide in the middle: a
//! stretch of the beginning and a stretch of the end, never the whole file.
//!
//! The track is named by the caller rather than left to the tool. Two episodes
//! of one season do not always carry their languages in the same order, and
//! comparing one episode's French against another's English would be comparing
//! two different pieces of sound and finding, correctly, that they share
//! nothing.

use std::ffi::OsString;
use std::path::Path;

use melyxar_core::time::Millis;
use tokio::process::Command as TokioCommand;

use crate::process::AskedToStop;
use crate::{FfmpegError, Result};

/// How many samples of one second come back.
///
/// The same number the crate that compares two stretches of sound reads in,
/// written twice because neither of those crates has any business knowing
/// about the other: one launches tools and the other does arithmetic. What
/// holds the two together is a test where both are in view, in the crate that
/// uses them.
pub const SAMPLES_A_SECOND: u32 = 8_000;

/// Builds the reading of one stretch of one sound track.
pub fn samples_arguments(file: &Path, track: i32, from: Millis, how_long: Millis) -> Vec<OsString> {
    vec![
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-nostdin"),
        // Before the file, so that the tool moves to the right place instead
        // of reading everything up to it and throwing it away. The end of a
        // three quarter hour episode is otherwise the whole episode.
        OsString::from("-ss"),
        OsString::from(seconds(from)),
        OsString::from("-i"),
        file.as_os_str().to_os_string(),
        OsString::from("-t"),
        OsString::from(seconds(how_long)),
        OsString::from("-map"),
        OsString::from(format!("0:{track}")),
        // Nothing but this one track. Without this the tool decodes the
        // picture of a film to write a few seconds of sound.
        OsString::from("-vn"),
        OsString::from("-sn"),
        OsString::from("-dn"),
        OsString::from("-ac"),
        OsString::from("1"),
        OsString::from("-ar"),
        OsString::from(SAMPLES_A_SECOND.to_string()),
        OsString::from("-f"),
        OsString::from("s16le"),
        OsString::from("-"),
    ]
}

/// Reads one stretch of one sound track as plain single channel samples.
///
/// A stretch of four minutes comes back as under four megabytes, which is why
/// it is answered whole rather than handed over as it arrives: what reads it
/// needs all of it at once anyway, to say where in it something sits.
///
/// A stretch the file does not reach comes back empty rather than as a fault.
/// Asking for the last four minutes of a file the analyser said was longer
/// than it is says nothing about the file being unreadable.
pub async fn samples_of(
    tool: &Path,
    file: &Path,
    track: i32,
    from: Millis,
    how_long: Millis,
    asked_to_stop: AskedToStop,
) -> Result<Vec<i16>> {
    let mut builder = TokioCommand::new(tool);
    builder.args(samples_arguments(file, track, from, how_long));
    let output = crate::process::output_of(builder, asked_to_stop).await?;

    if !output.status.success() {
        return Err(FfmpegError::from_output("ffmpeg", &output));
    }

    Ok(output
        .stdout
        .chunks_exact(2)
        .map(|pair| i16::from_le_bytes([pair[0], pair[1]]))
        .collect())
}

/// A moment written the way the tool reads one.
///
/// Whole milliseconds, which is what this project keeps time in, and a full
/// stop for the fraction whatever the machine's own idea of a decimal mark is.
fn seconds(moment: Millis) -> String {
    format!("{}.{:03}", moment.get() / 1_000, moment.get().abs() % 1_000)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn written(arguments: &[OsString]) -> Vec<String> {
        arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect()
    }

    fn after<'a>(arguments: &'a [String], option: &str) -> &'a str {
        let at = arguments
            .iter()
            .position(|value| value == option)
            .unwrap_or_else(|| panic!("{option} is asked for: {arguments:?}"));
        &arguments[at + 1]
    }

    #[test]
    fn the_tool_is_sent_to_the_stretch_instead_of_reading_up_to_it() {
        let arguments = written(&samples_arguments(
            &PathBuf::from("/series/Quiet.Harbour.S01E02.mkv"),
            1,
            Millis::new(2_400_000),
            Millis::new(240_000),
        ));

        let moved = arguments
            .iter()
            .position(|value| value == "-ss")
            .expect("the tool is moved");
        let file = arguments
            .iter()
            .position(|value| value == "-i")
            .expect("a file is read");
        assert!(
            moved < file,
            "before the file, or the tool reads the whole episode to reach the \
             end of it: {arguments:?}"
        );
        assert_eq!(after(&arguments, "-ss"), "2400.000");
        assert_eq!(after(&arguments, "-t"), "240.000");
    }

    #[test]
    fn the_track_the_caller_named_is_the_one_that_is_read() {
        let arguments = written(&samples_arguments(
            &PathBuf::from("/series/Quiet.Harbour.S01E02.mkv"),
            3,
            Millis::ZERO,
            Millis::new(1_000),
        ));
        assert_eq!(after(&arguments, "-map"), "0:3");
    }

    #[test]
    fn nothing_but_the_sound_is_decoded() {
        // The picture is almost the whole weight of a film, and none of it is
        // wanted here.
        let arguments = written(&samples_arguments(
            &PathBuf::from("/series/Quiet.Harbour.S01E02.mkv"),
            1,
            Millis::ZERO,
            Millis::new(1_000),
        ));
        for refused in ["-vn", "-sn", "-dn"] {
            assert!(arguments.iter().any(|value| value == refused), "{refused}");
        }
        assert_eq!(after(&arguments, "-ac"), "1");
        assert_eq!(after(&arguments, "-ar"), "8000");
        assert_eq!(after(&arguments, "-f"), "s16le");
        assert!(arguments.ends_with(&["-".to_string()]));
    }

    #[test]
    fn a_moment_is_written_with_a_full_stop_and_whole_milliseconds() {
        assert_eq!(seconds(Millis::ZERO), "0.000");
        assert_eq!(seconds(Millis::new(1)), "0.001");
        assert_eq!(seconds(Millis::new(90_500)), "90.500");
    }

    /// A file carrying two sound tracks at two different pitches, so that
    /// reading the wrong one is something a test can notice.
    async fn a_film_with_two_tracks(directory: &Path) -> PathBuf {
        let tools = crate::ToolPaths::discover(None, None).expect("the tools are installed here");
        let film = directory.join("Quiet.Harbour.S01E02.mkv");
        let mut making = tokio::process::Command::new(&tools.ffmpeg);
        making.args(["-hide_banner", "-loglevel", "error", "-y"]);
        making.args([
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=160x90:rate=8:duration=8",
        ]);
        making.args(["-f", "lavfi", "-i", "sine=frequency=500:duration=8"]);
        making.args(["-f", "lavfi", "-i", "sine=frequency=1500:duration=8"]);
        making.args(["-map", "0:v", "-map", "1:a", "-map", "2:a"]);
        making.args(["-c:v", "libx264", "-preset", "ultrafast", "-c:a", "aac"]);
        making.arg(&film);
        assert!(
            making.status().await.expect("the tool runs").success(),
            "a film carrying two sound tracks"
        );
        film
    }

    #[tokio::test]
    async fn a_real_stretch_comes_back_as_the_number_of_samples_it_should() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let tools = crate::ToolPaths::discover(None, None).expect("the tools are installed here");
        let film = a_film_with_two_tracks(directory.path()).await;

        let samples = samples_of(
            &tools.ffmpeg,
            &film,
            1,
            Millis::new(2_000),
            Millis::new(3_000),
            AskedToStop::never(),
        )
        .await
        .expect("three seconds of sound");

        let wanted = 3 * SAMPLES_A_SECOND as usize;
        assert!(
            samples.len().abs_diff(wanted) < SAMPLES_A_SECOND as usize / 10,
            "three seconds at eight thousand a second: {}",
            samples.len()
        );
        assert!(
            samples.iter().any(|sample| sample.abs() > 1_000),
            "a tone read back is not silence"
        );
    }

    #[tokio::test]
    async fn two_tracks_of_one_film_read_back_as_two_different_sounds() {
        // The whole reason the caller names the track: two episodes of a
        // season do not always carry their languages in the same order.
        let directory = tempfile::tempdir().expect("temporary directory");
        let tools = crate::ToolPaths::discover(None, None).expect("the tools are installed here");
        let film = a_film_with_two_tracks(directory.path()).await;

        let mut read = Vec::new();
        for track in [1, 2] {
            read.push(
                samples_of(
                    &tools.ffmpeg,
                    &film,
                    track,
                    Millis::new(1_000),
                    Millis::new(2_000),
                    AskedToStop::never(),
                )
                .await
                .unwrap_or_else(|error| panic!("track {track}: {error}")),
            );
        }

        // How often the sound crosses zero says which pitch it is without any
        // arithmetic worth the name: the higher tone crosses far more often.
        let crossings = |samples: &[i16]| {
            samples
                .windows(2)
                .filter(|pair| (pair[0] < 0) != (pair[1] < 0))
                .count()
        };
        assert!(
            crossings(&read[1]) > 2 * crossings(&read[0]),
            "the second track is the higher pitch: {} against {}",
            crossings(&read[0]),
            crossings(&read[1])
        );
    }

    #[tokio::test]
    async fn a_stretch_past_the_end_of_the_film_comes_back_empty() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let tools = crate::ToolPaths::discover(None, None).expect("the tools are installed here");
        let film = a_film_with_two_tracks(directory.path()).await;

        let samples = samples_of(
            &tools.ffmpeg,
            &film,
            1,
            Millis::new(600_000),
            Millis::new(10_000),
            AskedToStop::never(),
        )
        .await
        .expect("asking past the end says nothing, it does not fail");
        assert!(samples.is_empty());
    }

    #[tokio::test]
    async fn a_reading_nobody_wants_any_more_is_given_up_rather_than_finished() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let tools = crate::ToolPaths::discover(None, None).expect("the tools are installed here");
        let film = a_film_with_two_tracks(directory.path()).await;

        let (say, asked) = tokio::sync::watch::channel(true);
        let error = samples_of(
            &tools.ffmpeg,
            &film,
            1,
            Millis::ZERO,
            Millis::new(8_000),
            AskedToStop::when(asked),
        )
        .await
        .expect_err("a reading called off before it began is given up");
        assert!(matches!(error, FfmpegError::GivenUp));
        drop(say);
    }

    #[tokio::test]
    async fn a_file_that_holds_no_such_track_says_so() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let tools = crate::ToolPaths::discover(None, None).expect("the tools are installed here");
        let film = a_film_with_two_tracks(directory.path()).await;

        assert!(
            samples_of(
                &tools.ffmpeg,
                &film,
                9,
                Millis::ZERO,
                Millis::new(1_000),
                AskedToStop::never(),
            )
            .await
            .is_err(),
            "a track that is not there is a fault in what asked, not an empty \
             answer about the film"
        );
    }
}
