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

use melyxar_core::media::{Curve, HdrFormat};
use serde::{Deserialize, Serialize};

/// The codec no client has ever refused, and therefore the one produced when
/// nothing better was measured.
pub const ALWAYS_READ: &str = "h264";

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

/// A codec the client takes in a stream fed to it piece by piece, and how far
/// it takes it.
///
/// The height is the whole point. "Do you decode AV1" and "do you decode this
/// film, at this size, as fast as it plays" are different questions, and only
/// the second one decides whether somebody sees a picture. Measured on the
/// machine: the same browser and the same codec answer differently on two
/// computers, and differently again on the same computer with another film.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RebuiltCapability {
    /// Codec name as the analyser spells it.
    pub codec: String,
    /// Tallest picture the client decodes smoothly in this codec.
    ///
    /// Absent means no limit was measured, which is what a client too old to
    /// be asked the question says. Taken as no limit, because that is what it
    /// used to mean and refusing everything would rebuild nothing.
    #[serde(default)]
    pub max_height: Option<i32>,
    /// Whether the browser said this decode was power efficient, at the
    /// height above.
    ///
    /// "Decodes smoothly" and "decodes on a real decoder" are different
    /// questions too: a browser without hardware support for a codec can
    /// still call it smooth by falling back to software on a machine fast
    /// enough to keep up in a synthetic measurement, and still drop pictures
    /// once a real film asks more of it. Absent means the browser never
    /// answered the question, which is taken as neither answer: a codec is
    /// not punished for a browser that simply does not say.
    #[serde(default)]
    pub power_efficient: Option<bool>,
}

impl RebuiltCapability {
    pub fn any(codec: impl Into<String>) -> Self {
        Self {
            codec: codec.into(),
            max_height: None,
            power_efficient: None,
        }
    }

    /// Whether this codec covers a picture of that height.
    pub fn covers(&self, codec: &str, height: Option<i32>) -> bool {
        if !self.codec.eq_ignore_ascii_case(codec) {
            return false;
        }
        match (self.max_height, height) {
            (Some(max), Some(actual)) => actual <= max,
            // A height nobody knows is accepted: a film whose size was never
            // read would otherwise never be rebuilt at all.
            _ => true,
        }
    }

    /// Whether this decode is trusted for an automatic choice: not explicitly
    /// reported inefficient. A browser that never answered gets the benefit
    /// of the doubt, because most do not answer at all yet.
    pub fn efficient(&self) -> bool {
        self.power_efficient != Some(false)
    }
}

/// A codec whose wide gamut picture the client shows as it is, on one curve.
///
/// Asked of the platform codec by codec and curve by curve, because the
/// answer really is that specific: a screen switched to high dynamic range
/// shows it only through a decoder that hands it over as such, and a browser
/// does that for some codecs and not others.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WideGamutCapability {
    /// Codec name as the analyser spells it.
    pub codec: String,
    pub curve: Curve,
}

