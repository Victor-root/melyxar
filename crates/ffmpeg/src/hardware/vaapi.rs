//! Intel's and AMD's cards, driven by the open interface.
//!
//! Everything the tool is told about one of these cards is decided here, and
//! nothing here is read for any other maker's card: a fix for these cards
//! cannot reach Nvidia's.
//!
//! Wide gamut colour is converted by the card's own filter where the driver
//! has one, which Intel's has and AMD's has not. Otherwise Vulkan opened on the
//! same card reads the film and converts it, or converts what the processor
//! read, and the picture passes to the card's encoder: without leaving the
//! card where that was proved to work and look right, by way of the processor
//! otherwise.

use std::path::{Path, PathBuf};

use super::trials::WIDE_GAMUT_CODEC;
use super::{
    converted_by_vulkan, converted_by_vulkan_and_brought_down, handed_up_and_made_smaller,
    opening_with, vulkan_on_the_card, Card, CardPath, Driver, ToneMapping, WorkTally,
    DEVICE_NAME, VULKAN_DEVICE_NAME,
};
use crate::painting::PAINTED;

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

/// The maker number of Nvidia's cards.
///
/// Their files under `/dev/dri` are never tried through the open interface:
/// it is not how Nvidia's cards encode, and trying would only add refusals to
/// the report.
const NVIDIA: &str = "0x10de";

/// The maker number of AMD's cards.
const AMD: &str = "0x1002";

/// What the machine says of a card it lets fall asleep when idle, in the
/// device's `power/control`. Kept awake, it says `on`.
const SLEEPS_WHEN_IDLE: &str = "auto";

/// Where the machine keeps the names of the hardware it may carry, when it
/// keeps them at all. The first one found is read.
const HARDWARE_NAMES: &[&str] = &["/usr/share/misc/pci.ids", "/usr/share/hwdata/pci.ids"];

/// Where the subtitle lies on the picture.
///
/// As wide as the picture lets it be without changing shape, and never taller
/// than the picture either: the first bound meets a subtitle drawn for a frame
/// as wide as the picture, the second one drawn for a taller frame. Centred
/// across, at the foot.
const PLACEMENT: &str = "w='min(main_w,main_h*overlay_iw/overlay_ih)'\
    :h='min(main_h,main_w*overlay_ih/overlay_iw)'\
    :x='(main_w-w)/2':y='main_h-h'";

/// The open interface.
pub(super) struct Vaapi;

impl Driver for Vaapi {
    fn find(&self) -> Vec<Card> {
        open_interface_cards()
    }

