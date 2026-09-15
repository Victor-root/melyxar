//! Media sources and their tracks.
//!
//! A media source is one physical file. A track is one stream inside it. The
//! level of detail kept here matches what the detail page shows, and the
//! colour fields in particular are not decoration: they decide whether an
//! image or a stream has to be converted to standard dynamic range.

use std::path::PathBuf;

use crate::id::{LibraryRootId, MediaSourceId, TrackId, WorkId};
use crate::time::{Millis, Timestamp};

/// How a file is recognised again after being renamed or moved.
///
/// Size and modification time are cheap and cover the ordinary case. A short
/// content fingerprint is computed only when those two are ambiguous, because
/// reading bytes from a slow network share is expensive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileIdentity {
    pub size_bytes: i64,
    pub modified_at: Timestamp,
    /// Fingerprint of the first megabytes, when one was computed.
    pub content_fingerprint: Option<String>,
}

/// One physical file backing a work.
#[derive(Debug, Clone, PartialEq)]
pub struct MediaSource {
    pub id: MediaSourceId,
    pub work_id: WorkId,
    pub root_id: LibraryRootId,
    /// Path relative to the root, so that moving a library to another disk is
    /// a change of root rather than a full reindex.
    pub relative_path: PathBuf,
    pub container: Option<String>,
    pub duration: Option<Millis>,
    pub overall_bitrate: Option<i64>,
    pub identity: FileIdentity,
    pub added_at: Timestamp,
    pub tracks: Vec<Track>,
}

impl MediaSource {
    /// The file name, used for display and for title parsing.
    pub fn file_name(&self) -> &str {
        self.relative_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default()
    }

    pub fn video_tracks(&self) -> impl Iterator<Item = (&Track, &VideoDetails)> {
        self.tracks.iter().filter_map(|track| match &track.kind {
            TrackKind::Video(details) => Some((track, details)),
            _ => None,
        })
    }

    pub fn audio_tracks(&self) -> impl Iterator<Item = (&Track, &AudioDetails)> {
        self.tracks.iter().filter_map(|track| match &track.kind {
            TrackKind::Audio(details) => Some((track, details)),
            _ => None,
        })
    }

    pub fn subtitle_tracks(&self) -> impl Iterator<Item = (&Track, &SubtitleDetails)> {
        self.tracks.iter().filter_map(|track| match &track.kind {
            TrackKind::Subtitle(details) => Some((track, details)),
            _ => None,
        })
    }

    /// The video track a player would use: the one marked default, otherwise
    /// the first one present.
    pub fn primary_video(&self) -> Option<(&Track, &VideoDetails)> {
        self.video_tracks()
            .find(|(track, _)| track.is_default)
            .or_else(|| self.video_tracks().next())
    }
}

/// Fields shared by every kind of track.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub id: TrackId,
    pub source_id: MediaSourceId,
    /// Index of the stream inside the container, as reported by the analyser.
    /// This is what gets passed to the processing tool to select the stream.
    pub stream_index: i32,
    /// Two or three letter language code, when the file declares one.
    pub language: Option<String>,
    /// Human readable title carried by the file.
    pub title: Option<String>,
    pub is_default: bool,
    pub is_forced: bool,
    pub kind: TrackKind,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TrackKind {
    Video(VideoDetails),
    Audio(AudioDetails),
    Subtitle(SubtitleDetails),
}

impl TrackKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Video(_) => "video",
            Self::Audio(_) => "audio",
            Self::Subtitle(_) => "subtitle",
        }
    }
}

