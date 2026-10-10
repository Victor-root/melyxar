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
//!
//! What every card shares lives here and in the files beside it: what a card
//! is, the search that proves it (`search`), and the trials it is put through
//! (`trials`). What one maker's cards need lives in the file of the path they
//! are driven by, and nowhere else: `vaapi` for Intel's and AMD's, `cuda` for
//! Nvidia's. Each answers the same list of questions, the `Driver` trait, so a
//! fix made for one maker's cards is made where nothing of another's can be
//! reached, and a path that forgets a question does not build.

mod cuda;
mod search;
mod trials;
mod vaapi;

pub use search::{CardSearch, Trial};
pub(crate) use trials::{A_GENERATED_PICTURE_SIZE, TRIAL_HEIGHT};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use serde::Serialize;

/// The name the tool gives the card inside one invocation.
const DEVICE_NAME: &str = "card";

/// The name given to the Vulkan device opened on the same card.
const VULKAN_DEVICE_NAME: &str = "vk";

/// What Vulkan is asked to make of a wide gamut picture: standard range, in
/// the layout the encoder takes, converted the way the filter judges best for
/// that film.
const VULKAN_CONVERSION: &str = "format=nv12:colorspace=bt709:color_primaries=bt709\
    :color_trc=bt709:range=tv:tonemapping=auto";

/// The path a card is driven by, which decides everything the tool is told
/// about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CardPath {
    /// The open interface Intel's and AMD's cards are driven by.
    Vaapi,
    /// Nvidia's compute interface.
    Cuda,
}

impl CardPath {
    /// Every path, in the order their cards are tried.
    const ALL: [Self; 2] = [Self::Vaapi, Self::Cuda];

    /// The name the tool knows the path, its devices and its filters by.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vaapi => "vaapi",
            Self::Cuda => "cuda",
        }
    }

    /// Everything this path knows, kept in the one file that knows it.
    pub(crate) fn driver(self) -> &'static dyn Driver {
        match self {
            Self::Vaapi => &vaapi::Vaapi,
            Self::Cuda => &cuda::Cuda,
        }
    }
}

/// What one path knows about the cards it drives.
pub(crate) trait Driver: Sync {
    /// The cards of this machine the path drives, in the order the machine
    /// lists them, not yet asked anything.
    fn find(&self) -> Vec<Card>;

    /// What the tool calls this path's encoders, after the codec.
    fn encoders_are_called(&self) -> &'static str;

    /// The ways this path converts wide gamut colour, best first.
    fn recipes(&self) -> &'static [ToneMapping];

    /// What to put before the input so the tool opens the card, and reads
    /// the film on it when it does, colours converted by `recipe` when they
    /// are.
    ///
    /// Reading is an option of the input, and an option of the input placed
    /// after it applies to nothing.
    fn opening(&self, card: &Card, reads_the_film: bool, recipe: Option<ToneMapping>)
        -> Vec<String>;

    /// The filters the picture goes through, colours converted by `recipe`
    /// when they are.
    ///
    /// Where the picture comes from changes the chain entirely. A film the card
    /// read itself is already up there, and asking to hand it up again is how a
    /// card refuses a film it was reading perfectly well. A film the processor
    /// read has to be handed up first, and in the layout the work needs.
    fn chain(
        &self,
        card: &Card,
        scale_to_height: Option<i32>,
        recipe: Option<ToneMapping>,
        reads_the_film: bool,
    ) -> Vec<String>;

    /// What hands the picture back up to the card once it came down to the
    /// processor, for a recipe after which the encoder wants it on the card.
    /// It goes last, after a subtitle the processor lays on the way.
    fn back_up(&self, recipe: Option<ToneMapping>) -> Option<&'static str>;

    /// Whether the card reads a film in one codec for itself, colours
    /// converted by `recipe` when they are.
    fn reads_for(&self, card: &Card, codec: &str, recipe: Option<ToneMapping>) -> bool;

    /// What the encoder is told about the card it runs on.
    fn encoder_arguments(&self, card: &Card) -> Vec<String>;