    fn encoders_are_called(&self) -> &'static str {
        "vaapi"
    }

    // The card's own filter first: nothing leaves the card. Then Vulkan
    // reading the film itself, and last Vulkan handed what the processor read,
    // each with the picture passed to the encoder on the card before the same
    // by way of the processor. Handing Vulkan what the card's own reader read
    // without a copy is not offered: tried on an RX 6600, it reset the card,
    // and a trial run at every start must never do that.
    fn recipes(&self) -> &'static [ToneMapping] {
        &[
            ToneMapping::OwnFilter,
            ToneMapping::VulkanOnTheCard { vulkan_reads: true },
            ToneMapping::VulkanBesideTheCard { vulkan_reads: true },
            ToneMapping::VulkanOnTheCard { vulkan_reads: false },
            ToneMapping::VulkanBesideTheCard { vulkan_reads: false },
        ]
    }

    // Converting through Vulkan opens it on the same card, and the filters
    // work on Vulkan: the conversion only takes pictures from its own device,
    // which is why Vulkan is also what reads the film.
    fn opening(
        &self,
        card: &Card,
        reads_the_film: bool,
        recipe: Option<ToneMapping>,
    ) -> Vec<String> {
        match recipe {
            Some(ToneMapping::VulkanBesideTheCard { .. } | ToneMapping::VulkanOnTheCard { .. }) => opening_with(
                card,
                &vulkan_on_the_card(),
                VULKAN_DEVICE_NAME,
                ("vulkan", VULKAN_DEVICE_NAME),
                reads_the_film,
            ),
            _ => opening_with(card, &[], DEVICE_NAME, ("vaapi", DEVICE_NAME), reads_the_film),
        }
    }

    fn chain(
        &self,
        card: &Card,
        scale_to_height: Option<i32>,
        recipe: Option<ToneMapping>,
        reads_the_film: bool,
    ) -> Vec<String> {
        match recipe {
            Some(ToneMapping::VulkanBesideTheCard { .. }) => return through_vulkan(scale_to_height),
            // Handed a picture the processor read, Vulkan converts it and
            // hands it back to the processor, where the card cannot take it
            // without a copy: it is handed up first, with its ten bits.
            Some(ToneMapping::VulkanOnTheCard { .. }) => {
                let mut filters = match reads_the_film {
                    true => Vec::new(),
                    false => vec!["format=p010".to_string(), "hwupload".to_string()],
                };
                filters.push(converted_by_vulkan(scale_to_height));
                return filters;
            }
            _ => {}
        }
        let tone_map = recipe == Some(ToneMapping::OwnFilter);
        let mut filters =
            handed_up_and_made_smaller(card, scale_to_height, tone_map, reads_the_film);
        if tone_map {
            filters.push("tonemap_vaapi=format=nv12".to_string());
        }
        filters
    }

    // The encoder only takes a picture on the card. Beside the card, the
    // picture came down to the processor and is handed up again; on it, the
    // picture passes from Vulkan to the card where it is, and the card's own
    // filter brings it to the layout the encoder takes, in standard range.
    //
    // Mapped in reverse: the card makes the picture and Vulkan draws into it,
    // so it is laid out the way the card reads it. Made by Vulkan and read by
    // the card, a film rebuilt at 3840 by 1600 on an RX 6600 came out as a
    // strip of scattered colour above black, though a trial at that size had
    // come out right. Jellyfin passes it across the same way on AMD's cards.
    fn back_up(&self, recipe: Option<ToneMapping>) -> Option<&'static str> {
        match recipe {
            Some(ToneMapping::VulkanBesideTheCard { .. }) => {
                Some("format=nv12,hwupload=derive_device=vaapi")
            }
            Some(ToneMapping::VulkanOnTheCard { .. }) => Some(
                "hwmap=derive_device=vaapi:reverse=1,format=vaapi,\
                 scale_vaapi=format=nv12:out_color_matrix=bt709:out_range=tv",
            ),
            _ => None,
        }
    }

    // The card's own filter is fed by the card's own reader. Through Vulkan
    // it is Vulkan that reads, and it was proved to on the wide gamut sample
    // alone.
    fn reads_for(&self, card: &Card, codec: &str, recipe: Option<ToneMapping>) -> bool {
        match recipe {
            Some(
                ToneMapping::VulkanBesideTheCard { vulkan_reads }
                | ToneMapping::VulkanOnTheCard { vulkan_reads },
            ) => vulkan_reads && codec == WIDE_GAMUT_CODEC,
            _ => card.reads(codec),
        }
    }

    fn encoder_arguments(&self, _card: &Card) -> Vec<String> {
        Vec::new()
    }

    fn key_frame_arguments(&self) -> &'static [&'static str] {
        &[]
    }

    // AMD's encoder works in blocks of 64 across and 16 down, and the tool
    // before 8.0 sized the picture for blocks of 8: a 1080 line picture came
    // out 1088 lines tall in every key frame, the last 8 shown green.
    fn pads_hevc_without_saying(&self) -> bool {
        true
    }

    fn work_tally(&self, _card: &Card) -> WorkTally {
        WorkTally::PerHandle
    }

    // Measured on an RX 6600: woken from that sleep, Vulkan reading a wide
    // gamut film for the card's encoder ran at a fifteenth of real time and no
    // longer once the card was kept awake. Only AMD's were measured to.
    fn left_to_sleep(&self, card: &Card) -> bool {
        let Some(file) = card.device.file_name() else {
            return false;
        };
        let described = Path::new(DEVICE_DESCRIPTIONS).join(file).join("device");
        let read = |field: &str| {
            std::fs::read_to_string(described.join(field))
                .map(|value| value.trim().to_lowercase())
                .ok()
        };
        sleeps_and_wakes_slowly(read("vendor").as_deref(), read("power/control").as_deref())
    }

    // The first is the one the tool itself draws a subtitle in, so it costs
    // no conversion, and the second is there for a driver that does not take
    // it: which one a card accepts is a thing to establish, not to presume.
    fn subtitle_layouts(&self) -> &'static [&'static str] {
        &["bgra", "rgba"]
    }

    fn sizes_what_it_lays(&self) -> bool {
        true
    }

    // The subtitle is handed up to the card in a layout with an alpha
    // channel, which is what lets the card see through what is not lettering.
    // The picture is already up there. Handed up to the card by name, since
    // the filters may be working on Vulkan.
    fn paint(
        &self,
        picture: &str,
        before: Option<&str>,
        subtitle: i32,
        layout: &str,
        _sized: Option<(i32, i32)>,
    ) -> String {
        format!(
            "{picture}{}[picture];\
             [0:{subtitle}]format={layout},hwupload=derive_device=vaapi[words];\
             [picture][words]overlay_vaapi={PLACEMENT}{PAINTED}",
            before.unwrap_or("null"),
        )
    }
}

