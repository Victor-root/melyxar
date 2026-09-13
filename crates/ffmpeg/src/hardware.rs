//! Whether a card can really rebuild a picture, established by trying it.
//!
//! Three things have to be true before a card is used, and only the first can
//! be read anywhere: the tool was built with the hardware path, the machine
//! actually has the device, and the driver on it accepts the work. A build
//! listing a hardware path proves nothing about the second and third, and a
//! container that was never given the device looks exactly like a machine with
//! no card at all.
//!
//! So nothing here is read. A tiny picture is encoded on the card, once, at
//! start-up, for each codec worth having. What the tool printed when it
//! refused is kept word for word, because that sentence is the whole
//! difference between "the card is not being used" and knowing why.
//!
//! One trial needs more than pixels. Converting wide gamut colour starts from
//! the numbers describing the screen a film was graded on, so a picture made on
//! the spot is no witness at all: it carries none, the filter refuses it, and
//! the answer would be that no card converts colour. A sample is encoded with
//! those numbers written in, which is the shape a real film arrives in.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tokio::process::Command as TokioCommand;

use crate::capabilities::HardwareAcceleration;

/// Where the graphics devices of a Linux machine appear.
///
/// An unprivileged container has to be given this folder explicitly, and that
/// is the single commonest reason a card goes unused.
const GRAPHICS_DEVICES: &str = "/dev/dri";

/// The devices that do the work, as opposed to the ones that drive a screen.
const WORKING_DEVICE: &str = "renderD";

/// The codecs a card is asked about, cheapest for a viewer to decode last.
///
/// The first one is not the preferred one: that choice belongs with the client,
/// which is the only party that knows what it can decode. This is only the list
/// of what is worth establishing.
const WORTH_TRYING: &[&str] = &["h264", "hevc", "av1"];

/// The codec every client reads, and therefore the floor.
///
/// A card that cannot produce this one is not used at all: there would be
/// clients it could serve nothing to, and falling back per client is a worse
/// answer than a card that is simply not there.
const THE_FLOOR: &str = "h264";

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

/// The name the trial gives the card inside one invocation.
const DEVICE_NAME: &str = "card";

/// Size the trial works at. Small enough to take no time, large enough that a
/// driver does not refuse it for being absurd.
const TRIAL_HEIGHT: i32 = 180;

/// The picture every trial is run on, made on the spot.
const A_GENERATED_PICTURE: &str = "testsrc2=size=640x360:rate=25:duration=0.4";

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
const WIDE_GAMUT_CODEC: &str = "hevc";

/// What a trial reads.
enum TrialInput<'a> {
    /// A picture made on the spot, which costs nothing and suits every trial
    /// that only asks whether the driver accepts the work.
    Generated,
    /// A file written for the purpose, for the one trial that needs a picture
    /// carrying more than pixels.
    File(&'a Path),
}

/// A card this machine can really rebuild a picture on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    pub way: HardwareAcceleration,
    pub device: PathBuf,
    /// Codecs proven to come out of this card, each with the encoder that
    /// produced it. Proven one by one: a build carrying an encoder is not a
    /// driver that accepts it, and this generation of cards differs from the
    /// last one in exactly that.
    pub encoders: BTreeMap<String, String>,
    /// Codecs the card was proved to read for itself.
    ///
    /// Kept apart from what it writes, because they are not the same list and
    /// never have been: a card reads codecs it cannot write, and the generation
    /// that first wrote one had been reading it for years. A film in a codec
    /// that is not here is decoded by the processor and handed up, which works
    /// whatever the film holds.
    pub decoders: BTreeSet<String>,
    /// Whether the card can also make the picture smaller. It nearly always
    /// can; a card that cannot is used at the size of the film rather than not
    /// used at all.
    pub can_scale: bool,
    /// Whether the card can convert wide gamut colour to standard range.
    ///
    /// This is the expensive part of a wide gamut film, so a card that cannot
    /// do it is left out of those films entirely and they are rebuilt in
    /// software, where the ceiling on the picture size applies.
    pub can_tone_map: bool,
}