    /// What the encoder is told so that a forced key frame starts a segment a
    /// player can read on its own.
    fn key_frame_arguments(&self) -> &'static [&'static str];

    /// Where the server reads how busy the card is.
    fn work_tally(&self, card: &Card) -> WorkTally;

    /// The layouts a subtitle made of pictures is offered to the card in, in
    /// the order they are tried.
    fn subtitle_layouts(&self) -> &'static [&'static str];

    /// Whether the card's filter brings a subtitle to the size of the picture
    /// itself. When it does not, the server works the size out and the
    /// subtitle arrives at it.
    fn sizes_what_it_lays(&self) -> bool;

    /// The filter graph that lays a subtitle onto the picture on the card.
    ///
    /// `before` is everything done to the picture first: painting the words on
    /// and then shrinking would shrink the words with it.
    fn paint(
        &self,
        picture: &str,
        before: Option<&str>,
        subtitle: i32,
        layout: &str,
        sized: Option<(i32, i32)>,
    ) -> String;
}

/// Where the server reads how busy a card is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkTally {
    /// The tally the driver keeps for each program that has the card open.
    PerHandle,
    /// NVIDIA's management library, asked about the card at this slot on the
    /// machine.
    NvidiaLibrary { slot: String },
}

/// How a card converts wide gamut colour to standard range.
///
/// A recipe rather than a yes or a no, because the paths differ in more than
/// the name of a filter: which recipes a path offers, and in which order, is
/// its own business. The first one the card is proved to run is the one it
/// keeps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ToneMapping {
    /// The card's own conversion filter, fed by the card's own reader.
    OwnFilter,
    /// Vulkan on the same card converts, and the picture comes down to the
    /// processor at its final size for the encoder. Vulkan reads the film
    /// itself when it was proved to read a wide gamut one, and the processor
    /// reads it otherwise.
    ThroughVulkan { reads: bool },
    /// Vulkan on the same card converts, and the picture goes back up to the
    /// card for its encoder. Vulkan reads the film itself when it was proved
    /// to read a wide gamut one, and the processor reads it otherwise.
    VulkanBesideTheCard { vulkan_reads: bool },
}

impl ToneMapping {
    /// What the trial of this recipe is called in a report.
    fn trial_name(self) -> &'static str {
        match self {
            Self::OwnFilter => "convert_wide_gamut",
            Self::ThroughVulkan { reads: true } => "convert_wide_gamut_vulkan_reading",
            Self::ThroughVulkan { reads: false } => "convert_wide_gamut_vulkan",
            Self::VulkanBesideTheCard { vulkan_reads: true } => {
                "convert_wide_gamut_vulkan_reading_beside_the_card"
            }
            Self::VulkanBesideTheCard { vulkan_reads: false } => {
                "convert_wide_gamut_vulkan_beside_the_card"
            }
        }
    }

    /// Whether the card reads the sample in the trial of this recipe.
    fn read_in_its_trial(self) -> bool {
        match self {
            Self::OwnFilter => false,
            Self::ThroughVulkan { reads } => reads,
            Self::VulkanBesideTheCard { vulkan_reads } => vulkan_reads,
        }
    }

    /// Whether this recipe converts what the card's own reader reads, which is
    /// what a trial of that reader then runs through.
    fn fed_by_the_cards_own_reader(self) -> bool {
        matches!(self, Self::OwnFilter)
    }

    /// Whether the picture leaves the card once converted.
    pub fn brings_the_picture_down(self) -> bool {
        matches!(
            self,
            Self::ThroughVulkan { .. } | Self::VulkanBesideTheCard { .. }
        )
    }
}