/// Normalises a language tag to its three letter lowercase form where we can.
///
/// Containers and file names disagree wildly here, and a track whose language
/// is not recognised is a track a viewer cannot find in the picker. The rule
/// lives here because it has to give the same answer whether the tag came out
/// of a container or off the end of a file name.
pub fn normalise_language(value: &str) -> String {
    let lowered = value.trim().to_lowercase();
    let base = lowered
        .split(['-', '_'])
        .next()
        .unwrap_or(&lowered)
        .to_string();
    match base.as_str() {
        "fr" => "fre".to_string(),
        "en" => "eng".to_string(),
        "de" => "ger".to_string(),
        "es" => "spa".to_string(),
        "it" => "ita".to_string(),
        "ja" => "jpn".to_string(),
        "nl" => "dut".to_string(),
        "pt" => "por".to_string(),
        "ru" => "rus".to_string(),
        "zh" => "chi".to_string(),
        // Both three letter codes exist for several languages; keeping one of
        // each is what stops a picker from showing the same language twice.
        "fra" => "fre".to_string(),
        "deu" => "ger".to_string(),
        "nld" => "dut".to_string(),
        "zho" => "chi".to_string(),
        other => other.to_string(),
    }
}

/// One chapter of a source, either read from the file or generated at a fixed
/// interval when the file declares none.
///
/// Chapters are always addressed as the ordered list belonging to one source,
/// never one by one, so the identifier the storage gives them stays a storage
/// matter and does not appear here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chapter {
    pub ordinal: i32,
    pub start: Millis,
    pub title: Option<String>,
    /// Generated thumbnail, relative to the image cache. Always converted to
    /// standard range when the source is wide gamut, otherwise it comes out
    /// washed out and grey.
    pub thumbnail_path: Option<PathBuf>,
}

/// Colour description of a video track.
///
/// These four fields together say whether the track carries high dynamic
/// range. Without them, an image pulled out of a wide gamut file looks washed
/// out and grey, which is the defect seen on other media servers.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColorInfo {
    /// Colour primaries, for example the wide gamut used by high dynamic range.
    pub primaries: Option<String>,
    /// Matrix coefficients.
    pub space: Option<String>,
    /// Transfer characteristics. This is the field that identifies the two
    /// high dynamic range curves.
    pub transfer: Option<String>,
    /// Bits per sample, ten or more for high dynamic range.
    pub bit_depth: Option<i32>,
}

/// The high dynamic range flavour carried by a video track.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HdrFormat {
    /// Perceptual quantiser curve, the most common flavour.
    Hdr10,
    /// Hybrid log gamma, used by broadcasters.
    Hlg,
    /// Dolby Vision. Profile five carries no compatible base layer, so played
    /// as is it shows green and purple rather than a picture.
    DolbyVision { profile: Option<i32> },
}

impl HdrFormat {
    /// Whether the stream needs converting before a browser can show it with
    /// correct colours.
    ///
    /// Every flavour does: no browser on Linux displays high dynamic range
    /// properly today.
    pub fn needs_tone_mapping(self) -> bool {
        true
    }

    /// Dolby Vision profile five is the one that looks broken rather than
    /// merely washed out when played untouched.
    pub fn is_incompatible_without_conversion(self) -> bool {
        matches!(self, Self::DolbyVision { profile: Some(5) })
    }
}

/// How much of each edge of a frame is not part of the picture.
///
/// A film can carry its picture inside a larger frame and say, beside the
/// picture rather than inside it, how much of each edge to leave out. The size
/// such a film announces is the frame; the picture is what is left once these
/// are taken off, and the two are different shapes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Margins {
    pub top: i32,
    pub bottom: i32,
    pub left: i32,
    pub right: i32,
}

