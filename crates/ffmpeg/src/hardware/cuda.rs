//! Nvidia's cards, driven by its compute interface.
//!
//! Everything the tool is told about one of these cards is decided here, and
//! nothing here is read for any other maker's card: a fix for these cards
//! cannot reach Intel's or AMD's.
//!
//! The official tool carries no colour conversion of its own for these cards,
//! so wide gamut colour goes through Vulkan opened on the same card. It cannot
//! hand a picture from Nvidia's reader to Vulkan either, so Vulkan reads such a
//! film itself, or the processor does.

use std::path::Path;

use super::trials::WIDE_GAMUT_CODEC;
use super::{
    converted_by_vulkan, handed_up_and_made_smaller, opening_with, vulkan_on_the_card, Card,
    CardPath, Driver, ToneMapping, WorkTally, DEVICE_NAME, VULKAN_DEVICE_NAME,
};
use crate::painting::{AT_THE_FOOT, PAINTED};

/// Where Nvidia's driver lists its cards, one folder each, named after where
/// the card sits on the machine.
const NVIDIA_CARDS: &str = "/proc/driver/nvidia/gpus";

/// What a card's key starts with, before its slot on the machine.
const KEY_PREFIX: &str = "cuda:";

/// Where Nvidia's device files are, numbered after the cards.
const NVIDIA_DEVICES: &str = "/dev";

/// The layout Nvidia's filter needs the picture in to lay a transparent
/// subtitle on it, which is not the one its encoders are usually handed.
const UNDER_A_TRANSPARENT_SUBTITLE: &str = "yuv420p";

/// Nvidia's compute interface.
pub(super) struct Cuda;

impl Driver for Cuda {
    fn find(&self) -> Vec<Card> {
        nvidia_cards()
    }

    fn encoders_are_called(&self) -> &'static str {
        "nvenc"
    }

    // Vulkan reading the film was measured nearly twice as fast as the
    // processor reading it, so it is tried first.
    fn recipes(&self) -> &'static [ToneMapping] {
        &[
            ToneMapping::ThroughVulkan { reads: true },
            ToneMapping::ThroughVulkan { reads: false },
        ]
    }

    // Converting colour through Vulkan opens Vulkan on the same card as well,
    // and then it is Vulkan that reads and filters.
    fn opening(
        &self,
        card: &Card,
        reads_the_film: bool,
        recipe: Option<ToneMapping>,
    ) -> Vec<String> {
        match recipe {
            Some(ToneMapping::ThroughVulkan { .. }) => opening_with(
                card,
                &vulkan_on_the_card(),
                VULKAN_DEVICE_NAME,
                ("vulkan", VULKAN_DEVICE_NAME),
                reads_the_film,
            ),
            _ => opening_with(card, &[], DEVICE_NAME, ("cuda", DEVICE_NAME), reads_the_film),
        }
    }

    // Vulkan takes the picture from wherever it was read, makes it smaller
    // and converts it in one pass, and hands it down at its final size.
    fn chain(
        &self,
        card: &Card,
        scale_to_height: Option<i32>,
        recipe: Option<ToneMapping>,
        reads_the_film: bool,
    ) -> Vec<String> {
        match recipe {
            Some(ToneMapping::ThroughVulkan { .. }) => vec![
                converted_by_vulkan(scale_to_height),
                "hwdownload".to_string(),
                "format=nv12".to_string(),
            ],
            _ => handed_up_and_made_smaller(card, scale_to_height, false, reads_the_film),
        }
    }

    // Nvidia's encoders take a picture that came down to the processor as it
    // is.
    fn back_up(&self, _recipe: Option<ToneMapping>) -> Option<&'static str> {
        None
    }

    // Through Vulkan it is Vulkan that reads, and it was proved to on the
    // wide gamut sample alone.
    fn reads_for(&self, card: &Card, codec: &str, recipe: Option<ToneMapping>) -> bool {
        match recipe {
            Some(ToneMapping::ThroughVulkan { reads }) => reads && codec == WIDE_GAMUT_CODEC,
            _ => card.reads(codec),
        }
    }

    // Named the card by its number: a picture that comes down from Vulkan
    // reaches the encoder on the processor, and left to choose it would take
    // whichever card the driver puts first.
    fn encoder_arguments(&self, card: &Card) -> Vec<String> {
        vec!["-gpu".to_string(), card.address.clone()]
    }

    // Nvidia's encoders make a forced key frame a mere complete picture, and
    // a segment starting on one is a segment nothing can begin with.
    fn key_frame_arguments(&self) -> &'static [&'static str] {
        &["-forced-idr", "1"]
    }

    // The driver keeps no tally for the work done through the compute
    // interface, its own library is asked instead.
    fn work_tally(&self, card: &Card) -> WorkTally {
        WorkTally::NvidiaLibrary {
            slot: card.key.trim_start_matches(KEY_PREFIX).to_string(),
        }
    }

    // The filter takes transparency in one layout only.
    fn subtitle_layouts(&self) -> &'static [&'static str] {
        &["yuva420p"]
    }

    // It only places: what it lays has to arrive at its final size.
    fn sizes_what_it_lays(&self) -> bool {
        false
    }

    // The subtitle arrives at its size, in the layout that carries
    // transparency, onto a picture put in the one layout that accepts that.
    // The filter is told only where to put it.
    fn paint(
        &self,
        picture: &str,
        before: Option<&str>,
        subtitle: i32,
        layout: &str,
        sized: Option<(i32, i32)>,
    ) -> String {
        let before = before
            .map(|filters| format!("{filters},"))
            .unwrap_or_default();
        let size = sized
            .map(|(width, height)| format!("scale={width}:{height},"))
            .unwrap_or_default();
        format!(
            "{picture}{before}scale_cuda=format={UNDER_A_TRANSPARENT_SUBTITLE}[picture];\
             [0:{subtitle}]{size}format={layout},hwupload[words];\
             [picture][words]overlay_cuda={AT_THE_FOOT}{PAINTED}"
        )
    }
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
                CardPath::Cuda,
                format!("{KEY_PREFIX}{slot}"),
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

#[cfg(test)]
mod tests {
    use super::super::test_cards::{intel as card, nvidia as nvidia_card};
    use super::*;

    fn through_vulkan(reads: bool) -> Card {
        Card {
            tone_mapping: Some(ToneMapping::ThroughVulkan { reads }),
            ..nvidia_card()
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

    #[test]
    fn an_nvidia_card_converts_colour_through_vulkan_reading_first() {
        // Vulkan reading the film was measured nearly twice as fast as the
        // processor reading it, so it is tried first.
        assert_eq!(
            Cuda.recipes(),
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
                converted_by_vulkan(Some(1080)),
                "hwdownload".to_string(),
                "format=nv12".to_string(),
            ]
        );
        // Read by the processor, the same chain: Vulkan takes the picture
        // from wherever it was read.
        assert_eq!(
            through_vulkan(false).filters_for(None, true, false),
            vec![
                converted_by_vulkan(None),
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
    fn the_work_of_an_nvidia_card_is_asked_of_its_library_by_its_slot() {
        assert_eq!(
            nvidia_card().work_tally(),
            WorkTally::NvidiaLibrary {
                slot: "0000:0c:00.0".to_string()
            }
        );
        assert_eq!(card(true, true).work_tally(), WorkTally::PerHandle);
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
}