/// Everything a client says it can handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ClientProfile {
    /// Containers the client can open directly.
    pub containers: Vec<String>,
    pub video: Vec<VideoCapability>,
    /// Codecs the client accepts in a stream the server rebuilds and feeds to
    /// it in pieces.
    ///
    /// A separate question from the one above, and the client has to ask the
    /// platform separately too: a browser that opens a file of one codec does
    /// not always accept the same codec handed to it piece by piece, and the
    /// two are answered by two different parts of it.
    ///
    /// Empty means nothing was measured, and the server then produces the one
    /// codec every client reads.
    #[serde(default)]
    pub rebuilt_video: Vec<RebuiltCapability>,
    /// Audio codec names the client can decode.
    pub audio_codecs: Vec<String>,
    /// Most channels the client will accept. Browsers output stereo.
    #[serde(default)]
    pub max_audio_channels: Option<i32>,
    /// Subtitle formats the client can render itself, alongside the video.
    #[serde(default)]
    pub subtitle_formats: Vec<String>,
    /// The wide gamut pictures the client shows as they are.
    ///
    /// Empty for a screen of standard range, and for a client that says
    /// nothing: a film is then converted, which is the answer that never
    /// shows anybody a washed out picture.
    #[serde(default)]
    pub wide_gamut: Vec<WideGamutCapability>,
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

    /// Whether a stream rebuilt into this codec, at this height, is one the
    /// client will take.
    ///
    /// A client that measured nothing gets the codec every client reads, which
    /// is the only safe answer: producing a codec on a guess and being wrong
    /// is a black screen with no message on it.
    pub fn accepts_rebuilt(&self, codec: &str, height: Option<i32>) -> bool {
        if self.rebuilt_video.is_empty() {
            return codec.eq_ignore_ascii_case(ALWAYS_READ);
        }
        self.rebuilt_video
            .iter()
            .any(|known| known.covers(codec, height))
    }

    /// The tallest picture the client named for a rebuilt codec.
    ///
    /// Only for a codec it named a limit for: nothing for one it never
    /// mentioned, and nothing for one it set no limit on, which needs no
    /// shrinking to be accepted.
    pub fn tallest_rebuilt(&self, codec: &str) -> Option<i32> {
        self.rebuilt_video
            .iter()
            .find(|known| known.codec.eq_ignore_ascii_case(codec))
            .and_then(|known| known.max_height)
    }

    /// Whether this client's decode of a codec is trusted, on its own,
    /// without a height attached.
    ///
    /// The codec every client reads is always trusted: this question exists
    /// to keep an automatic choice from being pushed towards it by a doubtful
    /// report about a newer one, and it would defeat itself if that codec
    /// could fail the very question it is the fallback for. A codec never
    /// measured otherwise gets the same benefit of the doubt, because most
    /// browsers do not answer this question yet. Only an explicit "not
    /// efficient" on a newer codec counts against it here.
    pub fn efficient(&self, codec: &str) -> bool {
        codec.eq_ignore_ascii_case(ALWAYS_READ)
            || self
                .rebuilt_video
                .iter()
                .find(|known| known.codec.eq_ignore_ascii_case(codec))
                .is_none_or(RebuiltCapability::efficient)
    }

    /// Whether an automatic choice may prefer this codec, at this height,
    /// over the one every client reads.
    ///
    /// A stricter question than [`Self::accepts_rebuilt`]: covered as usual,
    /// and not reported inefficient to decode. The point of offering a newer
    /// codec is the bitrate it saves, and that is not saved by a decode the
    /// browser had to fall back to doing in software. A codec asked for
    /// directly skips this question entirely; only the automatic choice asks
    /// it.
    pub fn efficiently_accepts_rebuilt(&self, codec: &str, height: Option<i32>) -> bool {
        self.accepts_rebuilt(codec, height) && self.efficient(codec)
    }

    /// Whether a wide gamut picture of this codec and flavour is shown as it
    /// is: every curve the flavour needs, for that codec.
    pub fn shows_wide_gamut(&self, codec: &str, format: HdrFormat) -> bool {
        format.curves_needed().is_some_and(|curves| {
            curves.iter().all(|curve| {
                self.wide_gamut.iter().any(|shown| {
                    shown.curve == *curve && shown.codec.eq_ignore_ascii_case(codec)
                })
            })
        })
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
            // Only the one every client reads: this stands in for a client
            // that measured nothing, and a guess wrong here is a black screen.
            rebuilt_video: vec![RebuiltCapability::any(ALWAYS_READ)],
            audio_codecs: vec![
                "aac".into(),
                "mp3".into(),
                "opus".into(),
                "vorbis".into(),
                "flac".into(),
            ],
            max_audio_channels: Some(2),
            subtitle_formats: vec!["webvtt".into()],
            wide_gamut: Vec::new(),
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
    fn wide_gamut_is_shown_only_for_the_codec_and_every_curve_it_needs() {
        let mut profile = ClientProfile::conservative_browser();
        profile.wide_gamut = vec![WideGamutCapability {
            codec: "hevc".into(),
            curve: Curve::Pq,
        }];
        assert!(profile.shows_wide_gamut("HEVC", HdrFormat::Hdr10));
        assert!(!profile.shows_wide_gamut("av1", HdrFormat::Hdr10), "another codec");
        assert!(!profile.shows_wide_gamut("hevc", HdrFormat::Hlg), "another curve");
        assert!(
            !profile.shows_wide_gamut("hevc", HdrFormat::DolbyVision { profile: Some(8) }),
            "its base layer may be on the curve not shown"
        );

        profile.wide_gamut.push(WideGamutCapability {
            codec: "hevc".into(),
            curve: Curve::Hlg,
        });
        assert!(profile.shows_wide_gamut("hevc", HdrFormat::DolbyVision { profile: Some(8) }));
        assert!(
            !profile.shows_wide_gamut("hevc", HdrFormat::DolbyVision { profile: Some(5) }),
            "never handed over untouched, whatever the screen"
        );
    }

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
    fn a_client_that_measured_nothing_is_given_the_codec_every_client_reads() {
        // Producing a better codec on a guess and being wrong is a black
        // screen with nothing written on it.
        let silent = ClientProfile {
            rebuilt_video: Vec::new(),
            ..ClientProfile::conservative_browser()
        };
        assert!(silent.accepts_rebuilt("h264", Some(2160)));
        assert!(!silent.accepts_rebuilt("av1", Some(2160)));
        assert!(!silent.accepts_rebuilt("hevc", Some(2160)));
    }

    #[test]
    fn a_client_that_said_what_it_takes_is_taken_at_its_word() {
        let measured = ClientProfile {
            rebuilt_video: vec![
                RebuiltCapability::any("h264"),
                RebuiltCapability::any("hevc"),
                RebuiltCapability::any("av1"),
            ],
            ..ClientProfile::conservative_browser()
        };
        assert!(measured.accepts_rebuilt("av1", Some(2160)));
        assert!(measured.accepts_rebuilt("HEVC", Some(2160)));
        assert!(!measured.accepts_rebuilt("vp9", Some(2160)));
    }

    #[test]
    fn a_client_is_taken_at_its_word_about_how_far_it_goes_as_well() {
        // The whole reason the height is there. Asked whether it decodes AV1,
        // a browser says yes and means somewhere; asked whether it decodes a
        // 4K film in it as fast as it plays, it says no. Only the second
        // question decides whether anybody sees a picture, and a film rebuilt
        // past the answer buffers whole without ever showing a frame.
        let measured = ClientProfile {
            rebuilt_video: vec![
                RebuiltCapability::any("h264"),
                RebuiltCapability {
                    codec: "av1".into(),
                    max_height: Some(1080),
                    power_efficient: None,
                },
            ],
            ..ClientProfile::conservative_browser()
        };

        assert!(measured.accepts_rebuilt("av1", Some(1080)));
        assert!(measured.accepts_rebuilt("av1", Some(720)));
        assert!(
            !measured.accepts_rebuilt("av1", Some(2160)),
            "past what it answered is a picture that never appears"
        );
        assert!(
            measured.accepts_rebuilt("h264", Some(2160)),
            "and the codec it set no limit on goes as far as the film does"
        );
        assert!(
            measured.accepts_rebuilt("av1", None),
            "a film whose size was never read would otherwise never be rebuilt"
        );
    }

    #[test]
    fn a_codec_reported_smooth_but_not_efficient_is_not_trusted_automatically() {
        // A browser without a real decoder for a codec can still call it
        // smooth on a machine fast enough to fall back to software in a
        // synthetic measurement, and still drop pictures once a real film
        // asks more of it. The point of a newer codec is the bitrate it
        // saves, and that is not saved by a software decode.
        let profile = ClientProfile {
            rebuilt_video: vec![
                RebuiltCapability::any("h264"),
                RebuiltCapability {
                    codec: "av1".into(),
                    max_height: Some(2160),
                    power_efficient: Some(false),
                },
            ],
            ..ClientProfile::conservative_browser()
        };

        assert!(
            profile.accepts_rebuilt("av1", Some(2160)),
            "the browser did say it decodes this smoothly"
        );
        assert!(!profile.efficient("av1"));
        assert!(
            !profile.efficiently_accepts_rebuilt("av1", Some(2160)),
            "smooth on its own is not enough for an automatic choice"
        );
    }

    #[test]
    fn a_codec_never_measured_for_efficiency_is_not_punished_for_a_silent_browser() {
        // Most browsers do not answer this question yet, and an automatic
        // choice must not be pushed towards the codec every client reads
        // just because none of them say either way.
        let profile = ClientProfile {
            rebuilt_video: vec![RebuiltCapability::any("hevc")],
            ..ClientProfile::conservative_browser()
        };
        assert!(profile.efficient("hevc"));
        assert!(profile.efficiently_accepts_rebuilt("hevc", Some(2160)));
    }

    #[test]
    fn the_codec_every_client_reads_is_never_refused_for_being_inefficient() {
        // It is the fallback of last resort, and it would defeat itself if it
        // could fail the very question it exists to answer.
        let profile = ClientProfile {
            rebuilt_video: vec![RebuiltCapability {
                codec: "h264".into(),
                max_height: Some(2160),
                power_efficient: Some(false),
            }],
            ..ClientProfile::conservative_browser()
        };
        assert!(profile.efficient("h264"));
        assert!(profile.efficiently_accepts_rebuilt("h264", Some(2160)));
    }

    #[test]
    fn a_profile_is_read_from_what_a_page_really_sends() {
        // The shape on the wire, which is the one thing a type cannot check
        // on its own: a page and a server that disagree about it end up with
        // the server reading no profile at all and rebuilding every film into
        // the safe codec, quietly and for ever.
        let profile: ClientProfile = serde_json::from_str(
            r#"{"containers":["mp4"],"video":[{"codec":"h264"}],
                "rebuilt_video":[{"codec":"h264","max_height":2160},
                                 {"codec":"av1","max_height":1080,"power_efficient":false},
                                 {"codec":"hevc","max_height":null}],
                "audio_codecs":["aac"],"max_audio_channels":2,
                "subtitle_formats":["webvtt"],
                "wide_gamut":[{"codec":"hevc","curve":"pq"}],
                "max_height":null,"max_bitrate":null,
                "can_switch_tracks_in_container":false}"#,
        )
        .expect("what the page sends is what the server reads");

        assert!(profile.accepts_rebuilt("av1", Some(1080)));
        assert!(!profile.accepts_rebuilt("av1", Some(2160)));
        assert!(profile.accepts_rebuilt("h264", Some(2160)));
        assert!(
            profile.accepts_rebuilt("hevc", Some(2160)),
            "a codec the page measured no limit for is not limited"
        );
        assert!(!profile.efficient("av1"), "read from the wire, not assumed");
        assert!(profile.shows_wide_gamut("hevc", HdrFormat::Hdr10));
        assert!(
            profile.efficient("hevc"),
            "a codec the page said nothing about is not punished for it"
        );
    }

    #[test]
    fn the_fallback_profile_refuses_what_a_browser_really_cannot_play() {
        let profile = ClientProfile::conservative_browser();
        assert!(!profile.supports_audio_codec("eac3"));
        assert!(!profile.supports_audio_codec("dts"));
        assert!(!profile.supports_audio_codec("truehd"));
        assert!(profile.supports_audio_codec("aac"));
        assert!(
            profile.wide_gamut.is_empty(),
            "a client that said nothing is not taken for one that shows wide gamut"
        );
        assert!(
            !profile.can_switch_tracks_in_container,
            "a browser cannot switch tracks inside a file it plays directly"
        );
    }
}
