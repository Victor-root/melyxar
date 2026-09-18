//! What the installed encoder can actually do.
//!
//! Detected once at start-up by asking the tool itself, never assumed from a
//! version number. Distribution builds differ widely in which encoders and
//! hardware paths they were compiled with, and a decision engine that assumes
//! a capability the binary lacks fails at the worst possible moment, in the
//! middle of someone's film.

use std::collections::BTreeSet;
use std::process::Stdio;

use tokio::process::Command as TokioCommand;

use crate::hardware::{Card, CardSearch};
use crate::{FfmpegError, Result, ToolPaths};

/// A hardware path the encoder was built with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HardwareAcceleration {
    /// The open acceleration interface used by Intel and AMD cards on Linux.
    Vaapi,
    /// Intel's own interface, which the maintainer's card can use.
    QuickSync,
    /// Nvidia's interface.
    Nvenc,
    /// The portable compute interface, occasionally used for filtering.
    OpenCl,
    /// The portable graphics interface, used by some tone mapping filters.
    Vulkan,
}

impl HardwareAcceleration {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vaapi => "vaapi",
            Self::QuickSync => "qsv",
            Self::Nvenc => "nvenc",
            Self::OpenCl => "opencl",
            Self::Vulkan => "vulkan",
        }
    }
}

/// Everything the installed tool reports about itself.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Capabilities {
    /// First line of the version banner, as printed.
    pub version: String,
    pub encoders: BTreeSet<String>,
    pub decoders: BTreeSet<String>,
    pub filters: BTreeSet<String>,
    /// Hardware paths the build carries. What a build carries and what the
    /// machine can actually do are two different questions: the second one is
    /// answered by the search below, by trying.
    pub hardware: BTreeSet<HardwareAcceleration>,
    /// What was found when a card was looked for, whether or not one was.
    pub card_search: CardSearch,
}

impl Capabilities {
    /// Asks the tool what it supports, and the machine what it can do.
    pub async fn detect(tools: &ToolPaths) -> Result<Self> {
        let version = first_line(&run(&tools.ffmpeg, &["-hide_banner", "-version"]).await?);
        let encoders = parse_codec_list(&run(&tools.ffmpeg, &["-hide_banner", "-encoders"]).await?);
        let decoders = parse_codec_list(&run(&tools.ffmpeg, &["-hide_banner", "-decoders"]).await?);
        let filters = parse_filter_list(&run(&tools.ffmpeg, &["-hide_banner", "-filters"]).await?);
        let hardware =
            parse_hardware_list(&run(&tools.ffmpeg, &["-hide_banner", "-hwaccels"]).await?);
        let card_search = CardSearch::run(&tools.ffmpeg, &encoders).await;

        Ok(Self {
            version,
            encoders,
            decoders,
            filters,
            hardware,
            card_search,
        })
    }

    /// The card this machine can rebuild a picture on, when it has one that
    /// was proved to work.
    pub fn card(&self) -> Option<&Card> {
        self.card_search.card.as_ref()
    }

    pub fn has_encoder(&self, name: &str) -> bool {
        self.encoders.contains(name)
    }

    pub fn has_filter(&self, name: &str) -> bool {
        self.filters.contains(name)
    }

    pub fn has_hardware(&self, acceleration: HardwareAcceleration) -> bool {
        self.hardware.contains(&acceleration)
    }

    /// Whether the tool can produce the streams the first version needs.
    ///
    /// Software encoding to the universally readable video codec and to the
    /// matching audio codec is the floor: without it nothing can be served to
    /// a browser that cannot play a file directly.
    pub fn supports_minimum_targets(&self) -> bool {
        self.has_encoder("libx264") && (self.has_encoder("aac") || self.has_encoder("libfdk_aac"))
    }

    /// Name of the audio encoder to use for the browser target.
    pub fn audio_encoder(&self) -> Option<&'static str> {
        if self.has_encoder("libfdk_aac") {
            Some("libfdk_aac")
        } else if self.has_encoder("aac") {
            Some("aac")
        } else {
            None
        }
    }

    /// Filters needed to convert a wide gamut picture to standard range.
    ///
    /// Reported separately because the software path is slow enough to matter,
    /// and because a build lacking it cannot show a wide gamut film with
    /// correct colours at all, thumbnails included.
    pub fn can_tone_map_in_software(&self) -> bool {
        self.has_filter("zscale") || self.has_filter("tonemap")
    }
}

