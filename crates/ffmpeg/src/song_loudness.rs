//! How loud a song is, measured by reading it through.
//!
//! The measure every player levels songs by (EBU R128): how loud the whole
//! song sounds, and the highest its sound truly reaches, which says how much
//! it can be raised before it clips. Only the sound is read and nothing is
//! written; what is measured is kept by whoever asked.

use std::ffi::OsString;
use std::path::Path;

use melyxar_core::media::Loudness;
use tokio::process::Command as TokioCommand;

use crate::process::{AskedToStop, output_of};
use crate::{FfmpegError, Result};

/// Reads a song through for how loud it is.
pub async fn measure(tool: &Path, song: &Path, asked_to_stop: AskedToStop) -> Result<Loudness> {
    let mut builder = TokioCommand::new(tool);
    builder.args(arguments(song));
    let output = output_of(builder, asked_to_stop).await?;
    if !output.status.success() {
        return Err(FfmpegError::from_output("ffmpeg", &output));
    }
    let loudness = summary_of(&String::from_utf8_lossy(&output.stderr));
    if !loudness.is_measured() {
        return Err(FfmpegError::from_output("ffmpeg", &output));
    }
    Ok(loudness)
}

/// What the tool is told: the first sound of the song through the measure,
/// and nothing written anywhere.
pub fn arguments(song: &Path) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["-hide_banner", "-nostats", "-nostdin", "-i"]
        .map(OsString::from)
        .to_vec();
    arguments.push(song.as_os_str().to_os_string());
    arguments.extend(
        [
            "-map",
            "0:a:0",
            "-af",
            "ebur128=peak=true:framelog=quiet",
            "-f",
            "null",
            "-",
        ]
        .map(OsString::from),
    );
    arguments
}

/// Reads the summary the measure writes once the song is through.
fn summary_of(said: &str) -> Loudness {
    let summary = said.rsplit_once("Summary:").map_or("", |(_, after)| after);
    let value = |label: &str| {
        summary.lines().find_map(|line| {
            let rest = line.trim().strip_prefix(label)?;
            rest.split_whitespace().next()?.parse::<f64>().ok()
        })
    };
    Loudness {
        // A silent song reads as the floor of the measure, -70, and is kept
        // as that: measured, and too quiet for anything to be done about it.
        integrated_lufs: value("I:").filter(|lufs| lufs.is_finite()),
        true_peak_dbfs: value("Peak:").filter(|peak| peak.is_finite()),
        range_lu: value("LRA:").filter(|range| range.is_finite()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAID: &str = "\
[Parsed_ebur128_0 @ 0x55] Summary:

  Integrated loudness:
    I:         -14.3 LUFS
    Threshold: -24.6 LUFS

  Loudness range:
    LRA:         6.1 LU
    Threshold:  -34.5 LUFS
    LRA low:    -19.4 LUFS
    LRA high:   -13.3 LUFS

  True peak:
    Peak:        0.4 dBFS
";

    #[test]
    fn the_summary_says_how_loud_how_high_and_how_wide() {
        assert_eq!(
            summary_of(SAID),
            Loudness {
                integrated_lufs: Some(-14.3),
                true_peak_dbfs: Some(0.4),
                range_lu: Some(6.1),
            }
        );
    }

    #[test]
    fn silence_is_measured_at_the_floor_and_no_summary_is_nothing() {
        let silent = SAID
            .replace("-14.3", "-70.0")
            .replace("0.4 dBFS", "-inf dBFS");
        let read = summary_of(&silent);
        assert_eq!(read.integrated_lufs, Some(-70.0));
        assert_eq!(read.true_peak_dbfs, None);
        assert!(!summary_of("no summary here").is_measured());
    }

    #[tokio::test]
    async fn a_real_song_is_measured() {
        let Ok(tools) = crate::ToolPaths::discover(None, None) else {
            eprintln!("no media tool here, nothing was measured");
            return;
        };
        let song =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tags/tests/fixtures/one-second.flac");
        let loudness = measure(&tools.ffmpeg, &song, AskedToStop::never())
            .await
            .expect("measured");
        assert!(loudness.is_measured(), "{loudness:?}");
    }
}
