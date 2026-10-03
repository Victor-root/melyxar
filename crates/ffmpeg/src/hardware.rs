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
//! Every card of the machine is tried, each through the path it is driven by:
//! the open interface for Intel and AMD cards, Nvidia's own for Nvidia's. A
//! machine can carry one of each, and which of them does the work is a choice
//! made afterwards, among the cards that passed.
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
use crate::painting;

/// Where the graphics devices of a Linux machine appear.
///
/// An unprivileged container has to be given this folder explicitly, and that
/// is the single commonest reason a card goes unused.
const GRAPHICS_DEVICES: &str = "/dev/dri";

/// The devices that do the work, as opposed to the ones that drive a screen.
const WORKING_DEVICE: &str = "renderD";

/// Where the kernel describes each graphics device: who made it and where it
/// sits on the machine.
const DEVICE_DESCRIPTIONS: &str = "/sys/class/drm";

/// Where Nvidia's driver lists its cards, one folder each, named after where
/// the card sits on the machine.
const NVIDIA_CARDS: &str = "/proc/driver/nvidia/gpus";

/// Where Nvidia's device files are, numbered after the cards.
const NVIDIA_DEVICES: &str = "/dev";

/// The maker number of Nvidia's cards.
///
/// Their files under `/dev/dri` are never tried through the open interface:
/// it is not how Nvidia's cards encode, and trying would only add refusals to
/// the report.
const NVIDIA: &str = "0x10de";

/// Where the machine keeps the names of the hardware it may carry, when it
/// keeps them at all. The first one found is read.
const HARDWARE_NAMES: &[&str] = &["/usr/share/misc/pci.ids", "/usr/share/hwdata/pci.ids"];

/// The paths a card can be driven by, in the order their cards are tried.
const WAYS: &[HardwareAcceleration] = &[HardwareAcceleration::Vaapi, HardwareAcceleration::Cuda];

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

/// The name given to the Vulkan device opened on the same card, for a recipe
/// that converts colour through it.
const VULKAN_DEVICE_NAME: &str = "vk";

/// What Vulkan is asked to make of a wide gamut picture: standard range, in
/// the layout the encoder takes, converted the way the filter judges best for
/// that film.
const VULKAN_CONVERSION: &str = "format=nv12:colorspace=bt709:color_primaries=bt709\
    :color_trc=bt709:range=tv:tonemapping=auto";

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
const WIDE_GAMUT_CODEC: &str = "hevc";

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
enum TrialInput<'a> {
    /// A picture made on the spot, which costs nothing and suits every trial
    /// that only asks whether the driver accepts the work.
    Generated,
    /// A file written for the purpose, for the one trial that needs a picture
    /// carrying more than pixels.
    File(&'a Path),
}

/// How a card converts wide gamut colour to standard range.
///
/// A recipe rather than a yes or a no, because the paths differ in more than
/// the name of a filter. The official tool converts on Intel and AMD cards with
/// the card's own filter, and carries nothing of the kind for Nvidia's, where
/// the conversion goes through Vulkan opened on the same card. Each path offers
/// its recipes best first, and the first one the card is proved to run is the
/// one it keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToneMapping {
    /// The card's own conversion filter, fed by the card's own reader.
    OwnFilter,
    /// Vulkan on the same card converts, and the picture comes down to the
    /// processor at its final size for the encoder.
    ///
    /// The tool cannot hand a picture from Nvidia's reader to Vulkan, so
    /// Vulkan reads the film itself when it was proved to read a wide gamut
    /// one, and the processor reads it otherwise.
    ThroughVulkan { reads: bool },
}

impl ToneMapping {
    /// The recipes one path offers, best first.
    fn offered_on(way: HardwareAcceleration) -> &'static [Self] {
        match way {
            HardwareAcceleration::Vaapi => &[Self::OwnFilter],
            HardwareAcceleration::Cuda => &[
                Self::ThroughVulkan { reads: true },
                Self::ThroughVulkan { reads: false },
            ],
            _ => &[],
        }
    }

    /// What the trial of this recipe is called in a report.
    fn trial_name(self) -> &'static str {
        match self {
            Self::OwnFilter => "convert_wide_gamut",
            Self::ThroughVulkan { reads: true } => "convert_wide_gamut_vulkan_reading",
            Self::ThroughVulkan { reads: false } => "convert_wide_gamut_vulkan",
        }
    }

    /// Whether the picture leaves the card once converted.
    pub fn brings_the_picture_down(self) -> bool {
        matches!(self, Self::ThroughVulkan { .. })
    }
}