/// A card this machine can really rebuild a picture on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Card {
    pub way: CardPath,
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
        way: CardPath,
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

    /// Whether the picture comes down to the processor once its colours are
    /// converted, which is where a subtitle is then laid on it.
    pub fn brings_the_picture_down(&self, tone_map: bool) -> bool {
        tone_map
            && self
                .tone_mapping
                .is_some_and(ToneMapping::brings_the_picture_down)
    }

    /// Whether a film with a subtitle made of pictures can be rebuilt here:
    /// the card lays it when it was proved to, and the processor does when
    /// the picture comes down to it anyway, at its final size, which costs
    /// next to nothing.
    pub fn takes_a_picture_subtitle(&self, tone_map: bool) -> bool {
        self.picture_subtitle_layout.is_some() || self.brings_the_picture_down(tone_map)
    }

    /// The encoder that produces one codec here, when this card produces it.
    pub fn encoder_for(&self, codec: &str) -> Option<&str> {
        self.encoders.get(codec).map(String::as_str)
    }

    /// Whether the card's own reader was proved to read one codec.
    pub fn reads(&self, codec: &str) -> bool {
        self.decoders.contains(codec)
    }

    /// Whether the card reads a film in one codec for itself, given whether
    /// its colours are converted: the recipe that converts them can read it
    /// with something other than the card's own reader.
    pub fn reads_for(&self, codec: &str, tone_map: bool) -> bool {
        self.way
            .driver()
            .reads_for(self, codec, self.tone_mapping.filter(|_| tone_map))
    }

    /// Where the server reads how busy the card is.
    pub fn work_tally(&self) -> WorkTally {
        self.way.driver().work_tally(self)
    }

    /// What the encoder is told about the card it runs on.
    pub fn encoder_arguments(&self) -> Vec<String> {
        self.way.driver().encoder_arguments(self)
    }

    /// What the encoder is told so that a forced key frame starts a segment a
    /// player can read on its own.
    pub fn key_frame_arguments(&self) -> &'static [&'static str] {
        self.way.driver().key_frame_arguments()
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

    /// What to put before the input so the tool opens the card, and reads the
    /// film on it when it does.
    pub fn opening_arguments(&self, reads_the_film: bool, tone_map: bool) -> Vec<String> {
        self.opening(reads_the_film, self.tone_mapping.filter(|_| tone_map))
    }

    /// The same opening, for one given recipe, which is how a recipe is tried
    /// before the card keeps it.
    fn opening(&self, reads_the_film: bool, recipe: Option<ToneMapping>) -> Vec<String> {
        self.way.driver().opening(self, reads_the_film, recipe)
    }

    /// The filters the picture goes through on the card.
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

    /// The same filters, stopped where the picture comes down to the
    /// processor: what a subtitle the processor lays is laid after.
    pub fn filters_until_it_comes_down(
        &self,
        scale_to_height: Option<i32>,
        tone_map: bool,
        reads_the_film: bool,
    ) -> Vec<String> {
        self.way.driver().chain(
            self,
            scale_to_height,
            self.tone_mapping.filter(|_| tone_map),
            reads_the_film,
        )
    }

    /// What hands the picture back up to the card after it came down, when
    /// the encoder wants it there.
    pub fn back_up(&self, tone_map: bool) -> Option<&'static str> {
        self.way
            .driver()
            .back_up(self.tone_mapping.filter(|_| tone_map))
    }

    /// The same chain, for one given recipe, handed back up to the card at
    /// the end when the recipe brought it down.
    fn chain(
        &self,
        scale_to_height: Option<i32>,
        recipe: Option<ToneMapping>,
        reads_the_film: bool,
    ) -> Vec<String> {
        let driver = self.way.driver();
        let mut filters = driver.chain(self, scale_to_height, recipe, reads_the_film);
        filters.extend(driver.back_up(recipe).map(str::to_string));
        filters
    }
}

/// What the encoder of one codec is called on one path.
fn encoder_name(codec: &str, way: CardPath) -> String {
    format!("{codec}_{}", way.driver().encoders_are_called())
}

/// The card opened under its name and, when it reads the film, the reader
/// the film is read with: the part of an opening every path shares.
///
/// `also` is what a path opens beside the card, `filters_on` the device the
/// filters work on, and `reads_with` the reader and the device it reads on,
/// which are the card's own unless the path says otherwise.
fn opening_with(
    card: &Card,
    also: &[String],
    filters_on: &str,
    reads_with: (&str, &str),
    reads_the_film: bool,
) -> Vec<String> {
    let (reader, device) = reads_with;
    let mut arguments = vec![
        "-init_hw_device".to_string(),
        format!("{}={DEVICE_NAME}:{}", card.way.as_str(), card.address),
    ];
    arguments.extend_from_slice(also);
    arguments.extend(["-filter_hw_device".to_string(), filters_on.to_string()]);
    if reads_the_film {
        arguments.extend([
            "-hwaccel".to_string(),
            reader.to_string(),
            // Without this the card decodes and then hands every frame back
            // down to the processor, which is most of the cost of decoding and
            // all of the point of not doing it there.
            "-hwaccel_output_format".to_string(),
            reader.to_string(),
            "-hwaccel_device".to_string(),
            device.to_string(),
        ]);
    }
    arguments
}