impl Margins {
    /// Whether they take nothing off, which is the same as having none.
    pub fn are_nothing(&self) -> bool {
        self.top == 0 && self.bottom == 0 && self.left == 0 && self.right == 0
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoDetails {
    pub codec: String,
    pub profile: Option<String>,
    pub level: Option<i32>,
    /// The frame the film carries, margins included.
    ///
    /// Kept as the film states it. What anything showing or rebuilding a
    /// picture wants is the picture, which is `visible_width` and
    /// `visible_height`.
    pub width: i32,
    pub height: i32,
    /// Edges the film says are not part of its picture, when it says so.
    pub margins: Option<Margins>,
    pub aspect_ratio: Option<String>,
    pub is_interlaced: bool,
    pub frame_rate: Option<f64>,
    pub bitrate: Option<i64>,
    pub pixel_format: Option<String>,
    pub reference_frames: Option<i32>,
    pub color: ColorInfo,
    pub hdr: Option<HdrFormat>,
}

impl VideoDetails {
    /// How wide the picture is, once the margins are off.
    pub fn visible_width(&self) -> i32 {
        self.without_margins().0
    }

    /// How tall the picture is, once the margins are off.
    pub fn visible_height(&self) -> i32 {
        self.without_margins().1
    }

    /// The shape of the picture rather than of the frame around it.
    ///
    /// Margins that would leave nothing at all are treated as none: a film
    /// that describes itself into nonsense is still a film, and the frame is
    /// then the best answer anybody has.
    fn without_margins(&self) -> (i32, i32) {
        let Some(margins) = self.margins else {
            return (self.width, self.height);
        };
        let width = self.width - margins.left - margins.right;
        let height = self.height - margins.top - margins.bottom;
        match width > 0 && height > 0 {
            true => (width, height),
            false => (self.width, self.height),
        }
    }

    /// Whether any image taken out of this track has to be converted before
    /// it looks right, thumbnails included.
    pub fn needs_tone_mapping(&self) -> bool {
        self.hdr.is_some_and(HdrFormat::needs_tone_mapping)
    }

    /// Short human readable summary, of the kind shown above the play button.
    pub fn summary(&self) -> String {
        let definition = match self.visible_height() {
            h if h >= 2000 => "4K",
            h if h >= 1400 => "1440p",
            h if h >= 1000 => "1080p",
            h if h >= 700 => "720p",
            _ => "SD",
        };
        let mut parts = vec![definition.to_string()];
        if let Some(hdr) = self.hdr {
            parts.push(
                match hdr {
                    HdrFormat::Hdr10 => "HDR10",
                    HdrFormat::Hlg => "HLG",
                    HdrFormat::DolbyVision { .. } => "Dolby Vision",
                }
                .to_string(),
            );
        }
        parts.push(self.codec.to_uppercase());
        parts.join(" ")
    }
}

/// Loudness measured on an audio track, following the broadcast standard.
///
/// Measured during the cold path, never at playback time, and applied later
/// as a plain gain, which costs nothing.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Loudness {
    /// Integrated loudness over the whole track.
    pub integrated_lufs: Option<f64>,
    /// True peak, used to know how much gain can be applied safely.
    pub true_peak_dbfs: Option<f64>,
    /// Loudness range, which tells how compressed the material already is.
    pub range_lu: Option<f64>,
}

impl Loudness {
    pub fn is_measured(&self) -> bool {
        self.integrated_lufs.is_some()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AudioDetails {
    pub codec: String,
    pub profile: Option<String>,
    pub channels: i32,
    /// Human readable channel layout, for example the one shown as five point
    /// one in the interface.
    pub channel_layout: Option<String>,
    pub sample_rate: Option<i32>,
    pub bit_depth: Option<i32>,
    pub bitrate: Option<i64>,
    pub loudness: Loudness,
}

impl AudioDetails {
    /// Whether the track carries more channels than a browser will output.
    pub fn is_multichannel(&self) -> bool {
        self.channels > 2
    }
}

/// Subtitles come in three families that behave very differently, so the
/// distinction is part of the model rather than guessed from the codec name
/// at every call site.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SubtitleLayout {
    /// Plain text, convertible to the format browsers read.
    Text,
    /// Pictures. The only way to show them is to burn them into the video,
    /// which forces a full transcode.
    Bitmap,
}

#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleDetails {
    pub codec: String,
    pub layout: SubtitleLayout,
    /// Meant for viewers who are hard of hearing.
    pub is_hearing_impaired: bool,
    /// Stored in a separate file next to the media rather than inside it.
    pub is_external: bool,
    /// Path of the external file, relative to the root.
    pub external_relative_path: Option<PathBuf>,
}

impl SubtitleDetails {
    /// Whether showing this track forces a full video transcode.
    pub fn forces_full_transcode(&self) -> bool {
        matches!(self.layout, SubtitleLayout::Bitmap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn video(height: i32, codec: &str, hdr: Option<HdrFormat>) -> VideoDetails {
        VideoDetails {
            codec: codec.to_string(),
            profile: None,
            level: None,
            width: height * 16 / 9,
            height,
            margins: None,
            aspect_ratio: None,
            is_interlaced: false,
            frame_rate: None,
            bitrate: None,
            pixel_format: None,
            reference_frames: None,
            color: ColorInfo::default(),
            hdr,
        }
    }

    #[test]
    fn every_high_dynamic_range_flavour_needs_conversion() {
        for flavour in [
            HdrFormat::Hdr10,
            HdrFormat::Hlg,
            HdrFormat::DolbyVision { profile: Some(8) },
        ] {
            assert!(flavour.needs_tone_mapping());
        }
    }

    #[test]
    fn dolby_vision_profile_five_is_singled_out() {
        assert!(HdrFormat::DolbyVision { profile: Some(5) }.is_incompatible_without_conversion());
        assert!(!HdrFormat::DolbyVision { profile: Some(8) }.is_incompatible_without_conversion());
        assert!(!HdrFormat::Hdr10.is_incompatible_without_conversion());
    }

    #[test]
    fn a_standard_range_track_needs_no_conversion() {
        assert!(!video(1080, "h264", None).needs_tone_mapping());
    }

    #[test]
    fn a_wide_range_track_needs_conversion_even_for_a_thumbnail() {
        assert!(video(2160, "hevc", Some(HdrFormat::Hdr10)).needs_tone_mapping());
    }

    #[test]
    fn the_summary_names_the_definition_the_range_and_the_codec() {
        assert_eq!(
            video(2160, "hevc", Some(HdrFormat::Hdr10)).summary(),
            "4K HDR10 HEVC"
        );
        assert_eq!(video(1080, "h264", None).summary(), "1080p H264");
        assert_eq!(video(480, "mpeg4", None).summary(), "SD MPEG4");
    }

    #[test]
    fn every_definition_a_file_can_have_is_named() {
        // Each step of the ladder, so a copy is never announced as something
        // it is not. The boundaries are what a wide picture actually measures:
        // a 4K film is often 1600 lines tall rather than 2160.
        assert_eq!(video(2160, "hevc", None).summary(), "4K HEVC");
        assert_eq!(video(1440, "hevc", None).summary(), "1440p HEVC");
        assert_eq!(video(1200, "hevc", None).summary(), "1080p HEVC");
        assert_eq!(video(720, "h264", None).summary(), "720p H264");
        assert_eq!(video(576, "mpeg4", None).summary(), "SD MPEG4");
    }

    #[test]
    fn only_more_than_two_channels_counts_as_multichannel() {
        let stereo = AudioDetails {
            codec: "aac".into(),
            profile: None,
            channels: 2,
            channel_layout: Some("stereo".into()),
            sample_rate: None,
            bit_depth: None,
            bitrate: None,
            loudness: Loudness::default(),
        };
        let surround = AudioDetails {
            channels: 6,
            ..stereo.clone()
        };
        assert!(!stereo.is_multichannel());
        assert!(surround.is_multichannel());
    }

    #[test]
    fn picture_subtitles_force_a_full_transcode_and_text_ones_do_not() {
        let text = SubtitleDetails {
            codec: "subrip".into(),
            layout: SubtitleLayout::Text,
            is_hearing_impaired: false,
            is_external: false,
            external_relative_path: None,
        };
        let bitmap = SubtitleDetails {
            layout: SubtitleLayout::Bitmap,
            ..text.clone()
        };
        assert!(!text.forces_full_transcode());
        assert!(bitmap.forces_full_transcode());
    }

    fn track(kind: TrackKind, is_default: bool) -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 0,
            language: None,
            title: None,
            is_default,
            is_forced: false,
            kind,
        }
    }