/// Runs a tool and returns its combined output.
async fn run(tool: &std::path::Path, args: &[&str]) -> Result<String> {
    let output = TokioCommand::new(tool)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .await?;

    if !output.status.success() {
        return Err(FfmpegError::from_output("encoder", &output));
    }

    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    if text.trim().is_empty() {
        text = String::from_utf8_lossy(&output.stderr).into_owned();
    }
    Ok(text)
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().trim().to_string()
}

/// Reads a codec listing.
///
/// Each entry is a flags column followed by the name and a description. Only
/// the name matters, and the header block is skipped by requiring a line to
/// have a plausible flags column.
fn parse_codec_list(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim_start();
            // Header and separator lines carry no flags column.
            if trimmed.is_empty() || trimmed.starts_with('-') || trimmed.starts_with("Encoders") {
                return None;
            }
            let mut parts = line.split_whitespace();
            let flags = parts.next()?;
            // A flags column is short and made of markers, never a word.
            if flags.len() > 8 || flags.chars().any(|c| c.is_whitespace()) {
                return None;
            }
            let name = parts.next()?;
            // The listing header uses an equals sign as a legend marker.
            (!name.contains('=')).then(|| name.to_string())
        })
        .collect()
}

/// Reads a filter listing, which has the same shape as a codec listing.
fn parse_filter_list(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let flags = parts.next()?;
            if flags.len() > 5 {
                return None;
            }
            let name = parts.next()?;
            (!name.contains('=') && !name.contains('-')).then(|| name.to_string())
        })
        .collect()
}

/// Reads the hardware listing, which is one plain name per line.
fn parse_hardware_list(text: &str) -> BTreeSet<HardwareAcceleration> {
    text.lines()
        .map(str::trim)
        .filter_map(|line| match line {
            "vaapi" => Some(HardwareAcceleration::Vaapi),
            "qsv" => Some(HardwareAcceleration::QuickSync),
            "cuda" | "nvenc" | "nvdec" => Some(HardwareAcceleration::Nvenc),
            "opencl" => Some(HardwareAcceleration::OpenCl),
            "vulkan" => Some(HardwareAcceleration::Vulkan),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_codec_listing_yields_names_and_skips_its_header() {
        let listing = "Encoders:\n\
             V..... = Video\n\
             ------\n\
             V....D libx264              H.264 encoder\n\
             A....D aac                  AAC encoder\n";
        let names = parse_codec_list(listing);
        assert!(names.contains("libx264"));
        assert!(names.contains("aac"));
        assert!(!names.contains("Video"));
        assert!(!names.contains("Encoders:"));
    }

    #[test]
    fn a_hardware_listing_maps_the_names_we_care_about() {
        let listing = "Hardware acceleration methods:\nvaapi\nqsv\ncuda\nsomethingelse\n";
        let hardware = parse_hardware_list(listing);
        assert!(hardware.contains(&HardwareAcceleration::Vaapi));
        assert!(hardware.contains(&HardwareAcceleration::QuickSync));
        assert!(hardware.contains(&HardwareAcceleration::Nvenc));
        assert_eq!(hardware.len(), 3);
    }

    #[tokio::test]
    async fn the_installed_tool_reports_what_it_can_do() {
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let capabilities = Capabilities::detect(&tools)
            .await
            .expect("capabilities are readable");

        assert!(
            capabilities.version.to_lowercase().contains("ffmpeg"),
            "the version banner should name the tool: {}",
            capabilities.version
        );
        assert!(
            !capabilities.encoders.is_empty(),
            "an encoder listing must not be empty"
        );
        assert!(
            !capabilities.decoders.is_empty(),
            "a decoder listing must not be empty"
        );
        assert!(
            !capabilities.filters.is_empty(),
            "a filter listing must not be empty"
        );
    }

    #[tokio::test]
    async fn the_installed_tool_can_reach_the_targets_the_first_version_needs() {
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let capabilities = Capabilities::detect(&tools)
            .await
            .expect("capabilities are readable");

        assert!(
            capabilities.supports_minimum_targets(),
            "software encoding to the browser targets is the floor"
        );
        assert!(capabilities.audio_encoder().is_some());
    }
}