impl Card {
    /// The encoder that produces one codec here, when this card produces it.
    pub fn encoder_for(&self, codec: &str) -> Option<&str> {
        self.encoders.get(codec).map(String::as_str)
    }

    /// Whether the card was proved to read one codec for itself.
    pub fn reads(&self, codec: &str) -> bool {
        self.decoders.contains(codec)
    }

    /// What to put before the input so the tool opens the card.
    ///
    /// When the card reads the film too, that is said here rather than
    /// anywhere else: reading is an option of the input, and an option of the
    /// input placed after it applies to nothing.
    pub fn opening_arguments(&self, reads_the_film: bool) -> Vec<String> {
        let way = self.way.as_str();
        let mut arguments = vec![
            "-init_hw_device".to_string(),
            format!("{way}={DEVICE_NAME}:{}", self.device.display()),
            "-filter_hw_device".to_string(),
            DEVICE_NAME.to_string(),
        ];
        if reads_the_film {
            arguments.extend([
                "-hwaccel".to_string(),
                way.to_string(),
                // Without this the card decodes and then hands every frame
                // back down to the processor, which is most of the cost of
                // decoding and all of the point of not doing it there.
                "-hwaccel_output_format".to_string(),
                way.to_string(),
                "-hwaccel_device".to_string(),
                DEVICE_NAME.to_string(),
            ]);
        }
        arguments
    }

    /// The filters the picture goes through on the card.
    ///
    /// Where the picture comes from changes the chain entirely. A film the card
    /// read itself is already up there, and asking to hand it up again is how a
    /// card refuses a film it was reading perfectly well. A film the processor
    /// read has to be handed up first, and in the layout the work needs.
    pub fn filters_for(
        &self,
        scale_to_height: Option<i32>,
        tone_map: bool,
        reads_the_film: bool,
    ) -> Vec<String> {
        let mut filters = Vec::new();

        if !reads_the_film {
            // Wide gamut colour arrives with ten bits to a channel, and handing
            // it up as eight would throw away exactly what is about to be
            // converted.
            filters.push(
                match tone_map {
                    true => "format=p010",
                    false => "format=nv12",
                }
                .to_string(),
            );
            filters.push("hwupload".to_string());
        }

        // Made smaller first for the same reason as in software: converting
        // colours is the most expensive thing done to a picture, so doing it
        // on fewer pixels costs less.
        if let Some(height) = scale_to_height.filter(|_| self.can_scale) {
            filters.push(format!("scale_{}=w=-2:h={height}", self.way.as_str()));
        }
        if tone_map {
            filters.push(format!("tonemap_{}=format=nv12", self.way.as_str()));
        }

        filters
    }
}

/// One thing that was tried, and what came of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Trial {
    /// What was being established, as a code a page turns into a sentence.
    pub what: String,
    pub device: String,
    pub worked: bool,
    /// What the tool printed when it refused. Empty when it did not.
    pub said: String,
}

/// Everything the search found, whether or not it found a card.
///
/// Kept whole rather than reduced to a yes or a no: a card that was refused is
/// a question with an answer, and the answer is in here.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CardSearch {
    /// The devices the machine offered, by name. Empty means the container was
    /// never given them.
    pub devices: Vec<String>,
    pub trials: Vec<Trial>,
    pub card: Option<Card>,
}

/// The trial that tells a forbidden device from a driverless one.
const OPENING: &str = "open_the_device";

impl CardSearch {
    /// Whether a device was there and this account could open it.
    ///
    /// The question worth asking when no card was found: a device that would
    /// not open is a permission to grant, and a device that opened and then
    /// answered nothing is a driver to install. Nothing else separates them.
    pub fn a_device_opened(&self) -> bool {
        self.trials
            .iter()
            .any(|trial| trial.what == OPENING && trial.worked)
    }

