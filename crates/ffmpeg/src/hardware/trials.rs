//! The trials a card is put through: the samples it is shown and the runs of
//! the tool that ask it something. Nothing here knows one maker from another:
//! what a card is asked is decided by its path, through the card.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command as TokioCommand;

use super::Card;

/// How long one trial is given before the card is written off.
///
/// Generous, because this runs once at start-up on a machine that may be busy,
/// and a driver that takes a moment to wake is not a driver that is broken. A
/// driver that has locked up would otherwise hold the whole server down.
const TRIAL_PATIENCE: Duration = Duration::from_secs(20);

/// How much of what the tool printed is kept.
///
/// Enough for the sentence that names the fault, short enough that a report
/// stays readable.
const ENOUGH_TO_READ: usize = 400;

/// Size the trial works at. Small enough to take no time, large enough that a
/// driver does not refuse it for being absurd.
pub(crate) const TRIAL_HEIGHT: i32 = 180;

/// The picture every trial is run on, made on the spot.
const A_GENERATED_PICTURE: &str = "testsrc2=size=640x360:rate=25:duration=0.4";

/// The size of that picture.
pub(crate) const A_GENERATED_PICTURE_SIZE: (i32, i32) = (640, 360);

/// The screen a wide gamut sample says it was graded on.
///
/// The conversion filter does not ask for a picture labelled wide gamut: it
/// asks for the numbers describing the screen the film was graded on, because
/// those are what it converts from. No filter can add them to a picture made on
/// the spot, so a sample is encoded once with them written in, which is the
/// shape a real wide gamut film arrives in. The numbers themselves are the
/// ordinary ones for a reference screen; nothing here depends on their values.
const A_REFERENCE_SCREEN: &str = "master-display=G(8500,39850)B(6550,2300)R(35400,14600)\
     WP(15635,16450)L(40000000,50):max-cll=1000,400:log-level=none";

/// The encoder that writes those numbers into a sample.
const WRITES_THE_SCREEN: &str = "libx265";

/// The codec that sample comes out in.
///
/// It doubles as the witness for reading that codec, and it is the better one:
/// it carries ten bits to a channel, which is what every film worth a card is,
/// and reading eight proves nothing about reading ten.
pub(super) const WIDE_GAMUT_CODEC: &str = "hevc";

/// The software encoder that writes the witness of each codec, and the speed
/// it is asked to write it at.
///
/// A file written by one of these has the shape a real film arrives in. One
/// written by a card need not: Nvidia's H.264 does not say how many pictures a
/// reader has to keep, the tool then provides for more than Nvidia's reader
/// accepts, and a card that reads every real film was found unable to read its
/// own. A card that cannot write a codec also proves it reads it this way.
const SOFTWARE_WITNESSES: &[(&str, &str, &str)] = &[
    ("h264", "libx264", "ultrafast"),
    ("hevc", "libx265", "ultrafast"),
    ("av1", "libsvtav1", "12"),
];

/// What a trial reads.
pub(super) enum TrialInput<'a> {
    /// A picture made on the spot, which costs nothing and suits every trial
    /// that only asks whether the driver accepts the work.
    Generated,
    /// A file written for the purpose, for the one trial that needs a picture
    /// carrying more than pixels.
    File(&'a Path),
}

/// A wide gamut sample on disk, removed when the trial is done with it.
///
/// Tied to its own removal rather than deleted by hand: the trial can fail at
/// several points, and a file left in a temporary folder every time a server
/// starts is a file left there for ever.
pub(super) struct Sample(PathBuf);

