//! The trials a card is put through: the samples it is shown and the runs of
//! the tool that ask it something. Nothing here knows one maker from another:
//! what a card is asked is decided by its path, through the card.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
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
    /// A file of its own in the temporary folder: two trials running at
    /// once, or a server and its tests, never write over each other's.
    pub(super) fn named(name: &str) -> Self {
        static MADE: AtomicUsize = AtomicUsize::new(0);
        let count = MADE.fetch_add(1, Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!("{name}-{}-{count}.mp4", std::process::id())))
    }

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
        Sample::named("melyxar-card-trial");

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
    run_keeping(ffmpeg, arguments, false)
        .await
        .map(|(worked, said, _)| (worked, said))
}

/// The same, keeping what the tool wrote out when `keeps_what_it_wrote`.
async fn run_keeping(
    ffmpeg: &Path,
    arguments: impl IntoIterator<Item = String>,
    keeps_what_it_wrote: bool,
) -> std::result::Result<(bool, String, Vec<u8>), String> {
    let spawned = TokioCommand::new(ffmpeg)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(if keeps_what_it_wrote { Stdio::piped() } else { Stdio::null() })
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
            output.stdout,
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
    let sample = Sample::named(&format!("melyxar-card-reads-{named}"));

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

/// How small a picture is brought before it is looked at: enough to tell its
/// colours and its shapes, nothing more.
const LOOKED_AT: (usize, usize) = (64, 36);

/// The height a picture passed to the card without a copy is looked at:
/// that of the largest film rebuilt, which is rebuilt at its own size.
///
/// A card may lay out a large picture in memory unlike a small one, and a
/// picture handed across in a layout its receiver misreads comes out as
/// scattered stripes. Seen on an RX 6600 with a film rebuilt at 3840 by 1600,
/// after the same trial at 180 and at 1080 lines had come out right.
pub(crate) const LOOKED_AT_HEIGHT: i32 = 2160;

/// How far the card's picture may stand from the processor's, on average, in
/// brightness and in each of the two colour planes, out of 255.
///
/// Wide, since the two convert by different curves and neither is wrong:
/// measured on the sample through Mesa's Vulkan, a right picture stood 65 away
/// in brightness and 23 in colour. A green one stood 136 and 126 away on the
/// two colour planes, and a black one stands as far on all three.
const ALLOWED_BRIGHTNESS: f64 = 90.0;
const ALLOWED_COLOUR: f64 = 45.0;

/// How closely the shapes of the card's picture must follow the processor's,
/// as the correlation of their brightness, from 1 for the same shapes to 0
/// for none in common.
///
/// Measured on the sample through Mesa's Vulkan: a right picture 0.78, the
/// different curve accounting for the rest. The same picture scattered or
/// read with the wrong stride, as a misread layout does, stays under 0.15.
const ALIKE_IN_SHAPE: f64 = 0.5;

/// Converts the wide gamut sample on the card the way a film would be, and
/// sets one picture of it beside the same converted on the processor.
///
/// The one trial that looks at what came out rather than at whether anything
/// refused: a picture handed from Vulkan to the card came out green once,
/// and scattered in stripes another time, through paths where every step
/// said it had worked.
pub(super) async fn looks_right(
    ffmpeg: &Path,
    opening: Vec<String>,
    encoder: &str,
    sample: &Path,
    filters: &str,
) -> (bool, String) {
    let made = Sample::named("melyxar-card-looked-at");
    let converting = QUIET
        .map(str::to_string)
        .into_iter()
        .chain(opening)
        .chain(
            [
                "-i",
                &sample.display().to_string(),
                "-vf",
                filters,
                "-frames:v",
                "1",
                "-c:v",
                encoder,
                "-y",
                &made.path().display().to_string(),
            ]
            .map(str::to_string),
        );
    if let Ok((false, said)) | Err(said) = run_briefly(ffmpeg, converting).await {
        return (false, said);
    }

    let card = looked_at(ffmpeg, made.path(), "").await;
    let processor = looked_at(
        ffmpeg,
        sample,
        &format!("{WHAT_THE_SAMPLE_IS},{},", crate::command::TONE_MAP_FILTER),
    )
    .await;
    let judged = match (card, processor) {
        (Ok(card), Ok(processor)) => alike(&card, &processor),
        // A card is never refused for want of something to compare it with:
        // the sample as it was read keeps the shapes, and a right picture
        // stays near no colour at all on average.
        (Ok(card), Err(said)) => match looked_at(ffmpeg, sample, "").await {
            Ok(read) => alike(&card, &colourless_beside(&read)).map(|alike| {
                format!(
                    "{alike}, judged on the sample as read, the processor's conversion \
                     failed: {said}"
                )
            }),
            Err(also) => Err(format!(
                "nothing to compare the card's picture with: {said} | {also}"
            )),
        },
        (Err(said), _) => Err(said),
    };
    match judged {
        Ok(said) => (true, said),
        Err(said) => (false, said),
    }
}

/// Sets a picture a card wrote beside the same written by the processor and
/// says whether they look alike, both being of the same picture with no
/// colours to convert.
pub(super) async fn written_alike(ffmpeg: &Path, card: &Path, processor: &Path) -> (bool, String) {
    let judged = match (
        looked_at(ffmpeg, card, "").await,
        looked_at(ffmpeg, processor, "").await,
    ) {
        (Ok(card), Ok(processor)) => alike(&card, &processor),
        (Err(said), _) | (_, Err(said)) => Err(said),
    };
    match judged {
        Ok(said) => (true, said),
        Err(said) => (false, said),
    }
}

/// What a trial that writes no more than it must says to the tool first.
const QUIET: [&str; 4] = ["-hide_banner", "-nostdin", "-loglevel", "error"];

/// One picture of `input`, after `before`, brought to `LOOKED_AT` in the
/// layout of four brightness samples to one of each colour.
async fn looked_at(ffmpeg: &Path, input: &Path, before: &str) -> std::result::Result<Vec<u8>, String> {
    let (across, down) = LOOKED_AT;
    let arguments = QUIET.map(str::to_string).into_iter().chain(
        [
            "-i",
            &input.display().to_string(),
            "-vf",
            &format!("{before}scale={across}:{down},format=yuv420p"),
            "-frames:v",
            "1",
            "-f",
            "rawvideo",
            "-",
        ]
        .map(str::to_string),
    );
    match run_keeping(ffmpeg, arguments, true).await {
        Ok((true, _, picture)) => Ok(picture),
        Ok((false, said, _)) | Err(said) => {
            Err(format!("the picture could not be looked at: {said}"))
        }
    }
}

/// What the sample is, told to the processor's conversion rather than left
/// to what its reader finds in the file: that conversion refused it on a
/// machine where it found nothing there.
const WHAT_THE_SAMPLE_IS: &str =
    "setparams=range=tv:color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc";

/// The shapes of `picture` with no colour at all, which is what a picture
/// whose colours are right averages out near.
fn colourless_beside(picture: &[u8]) -> Vec<u8> {
    let (across, down) = LOOKED_AT;
    let brightness = (across * down).min(picture.len());
    let mut colourless = picture[..brightness].to_vec();
    colourless.resize(picture.len(), 128);
    colourless
}

/// Whether two pictures of `LOOKED_AT` in the layout of four brightness
/// samples to one of each colour look alike, in colour and in shape, saying
/// how they compare either way.
fn alike(card: &[u8], processor: &[u8]) -> std::result::Result<String, String> {
    let (across, down) = LOOKED_AT;
    let brightness = across * down;
    let colour = brightness / 4;
    if card.len() != brightness + 2 * colour || processor.len() != card.len() {
        return Err(format!(
            "a picture of {} bytes was expected, the card's has {} and the processor's {}",
            brightness + 2 * colour,
            card.len(),
            processor.len()
        ));
    }
    let averages = |picture: &[u8]| {
        (
            mean(&picture[..brightness]),
            mean(&picture[brightness..brightness + colour]),
            mean(&picture[brightness + colour..]),
        )
    };
    let (card_y, card_u, card_v) = averages(card);
    let (processor_y, processor_u, processor_v) = averages(processor);
    let shape = correlation(&card[..brightness], &processor[..brightness]);
    let said = format!(
        "card Y/U/V {card_y:.0}/{card_u:.0}/{card_v:.0}, \
         processor {processor_y:.0}/{processor_u:.0}/{processor_v:.0}, shapes {shape:.2}"
    );
    let close = (card_y - processor_y).abs() <= ALLOWED_BRIGHTNESS
        && (card_u - processor_u).abs() <= ALLOWED_COLOUR
        && (card_v - processor_v).abs() <= ALLOWED_COLOUR
        && shape >= ALIKE_IN_SHAPE;
    match close {
        true => Ok(said),
        false => Err(format!("the card's picture does not look like the processor's: {said}")),
    }
}

fn mean(values: &[u8]) -> f64 {
    values.iter().map(|&value| f64::from(value)).sum::<f64>() / values.len() as f64
}

/// The correlation of two runs of values of the same length: 1 when one
/// rises wherever the other does, 0 when they have nothing to do with each
/// other, and 0 too when either is flat, having no shape to follow.
fn correlation(one: &[u8], other: &[u8]) -> f64 {
    let (one_mean, other_mean) = (mean(one), mean(other));
    let (mut together, mut one_spread, mut other_spread) = (0.0, 0.0, 0.0);
    for (&a, &b) in one.iter().zip(other) {
        let (a, b) = (f64::from(a) - one_mean, f64::from(b) - other_mean);
        together += a * b;
        one_spread += a * a;
        other_spread += b * b;
    }
    match one_spread * other_spread {
        spread if spread > 0.0 => together / spread.sqrt(),
        _ => 0.0,
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

    /// A picture of `LOOKED_AT` of one colour, striped across around one
    /// brightness, so that it has a shape to follow.
    fn picture_of(y: u8, u: u8, v: u8) -> Vec<u8> {
        let (across, down) = LOOKED_AT;
        let brightness = across * down;
        let mut picture: Vec<u8> = (0..down)
            .flat_map(|row| {
                let striped = match row % 2 {
                    0 => y.saturating_sub(8),
                    _ => y.saturating_add(8),
                };
                std::iter::repeat_n(striped, across)
            })
            .collect();
        picture.extend(std::iter::repeat_n(u, brightness / 4));
        picture.extend(std::iter::repeat_n(v, brightness / 4));
        picture
    }

    #[test]
    fn a_green_picture_does_not_look_like_the_processors() {
        // The averages measured on the sample: the processor's conversion,
        // Vulkan's right one, and Vulkan's green one.
        let processor = picture_of(79, 137, 127);
        assert!(alike(&picture_of(144, 114, 115), &processor).is_ok(), "another curve");
        let green = alike(&picture_of(129, 1, 1), &processor).expect_err("green");
        assert!(green.contains("129/1/1"), "{green}");
        assert!(alike(&picture_of(0, 0, 0), &processor).is_err(), "black");
        assert!(alike(&picture_of(79, 137, 190), &processor).is_err(), "too red");
        assert!(alike(&[0; 10], &processor).is_err(), "not a whole picture");
    }

    #[test]
    fn without_the_processors_picture_a_green_one_is_still_caught() {
        let right = picture_of(144, 114, 115);
        assert!(alike(&right, &colourless_beside(&right)).is_ok());
        let green = picture_of(129, 1, 1);
        assert!(alike(&green, &colourless_beside(&green)).is_err());
        assert!(alike(&[0; 10], &colourless_beside(&[0; 10])).is_err());
    }

    #[test]
    fn a_picture_scattered_in_stripes_does_not_look_like_the_processors() {
        // A layout misread on the way to the encoder keeps the averages and
        // loses the shapes: the same values, read with the wrong stride.
        let (across, down) = LOOKED_AT;
        let processor: Vec<u8> = (0..across * down)
            .map(|at| u8::try_from(at % across * 3).expect("under 255"))
            .chain(std::iter::repeat_n(128, across * down / 2))
            .collect();
        assert!(alike(&processor, &processor).is_ok());
        let scattered: Vec<u8> = (0..across * down)
            .map(|at| processor[at * 7 % (across * down)])
            .chain(processor[across * down..].iter().copied())
            .collect();
        let said = alike(&scattered, &processor).expect_err("scattered");
        assert!(said.contains("shapes"), "{said}");
        let flat = picture_of(100, 128, 128);
        assert!(alike(&picture_of(100, 128, 128), &flat).is_ok(), "stripes follow stripes");
        let even: Vec<u8> = std::iter::repeat_n(100, across * down)
            .chain(std::iter::repeat_n(128, across * down / 2))
            .collect();
        assert!(alike(&even, &flat).is_err(), "nothing to follow");
    }

    #[tokio::test]
    async fn the_processors_own_conversion_looks_like_itself() {
        // The comparison run end to end with what every machine has: the
        // sample converted on the processor and written by a software
        // encoder, looked at as the card's would be.
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let sample = wide_gamut_sample(&tools.ffmpeg).await.expect("a sample is written");
        let (worked, said) = looks_right(
            &tools.ffmpeg,
            Vec::new(),
            "libx264",
            sample.path(),
            crate::command::TONE_MAP_FILTER,
        )
        .await;
        assert!(worked, "{said}");
        assert!(said.contains("card Y/U/V"), "{said}");
    }

    #[tokio::test]
    async fn the_processors_painted_picture_is_written_and_looked_at() {
        // What a card's painted picture is set beside, written by every
        // build, and judged alike to itself.
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let painted = Sample::named("melyxar-processor-painted-test");
        let (worked, said) = run_briefly(
            &tools.ffmpeg,
            crate::painting::painted_by_the_processor_arguments(painted.path()),
        )
        .await
        .expect("the tool runs");
        assert!(worked, "{said}");
        let (alike, said) = written_alike(&tools.ffmpeg, painted.path(), painted.path()).await;
        assert!(alike, "{said}");
        assert!(said.contains("shapes 1.00"), "{said}");
    }

    #[tokio::test]
    async fn a_picture_left_unconverted_is_caught() {
        // Wide gamut colour left as it was is what a wrong conversion looks
        // like at its mildest: washed out and off in colour.
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let sample = wide_gamut_sample(&tools.ffmpeg).await.expect("a sample is written");
        let (worked, said) = looks_right(
            &tools.ffmpeg,
            Vec::new(),
            "libx264",
            sample.path(),
            "format=yuv420p,lutyuv=u=val*0.3:v=val*0.3",
        )
        .await;
        assert!(!worked, "{said}");
    }

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