/// The chain that converts colour through Vulkan beside the card.
///
/// The conversion takes the picture from wherever it was read, makes it
/// smaller as it converts, and it comes down at its final size to be handed
/// back up to the card. Handing it back up is `back_up`'s.
fn through_vulkan(scale_to_height: Option<i32>) -> Vec<String> {
    converted_by_vulkan_and_brought_down(scale_to_height)
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
                CardPath::Vaapi,
                format!("vaapi:{}", slot.unwrap_or_else(|| address.clone())),
                name,
                device,
                address,
            ))
        })
        .collect()
}

/// Whether a card of this maker, told this about its sleep, is left to fall
/// asleep and come back too slowly.
fn sleeps_and_wakes_slowly(vendor: Option<&str>, power_control: Option<&str>) -> bool {
    vendor == Some(AMD) && power_control == Some(SLEEPS_WHEN_IDLE)
}

/// Who made a card, from its maker number.
fn maker_of(vendor: &str) -> String {
    match vendor {
        "0x8086" => "Intel".to_string(),
        AMD => "AMD".to_string(),
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

#[cfg(test)]
mod tests {
    use super::super::test_cards::intel as card;
    use super::*;

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

    fn through_vulkan(vulkan_reads: bool) -> Card {
        Card {
            tone_mapping: Some(ToneMapping::VulkanBesideTheCard { vulkan_reads }),
            ..card(true, false)
        }
    }

    #[test]
    fn only_an_amd_card_left_to_fall_asleep_is_worth_a_word() {
        assert!(sleeps_and_wakes_slowly(Some(AMD), Some("auto")));
        assert!(!sleeps_and_wakes_slowly(Some(AMD), Some("on")), "kept awake");
        assert!(!sleeps_and_wakes_slowly(Some("0x8086"), Some("auto")), "Intel's");
        assert!(!sleeps_and_wakes_slowly(Some(AMD), None), "nothing said");
        assert!(!sleeps_and_wakes_slowly(None, Some("auto")));
    }

    #[test]
    fn a_card_without_its_own_conversion_tries_vulkan_reading_first() {
        // Kept on the card before brought down, for each way of reading: a
        // card whose picture comes out wrong on the card still converts the
        // way it did before.
        assert_eq!(
            Vaapi.recipes(),
            &[
                ToneMapping::OwnFilter,
                ToneMapping::VulkanOnTheCard { vulkan_reads: true },
                ToneMapping::VulkanBesideTheCard { vulkan_reads: true },
                ToneMapping::VulkanOnTheCard { vulkan_reads: false },
                ToneMapping::VulkanBesideTheCard { vulkan_reads: false },
            ]
        );
    }

    fn on_the_card(vulkan_reads: bool) -> Card {
        Card {
            tone_mapping: Some(ToneMapping::VulkanOnTheCard { vulkan_reads }),
            ..card(true, false)
        }
    }

    #[test]
    fn on_the_card_the_film_is_opened_as_beside_it() {
        for reads in [true, false] {
            assert_eq!(
                on_the_card(true).opening_arguments(reads, true),
                through_vulkan(true).opening_arguments(reads, true)
            );
        }
        assert!(on_the_card(true).reads_for("hevc", true));
        assert!(!on_the_card(true).reads_for("av1", true));
        assert!(!on_the_card(false).reads_for("hevc", true));
    }

    #[test]
    fn on_the_card_the_converted_picture_passes_to_the_encoder_without_coming_down() {
        let passed_on = "hwmap=derive_device=vaapi:reverse=1,format=vaapi,\
             scale_vaapi=format=nv12:out_color_matrix=bt709:out_range=tv"
            .to_string();
        let read_by_vulkan = on_the_card(true).filters_for(Some(1080), true, true);
        assert_eq!(read_by_vulkan, vec![converted_by_vulkan(Some(1080)), passed_on.clone()]);
        // Vulkan hands a picture the processor read back to the processor,
        // where the card cannot map it from: it goes up first.
        let read_by_the_processor = on_the_card(false).filters_for(Some(1080), true, false);
        assert_eq!(
            read_by_the_processor,
            vec![
                "format=p010".to_string(),
                "hwupload".to_string(),
                converted_by_vulkan(Some(1080)),
                passed_on,
            ]
        );
        for filters in [read_by_vulkan, read_by_the_processor] {
            assert!(!filters.iter().any(|value| value.contains("hwdownload")), "{filters:?}");
        }
        // One plane, full range, which is what passes across whole; the
        // card's filter brings it to the encoder's layout in standard range.
        assert!(converted_by_vulkan(None).contains("format=rgba"));
        assert!(converted_by_vulkan(None).contains("range=pc"));
        // A film whose colours are left alone is not touched by any of it.
        assert_eq!(
            on_the_card(true).filters_for(None, false, true),
            card(true, false).filters_for(None, false, true)
        );
    }

    #[test]
    fn on_the_card_a_picture_subtitle_is_laid_by_vulkan() {
        let converting = on_the_card(true);
        assert!(!converting.brings_the_picture_down(true));
        assert!(converting.lays_on_vulkan(true));
        assert!(converting.takes_a_picture_subtitle(true), "AMD lays none of its own");
        assert!(!converting.lays_on_vulkan(false), "the picture is never on Vulkan");
        assert!(!converting.takes_a_picture_subtitle(false));
        assert_eq!(
            converting.filters_until_it_comes_down(Some(720), true, true),
            vec![converted_by_vulkan(Some(720))]
        );
    }

    #[test]
    fn through_vulkan_the_film_is_read_by_vulkan_and_the_filters_work_on_it() {
        assert_eq!(
            through_vulkan(true).opening_arguments(true, true),
            vec![
                "-init_hw_device".to_string(),
                "vaapi=card:/dev/dri/renderD128".to_string(),
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
        // A film whose colours are left alone is read by the card as before.
        assert_eq!(
            through_vulkan(true).opening_arguments(true, false),
            card(true, false).opening_arguments(true, false)
        );
    }

    #[test]
    fn what_vulkan_reads_is_told_apart_from_what_the_card_reads() {
        let reading = through_vulkan(true);
        assert!(reading.reads_for("hevc", true), "proved on the wide gamut sample");
        assert!(!reading.reads_for("av1", true), "never proved through Vulkan");
        assert!(reading.reads_for("hevc", false), "the card's own reader was");
        assert!(!through_vulkan(false).reads_for("hevc", true));
    }

    #[test]
    fn through_vulkan_the_picture_goes_back_to_the_card_at_its_final_size() {
        // Read by Vulkan or by the processor, the same chain: the conversion
        // takes the picture from wherever it was read.
        for reads_the_film in [true, false] {
            let mut expected = converted_by_vulkan_and_brought_down(Some(1080));
            expected.push("format=nv12,hwupload=derive_device=vaapi".to_string());
            assert_eq!(
                through_vulkan(true).filters_for(Some(1080), true, reads_the_film),
                expected
            );
        }
    }

    #[test]
    fn through_vulkan_a_picture_subtitle_is_laid_while_the_picture_is_down() {
        // The card does not lay it, and the picture is on the processor at
        // its final size before it goes back up: the processor lays it there.
        let converting = through_vulkan(true);
        assert!(converting.takes_a_picture_subtitle(true));
        assert!(!converting.takes_a_picture_subtitle(false), "the picture stays up");
        assert_eq!(
            converting.filters_until_it_comes_down(Some(720), true, true),
            converted_by_vulkan_and_brought_down(Some(720))
        );
        assert_eq!(
            converting.back_up(true),
            Some("format=nv12,hwupload=derive_device=vaapi")
        );
        assert_eq!(converting.back_up(false), None);
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
}