impl Sample {
    pub(super) fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Sample {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Writes a fraction of a second of wide gamut picture carrying the numbers of
/// the screen it says it was graded on.
///
/// The conversion filter asks for those numbers rather than for a label, and
/// no filter can add them to a picture made on the spot. Encoding a sample is
/// the only way to ask the card the question a real film will ask it.
pub(super) async fn wide_gamut_sample(ffmpeg: &Path) -> std::result::Result<Sample, String> {
    let sample =
        Sample(std::env::temp_dir().join(format!("melyxar-card-trial-{}.mp4", std::process::id())));

    let arguments = [
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-y",
        "-f",
        "lavfi",
        "-i",
        A_GENERATED_PICTURE,
        "-an",
        "-c:v",
        WRITES_THE_SCREEN,
        "-preset",
        "ultrafast",
        "-pix_fmt",
        "yuv420p10le",
        "-color_primaries",
        "bt2020",
        "-color_trc",
        "smpte2084",
        "-colorspace",
        "bt2020nc",
        "-x265-params",
        A_REFERENCE_SCREEN,
        &sample.path().display().to_string(),
    ]
    .map(str::to_string);

    match run_briefly(ffmpeg, arguments).await {
        Ok((true, _)) => Ok(sample),
        Ok((false, said)) => Err(format!(
            "a wide gamut sample could not be made, so the card was never asked whether it \
             converts colour: {said}"
        )),
        Err(said) => Err(said),
    }
}

/// Runs the tool once and says whether it was happy and what it printed.
///
/// Every trial here is a fraction of a second of work, so the only thing worth
/// guarding against is a driver that has locked up: the run is dropped when its
/// time is up, and the process goes with it.
pub(super) async fn run_briefly(
    ffmpeg: &Path,
    arguments: impl IntoIterator<Item = String>,
) -> std::result::Result<(bool, String), String> {
    let spawned = TokioCommand::new(ffmpeg)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        // A driver that has locked up must not hold the server down: this is
        // what makes dropping the run put an end to the process rather than
        // orphan it.
        .kill_on_drop(true)
        .spawn();

    let Ok(child) = spawned else {
        return Err("the media tool could not be started".to_string());
    };

    match tokio::time::timeout(TRIAL_PATIENCE, child.wait_with_output()).await {
        Ok(Ok(output)) => Ok((
            output.status.success(),
            shortened(&String::from_utf8_lossy(&output.stderr)),
        )),
        Ok(Err(error)) => Err(error.to_string()),
        Err(_) => Err(format!(
            "the card did not answer within {} seconds",
            TRIAL_PATIENCE.as_secs()
        )),
    }
}

/// What writes the witness of one codec.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum WitnessWriter {
    /// A software encoder of the build, at a given speed.
    Software { encoder: String, speed: String },
    /// The card's own encoder.
    Card(String),
}

/// Who writes the witness of one codec: a software encoder of the build where
/// it carries one, otherwise the card, otherwise nobody.
pub(super) fn witness_writer(
    codec: &str,
    card: &Card,
    built_with: &BTreeSet<String>,
) -> Option<WitnessWriter> {
    SOFTWARE_WITNESSES
        .iter()
        .find(|(witnessed, encoder, _)| *witnessed == codec && built_with.contains(*encoder))
        .map(|(_, encoder, speed)| WitnessWriter::Software {
            encoder: (*encoder).to_string(),
            speed: (*speed).to_string(),
        })
        .or_else(|| {
            card.encoder_for(codec)
                .map(|encoder| WitnessWriter::Card(encoder.to_string()))
        })
}

/// Writes a fraction of a second of film in one codec.
pub(super) async fn sample_in(
    ffmpeg: &Path,
    card: &Card,
    writer: &WitnessWriter,
) -> std::result::Result<Sample, String> {
    let (opening, encoding) = match writer {
        WitnessWriter::Software { encoder, speed } => (
            Vec::new(),
            vec![
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "-c:v".to_string(),
                encoder.clone(),
                "-preset".to_string(),
                speed.clone(),
            ],
        ),
        WitnessWriter::Card(encoder) => (
            card.opening_arguments(false, false),
            vec![
                "-vf".to_string(),
                "format=nv12,hwupload".to_string(),
                "-c:v".to_string(),
                encoder.clone(),
            ],
        ),
    };
    let named = match writer {
        WitnessWriter::Software { encoder, .. } | WitnessWriter::Card(encoder) => encoder,
    };
    let sample = Sample(std::env::temp_dir().join(format!(
        "melyxar-card-reads-{named}-{}.mp4",
        std::process::id()
    )));

    let arguments = ["-hide_banner", "-nostdin", "-loglevel", "error", "-y"]
        .map(str::to_string)
        .into_iter()
        .chain(opening)
        .chain(["-f", "lavfi", "-i", A_GENERATED_PICTURE, "-an"].map(str::to_string))
        .chain(encoding)
        .chain([sample.path().display().to_string()]);

    match run_briefly(ffmpeg, arguments).await {
        Ok((true, _)) => Ok(sample),
        Ok((false, said)) => Err(said),
        Err(said) => Err(said),
    }
}

/// Hands the card a film and asks it to read it for itself.
///
/// Filtered the way a real film of that codec would be, because reading is
/// only half of it: a card that reads a film and then offers the frames in a
/// layout the encoder refuses has not read it, as far as a viewer is
/// concerned, and that refusal comes at the end of the chain rather than at
/// the reading.
pub(super) async fn try_reading(
    ffmpeg: &Path,
    opening: Vec<String>,
    encoder: &str,
    sample: &Path,
    filters: &str,
) -> (bool, String) {
    let arguments = ["-hide_banner", "-nostdin", "-loglevel", "error"]
        .map(str::to_string)
        .into_iter()
        .chain(opening)
        .chain(
            [
                "-i",
                &sample.display().to_string(),
                "-vf",
                filters,
                "-c:v",
                encoder,
                "-f",
                "null",
                "-",
            ]
            .map(str::to_string),
        );

    match run_briefly(ffmpeg, arguments).await {
        Ok(outcome) => outcome,
        Err(said) => (false, said),
    }
}

/// Encodes a fraction of a second of picture through one chain on the card.
///
/// What is being established is whether the driver accepts the work, and for
/// all but one of these a picture made on the spot is as good a witness as any
/// film on the disk.
pub(super) async fn try_it(
    ffmpeg: &Path,
    opening: Vec<String>,
    encoder: &str,
    input: TrialInput<'_>,
    filters: &str,
) -> (bool, String) {
    let read = match input {
        TrialInput::Generated => vec![
            "-f".to_string(),
            "lavfi".to_string(),
            "-i".to_string(),
            A_GENERATED_PICTURE.to_string(),
        ],
        TrialInput::File(path) => vec!["-i".to_string(), path.display().to_string()],
    };

    let arguments = ["-hide_banner", "-nostdin", "-loglevel", "error"]
        .map(str::to_string)
        .into_iter()
        .chain(opening)
        .chain(read)
        .chain(["-vf", filters, "-c:v", encoder, "-f", "null", "-"].map(str::to_string));

    match run_briefly(ffmpeg, arguments).await {
        Ok(outcome) => outcome,
        Err(said) => (false, said),
    }
}

/// What the tool printed, trimmed to something a report can hold.
fn shortened(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.chars().count() <= ENOUGH_TO_READ {
        return trimmed.to_string();
    }
    let kept: String = trimmed.chars().take(ENOUGH_TO_READ).collect();
    format!("{kept}...")
}

#[cfg(test)]
mod tests {
    use super::super::test_cards::nvidia as nvidia_card;
    use super::*;
    use crate::ToolPaths;

