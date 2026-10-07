//! What a song is read through for: how loud it is, and how its sound is
//! spread.
//!
//! Both are asked of one pass of the tool. The measure every player levels
//! songs by (EBU R128) says how loud the whole song sounds and the highest its
//! sound truly reaches, which says how much it can be raised before it clips;
//! the second output is the sound itself as plain single channel samples, for
//! whoever writes its spectrum down. A song is decoded once for both, and for
//! only the one still wanted when the other is already known. Nothing is
//! written anywhere, and nothing of the sound is kept here: it is handed over
//! as it comes, since an hour of it is a third of a gigabyte and a library of
//! music holds audiobooks of dozens of hours.

use std::ffi::OsString;
use std::path::Path;

use melyxar_core::media::Loudness;
use tokio::process::Command as TokioCommand;

use crate::process::{AskedToStop, output_handed_over};
use crate::{FfmpegError, Result};

/// How many samples of one second come back for the spectrum.
///
/// The same number the crate that writes the spectrum down reads in, written
/// twice because neither of those crates has any business knowing about the
/// other: one launches tools and the other does arithmetic. A test where both
/// are in view, in the crate that uses them, holds the two together.
pub const SPECTRUM_SAMPLES_A_SECOND: u32 = 16_000;

/// What is asked of a song.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Wanted {
    pub loudness: bool,
    pub spectrum: bool,
}

/// Reads a song through once for what is wanted of it, and answers its
/// loudness when it was asked for.
///
/// The samples of its sound, when they are wanted, are given to `on_samples`
/// as they come, in pieces of no particular length, on a thread of its own:
/// it may work as hard as it likes on them.
pub async fn analyse(
    tools: &crate::ToolPaths,
    song: &Path,
    wanted: Wanted,
    asked_to_stop: AskedToStop,
    mut on_samples: impl FnMut(&[i16]) + Send + 'static,
) -> Result<Option<Loudness>> {
    let mut builder = TokioCommand::new(&tools.ffmpeg);
    let mut args = arguments(song, wanted);
    crate::formats::guard_the_one_input(&mut args, tools.allowed_formats());
    builder.args(args);
    // A sample is two bytes, and the pipe may cut between them.
    let mut cut_in_two = None;
    let output = output_handed_over(builder, asked_to_stop, move |bytes| {
        on_samples(&samples_of(&mut cut_in_two, bytes));
    })
    .await?;
    if !output.status.success() {
        return Err(FfmpegError::from_output("ffmpeg", &output));
    }
    if !wanted.loudness {
        return Ok(None);
    }
    let loudness = summary_of(&String::from_utf8_lossy(&output.stderr));
    if !loudness.is_measured() {
        return Err(FfmpegError::from_output("ffmpeg", &output));
    }
    Ok(Some(loudness))
}

/// The samples these bytes hold, the first one finished with the byte the
/// last piece ended on, and the last byte kept when it begins one.
fn samples_of(cut_in_two: &mut Option<u8>, mut bytes: &[u8]) -> Vec<i16> {
    let mut samples = Vec::with_capacity(bytes.len() / 2 + 1);
    if let Some(first) = cut_in_two.take() {
        match bytes.split_first() {
            Some((second, rest)) => {
                samples.push(i16::from_le_bytes([first, *second]));
                bytes = rest;
            }
            None => *cut_in_two = Some(first),
        }
    }
    let mut pairs = bytes.chunks_exact(2);
    samples.extend(pairs.by_ref().map(|pair| i16::from_le_bytes([pair[0], pair[1]])));
    if let Some(last) = pairs.remainder().first() {
        *cut_in_two = Some(*last);
    }
    samples
}