/// A card this machine can really rebuild a picture on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    pub way: HardwareAcceleration,
    /// What the card is kept under when it is chosen: its path and where it
    /// sits on the machine. The name of its device file can change from one
    /// start to the next, where it sits does not.
    pub key: String,
    /// What a person calls it.
    pub name: String,
    /// The file opened to reach it, which is also what is checked to say it is
    /// still there.
    pub device: PathBuf,
    /// How the tool is told which card it is: its device file on the open
    /// interface, its number on Nvidia's.
    pub address: String,
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
    /// How the card converts wide gamut colour to standard range, when one way
    /// was proved to work on it.
    ///
    /// This is the expensive part of a wide gamut film, so a card without one
    /// is left out of those films entirely and they are rebuilt in software,
    /// where the ceiling on the picture size applies.
    pub tone_mapping: Option<ToneMapping>,
    /// The layout a subtitle made of pictures is handed up to the card in,
    /// when the card was proved to lay one onto a picture.
    ///
    /// Absent when it was not, and a film with such a subtitle is then
    /// rebuilt by the processor: handing the picture down to lay the subtitle
    /// on it and up again was measured to hold gigabytes and to run at a
    /// fraction of real time.
    pub picture_subtitle_layout: Option<String>,
}

impl Card {
    /// A card found on the machine and not yet asked anything.
    pub(crate) fn unproved(
        way: HardwareAcceleration,
        key: String,
        name: String,
        device: PathBuf,
        address: String,
    ) -> Self {
        Self {
            way,
            key,
            name,
            device,
            address,
            encoders: BTreeMap::new(),
            decoders: BTreeSet::new(),
            can_scale: true,
            tone_mapping: None,
            picture_subtitle_layout: None,
        }
    }

    /// The layout a subtitle made of pictures is handed up in, when this card
    /// was proved to paint one onto a picture.
    pub fn picture_subtitle_layout(&self) -> Option<&str> {
        self.picture_subtitle_layout.as_deref()
    }

    /// Whether the card was proved to convert wide gamut colour.
    pub fn can_tone_map(&self) -> bool {
        self.tone_mapping.is_some()
    }

    /// The encoder that produces one codec here, when this card produces it.
    pub fn encoder_for(&self, codec: &str) -> Option<&str> {
        self.encoders.get(codec).map(String::as_str)
    }

    /// Whether the card was proved to read one codec for itself.
    pub fn reads(&self, codec: &str) -> bool {
        self.decoders.contains(codec)
    }

    /// Whether the card reads a film in one codec for itself, given whether
    /// its colours are converted: through Vulkan it is Vulkan that reads, and
    /// it was proved to on the wide gamut sample alone.
    pub fn reads_for(&self, codec: &str, tone_map: bool) -> bool {
        match self.tone_mapping.filter(|_| tone_map) {
            Some(ToneMapping::ThroughVulkan { reads }) => reads && codec == WIDE_GAMUT_CODEC,
            _ => self.reads(codec),
        }
    }

    /// Whether the driver keeps a tally of the card's work for each program
    /// that has it open, which is how the server reads how busy it is.
    ///
    /// Nvidia's does not for the work done through its compute interface, and
    /// a card read as idle while it converts is worse than one read as
    /// nothing.
    pub fn tallies_its_work(&self) -> bool {
        self.way != HardwareAcceleration::Cuda
    }

    /// What the encoder is told about the card it runs on.
    ///
    /// Nvidia's encoders are named the card by its number: a picture that comes
    /// down from Vulkan reaches them on the processor, and left to choose they
    /// would take whichever card the driver puts first.
    pub fn encoder_arguments(&self) -> Vec<String> {
        match self.way {
            HardwareAcceleration::Cuda => vec!["-gpu".to_string(), self.address.clone()],
            _ => Vec::new(),
        }
    }