    #[test]
    fn what_the_tool_printed_is_kept_short_enough_to_read() {
        assert_eq!(shortened("  refused outright  "), "refused outright");
        let long = "a".repeat(ENOUGH_TO_READ + 50);
        let kept = shortened(&long);
        assert_eq!(kept.chars().count(), ENOUGH_TO_READ + 3);
        assert!(kept.ends_with("..."));
    }

    #[test]
    fn a_witness_is_written_in_software_where_the_build_can_and_by_the_card_otherwise() {
        let built_with: BTreeSet<String> = ["libx264", "libsvtav1", "h264_nvenc"]
            .into_iter()
            .map(str::to_string)
            .collect();
        let card = nvidia_card();

        // The shape a real film arrives in, which Nvidia's own H.264 is not.
        assert_eq!(
            witness_writer("h264", &card, &built_with),
            Some(WitnessWriter::Software {
                encoder: "libx264".to_string(),
                speed: "ultrafast".to_string(),
            })
        );
        // A card that cannot write a codec still proves it reads it.
        assert!(matches!(
            witness_writer("av1", &card, &built_with),
            Some(WitnessWriter::Software { encoder, .. }) if encoder == "libsvtav1"
        ));
        // A build without the software encoder: the card writes it.
        assert_eq!(
            witness_writer("hevc", &card, &built_with),
            Some(WitnessWriter::Card("hevc_nvenc".to_string()))
        );
        // Nobody can write it: nothing is asked.
        assert_eq!(witness_writer("av1", &card, &BTreeSet::new()), None);
    }

    #[tokio::test]
    async fn a_witness_written_in_software_is_a_film_in_its_codec() {
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        for (codec, encoder, speed) in SOFTWARE_WITNESSES {
            let writer = WitnessWriter::Software {
                encoder: (*encoder).to_string(),
                speed: (*speed).to_string(),
            };
            let sample = match sample_in(&tools.ffmpeg, &nvidia_card(), &writer).await {
                Ok(sample) => sample,
                // A build without this encoder never asks for it.
                Err(said) if said.contains("Unknown encoder") => continue,
                Err(said) => panic!("{encoder} wrote nothing: {said}"),
            };
            let read = tokio::process::Command::new(&tools.ffprobe)
                .args(["-v", "error", "-show_entries", "stream=codec_name", "-of", "csv=p=0"])
                .arg(sample.path())
                .output()
                .await
                .expect("the analyser runs");
            assert_eq!(String::from_utf8_lossy(&read.stdout).trim(), *codec);
        }
    }

    #[tokio::test]
    async fn a_wide_gamut_sample_carries_the_screen_it_says_it_was_graded_on() {
        // The whole reason the sample exists. The conversion filter does not
        // ask for a picture labelled wide gamut, it asks for these numbers,
        // and a picture made on the spot has none: the trial was establishing
        // that a card cannot convert colour on every card that can.
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let sample = wide_gamut_sample(&tools.ffmpeg)
            .await
            .expect("a sample is written");
        let kept = sample.path().to_path_buf();

        let read = tokio::process::Command::new(&tools.ffprobe)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-select_streams",
                "v:0",
                "-read_intervals",
                "%+#1",
                "-show_entries",
                "frame=color_transfer:side_data=side_data_type",
            ])
            .arg(sample.path())
            .output()
            .await
            .expect("the analyser runs");
        let described = String::from_utf8_lossy(&read.stdout);

        assert!(
            described.contains("Mastering display metadata"),
            "without these the filter refuses, and refuses rightly: {described}"
        );
        assert!(described.contains("smpte2084"), "{described}");

        drop(sample);
        assert!(
            !kept.exists(),
            "a file left behind every time a server starts is a file left there for ever"
        );
    }
}