    /// Looks for a card and proves what it can do, or explains itself.
    ///
    /// `encoders` is what the tool was built with: there is no point trying a
    /// path the binary does not carry, and saying so is a clearer answer than
    /// a driver failure.
    pub async fn run(ffmpeg: &Path, encoders: &BTreeSet<String>) -> Self {
        let devices = working_devices();
        let mut search = Self {
            devices: devices
                .iter()
                .map(|path| path.display().to_string())
                .collect(),
            ..Self::default()
        };

        let way = HardwareAcceleration::Vaapi;
        let floor = encoder_name(THE_FLOOR, way);
        if !encoders.contains(&floor) {
            search.trials.push(Trial {
                what: "built_with_the_path".to_string(),
                device: String::new(),
                worked: false,
                said: format!("this build carries no {floor} encoder"),
            });
            return search;
        }

        for device in devices {
            if let Some(card) = search.try_this_device(ffmpeg, way, device, encoders).await {
                search.card = Some(card);
                return search;
            }
        }

        search
    }

    /// Establishes what one device can do, or nothing when it cannot encode at
    /// all.
    async fn try_this_device(
        &mut self,
        ffmpeg: &Path,
        way: HardwareAcceleration,
        device: PathBuf,
        built_with: &BTreeSet<String>,
    ) -> Option<Card> {
        let named = device.display().to_string();

        // Asked first, because it is what tells the two failures apart. A
        // device that will not open is an account that is not allowed to use
        // it; a device that opens and then answers nothing is a driver that is
        // not installed. Both come out of the media tool as the same sentence
        // about no display being found, and they are fixed in entirely
        // different places.
        if let Err(error) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&device)
        {
            self.trials.push(Trial {
                what: OPENING.to_string(),
                device: named,
                worked: false,
                said: format!(
                    "{error}; the account this server runs as has to be allowed to open it, \
                     which in an unprivileged container means belonging to the group that \
                     owns it inside the container"
                ),
            });
            return None;
        }
        self.trials.push(Trial {
            what: OPENING.to_string(),
            device: named.clone(),
            worked: true,
            said: String::new(),
        });

        let mut encoders = BTreeMap::new();
        for codec in WORTH_TRYING {
            let encoder = encoder_name(codec, way);
            if !built_with.contains(&encoder) {
                continue;
            }
            let (worked, said) = try_it(
                ffmpeg,
                way,
                &device,
                &encoder,
                TrialInput::Generated,
                "format=nv12,hwupload",
            )
            .await;
            self.trials.push(Trial {
                what: format!("rebuild_{codec}"),
                device: named.clone(),
                worked,
                said,
            });
            if worked {
                encoders.insert((*codec).to_string(), encoder);
            }
        }

        // Without the codec every client reads there would be clients this
        // card could serve nothing to, which is worse than no card at all.
        if !encoders.contains_key(THE_FLOOR) {
            return None;
        }
        let floor = encoders[THE_FLOOR].clone();

        // The two remaining trials run the chain a real film will run, rather
        // than something that resembles it: a filter that works on its own and
        // refuses what comes out of the one before it is exactly the failure
        // that only shows up in the middle of somebody's film.
        let mut card = Card {
            way,
            device,
            encoders,
            decoders: BTreeSet::new(),
            can_scale: true,
            can_tone_map: false,
        };

        let chain = card.filters_for(Some(TRIAL_HEIGHT), false, false).join(",");
        let (can_scale, said) = try_it(
            ffmpeg,
            way,
            &card.device,
            &floor,
            TrialInput::Generated,
            &chain,
        )
        .await;
        self.trials.push(Trial {
            what: "make_it_smaller".to_string(),
            device: named.clone(),
            worked: can_scale,
            said,
        });
        card.can_scale = can_scale;

