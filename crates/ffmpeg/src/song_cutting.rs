//! A song cut short at either end, without its sound being made again.
//!
//! What a person trims from a song is mostly silence a ripper left before the
//! first note or after the last. The sound is carried over as it is, frame by
//! frame, so nothing of it is lost to a second encoding: the ends land on the
//! frame nearest the moment asked for, a few hundredths of a second at most.
//!
//! FLAC alone is encoded again, which loses nothing either since it is
//! lossless: carried over as it is, it kept the length of the whole song in
//! its header, and every player read the old length.
//!
//! Only the sound is written. The tags and the pictures are put back by the
//! crate that reads and writes them, which keeps everything a file carries
//! rather than what this tool happens to know how to carry over.

use std::ffi::OsString;
use std::path::Path;

use melyxar_core::time::Millis;
use tokio::process::Command as TokioCommand;

use crate::process::AskedToStop;
use crate::{FfmpegError, Result};

/// Builds the cut of `file` from `from` to `to`, written to `written`, whose
/// extension says what it is written as.
pub fn cut_arguments(file: &Path, from: Millis, to: Millis, written: &Path) -> Vec<OsString> {
    vec![
        OsString::from("-hide_banner"),
        OsString::from("-loglevel"),
        OsString::from("error"),
        OsString::from("-nostdin"),
        // Before the file, so the tool goes straight there.
        OsString::from("-ss"),
        OsString::from(seconds(from)),
        OsString::from("-i"),
        file.as_os_str().to_os_string(),
        OsString::from("-t"),
        OsString::from(seconds(Millis::new(to.get() - from.get()))),
        OsString::from("-map"),
        OsString::from("0:a"),
        OsString::from("-map_metadata"),
        OsString::from("-1"),
        OsString::from("-c"),
        OsString::from(if is_flac(written) { "flac" } else { "copy" }),
        OsString::from("-y"),
        written.as_os_str().to_os_string(),
    ]
}

/// Whether a file is written as FLAC, by its name.
fn is_flac(file: &Path) -> bool {
    file.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("flac"))
}

/// Writes the cut of a song beside it. Whoever asked puts it in place.
pub async fn cut(
    tools: &crate::ToolPaths,
    file: &Path,
    from: Millis,
    to: Millis,
    written: &Path,
) -> Result<()> {
    let mut builder = TokioCommand::new(&tools.ffmpeg);
    let mut args = cut_arguments(file, from, to, written);
    crate::formats::guard_the_one_input(&mut args, tools.allowed_formats());
    builder.args(args);
    let output = crate::process::output_of(builder, AskedToStop::never()).await?;
    if !output.status.success() {
        return Err(FfmpegError::from_output("ffmpeg", &output));
    }
    Ok(())
}

/// A moment written the way the tool reads one, a full stop before the
/// fraction whatever the machine's own decimal mark is.
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

    #[test]
    fn a_song_is_cut_by_its_two_ends_and_its_sound_carried_over_as_it_is() {
        let arguments = written(&cut_arguments(
            &PathBuf::from("/music/Quiet Harbour/01 - Low Tide.flac"),
            Millis::new(2_350),
            Millis::new(184_000),
            &PathBuf::from("/music/Quiet Harbour/01 - Low Tide.cut.flac"),
        ));
        let at = |option: &str| arguments.iter().position(|value| value == option).expect(option);
        assert_eq!(arguments[at("-ss") + 1], "2.350");
        assert!(at("-ss") < at("-i"), "the tool goes straight to the start");
        assert_eq!(arguments[at("-t") + 1], "181.650", "how long, not where it ends");
        assert_eq!(arguments[at("-c") + 1], "flac", "lossless, so encoded again for a true header");
        assert_eq!(arguments[at("-map") + 1], "0:a", "the sound alone");
        assert_eq!(arguments[at("-map_metadata") + 1], "-1", "the tags are put back apart");
        assert_eq!(arguments.last().map(String::as_str), Some("/music/Quiet Harbour/01 - Low Tide.cut.flac"));
    }

    #[test]
    fn a_lossy_song_is_never_encoded_again() {
        let arguments = written(&cut_arguments(
            &PathBuf::from("/music/Quiet Harbour/02 - Ebb.mp3"),
            Millis::new(0),
            Millis::new(60_000),
            &PathBuf::from("/music/Quiet Harbour/02 - Ebb.cut.MP3"),
        ));
        let at = arguments.iter().position(|value| value == "-c").expect("-c");
        assert_eq!(arguments[at + 1], "copy");
    }

    #[tokio::test]
    async fn a_real_song_comes_out_as_long_as_it_was_cut() {
        let tools = crate::testing::tools();
        let folder = tempfile::tempdir().expect("folder");
        let song = folder.path().join("tone.flac");
        let made = std::process::Command::new(&tools.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "sine=frequency=440:duration=6"])
            .arg(&song)
            .status()
            .expect("the tool runs");
        assert!(made.success());

        let cut_song = folder.path().join("tone.cut.flac");
        cut(&tools, &song, Millis::new(1_000), Millis::new(4_000), &cut_song)
            .await
            .expect("cut");
        let probed = crate::probe::probe(&tools, &cut_song).await.expect("probed");
        let seconds: f64 = probed
            .format
            .duration
            .as_deref()
            .and_then(|duration| duration.parse().ok())
            .expect("a length");
        assert!((seconds - 3.0).abs() < 0.1, "{seconds}");
    }
}
