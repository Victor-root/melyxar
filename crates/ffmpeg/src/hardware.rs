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

/// What labels a generated picture as a wide gamut one, for the trial alone.
///
/// The conversion filter refuses anything that is not wide gamut, and rightly:
/// there would be nothing to convert. A real film arrives carrying these
/// labels, and a test pattern does not, so the trial puts them on. It is the
/// one thing the trial adds to the chain a real film goes through.
const LABELLED_WIDE_GAMUT: &str =
    "setparams=color_primaries=bt2020:color_trc=smpte2084:colorspace=bt2020nc";

/// Size the trial works at. Small enough to take no time, large enough that a
/// driver does not refuse it for being absurd.
const TRIAL_HEIGHT: i32 = 180;

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

    /// What to put before the input so the tool opens the card.
    pub fn opening_arguments(&self) -> Vec<String> {
        vec![
            "-init_hw_device".to_string(),
            format!(
                "{}={DEVICE_NAME}:{}",
                self.way.as_str(),
                self.device.display()
            ),
            "-filter_hw_device".to_string(),
            DEVICE_NAME.to_string(),
        ]
    }

    /// The filters that put a picture onto the card, and what happens to it
    /// there.
    ///
    /// The picture is decoded in software and handed up. Decoding on the card
    /// as well is a further step: it depends on the codec of each film, where
    /// this works whatever the film holds, and the expensive half of the work
    /// is the half that moves here.
    pub fn filters_for(&self, scale_to_height: Option<i32>, tone_map: bool) -> Vec<String> {
        let mut filters = Vec::new();

        // Wide gamut colour arrives with ten bits to a channel, and handing it
        // up as eight would throw away exactly what is about to be converted.
        filters.push(
            match tone_map {
                true => "format=p010",
                false => "format=nv12",
            }
            .to_string(),
        );
        filters.push("hwupload".to_string());

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

impl CardSearch {
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
        let mut encoders = BTreeMap::new();

        for codec in WORTH_TRYING {
            let encoder = encoder_name(codec, way);
            if !built_with.contains(&encoder) {
                continue;
            }
            let (worked, said) =
                try_it(ffmpeg, way, &device, &encoder, "format=nv12,hwupload").await;
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
            can_scale: true,
            can_tone_map: false,
        };

        let chain = card.filters_for(Some(TRIAL_HEIGHT), false).join(",");
        let (can_scale, said) = try_it(ffmpeg, way, &card.device, &floor, &chain).await;
        self.trials.push(Trial {
            what: "make_it_smaller".to_string(),
            device: named.clone(),
            worked: can_scale,
            said,
        });
        card.can_scale = can_scale;

        let mut chain = card.filters_for(Some(TRIAL_HEIGHT), true);
        chain.insert(1, LABELLED_WIDE_GAMUT.to_string());
        let (can_tone_map, said) =
            try_it(ffmpeg, way, &card.device, &floor, &chain.join(",")).await;
        self.trials.push(Trial {
            what: "convert_wide_gamut".to_string(),
            device: named,
            worked: can_tone_map,
            said,
        });
        card.can_tone_map = can_tone_map;

        Some(card)
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

/// Encodes a fraction of a second of a generated picture through one chain.
///
/// A generated picture rather than a film: what is being established is
/// whether the driver accepts the work, and no film on the disk is a better
/// witness to that than four hundred milliseconds of test pattern.
async fn try_it(
    ffmpeg: &Path,
    way: HardwareAcceleration,
    device: &Path,
    encoder: &str,
    filters: &str,
) -> (bool, String) {
    let arguments = [
        "-hide_banner",
        "-nostdin",
        "-loglevel",
        "error",
        "-init_hw_device",
        &format!("{}={DEVICE_NAME}:{}", way.as_str(), device.display()),
        "-filter_hw_device",
        DEVICE_NAME,
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=640x360:rate=25:duration=0.4",
        "-vf",
        filters,
        "-c:v",
        encoder,
        "-f",
        "null",
        "-",
    ]
    .map(str::to_string);

    let spawned = TokioCommand::new(ffmpeg)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        // A driver that has locked up must not hold the server down: the trial
        // is dropped when its time is up, and this is what makes dropping it
        // put an end to the process rather than orphan it.
        .kill_on_drop(true)
        .spawn();

    let Ok(child) = spawned else {
        return (false, "the media tool could not be started".to_string());
    };

    match tokio::time::timeout(TRIAL_PATIENCE, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            let said = shortened(&String::from_utf8_lossy(&output.stderr));
            (output.status.success(), said)
        }
        Ok(Err(error)) => (false, error.to_string()),
        Err(_) => (
            false,
            format!(
                "the card did not answer within {} seconds",
                TRIAL_PATIENCE.as_secs()
            ),
        ),
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
            can_scale,
            can_tone_map,
        }
    }

    #[test]
    fn a_card_says_where_it_is_before_the_film_is_opened() {
        // The tool has to be told about the card before it reads anything: a
        // device named afterwards is a device the filters cannot reach.
        assert_eq!(
            card(true, true).opening_arguments(),
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
        let filters = card(true, true).filters_for(Some(1080), true);
        assert_eq!(filters[0], "format=p010");
        assert_eq!(filters[1], "hwupload");
        assert!(filters.iter().any(|value| value.contains("tonemap_vaapi")));

        let plain = card(true, true).filters_for(None, false);
        assert_eq!(
            plain,
            vec!["format=nv12".to_string(), "hwupload".to_string()]
        );
    }

    #[test]
    fn the_picture_is_made_smaller_before_its_colours_are_converted() {
        // The same reason as in software: converting colours is the most
        // expensive thing done to a picture, so it is done on fewer pixels.
        let filters = card(true, true).filters_for(Some(1080), true);
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
        let filters = card(false, true).filters_for(Some(1080), false);
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