        // The one trial that cannot be run on a picture made on the spot. It
        // also stands in for a real wide gamut film everywhere below: it is
        // one, down to the ten bits and the screen it says it was graded on.
        let wide_gamut = wide_gamut_sample(ffmpeg).await;
        match &wide_gamut {
            Err(said) => self.trials.push(Trial {
                what: "make_a_wide_gamut_sample".to_string(),
                device: String::new(),
                worked: false,
                said: said.clone(),
            }),
            Ok(sample) => {
                let chain = card.filters_for(Some(TRIAL_HEIGHT), true, false).join(",");
                let (can_tone_map, said) = try_it(
                    ffmpeg,
                    way,
                    &card.device,
                    &floor,
                    TrialInput::File(sample.path()),
                    &chain,
                )
                .await;
                self.trials.push(Trial {
                    what: "convert_wide_gamut".to_string(),
                    device: named.clone(),
                    worked: can_tone_map,
                    said,
                });
                card.can_tone_map = can_tone_map;
            }
        }

        card.decoders = self
            .which_codecs_it_reads(ffmpeg, &card, &floor, wide_gamut.ok(), &named)
            .await;

        Some(card)
    }

    /// Establishes which codecs the card reads for itself.
    ///
    /// Reading is asked separately from writing because they are separate
    /// abilities, and the only honest way to ask is to hand the card a film in
    /// that codec and see. Each sample is written first, by the card itself
    /// where it can, so the question does not become "is this build carrying a
    /// software encoder for that codec".
    async fn which_codecs_it_reads(
        &mut self,
        ffmpeg: &Path,
        card: &Card,
        floor: &str,
        wide_gamut: Option<Sample>,
        named: &str,
    ) -> BTreeSet<String> {
        let mut reads = BTreeSet::new();

        for codec in WORTH_TRYING {
            // A wide gamut sample carries ten bits to a channel, which is what
            // every film worth the card is, and reading eight proves nothing
            // about reading ten. Where one exists it is the better witness.
            let sample = match (*codec == WIDE_GAMUT_CODEC, &wide_gamut) {
                (true, Some(sample)) => Some(Kept::Borrowed(sample)),
                _ => match card.encoder_for(codec) {
                    Some(encoder) => match sample_in(ffmpeg, card, encoder).await {
                        Ok(made) => Some(Kept::Owned(made)),
                        Err(said) => {
                            self.trials.push(Trial {
                                what: format!("make_a_{codec}_sample"),
                                device: named.to_string(),
                                worked: false,
                                said,
                            });
                            None
                        }
                    },
                    // Nothing here can write this codec, so nothing here can
                    // ask whether the card reads it. Said plainly rather than
                    // recorded as a refusal it never made.
                    None => None,
                },
            };
            let Some(sample) = sample else { continue };

            let (worked, said) = try_reading(ffmpeg, card, floor, sample.path()).await;
            self.trials.push(Trial {
                what: format!("read_{codec}"),
                device: named.to_string(),
                worked,
                said,
            });
            if worked {
                reads.insert((*codec).to_string());
            }
        }

        reads
    }
}

/// A sample this trial owns, or one it was lent.
enum Kept<'a> {
    Owned(Sample),
    Borrowed(&'a Sample),
}

impl Kept<'_> {
    fn path(&self) -> &Path {
        match self {
            Self::Owned(sample) => sample.path(),
            Self::Borrowed(sample) => sample.path(),
        }
    }
}

/// What the encoder of one codec is called on one hardware path.
fn encoder_name(codec: &str, way: HardwareAcceleration) -> String {
    format!("{codec}_{}", way.as_str())
}

/// The devices that do the work, in the order the machine lists them.
fn working_devices() -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(GRAPHICS_DEVICES) else {
        return Vec::new();
    };
    let mut devices: Vec<PathBuf> = entries
        .filter_map(std::result::Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(WORKING_DEVICE)
        })
        .map(|entry| entry.path())
        .collect();
    devices.sort();
    devices
}

/// A wide gamut sample on disk, removed when the trial is done with it.
///
/// Tied to its own removal rather than deleted by hand: the trial can fail at
/// several points, and a file left in a temporary folder every time a server
/// starts is a file left there for ever.
struct Sample(PathBuf);

