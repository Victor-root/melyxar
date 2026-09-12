//! What a client can actually play.
//!
//! Measured by the client and sent to the server, never guessed from a browser
//! name. That is the whole point: a rule keyed on the browser becomes "this
//! browser, except on this system, except in this version" within a year,
//! whereas a profile stays true because whoever sends it asked the platform
//! itself.
//!
//! It also means a future television client needs no change here at all: it
//! simply sends a different profile.

use serde::{Deserialize, Serialize};

/// A video codec the client can decode, with the limits it holds to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VideoCapability {
    /// Codec name as the analyser spells it.
    pub codec: String,
    /// Profiles accepted. Empty means any profile of that codec.
    #[serde(default)]
    pub profiles: Vec<String>,
    /// Highest level accepted, in the units the analyser reports.
    #[serde(default)]
    pub max_level: Option<i32>,
}

impl VideoCapability {
    pub fn any(codec: impl Into<String>) -> Self {
        Self {
            codec: codec.into(),
            profiles: Vec::new(),
            max_level: None,
        }
    }

    /// Whether this capability covers a stream.
    pub fn covers(&self, codec: &str, profile: Option<&str>, level: Option<i32>) -> bool {
        if !self.codec.eq_ignore_ascii_case(codec) {
            return false;
        }
        if !self.profiles.is_empty() {
            let accepted = profile.is_some_and(|value| {
                self.profiles
                    .iter()
                    .any(|allowed| allowed.eq_ignore_ascii_case(value))
            });
            if !accepted {
                return false;
            }
        }
        match (self.max_level, level) {
            (Some(max), Some(actual)) => actual <= max,
            // An unknown level is accepted: refusing it would transcode
            // perfectly playable files for no reason.
            _ => true,
        }
    }
}

/// Everything a client says it can handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientProfile {
    /// Containers the client can open directly.
    pub containers: Vec<String>,
    pub video: Vec<VideoCapability>,
    /// Audio codec names the client can decode.
    pub audio_codecs: Vec<String>,
    /// Most channels the client will accept. Browsers output stereo.
    #[serde(default)]
    pub max_audio_channels: Option<i32>,
    /// Subtitle formats the client can render itself, alongside the video.
    #[serde(default)]
    pub subtitle_formats: Vec<String>,
    /// Whether the client displays wide gamut colour correctly.
    ///
    /// No browser on Linux does today, so this is false in practice and every
    /// wide gamut film gets converted.
    #[serde(default)]
    pub supports_hdr: bool,
    /// Tallest picture the client will accept.
    #[serde(default)]
    pub max_height: Option<i32>,
    /// Highest overall rate the connection should carry.
    #[serde(default)]
    pub max_bitrate: Option<i64>,
    /// Whether the client can switch tracks inside a file it plays directly.
    ///
    /// Browsers cannot: picking any track other than the one the file marks as
    /// default means the server has to rebuild the stream.
    #[serde(default)]
    pub can_switch_tracks_in_container: bool,
}

impl ClientProfile {
    pub fn supports_container(&self, container: &str) -> bool {
        // The analyser reports a list of names for one container, such as a
        // family sharing a demuxer, so any of them counts as a match.
        container.split(',').any(|name| {
            self.containers
                .iter()
                .any(|known| known.eq_ignore_ascii_case(name.trim()))
        })
    }

    pub fn supports_video(&self, codec: &str, profile: Option<&str>, level: Option<i32>) -> bool {
        self.video
            .iter()
            .any(|capability| capability.covers(codec, profile, level))
    }

    pub fn supports_audio_codec(&self, codec: &str) -> bool {
        self.audio_codecs
            .iter()
            .any(|known| known.eq_ignore_ascii_case(codec))
    }

    pub fn supports_subtitle_format(&self, codec: &str) -> bool {
        self.subtitle_formats
            .iter()
            .any(|known| known.eq_ignore_ascii_case(codec))
    }

    /// A profile close to what a desktop browser reports today.
    ///
    /// Used as a fallback when a client sends nothing, and as the baseline in
    /// tests. Deliberately conservative: a missing profile should lead to a
    /// stream that plays, not to a gamble.
    pub fn conservative_browser() -> Self {
        Self {
            containers: vec!["mp4".into(), "webm".into(), "mov".into()],
            video: vec![
                VideoCapability::any("h264"),
                VideoCapability::any("vp9"),
                VideoCapability::any("av1"),
            ],
            audio_codecs: vec![
                "aac".into(),
                "mp3".into(),
                "opus".into(),
                "vorbis".into(),
                "flac".into(),
            ],
            max_audio_channels: Some(2),
            subtitle_formats: vec!["webvtt".into()],
            supports_hdr: false,
            max_height: None,
            max_bitrate: None,
            can_switch_tracks_in_container: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_capability_without_restrictions_covers_any_profile_and_level() {
        let capability = VideoCapability::any("h264");
        assert!(capability.covers("h264", Some("High"), Some(51)));
        assert!(capability.covers("H264", None, None));
        assert!(!capability.covers("hevc", None, None));
    }

    #[test]
    fn a_restricted_capability_refuses_a_profile_it_does_not_list() {
        let capability = VideoCapability {
            codec: "h264".into(),
            profiles: vec!["Baseline".into(), "Main".into(), "High".into()],
            max_level: Some(51),
        };
        assert!(capability.covers("h264", Some("high"), Some(40)));
        assert!(!capability.covers("h264", Some("High 10"), Some(40)));
    }

    #[test]
    fn a_level_above_the_limit_is_refused() {
        let capability = VideoCapability {
            codec: "h264".into(),
            profiles: Vec::new(),
            max_level: Some(41),
        };
        assert!(capability.covers("h264", None, Some(41)));
        assert!(!capability.covers("h264", None, Some(51)));
    }

    #[test]
    fn an_unknown_level_is_accepted_rather_than_transcoded_for_nothing() {
        let capability = VideoCapability {
            codec: "h264".into(),
            profiles: Vec::new(),
            max_level: Some(41),
        };
        assert!(capability.covers("h264", None, None));
    }

    #[test]
    fn a_container_is_matched_by_name_ignoring_case_and_spacing() {
        let profile = ClientProfile::conservative_browser();
        assert!(profile.supports_container("mp4"));
        assert!(profile.supports_container(" MP4 "));
        assert!(!profile.supports_container("matroska"));
        assert!(!profile.supports_container("avi"));
    }

    #[test]
    fn the_fallback_profile_refuses_what_a_browser_really_cannot_play() {
        let profile = ClientProfile::conservative_browser();
        assert!(!profile.supports_audio_codec("eac3"));
        assert!(!profile.supports_audio_codec("dts"));
        assert!(!profile.supports_audio_codec("truehd"));
        assert!(profile.supports_audio_codec("aac"));
        assert!(!profile.supports_hdr, "no browser shows wide gamut today");
        assert!(
            !profile.can_switch_tracks_in_container,
            "a browser cannot switch tracks inside a file it plays directly"
        );
    }
}