    fn soundtrack() -> TrackKind {
        TrackKind::Audio(AudioDetails {
            codec: "eac3".into(),
            profile: None,
            channels: 6,
            channel_layout: Some("5.1".into()),
            sample_rate: Some(48_000),
            bit_depth: None,
            bitrate: None,
            loudness: Loudness::default(),
        })
    }

    fn caption() -> TrackKind {
        TrackKind::Subtitle(SubtitleDetails {
            codec: "subrip".into(),
            layout: SubtitleLayout::Text,
            is_hearing_impaired: false,
            is_external: false,
            external_relative_path: None,
        })
    }

    fn source_with(tracks: Vec<Track>) -> MediaSource {
        MediaSource {
            id: MediaSourceId::new(),
            work_id: WorkId::new(),
            root_id: LibraryRootId::new(),
            relative_path: PathBuf::from("Something/Quiet.Harbour.2019.mkv"),
            container: Some("matroska,webm".into()),
            duration: Some(Millis::new(7_200_000)),
            overall_bitrate: None,
            identity: FileIdentity {
                size_bytes: 1_000,
                modified_at: crate::time::now(),
                content_fingerprint: None,
            },
            added_at: crate::time::now(),
            tracks,
        }
    }

    #[test]
    fn each_kind_of_track_is_read_back_on_its_own() {
        let source = source_with(vec![
            track(TrackKind::Video(video(1080, "h264", None)), true),
            track(soundtrack(), true),
            track(soundtrack(), false),
            track(caption(), false),
        ]);

        assert_eq!(source.video_tracks().count(), 1);
        assert_eq!(source.audio_tracks().count(), 2);
        assert_eq!(source.subtitle_tracks().count(), 1);
        assert_eq!(
            source.tracks[0].kind.as_str(),
            "video",
            "these words travel to the interface and must not drift"
        );
        assert_eq!(source.tracks[1].kind.as_str(), "audio");
        assert_eq!(source.tracks[3].kind.as_str(), "subtitle");
    }