impl Sample {
    fn path(&self) -> &Path {
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
async fn wide_gamut_sample(ffmpeg: &Path) -> std::result::Result<Sample, String> {
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
async fn run_briefly(
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

/// Writes a fraction of a second of film in one codec, on the card itself.
///
/// Made by the card rather than by a software encoder so the question stays
/// the one being asked. A build lacking a software encoder for a codec would
/// otherwise come out as a card that cannot read it.
async fn sample_in(
    ffmpeg: &Path,
    card: &Card,
    encoder: &str,
) -> std::result::Result<Sample, String> {
    let sample = Sample(std::env::temp_dir().join(format!(
        "melyxar-card-reads-{encoder}-{}.mp4",
        std::process::id()
    )));

    let arguments = ["-hide_banner", "-nostdin", "-loglevel", "error", "-y"]
        .map(str::to_string)
        .into_iter()
        .chain(card.opening_arguments(false))
        .chain(
            [
                "-f",
                "lavfi",
                "-i",
                A_GENERATED_PICTURE,
                "-an",
                "-vf",
                "format=nv12,hwupload",
                "-c:v",
                encoder,
                &sample.path().display().to_string(),
            ]
            .map(str::to_string),
        );

    match run_briefly(ffmpeg, arguments).await {
        Ok((true, _)) => Ok(sample),
        Ok((false, said)) => Err(said),
        Err(said) => Err(said),
    }
}

/// Hands the card a film and asks it to read it for itself.
///
/// Nothing is filtered: what is being established is whether the card takes
/// the film in, and the frames come out of it already up there, so the encoder
/// takes them as they are.
async fn try_reading(ffmpeg: &Path, card: &Card, encoder: &str, sample: &Path) -> (bool, String) {
    let arguments = ["-hide_banner", "-nostdin", "-loglevel", "error"]
        .map(str::to_string)
        .into_iter()
        .chain(card.opening_arguments(true))
        .chain(
            [
                "-i",
                &sample.display().to_string(),
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
async fn try_it(
    ffmpeg: &Path,
    way: HardwareAcceleration,
    device: &Path,
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

    let arguments = [
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-init_hw_device",
        &format!("{}={DEVICE_NAME}:{}", way.as_str(), device.display()),
        "-filter_hw_device",
        DEVICE_NAME,
    ]
    .map(str::to_string)
    .into_iter()
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
    use super::*;
    use crate::ToolPaths;

    fn card(can_scale: bool, can_tone_map: bool) -> Card {
        Card {
            way: HardwareAcceleration::Vaapi,
            device: PathBuf::from("/dev/dri/renderD128"),
            encoders: [
                ("h264".to_string(), "h264_vaapi".to_string()),
                ("av1".to_string(), "av1_vaapi".to_string()),
            ]
            .into_iter()
            .collect(),
            decoders: ["hevc".to_string()].into_iter().collect(),
            can_scale,
            can_tone_map,
        }
    }

    #[test]
    fn a_card_says_where_it_is_before_the_film_is_opened() {
        // The tool has to be told about the card before it reads anything: a
        // device named afterwards is a device the filters cannot reach.
        assert_eq!(
            card(true, true).opening_arguments(false),
            vec![
                "-init_hw_device".to_string(),
                "vaapi=card:/dev/dri/renderD128".to_string(),
                "-filter_hw_device".to_string(),
                "card".to_string(),
            ]
        );
    }

    #[test]
    fn a_card_only_offers_a_codec_it_was_proved_to_produce() {
        let card = card(true, true);
        assert_eq!(card.encoder_for("h264"), Some("h264_vaapi"));
        assert_eq!(card.encoder_for("av1"), Some("av1_vaapi"));
        assert_eq!(
            card.encoder_for("hevc"),
            None,
            "a build carrying an encoder is not a driver that accepts it"
        );
    }

    #[test]
    fn wide_gamut_colour_is_handed_to_the_card_with_all_its_bits() {
        // Handing it up as eight bits would throw away exactly what is about
        // to be converted, and the conversion would have nothing to work with.
        let filters = card(true, true).filters_for(Some(1080), true, false);
        assert_eq!(filters[0], "format=p010");
        assert_eq!(filters[1], "hwupload");
        assert!(filters.iter().any(|value| value.contains("tonemap_vaapi")));

        let plain = card(true, true).filters_for(None, false, false);
        assert_eq!(
            plain,
            vec!["format=nv12".to_string(), "hwupload".to_string()]
        );
    }

    #[test]
    fn the_picture_is_made_smaller_before_its_colours_are_converted() {
        // The same reason as in software: converting colours is the most
        // expensive thing done to a picture, so it is done on fewer pixels.
        let filters = card(true, true).filters_for(Some(1080), true, false);
        let scale = filters
            .iter()
            .position(|value| value.starts_with("scale_vaapi"))
            .expect("the picture is made smaller");
        let tone_map = filters
            .iter()
            .position(|value| value.starts_with("tonemap_vaapi"))
            .expect("the colours are converted");
        assert!(scale < tone_map, "{filters:?}");
    }

    #[test]
    fn a_card_that_cannot_make_a_picture_smaller_is_still_used_at_full_size() {
        let filters = card(false, true).filters_for(Some(1080), false, false);
        assert!(
            !filters.iter().any(|value| value.starts_with("scale_")),
            "asking for something the card refused is how a film stops playing: {filters:?}"
        );
        assert!(filters.iter().any(|value| value == "hwupload"));
    }

    #[test]
    fn what_the_tool_printed_is_kept_short_enough_to_read() {
        assert_eq!(shortened("  refused outright  "), "refused outright");
        let long = "a".repeat(ENOUGH_TO_READ + 50);
        let kept = shortened(&long);
        assert_eq!(kept.chars().count(), ENOUGH_TO_READ + 3);
        assert!(kept.ends_with("..."));
    }

    #[tokio::test]
    async fn a_path_this_build_does_not_carry_is_said_plainly_rather_than_tried() {
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let search = CardSearch::run(&tools.ffmpeg, &BTreeSet::new()).await;

        assert!(search.card.is_none());
        assert_eq!(search.trials.len(), 1);
        assert_eq!(search.trials[0].what, "built_with_the_path");
        assert!(search.trials[0].said.contains("h264_vaapi"));
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

    #[test]
    fn a_forbidden_device_and_a_driverless_one_are_told_apart() {
        // The media tool words both the same way, as no display being found,
        // and they are fixed in entirely different places: one is a permission
        // to grant, the other a package to install.
        let opened = CardSearch {
            trials: vec![Trial {
                what: OPENING.to_string(),
                device: "/dev/dri/renderD128".to_string(),
                worked: true,
                said: String::new(),
            }],
            ..CardSearch::default()
        };
        assert!(opened.a_device_opened());

        let forbidden = CardSearch {
            trials: vec![Trial {
                what: OPENING.to_string(),
                device: "/dev/dri/renderD128".to_string(),
                worked: false,
                said: "permission denied".to_string(),
            }],
            ..CardSearch::default()
        };
        assert!(!forbidden.a_device_opened());
        assert!(!CardSearch::default().a_device_opened());
    }

    #[tokio::test]
    async fn a_machine_with_no_card_says_so_without_claiming_one() {
        // What a container that was never given the graphics device looks
        // like, and what this working environment is.
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let encoders = ["h264_vaapi".to_string()].into_iter().collect();
        let search = CardSearch::run(&tools.ffmpeg, &encoders).await;

        if search.devices.is_empty() {
            assert!(search.card.is_none(), "there is no device to have used");
            assert!(
                search.trials.is_empty(),
                "nothing to try, and nothing is claimed: {:?}",
                search.trials
            );
        } else {
            // A machine that does have one: whatever the outcome, every trial
            // that failed carries what the tool said about it.
            for trial in &search.trials {
                assert!(
                    trial.worked || !trial.said.is_empty(),
                    "a refusal with nothing to read is a debugging session: {trial:?}"
                );
            }
        }
    }
}