    /// What the encoder is told so that a forced key frame starts a segment
    /// a player can read on its own.
    ///
    /// Nvidia's encoders make a forced key frame a mere complete picture, and
    /// a segment starting on one is a segment nothing can begin with.
    pub fn key_frame_arguments(&self) -> &'static [&'static str] {
        match self.way {
            HardwareAcceleration::Cuda => &["-forced-idr", "1"],
            _ => &[],
        }
    }

    /// How much this card keeps away from the processor, compared between
    /// cards when nobody chose one: converting colour first, since a film
    /// that needs it and is refused goes to the processor whole, then painting
    /// subtitles for the same reason, then how many codecs it writes and
    /// reads.
    fn reach(&self) -> (bool, bool, usize, usize) {
        (
            self.can_tone_map(),
            self.picture_subtitle_layout.is_some(),
            self.encoders.len(),
            self.decoders.len(),
        )
    }

    /// What to put before the input so the tool opens the card.
    ///
    /// When the card reads the film too, that is said here rather than
    /// anywhere else: reading is an option of the input, and an option of the
    /// input placed after it applies to nothing. Converting colour through
    /// Vulkan opens Vulkan on the same card as well, and then it is Vulkan
    /// that reads and filters.
    pub fn opening_arguments(&self, reads_the_film: bool, tone_map: bool) -> Vec<String> {
        self.opening(reads_the_film, self.tone_mapping.filter(|_| tone_map))
    }

    /// The same opening, for one given recipe.
    fn opening(&self, reads_the_film: bool, tone_mapping: Option<ToneMapping>) -> Vec<String> {
        let way = self.way.as_str();
        let mut arguments = vec![
            "-init_hw_device".to_string(),
            format!("{way}={DEVICE_NAME}:{}", self.address),
        ];
        let (reader, device) = match tone_mapping {
            Some(ToneMapping::ThroughVulkan { .. }) => {
                arguments.extend([
                    "-init_hw_device".to_string(),
                    format!("vulkan={VULKAN_DEVICE_NAME}@{DEVICE_NAME}"),
                ]);
                ("vulkan", VULKAN_DEVICE_NAME)
            }
            _ => (way, DEVICE_NAME),
        };
        arguments.extend(["-filter_hw_device".to_string(), device.to_string()]);
        if reads_the_film {
            arguments.extend([
                "-hwaccel".to_string(),
                reader.to_string(),
                // Without this the card decodes and then hands every frame
                // back down to the processor, which is most of the cost of
                // decoding and all of the point of not doing it there.
                "-hwaccel_output_format".to_string(),
                reader.to_string(),
                "-hwaccel_device".to_string(),
                device.to_string(),
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
        self.chain(
            scale_to_height,
            self.tone_mapping.filter(|_| tone_map),
            reads_the_film,
        )
    }

    /// The same chain, converting colour by one given recipe, which is how a
    /// recipe is tried before the card keeps it.
    fn chain(
        &self,
        scale_to_height: Option<i32>,
        tone_mapping: Option<ToneMapping>,
        reads_the_film: bool,
    ) -> Vec<String> {
        // Vulkan takes the picture from wherever it was read, makes it smaller
        // and converts it in one pass, and hands it down at its final size.
        if let Some(ToneMapping::ThroughVulkan { .. }) = tone_mapping {
            let size = scale_to_height
                .map(|height| format!("w=-2:h={height}:"))
                .unwrap_or_default();
            return vec![
                format!("libplacebo={size}{VULKAN_CONVERSION}"),
                "hwdownload".to_string(),
                "format=nv12".to_string(),
            ];
        }

        let way = self.way.as_str();
        let tone_map = tone_mapping.is_some();
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
        match (scale_to_height.filter(|_| self.can_scale), tone_map) {
            // Converting the colours is what leaves the picture in the layout
            // the encoder takes, so the resize before it must not throw away
            // the bits that conversion works from.
            (Some(height), true) => filters.push(format!("scale_{way}=w=-2:h={height}")),
            (Some(height), false) => {
                filters.push(format!("scale_{way}=w=-2:h={height}:format=nv12"))
            }
            // A film the card read itself is handed over in the layout it was
            // written in, and anything worth a card is written with ten bits
            // to a channel where the encoder takes eight. One more pass on the
            // card costs nothing measurable; leaving it out is a refusal in
            // the middle of somebody's film.
            (None, false) if reads_the_film => filters.push(format!("scale_{way}=format=nv12")),
            (None, _) => {}
        }
        if tone_mapping == Some(ToneMapping::OwnFilter) {
            filters.push(format!("tonemap_{way}=format=nv12"));
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
    /// Every card that passed, in the order they were found.
    pub cards: Vec<Card>,
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

    /// The card that rebuilds pictures: the one chosen, when it passed here,
    /// otherwise the one that keeps the most away from the processor, the
    /// first found among equals.
    pub fn card(&self, chosen: Option<&str>) -> Option<&Card> {
        chosen
            .and_then(|key| self.cards.iter().find(|card| card.key == key))
            .or_else(|| self.cards.iter().rev().max_by_key(|card| card.reach()))
    }

    /// Looks for every card and proves what each can do, or explains itself.
    ///
    /// `encoders` is what the tool was built with: there is no point trying a
    /// path the binary does not carry, and saying so is a clearer answer than
    /// a driver failure.
    pub async fn run(ffmpeg: &Path, encoders: &BTreeSet<String>) -> Self {
        let mut search = Self::default();
        let mut found = Vec::new();

        for way in WAYS {
            let cards = cards_on(*way);
            tracing::debug!(
                way = way.as_str(),
                found = ?cards.iter().map(|card| (&card.key, &card.name, &card.address)).collect::<Vec<_>>(),
                "looked for the cards this path drives"
            );
            search
                .devices
                .extend(cards.iter().map(|card| card.device.display().to_string()));
            found.extend(search.what_this_build_drives(*way, cards, encoders));
        }
        if found.is_empty() {
            return search;
        }

        // Made once for every card: it stands in for a real wide gamut film
        // and nothing about it depends on the card it is shown to.
        let wide_gamut = wide_gamut_sample(ffmpeg).await;
        if let Err(said) = &wide_gamut {
            search.trials.push(Trial {
                what: "make_a_wide_gamut_sample".to_string(),
                device: String::new(),
                worked: false,
                said: said.clone(),
            });
        }
        let wide_gamut = wide_gamut.ok();

        for card in found {
            if let Some(card) = search
                .try_this_card(ffmpeg, card, encoders, wide_gamut.as_ref())
                .await
            {
                search.cards.push(card);
            }
        }

        search
    }

    /// The cards of one path, when the tool was built to drive that path.
    ///
    /// Said only of a machine that has such a card: a build without Nvidia's
    /// encoders is no news on a machine without an Nvidia card.
    fn what_this_build_drives(
        &mut self,
        way: HardwareAcceleration,
        cards: Vec<Card>,
        encoders: &BTreeSet<String>,
    ) -> Vec<Card> {
        let floor = encoder_name(THE_FLOOR, way);
        if cards.is_empty() || encoders.contains(&floor) {
            return cards;
        }
        for card in &cards {
            self.trials.push(Trial {
                what: "built_with_the_path".to_string(),
                device: card.device.display().to_string(),
                worked: false,
                said: format!("this build carries no {floor} encoder"),
            });
        }
        Vec::new()
    }

    /// Establishes what one card can do, or nothing when it cannot encode at
    /// all.
    async fn try_this_card(
        &mut self,
        ffmpeg: &Path,
        mut card: Card,
        built_with: &BTreeSet<String>,
        wide_gamut: Option<&Sample>,
    ) -> Option<Card> {
        let named = card.device.display().to_string();

        // Asked first, because it is what tells the two failures apart. A
        // device that will not open is an account that is not allowed to use
        // it; a device that opens and then answers nothing is a driver that is
        // not installed. Both come out of the media tool as the same sentence
        // about no display being found, and they are fixed in entirely
        // different places.
        if let Err(error) = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&card.device)
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

        for codec in WORTH_TRYING {
            let encoder = encoder_name(codec, card.way);
            if !built_with.contains(&encoder) {
                continue;
            }
            let (worked, said) = try_it(
                ffmpeg,
                card.opening_arguments(false, false),
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
                card.encoders.insert((*codec).to_string(), encoder);
            }
        }

        // Without the codec every client reads there would be clients this
        // card could serve nothing to, which is worse than no card at all.
        let floor = card.encoder_for(THE_FLOOR)?.to_string();

        // The remaining trials run the chain a real film will run, rather than
        // something that resembles it: a filter that works on its own and
        // refuses what comes out of the one before it is exactly the failure
        // that only shows up in the middle of somebody's film.
        let chain = card.chain(Some(TRIAL_HEIGHT), None, false).join(",");
        let (can_scale, said) = try_it(
            ffmpeg,
            card.opening_arguments(false, false),
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

        card.tone_mapping = self
            .how_it_converts_colour(ffmpeg, &card, &floor, wide_gamut)
            .await;

        card.picture_subtitle_layout = self.which_layout_it_paints_in(ffmpeg, &card, &floor).await;

        card.decoders = self
            .which_codecs_it_reads(ffmpeg, &card, &floor, wide_gamut, &named, built_with)
            .await;

        Some(card)
    }

    /// Establishes how the card converts wide gamut colour, trying what its
    /// path offers best first and keeping the first that works.
    ///
    /// The one trial that cannot be run on a picture made on the spot: it is
    /// run on the sample, which is a real wide gamut film down to the ten bits
    /// and the screen it says it was graded on.
    async fn how_it_converts_colour(
        &mut self,
        ffmpeg: &Path,
        card: &Card,
        floor: &str,
        wide_gamut: Option<&Sample>,
    ) -> Option<ToneMapping> {
        // Said once already, when the sample could not be made.
        let sample = wide_gamut?;

        for recipe in ToneMapping::offered_on(card.way) {
            let reads = match recipe {
                ToneMapping::ThroughVulkan { reads } => *reads,
                ToneMapping::OwnFilter => false,
            };
            let chain = card.chain(Some(TRIAL_HEIGHT), Some(*recipe), reads).join(",");
            let (worked, said) = try_it(
                ffmpeg,
                card.opening(reads, Some(*recipe)),
                floor,
                TrialInput::File(sample.path()),
                &chain,
            )
            .await;
            self.trials.push(Trial {
                what: recipe.trial_name().to_string(),
                device: card.device.display().to_string(),
                worked,
                said,
            });
            if worked {
                return Some(*recipe);
            }
        }
        None
    }

    /// Establishes whether the card lays a subtitle made of pictures onto a
    /// picture, and in which layout it takes the subtitle.
    ///
    /// Run through the same graph a film will run, so that what is proved is
    /// the placement and the hand-up and not something that resembles them. The
    /// layouts are tried in order and the first that works is kept, each with
    /// what the tool said when it refused.
    async fn which_layout_it_paints_in(
        &mut self,
        ffmpeg: &Path,
        card: &Card,
        floor: &str,
    ) -> Option<String> {
        for layout in painting::layouts(card.way) {
            let (worked, said) =
                match run_briefly(ffmpeg, painting::trial_arguments(card, floor, layout)).await {
                    Ok(outcome) => outcome,
                    Err(said) => (false, said),
                };
            self.trials.push(Trial {
                what: format!("paint_a_picture_subtitle_{layout}"),
                device: card.device.display().to_string(),
                worked,
                said,
            });
            if worked {
                return Some((*layout).to_string());
            }
        }
        None
    }

    /// Establishes which codecs the card reads for itself.
    ///
    /// Reading is asked separately from writing because they are separate
    /// abilities, and the only honest way to ask is to hand the card a film in
    /// that codec and see. Each sample is written first, by a software encoder
    /// of the build where it carries one, since that is the shape a real film
    /// arrives in, and by the card itself otherwise, so the question does not
    /// become "is this build carrying a software encoder for that codec".
    async fn which_codecs_it_reads(
        &mut self,
        ffmpeg: &Path,
        card: &Card,
        floor: &str,
        wide_gamut: Option<&Sample>,
        named: &str,
        built_with: &BTreeSet<String>,
    ) -> BTreeSet<String> {
        let mut reads = BTreeSet::new();

        for codec in WORTH_TRYING {
            // A wide gamut sample carries ten bits to a channel, which is what
            // every film worth the card is, and reading eight proves nothing
            // about reading ten. Where one exists it is the better witness.
            let sample = match (*codec == WIDE_GAMUT_CODEC, wide_gamut) {
                (true, Some(sample)) => Some(Kept::Borrowed(sample)),
                _ => match witness_writer(codec, card, built_with) {
                    Some(writer) => match sample_in(ffmpeg, card, &writer).await {
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

            // Run through the chain a real film in that codec runs: the wide
            // gamut one is a wide gamut film and goes through the conversion,
            // the others through what an ordinary film goes through. A card
            // that reads a film and then offers the frames in a layout the
            // encoder refuses has not read it, as far as a viewer is
            // concerned, and that refusal comes at the end of the chain.
            //
            // What is asked here is what the card's own reader reads, so the
            // wide gamut sample is converted only by a recipe that reader
            // feeds: Vulkan reads for itself, and is proved to with its recipe.
            let tone_map =
                *codec == WIDE_GAMUT_CODEC && card.tone_mapping == Some(ToneMapping::OwnFilter);
            let chain = card.filters_for(None, tone_map, true).join(",");
            let (worked, said) = try_reading(
                ffmpeg,
                card.opening_arguments(true, tone_map),
                floor,
                sample.path(),
                &chain,
            )
            .await;
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
    format!("{codec}_{}", way.encoders_are_called())
}

/// The cards one path can drive, in the order the machine lists them.
fn cards_on(way: HardwareAcceleration) -> Vec<Card> {
    match way {
        HardwareAcceleration::Vaapi => open_interface_cards(),
        HardwareAcceleration::Cuda => nvidia_cards(),
        _ => Vec::new(),
    }
}

/// The cards driven by the open interface: every working device under
/// `/dev/dri` that is not Nvidia's.
fn open_interface_cards() -> Vec<Card> {
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
        .into_iter()
        .filter_map(|device| {
            let file = device.file_name()?.to_string_lossy().into_owned();
            let described = Path::new(DEVICE_DESCRIPTIONS).join(&file).join("device");
            let read = |field: &str| {
                std::fs::read_to_string(described.join(field))
                    .map(|value| value.trim().to_lowercase())
                    .ok()
            };
            let vendor = read("vendor");
            if vendor.as_deref() == Some(NVIDIA) {
                tracing::debug!(
                    device = %device.display(),
                    "an Nvidia card's device is left to the path Nvidia's cards are driven by"
                );
                return None;
            }
            // Where it sits on the machine, read from where its description
            // leads. A machine that does not say keeps the device file, which
            // is the best name left.
            let slot = std::fs::canonicalize(&described)
                .ok()
                .and_then(|path| path.file_name().map(|name| name.to_string_lossy().into_owned()));
            let name = match (&vendor, read("device")) {
                // Two cards of the same maker would otherwise read the same.
                (Some(vendor), Some(model)) => model_named(vendor, &model)
                    .unwrap_or_else(|| format!("{} ({file})", maker_of(vendor))),
                (Some(vendor), None) => format!("{} ({file})", maker_of(vendor)),
                _ => file.clone(),
            };
            let address = device.display().to_string();
            Some(Card::unproved(
                HardwareAcceleration::Vaapi,
                format!("vaapi:{}", slot.unwrap_or_else(|| address.clone())),
                name,
                device,
                address,
            ))
        })
        .collect()
}

/// The cards Nvidia's driver lists, numbered in the order of where they sit
/// on the machine.
///
/// The tool names one of them by that number. The server is started with the
/// compute interface asked to count in that same order, which makes the two
/// agree whatever card the driver would put first on its own.
fn nvidia_cards() -> Vec<Card> {
    let Ok(entries) = std::fs::read_dir(NVIDIA_CARDS) else {
        return Vec::new();
    };
    let mut slots: Vec<String> = entries
        .filter_map(std::result::Result::ok)
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    slots.sort();

    slots
        .into_iter()
        .enumerate()
        .filter_map(|(number, slot)| {
            let information =
                std::fs::read_to_string(Path::new(NVIDIA_CARDS).join(&slot).join("information"))
                    .ok()?;
            let described = nvidia_information(&information);
            Some(Card::unproved(
                HardwareAcceleration::Cuda,
                format!("cuda:{slot}"),
                described.model.unwrap_or_else(|| "NVIDIA".to_string()),
                Path::new(NVIDIA_DEVICES).join(format!("nvidia{}", described.minor.unwrap_or(number))),
                number.to_string(),
            ))
        })
        .collect()
}

/// What Nvidia's driver says of one card.
#[derive(Debug, Default, PartialEq, Eq)]
struct NvidiaInformation {
    model: Option<String>,
    /// The number of its device file.
    minor: Option<usize>,
}

fn nvidia_information(text: &str) -> NvidiaInformation {
    let mut described = NvidiaInformation::default();
    for (field, value) in text.lines().filter_map(|line| line.split_once(':')) {
        let value = value.trim();
        match field.trim() {
            "Model" if !value.is_empty() => described.model = Some(value.to_string()),
            "Device Minor" => described.minor = value.parse().ok(),
            _ => {}
        }
    }
    described
}

/// Who made a card, from its maker number.
fn maker_of(vendor: &str) -> String {
    match vendor {
        "0x8086" => "Intel".to_string(),
        "0x1002" => "AMD".to_string(),
        NVIDIA => "NVIDIA".to_string(),
        other => other.to_string(),
    }
}

/// What the machine's list of hardware names calls one card, when it keeps
/// that list.
fn model_named(vendor: &str, model: &str) -> Option<String> {
    let list = HARDWARE_NAMES
        .iter()
        .find_map(|path| std::fs::read_to_string(path).ok())?;
    model_in(&list, vendor, model).map(|model| format!("{} {model}", maker_of(vendor)))
}

/// Finds a card's model in the list of hardware names.
///
/// The list names each maker on a line of its own, by number, and its models
/// on the lines under it, each set in by one tab. Numbers are written there
/// without the `0x` the kernel puts in front of them.
fn model_in(list: &str, vendor: &str, model: &str) -> Option<String> {
    let vendor = vendor.trim_start_matches("0x");
    let model = model.trim_start_matches("0x");
    let mut under_the_maker = false;
    for line in list.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }
        match line.strip_prefix('\t') {
            None => {
                if under_the_maker {
                    return None;
                }
                under_the_maker = line.split_whitespace().next() == Some(vendor);
            }
            // A line set in twice is a variant of the model above it.
            Some(entry) if under_the_maker && !entry.starts_with('\t') => {
                if let Some((number, name)) = entry.split_once(char::is_whitespace)
                    && number == model
                {
                    return Some(name.trim().to_string());
                }
            }
            Some(_) => {}
        }
    }
    None
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

/// What writes the witness of one codec.
#[derive(Debug, PartialEq, Eq)]
enum WitnessWriter {
    /// A software encoder of the build, at a given speed.
    Software { encoder: String, speed: String },
    /// The card's own encoder.
    Card(String),
}

/// Who writes the witness of one codec: a software encoder of the build where
/// it carries one, otherwise the card, otherwise nobody.
fn witness_writer(
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
async fn sample_in(
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
async fn try_reading(
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
async fn try_it(
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
    use super::*;
    use crate::ToolPaths;

    fn card(can_scale: bool, can_tone_map: bool) -> Card {
        Card {
            encoders: [
                ("h264".to_string(), "h264_vaapi".to_string()),
                ("av1".to_string(), "av1_vaapi".to_string()),
            ]
            .into_iter()
            .collect(),
            decoders: ["hevc".to_string()].into_iter().collect(),
            can_scale,
            tone_mapping: can_tone_map.then_some(ToneMapping::OwnFilter),
            ..Card::unproved(
                HardwareAcceleration::Vaapi,
                "vaapi:0000:03:00.0".to_string(),
                "Intel DG2 [Arc A380]".to_string(),
                PathBuf::from("/dev/dri/renderD128"),
                "/dev/dri/renderD128".to_string(),
            )
        }
    }

    fn nvidia_card() -> Card {
        Card {
            encoders: [
                ("h264".to_string(), "h264_nvenc".to_string()),
                ("hevc".to_string(), "hevc_nvenc".to_string()),
            ]
            .into_iter()
            .collect(),
            decoders: ["h264".to_string(), "hevc".to_string(), "av1".to_string()]
                .into_iter()
                .collect(),
            ..Card::unproved(
                HardwareAcceleration::Cuda,
                "cuda:0000:0c:00.0".to_string(),
                "NVIDIA GeForce RTX 3060".to_string(),
                PathBuf::from("/dev/nvidia0"),
                "0".to_string(),
            )
        }
    }

    #[test]
    fn an_nvidia_card_is_named_by_its_number_and_written_by_its_own_encoders() {
        // The tool names one of these cards by its number, never by a file,
        // and calls its encoders after Nvidia's encoder rather than after the
        // path.
        let card = nvidia_card();
        assert_eq!(
            card.opening_arguments(true, false),
            vec![
                "-init_hw_device".to_string(),
                "cuda=card:0".to_string(),
                "-filter_hw_device".to_string(),
                "card".to_string(),
                "-hwaccel".to_string(),
                "cuda".to_string(),
                "-hwaccel_output_format".to_string(),
                "cuda".to_string(),
                "-hwaccel_device".to_string(),
                "card".to_string(),
            ]
        );
        assert_eq!(encoder_name("hevc", HardwareAcceleration::Cuda), "hevc_nvenc");
        assert_eq!(encoder_name("hevc", HardwareAcceleration::Vaapi), "hevc_vaapi");
    }

    #[test]
    fn an_nvidia_card_makes_a_picture_smaller_with_its_own_filter() {
        assert_eq!(
            nvidia_card().filters_for(Some(1080), false, true),
            vec!["scale_cuda=w=-2:h=1080:format=nv12".to_string()]
        );
        assert_eq!(
            nvidia_card().filters_for(None, false, false),
            vec!["format=nv12".to_string(), "hwupload".to_string()]
        );
    }

    #[test]
    fn a_card_with_no_way_to_convert_colour_is_never_asked_to() {
        // A chain asking for a conversion the card was never proved to make
        // would be refused in the middle of a film.
        let filters = nvidia_card().filters_for(Some(1080), true, true);
        assert!(
            !filters.iter().any(|value| value.contains("tonemap") || value.contains("libplacebo")),
            "{filters:?}"
        );
        assert!(!nvidia_card().can_tone_map());
    }

    fn through_vulkan(reads: bool) -> Card {
        Card {
            tone_mapping: Some(ToneMapping::ThroughVulkan { reads }),
            ..nvidia_card()
        }
    }

    #[test]
    fn an_nvidia_card_converts_colour_through_vulkan_reading_first() {
        // Vulkan reading the film was measured nearly twice as fast as the
        // processor reading it, so it is tried first.
        assert_eq!(
            ToneMapping::offered_on(HardwareAcceleration::Cuda),
            &[
                ToneMapping::ThroughVulkan { reads: true },
                ToneMapping::ThroughVulkan { reads: false },
            ]
        );
    }

    #[test]
    fn through_vulkan_the_film_is_opened_and_read_by_vulkan_on_the_same_card() {
        assert_eq!(
            through_vulkan(true).opening_arguments(true, true),
            vec![
                "-init_hw_device".to_string(),
                "cuda=card:0".to_string(),
                "-init_hw_device".to_string(),
                "vulkan=vk@card".to_string(),
                "-filter_hw_device".to_string(),
                "vk".to_string(),
                "-hwaccel".to_string(),
                "vulkan".to_string(),
                "-hwaccel_output_format".to_string(),
                "vulkan".to_string(),
                "-hwaccel_device".to_string(),
                "vk".to_string(),
            ]
        );
        // A film whose colours are left alone opens the card as before.
        assert_eq!(
            through_vulkan(true).opening_arguments(true, false),
            nvidia_card().opening_arguments(true, false)
        );
    }

    #[test]
    fn through_vulkan_the_picture_comes_down_at_its_final_size() {
        assert_eq!(
            through_vulkan(true).filters_for(Some(1080), true, true),
            vec![
                format!("libplacebo=w=-2:h=1080:{VULKAN_CONVERSION}"),
                "hwdownload".to_string(),
                "format=nv12".to_string(),
            ]
        );
        // Read by the processor, the same chain: Vulkan takes the picture
        // from wherever it was read.
        assert_eq!(
            through_vulkan(false).filters_for(None, true, false),
            vec![
                format!("libplacebo={VULKAN_CONVERSION}"),
                "hwdownload".to_string(),
                "format=nv12".to_string(),
            ]
        );
    }

    #[test]
    fn what_vulkan_reads_is_told_apart_from_what_nvidias_reader_reads() {
        let reading = through_vulkan(true);
        assert!(reading.reads_for("hevc", true), "proved on the wide gamut sample");
        assert!(!reading.reads_for("av1", true), "never proved through Vulkan");
        assert!(reading.reads_for("av1", false), "Nvidia's reader was");
        assert!(!through_vulkan(false).reads_for("hevc", true));
        // The card's own filter is fed by the card's own reader.
        assert!(card(true, true).reads_for("hevc", true));
    }

    #[test]
    fn an_nvidia_encoder_is_named_its_card() {
        assert_eq!(nvidia_card().encoder_arguments(), vec!["-gpu", "0"]);
        assert!(card(true, true).encoder_arguments().is_empty());
    }

    #[test]
    fn a_forced_key_frame_on_an_nvidia_card_starts_a_segment_a_player_can_begin_with() {
        assert_eq!(nvidia_card().key_frame_arguments(), &["-forced-idr", "1"]);
        assert!(card(true, true).key_frame_arguments().is_empty());
    }

    #[test]
    fn the_work_of_an_nvidia_card_is_said_to_be_unknown_rather_than_idle() {
        assert!(!nvidia_card().tallies_its_work());
        assert!(card(true, true).tallies_its_work());
    }

    #[test]
    fn the_chosen_card_does_the_work_when_it_passed_here() {
        let search = CardSearch {
            cards: vec![card(true, true), nvidia_card()],
            ..CardSearch::default()
        };
        assert_eq!(
            search.card(Some("cuda:0000:0c:00.0")).map(|card| card.way),
            Some(HardwareAcceleration::Cuda)
        );
        // A choice kept for a card that is gone is no reason to use none.
        assert_eq!(
            search.card(Some("cuda:0000:01:00.0")).map(|card| card.way),
            Some(HardwareAcceleration::Vaapi)
        );
    }

    #[test]
    fn without_a_choice_the_card_that_keeps_the_most_off_the_processor_works() {
        // Converting colour first: a film that needs it and is refused goes
        // to the processor whole, whatever else the other card does better.
        let search = CardSearch {
            cards: vec![nvidia_card(), card(true, true)],
            ..CardSearch::default()
        };
        assert_eq!(
            search.card(None).map(|card| card.way),
            Some(HardwareAcceleration::Vaapi)
        );

        // Among equals, the first one found.
        let twins = CardSearch {
            cards: vec![
                card(true, false),
                Card {
                    key: "vaapi:0000:04:00.0".to_string(),
                    ..card(true, false)
                },
            ],
            ..CardSearch::default()
        };
        assert_eq!(
            twins.card(None).map(|card| card.key.as_str()),
            Some("vaapi:0000:03:00.0")
        );
        assert!(CardSearch::default().card(None).is_none());
    }

    #[test]
    fn what_nvidias_driver_says_of_a_card_is_read() {
        let information = "Model: \t\t NVIDIA GeForce RTX 3060\n\
             IRQ:   \t\t 140\n\
             Bus Location: \t 0000:0c:00.0\n\
             Device Minor: \t 0\n\
             GPU Excluded:\t No\n";
        assert_eq!(
            nvidia_information(information),
            NvidiaInformation {
                model: Some("NVIDIA GeForce RTX 3060".to_string()),
                minor: Some(0),
            }
        );
        assert_eq!(nvidia_information(""), NvidiaInformation::default());
    }

    #[test]
    fn a_cards_model_is_found_under_its_maker_in_the_list_of_names() {
        let list = "# a comment\n\
             1002  Advanced Micro Devices, Inc. [AMD/ATI]\n\
             \t56a5  Something else\n\
             8086  Intel Corporation\n\
             \t5690  DG2 [Arc A770M]\n\
             \t56a5  DG2 [Arc A380]\n\
             \t\t1234 5678  A variant\n\
             10de  NVIDIA Corporation\n";
        assert_eq!(
            model_in(list, "0x8086", "0x56a5"),
            Some("DG2 [Arc A380]".to_string())
        );
        assert_eq!(model_in(list, "0x8086", "0xffff"), None);
        assert_eq!(model_in(list, "0x1234", "0x56a5"), None);
    }

    #[test]
    fn a_card_says_where_it_is_before_the_film_is_opened() {
        // The tool has to be told about the card before it reads anything: a
        // device named afterwards is a device the filters cannot reach.
        assert_eq!(
            card(true, true).opening_arguments(false, false),
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
    fn a_film_the_card_read_itself_is_put_in_a_layout_the_encoder_takes() {
        // A card hands a film over in the layout it was written in, and
        // anything worth a card is written with ten bits to a channel where
        // the encoder takes eight. Leaving that out is a refusal in the middle
        // of somebody's film, and the trial never saw it because the trial was
        // not running the chain a real film runs.
        assert_eq!(
            card(true, true).filters_for(None, false, true),
            vec!["scale_vaapi=format=nv12".to_string()]
        );

        // Converting the colours already ends in that layout, so nothing is
        // added in front of it.
        assert_eq!(
            card(true, true).filters_for(None, true, true),
            vec!["tonemap_vaapi=format=nv12".to_string()]
        );
    }

    #[test]
    fn a_resize_before_a_conversion_keeps_the_bits_the_conversion_works_from() {
        // Made smaller first, which is the whole point, but not made smaller
        // and flattened first: the conversion starts from those bits.
        let converting = card(true, true).filters_for(Some(1080), true, true);
        assert_eq!(
            converting,
            vec![
                "scale_vaapi=w=-2:h=1080".to_string(),
                "tonemap_vaapi=format=nv12".to_string(),
            ]
        );

        // Nothing converts the colours here, so the resize is what has to
        // leave the picture in the layout the encoder takes.
        assert_eq!(
            card(true, true).filters_for(Some(1080), false, true),
            vec!["scale_vaapi=w=-2:h=1080:format=nv12".to_string()]
        );
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

    #[test]
    fn a_path_this_build_does_not_carry_is_said_plainly_rather_than_tried() {
        let mut search = CardSearch::default();
        let kept = search.what_this_build_drives(
            HardwareAcceleration::Cuda,
            vec![nvidia_card()],
            &["h264_vaapi".to_string()].into_iter().collect(),
        );

        assert!(kept.is_empty());
        assert_eq!(search.trials.len(), 1);
        assert_eq!(search.trials[0].what, "built_with_the_path");
        assert_eq!(search.trials[0].device, "/dev/nvidia0");
        assert!(search.trials[0].said.contains("h264_nvenc"));
    }

    #[test]
    fn a_build_without_a_path_is_no_news_on_a_machine_without_its_cards() {
        let mut search = CardSearch::default();
        let kept =
            search.what_this_build_drives(HardwareAcceleration::Cuda, Vec::new(), &BTreeSet::new());
        assert!(kept.is_empty());
        assert!(search.trials.is_empty(), "{:?}", search.trials);

        let carried = search.what_this_build_drives(
            HardwareAcceleration::Cuda,
            vec![nvidia_card()],
            &["h264_nvenc".to_string()].into_iter().collect(),
        );
        assert_eq!(carried.len(), 1);
        assert!(search.trials.is_empty());
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
        let encoders = ["h264_vaapi".to_string(), "h264_nvenc".to_string()]
            .into_iter()
            .collect();
        let search = CardSearch::run(&tools.ffmpeg, &encoders).await;

        if search.devices.is_empty() {
            assert!(search.cards.is_empty(), "there is no device to have used");
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