/// What the tool is told: the first sound of the song through the measure,
/// and through a single channel at the rate of the spectrum, whichever is
/// wanted, and nothing written anywhere.
pub fn arguments(song: &Path, wanted: Wanted) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = ["-hide_banner", "-nostats", "-nostdin", "-i"]
        .map(OsString::from)
        .to_vec();
    arguments.push(song.as_os_str().to_os_string());
    if wanted.loudness {
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
    }
    if wanted.spectrum {
        arguments.extend(
            [
                "-map",
                "0:a:0",
                "-vn",
                "-sn",
                "-dn",
                "-ac",
                "1",
                "-ar",
                &SPECTRUM_SAMPLES_A_SECOND.to_string(),
                "-f",
                "s16le",
                "pipe:1",
            ]
            .map(OsString::from),
        );
    }
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
    use std::sync::{Arc, Mutex};

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

    const BOTH: Wanted = Wanted {
        loudness: true,
        spectrum: true,
    };

    fn the_words(arguments: &[OsString]) -> Vec<String> {
        arguments
            .iter()
            .map(|word| word.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn what_is_wanted_is_what_the_tool_is_asked_for() {
        let song = Path::new("/music/a.flac");
        let both = the_words(&arguments(song, BOTH));
        assert!(both.contains(&"ebur128=peak=true:framelog=quiet".to_string()));
        assert!(both.contains(&"s16le".to_string()));
        assert_eq!(both.iter().filter(|word| *word == "-i").count(), 1);

        let only_loudness = the_words(&arguments(
            song,
            Wanted {
                loudness: true,
                spectrum: false,
            },
        ));
        assert!(!only_loudness.contains(&"s16le".to_string()));

        let only_spectrum = the_words(&arguments(
            song,
            Wanted {
                loudness: false,
                spectrum: true,
            },
        ));
        assert!(!only_spectrum.iter().any(|word| word.starts_with("ebur128")));
        assert!(only_spectrum.contains(&SPECTRUM_SAMPLES_A_SECOND.to_string()));
    }

    #[tokio::test]
    async fn a_real_song_is_read_once_for_both() {
        let Ok(tools) = crate::ToolPaths::discover(None, None) else {
            eprintln!("no media tool here, nothing was measured");
            return;
        };
        let song =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tags/tests/fixtures/one-second.flac");
        // Counted, and where: the test runs on the one thread of its runtime,
        // so sound heard on that thread would be sound heard where requests
        // are served.
        let heard = Arc::new(Mutex::new((0usize, false)));
        let hearing = Arc::clone(&heard);
        let serving = std::thread::current().id();
        let loudness = analyse(&tools, &song, BOTH, AskedToStop::never(), move |samples| {
            let mut heard = hearing.lock().expect("counted");
            heard.0 += samples.len();
            heard.1 |= std::thread::current().id() == serving;
        })
        .await
        .expect("read")
        .expect("measured");
        assert!(loudness.is_measured(), "{loudness:?}");
        let (heard, where_requests_are_served) = *heard.lock().expect("counted");
        assert!(!where_requests_are_served, "the sound is heard on a thread of its own");
        let seconds = heard as f64 / f64::from(SPECTRUM_SAMPLES_A_SECOND);
        assert!(
            (0.9..1.1).contains(&seconds),
            "{seconds} seconds of samples"
        );
    }

    #[tokio::test]
    async fn a_song_whose_loudness_is_known_is_only_read_for_its_sound() {
        let Ok(tools) = crate::ToolPaths::discover(None, None) else {
            eprintln!("no media tool here, nothing was read");
            return;
        };
        let song =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../tags/tests/fixtures/one-second.flac");
        let wanted = Wanted {
            loudness: false,
            spectrum: true,
        };
        let heard = Arc::new(Mutex::new(0usize));
        let hearing = Arc::clone(&heard);
        let loudness = analyse(&tools, &song, wanted, AskedToStop::never(), move |samples| {
            *hearing.lock().expect("counted") += samples.len();
        })
        .await
        .expect("read");
        assert_eq!(loudness, None);
        assert!(*heard.lock().expect("counted") > 0);
    }

    #[test]
    fn a_sample_cut_in_two_by_the_pipe_is_put_back_together() {
        let sound: Vec<i16> = vec![1, -2, 300, -32768, 32767, 12345];
        let bytes: Vec<u8> = sound.iter().flat_map(|sample| sample.to_le_bytes()).collect();
        for cut in 1..bytes.len() {
            let mut cut_in_two = None;
            let mut heard = samples_of(&mut cut_in_two, &bytes[..cut]);
            heard.extend(samples_of(&mut cut_in_two, &bytes[cut..]));
            assert_eq!(heard, sound, "cut after byte {cut}");
            assert_eq!(cut_in_two, None);
        }
        let mut cut_in_two = None;
        let mut heard = Vec::new();
        for byte in &bytes {
            heard.extend(samples_of(&mut cut_in_two, std::slice::from_ref(byte)));
        }
        assert_eq!(heard, sound, "one byte at a time");
    }
}