/// Vulkan opened on the card already opened, chosen by the tool as the same
/// card rather than whichever the machine lists first.
fn vulkan_on_the_card() -> Vec<String> {
    vec![
        "-init_hw_device".to_string(),
        format!("vulkan={VULKAN_DEVICE_NAME}@{DEVICE_NAME}"),
    ]
}

/// Vulkan making the picture smaller and converting its colours in one pass.
fn converted_by_vulkan(scale_to_height: Option<i32>) -> String {
    let size = scale_to_height
        .map(|height| format!("w=-2:h={height}:"))
        .unwrap_or_default();
    format!("libplacebo={size}{VULKAN_CONVERSION}")
}

/// The part of a chain every path shares: the picture handed up when the
/// processor read it, and made smaller on the card by its own filter.
///
/// `keeps_ten_bits` is for a picture whose colours are converted next on the
/// card: the conversion starts from those bits.
fn handed_up_and_made_smaller(
    card: &Card,
    scale_to_height: Option<i32>,
    keeps_ten_bits: bool,
    reads_the_film: bool,
) -> Vec<String> {
    let way = card.way.as_str();
    let mut filters = Vec::new();

    if !reads_the_film {
        // Wide gamut colour arrives with ten bits to a channel, and handing
        // it up as eight would throw away exactly what is about to be
        // converted.
        filters.push(
            match keeps_ten_bits {
                true => "format=p010",
                false => "format=nv12",
            }
            .to_string(),
        );
        filters.push("hwupload".to_string());
    }

    // Made smaller first for the same reason as in software: converting
    // colours is the most expensive thing done to a picture, so doing it on
    // fewer pixels costs less.
    match (scale_to_height.filter(|_| card.can_scale), keeps_ten_bits) {
        // Converting the colours is what leaves the picture in the layout the
        // encoder takes, so the resize before it must not throw away the bits
        // that conversion works from.
        (Some(height), true) => filters.push(format!("scale_{way}=w=-2:h={height}")),
        (Some(height), false) => filters.push(format!("scale_{way}=w=-2:h={height}:format=nv12")),
        // A film the card read itself is handed over in the layout it was
        // written in, and anything worth a card is written with ten bits to a
        // channel where the encoder takes eight. One more pass on the card
        // costs nothing measurable; leaving it out is a refusal in the middle
        // of somebody's film.
        (None, false) if reads_the_film => filters.push(format!("scale_{way}=format=nv12")),
        (None, _) => {}
    }

    filters
}

/// Cards the tests of every file of this module are run against.
#[cfg(test)]
pub(crate) mod test_cards {
    use super::*;

    /// An Intel card, with a say in what it was proved able to do.
    pub(crate) fn intel(can_scale: bool, can_tone_map: bool) -> Card {
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
                CardPath::Vaapi,
                "vaapi:0000:03:00.0".to_string(),
                "Intel DG2 [Arc A380]".to_string(),
                PathBuf::from("/dev/dri/renderD128"),
                "/dev/dri/renderD128".to_string(),
            )
        }
    }

    /// An Nvidia card that was proved to do what an RTX 3060 does, but convert
    /// colour.
    pub(crate) fn nvidia() -> Card {
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
                CardPath::Cuda,
                "cuda:0000:0c:00.0".to_string(),
                "NVIDIA GeForce RTX 3060".to_string(),
                PathBuf::from("/dev/nvidia0"),
                "0".to_string(),
            )
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_cards::intel as card;
    use super::*;

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
    fn every_path_names_its_encoders_its_own_way() {
        assert_eq!(encoder_name("hevc", CardPath::Cuda), "hevc_nvenc");
        assert_eq!(encoder_name("hevc", CardPath::Vaapi), "hevc_vaapi");
    }
}
