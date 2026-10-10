//! Intel's and AMD's cards, driven by the open interface.
//!
//! Everything the tool is told about one of these cards is decided here, and
//! nothing here is read for any other maker's card: a fix for these cards
//! cannot reach Nvidia's.
//!
//! Wide gamut colour is converted by the card's own filter where the driver
//! has one, which Intel's has and AMD's has not. Otherwise the processor reads
//! the film, Vulkan opened on the same card converts it, and the picture goes
//! back to the card for its encoder.

use std::path::{Path, PathBuf};

use super::{
    converted_by_vulkan, handed_up_and_made_smaller, opening_with, vulkan_on_the_card, Card,
    CardPath, Driver, ToneMapping, WorkTally, DEVICE_NAME, VULKAN_DEVICE_NAME,
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

    // The card's own filter first: nothing leaves the card. Then Vulkan,
    // handed what the processor read. Handing Vulkan what the card read
    // without a copy is not offered: tried on an RX 6600, it reset the card,
    // and a trial run at every start must never do that.
    fn recipes(&self) -> &'static [ToneMapping] {
        &[ToneMapping::OwnFilter, ToneMapping::VulkanBesideTheCard]
    }

    // Converting through Vulkan opens it on the same card, and the filters
    // work on Vulkan: the conversion only takes pictures from its own device.
    fn opening(
        &self,
        card: &Card,
        reads_the_film: bool,
        recipe: Option<ToneMapping>,
    ) -> Vec<String> {
        match recipe {
            Some(ToneMapping::VulkanBesideTheCard) => opening_with(
                card,
                &vulkan_on_the_card(),
                VULKAN_DEVICE_NAME,
                ("vaapi", DEVICE_NAME),
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
        if recipe == Some(ToneMapping::VulkanBesideTheCard) {
            return through_vulkan(scale_to_height);
        }
        let tone_map = recipe == Some(ToneMapping::OwnFilter);
        let mut filters =
            handed_up_and_made_smaller(card, scale_to_height, tone_map, reads_the_film);
        if tone_map {
            filters.push("tonemap_vaapi=format=nv12".to_string());
        }
        filters
    }

    // The card's own filter is fed by the card's own reader, and Vulkan by
    // the processor.
    fn reads_for(&self, card: &Card, codec: &str, recipe: Option<ToneMapping>) -> bool {
        recipe != Some(ToneMapping::VulkanBesideTheCard) && card.reads(codec)
    }

    fn encoder_arguments(&self, _card: &Card) -> Vec<String> {
        Vec::new()
    }

    fn key_frame_arguments(&self) -> &'static [&'static str] {
        &[]
    }

    fn work_tally(&self, _card: &Card) -> WorkTally {
        WorkTally::PerHandle
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

/// The chain that converts colour through Vulkan.
///
/// The conversion takes the picture the processor read as it is, makes it
/// smaller as it converts, and it comes down at its final size to be handed
/// back up to the card, since Vulkan cannot hand it back without a copy in the
/// tool as it is published.
fn through_vulkan(scale_to_height: Option<i32>) -> Vec<String> {
    vec![
        converted_by_vulkan(scale_to_height),
        "hwdownload".to_string(),
        "format=nv12".to_string(),
        "hwupload=derive_device=vaapi".to_string(),
    ]
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

    fn through_vulkan() -> Card {
        Card {
            tone_mapping: Some(ToneMapping::VulkanBesideTheCard),
            ..card(true, false)
        }
    }

    #[test]
    fn a_card_without_its_own_conversion_tries_vulkan_after_it() {
        assert_eq!(
            Vaapi.recipes(),
            &[ToneMapping::OwnFilter, ToneMapping::VulkanBesideTheCard]
        );
    }

    #[test]
    fn through_vulkan_the_processor_reads_and_the_filters_work_on_vulkan() {
        // Asked to read a film it was proved to read, the card is still not
        // handed one whose colours Vulkan converts.
        let converting = through_vulkan();
        let reads = converting.reads_for("hevc", true);
        assert!(!reads);
        assert_eq!(
            converting.opening_arguments(reads, true),
            vec![
                "-init_hw_device".to_string(),
                "vaapi=card:/dev/dri/renderD128".to_string(),
                "-init_hw_device".to_string(),
                "vulkan=vk@card".to_string(),
                "-filter_hw_device".to_string(),
                "vk".to_string(),
            ]
        );
        // A film whose colours are left alone is read by the card as before.
        assert!(converting.reads_for("hevc", false));
        assert_eq!(
            converting.opening_arguments(true, false),
            card(true, false).opening_arguments(true, false)
        );
    }

    #[test]
    fn through_vulkan_the_picture_goes_back_to_the_card_at_its_final_size() {
        assert_eq!(
            through_vulkan().filters_for(Some(1080), true, false),
            vec![
                converted_by_vulkan(Some(1080)),
                "hwdownload".to_string(),
                "format=nv12".to_string(),
                "hwupload=derive_device=vaapi".to_string(),
            ]
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
}