    #[test]
    fn the_picture_a_player_uses_is_the_one_the_file_marked_default() {
        let source = source_with(vec![
            track(TrackKind::Video(video(480, "mpeg4", None)), false),
            track(TrackKind::Video(video(2160, "hevc", None)), true),
        ]);
        let (_, chosen) = source.primary_video().expect("a film has a picture");
        assert_eq!(chosen.height, 2160);

        // A file that marks none still plays: the first one is the picture.
        let unmarked = source_with(vec![
            track(TrackKind::Video(video(480, "mpeg4", None)), false),
            track(TrackKind::Video(video(2160, "hevc", None)), false),
        ]);
        assert_eq!(unmarked.primary_video().expect("a picture").1.height, 480);

        assert!(
            source_with(vec![track(soundtrack(), true)])
                .primary_video()
                .is_none(),
            "a file with no picture has none to offer"
        );
    }

    #[test]
    fn a_source_is_named_by_its_file_and_not_by_its_folders() {
        let source = source_with(Vec::new());
        assert_eq!(source.file_name(), "Quiet.Harbour.2019.mkv");
    }

    #[test]
    fn a_language_written_any_of_the_usual_ways_comes_out_the_same() {
        // What a picker needs: one entry per language, whatever the file says.
        for written in ["fr", "fra", "fre", "FR", " fr-FR ", "fr_CA"] {
            assert_eq!(
                normalise_language(written),
                "fre",
                "{written} is the same language as the others"
            );
        }
        assert_eq!(normalise_language("en"), "eng");
        assert_eq!(normalise_language("zho"), "chi");
        assert_eq!(
            normalise_language("und"),
            "und",
            "a code nobody knows is passed on rather than invented"
        );
    }

    #[test]
    fn a_soundtrack_nobody_measured_says_so() {
        let mut loudness = Loudness::default();
        assert!(
            !loudness.is_measured(),
            "without a measurement there is no gain to apply"
        );
        loudness.integrated_lufs = Some(-23.0);
        assert!(loudness.is_measured());
    }
}
