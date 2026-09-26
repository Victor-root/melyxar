//! Turns an analyser report into the domain model of a media file.
//!
//! The split matters: the tool crate runs the analyser and hands back plain
//! data, and this crate decides what that data means. Every judgement lives
//! here, which is why they can all be tested against captured reports without
//! a single file on disk.
//!
//! The judgement that matters most is about colour. Whether a stream carries
//! wide gamut, and which flavour, decides both whether it can be played as is
//! and whether a still image taken from it needs converting. Getting it wrong
//! produces the washed out grey thumbnails seen on other servers.

#![forbid(unsafe_code)]

use melyxar_core::id::{MediaSourceId, TrackId};
use melyxar_core::media::{
    normalise_language, AudioDetails, Chapter, ColorInfo, HdrFormat, Loudness, SubtitleDetails,
    SubtitleLayout, Track, TrackKind, VideoDetails,
};
use melyxar_core::time::Millis;
use melyxar_ffmpeg::probe::{parse_rational, ProbeChapter, ProbeReport, ProbeStream};

/// How the picture and the sound of one file line up against each other.
///
/// The sound running ahead of the picture takes two shapes, and neither is
/// visible anywhere but here. A constant offset, which the container declares
/// and which nothing was reading. And a drift, where the two streams are
/// simply not the same length, so they part company as the film goes on.
///
/// Everything is in milliseconds, counted from the picture: a positive offset
/// means the sound starts after the picture.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HowTheStreamsLineUp {
    pub video_starts_at: Option<i64>,
    pub audio_starts_at: Option<i64>,
    pub video_runs_for: Option<i64>,
    pub audio_runs_for: Option<i64>,
}

impl HowTheStreamsLineUp {
    /// Reads it off a report, from the first stream of each kind.
    ///
    /// The first of each kind rather than the chosen ones: this is about the
    /// file, and it is read once when the file is analysed, long before
    /// anybody picks a soundtrack.
    pub fn of(report: &ProbeReport) -> Self {
        let video = report.streams.iter().find(|stream| stream.is_video());
        let audio = report.streams.iter().find(|stream| stream.is_audio());
        Self {
            video_starts_at: video.and_then(ProbeStream::starts_at_ms),
            audio_starts_at: audio.and_then(ProbeStream::starts_at_ms),
            video_runs_for: video.and_then(ProbeStream::runs_for_ms),
            audio_runs_for: audio.and_then(ProbeStream::runs_for_ms),
        }
    }

    /// How far the sound starts after the picture, when both say.
    pub fn offset(&self) -> Option<i64> {
        Some(self.audio_starts_at? - self.video_starts_at?)
    }

    /// How much longer the sound runs than the picture, when both say.
    ///
    /// Two streams of different lengths were recorded at rates that do not
    /// quite agree, and they part company steadily rather than all at once.
    pub fn drift(&self) -> Option<i64> {
        Some(self.audio_runs_for? - self.video_runs_for?)
    }

    /// Whether either of them is worth reading.
    ///
    /// A file whose streams start together and run the same length has
    /// nothing to say, and saying it for every film would bury the ones that
    /// do.
    pub fn is_worth_saying(&self) -> bool {
        let past = |value: Option<i64>| value.is_some_and(|value| value.abs() >= NOTICEABLE_MS);
        past(self.offset()) || past(self.drift())
    }
}

/// How far apart two streams have to be before it is worth a word.
///
/// A frame at twenty five pictures a second is forty milliseconds, and a
/// difference smaller than that cannot be seen or heard by anyone.
const NOTICEABLE_MS: i64 = 40;

/// What a file turned out to be.
#[derive(Debug, Clone, PartialEq)]
pub struct AnalysedFile {
    pub container: Option<String>,
    pub duration: Option<Millis>,
    pub overall_bitrate: Option<i64>,
    pub tracks: Vec<Track>,
    pub chapters: Vec<Chapter>,
}

impl AnalysedFile {
    /// Reads a report into the domain model.
    pub fn from_report(report: &ProbeReport, source_id: MediaSourceId) -> Self {
        let tracks = report
            .streams
            .iter()
            .filter_map(|stream| track_from_stream(stream, source_id))
            .collect();

        Self {
            container: report.format.format_name.clone(),
            duration: report
                .format
                .duration
                .as_deref()
                .and_then(|value| value.parse::<f64>().ok())
                .filter(|seconds| *seconds > 0.0)
                .map(Millis::from_seconds_f64),
            overall_bitrate: report
                .format
                .bit_rate
                .as_deref()
                .and_then(|value| value.parse().ok()),
            tracks,
            chapters: chapters_from_report(&report.chapters),
        }
    }
}

/// Builds one track, or nothing for a stream kind we do not model, such as an
/// attached font or a cover image.
fn track_from_stream(stream: &ProbeStream, source_id: MediaSourceId) -> Option<Track> {
    let kind = if stream.is_video() {
        // A cover image is reported as a video stream with a single frame.
        // Treating it as a video track would make a music file look like a
        // film, so it is skipped.
        if stream.has_disposition("attached_pic") {
            return None;
        }
        TrackKind::Video(video_details(stream))
    } else if stream.is_audio() {
        TrackKind::Audio(audio_details(stream))
    } else if stream.is_subtitle() {
        TrackKind::Subtitle(subtitle_details(stream))
    } else {
        return None;
    };

    Some(Track {
        id: TrackId::new(),
        source_id,
        stream_index: stream.index,
        language: stream
            .tag("language")
            .map(normalise_language)
            .filter(|value| !value.is_empty() && value != "und"),
        title: stream.tag("title").map(str::to_string),
        is_default: stream.has_disposition("default"),
        is_forced: stream.has_disposition("forced"),
        kind,
    })
}

fn video_details(stream: &ProbeStream) -> VideoDetails {
    let color = ColorInfo {
        primaries: stream.color_primaries.clone(),
        space: stream.color_space.clone(),
        transfer: stream.color_transfer.clone(),
        bit_depth: stream
            .bits_per_raw_sample
            .as_deref()
            .and_then(|value| value.parse().ok())
            .or_else(|| bit_depth_from_pixel_format(stream.pix_fmt.as_deref())),
    };

    VideoDetails {
        codec: stream
            .codec_name
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        profile: stream.profile.clone(),
        level: stream.level.filter(|value| *value > 0),
        width: stream.width.unwrap_or(0),
        height: stream.height.unwrap_or(0),
        margins: stream.margins_to_cut(),
        aspect_ratio: stream.display_aspect_ratio.clone(),
        is_interlaced: is_interlaced(stream.field_order.as_deref()),
        frame_rate: stream
            .avg_frame_rate
            .as_deref()
            .and_then(parse_rational)
            .filter(|rate| *rate > 0.0)
            .or_else(|| {
                stream
                    .r_frame_rate
                    .as_deref()
                    .and_then(parse_rational)
                    .filter(|rate| *rate > 0.0)
            }),
        bitrate: stream.bitrate(),
        pixel_format: stream.pix_fmt.clone(),
        reference_frames: stream.refs.filter(|value| *value > 0),
        hdr: detect_hdr(stream, &color),
        color,
    }
}

/// Works out whether a stream carries wide gamut colour, and which flavour.
///
/// Dolby Vision is checked first because a stream can carry both it and a
/// standard curve, and the flavour with the stricter handling has to win.
/// Beyond that the transfer characteristics are the deciding field: they name
/// the curve, and the two wide gamut curves have names of their own.
pub fn detect_hdr(stream: &ProbeStream, color: &ColorInfo) -> Option<HdrFormat> {
    if let Some(profile) = stream.dolby_vision_profile() {
        return Some(HdrFormat::DolbyVision {
            profile: Some(profile),
        });
    }

    let transfer = color.transfer.as_deref()?.to_lowercase();
    match transfer.as_str() {
        // Perceptual quantiser, the common flavour.
        "smpte2084" | "smpte st 2084" | "pq" => Some(HdrFormat::Hdr10),
        // Hybrid log gamma, used by broadcasters.
        "arib-std-b67" | "hlg" => Some(HdrFormat::Hlg),
        _ => None,
    }
}

/// Reads the sample depth out of the pixel format name.
///
/// A fallback: the dedicated field is often missing, and the depth matters
/// because ten bits is one of the signs of a wide gamut stream.
fn bit_depth_from_pixel_format(format: Option<&str>) -> Option<i32> {
    let format = format?;
    // A deeper format carries its depth right after the plane marker, as in a
    // ten bit planar layout. Anything deeper than eight bits says so.
    if let Some(position) = format.find('p') {
        let digits: String = format[position + 1..]
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(depth) = digits.parse::<i32>() {
            return Some(depth);
        }
    }
    // No depth spelled out means the ordinary eight bits, but only for a
    // format we recognise: guessing on an unknown name would be worse than
    // admitting we do not know.
    let known_family = ["yuv", "yuvj", "gbr", "rgb", "bgr", "gray", "nv"]
        .iter()
        .any(|family| format.starts_with(family));
    known_family.then_some(8)
}

/// Whether the picture is interlaced.
///
/// Anything other than progressive is treated as interlaced, because the
/// consequence is the same: the picture needs deinterlacing before it looks
/// right on a computer screen.
fn is_interlaced(field_order: Option<&str>) -> bool {
    !matches!(field_order, None | Some("progressive") | Some("unknown"))
}

fn audio_details(stream: &ProbeStream) -> AudioDetails {
    AudioDetails {
        codec: stream
            .codec_name
            .clone()
            .unwrap_or_else(|| "unknown".into()),
        profile: stream.profile.clone(),
        channels: stream.channels.unwrap_or(0),
        channel_layout: stream.channel_layout.clone(),
        sample_rate: stream.sample_rate.as_deref().and_then(|v| v.parse().ok()),
        bit_depth: stream.bits_per_sample.filter(|value| *value > 0),
        bitrate: stream.bitrate(),
        // Left empty here. Loudness is measured by a separate background pass,
        // because measuring it means reading the whole file.
        loudness: Loudness::default(),
    }
}

fn subtitle_details(stream: &ProbeStream) -> SubtitleDetails {
    let codec = stream
        .codec_name
        .clone()
        .unwrap_or_else(|| "unknown".into());
    SubtitleDetails {
        layout: subtitle_layout(&codec),
        codec,
        is_hearing_impaired: stream.has_disposition("hearing_impaired"),
        is_external: false,
        external_relative_path: None,
    }
}

/// Sorts a subtitle codec into one of the two families.
///
/// The distinction is not cosmetic: a picture subtitle can only be burnt into
/// the video, which forces a full transcode, whereas a text one is converted
/// and delivered alongside. Unknown codecs are treated as pictures, the
/// cautious side: burning in always works, whereas a failed conversion leaves
/// the viewer with no subtitles at all.
pub fn subtitle_layout(codec: &str) -> SubtitleLayout {
    match codec.to_lowercase().as_str() {
        "subrip" | "srt" | "ass" | "ssa" | "webvtt" | "vtt" | "mov_text" | "text" | "subviewer"
        | "microdvd" | "sami" | "realtext" | "stl" | "eia_608" | "subviewer1" => {
            SubtitleLayout::Text
        }
        _ => SubtitleLayout::Bitmap,
    }
}

fn chapters_from_report(chapters: &[ProbeChapter]) -> Vec<Chapter> {
    chapters
        .iter()
        .enumerate()
        .filter_map(|(index, chapter)| {
            let start = chapter.start_time.as_deref()?.parse::<f64>().ok()?;
            Some(Chapter {
                ordinal: index as i32 + 1,
                start: Millis::from_seconds_f64(start),
                title: chapter.title().map(str::to_string),
                thumbnail_path: None,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_ffmpeg::probe::parse_report;

    fn analyse(json: &str) -> AnalysedFile {
        let report = parse_report(json).expect("the report parses");
        AnalysedFile::from_report(&report, MediaSourceId::new())
    }

    fn lining_up(json: &str) -> HowTheStreamsLineUp {
        HowTheStreamsLineUp::of(&parse_report(json).expect("the report parses"))
    }

    #[test]
    fn the_bitrate_a_matroska_file_keeps_in_a_tag_is_read_there() {
        let file = analyse(
            r#"{"streams":[
                {"index":0,"codec_type":"video","codec_name":"hevc","width":1920,"height":800,
                 "tags":{"BPS":"7012345"}},
                {"index":1,"codec_type":"audio","codec_name":"ac3","bit_rate":"640000",
                 "tags":{"BPS":"1"}},
                {"index":2,"codec_type":"audio","codec_name":"aac","tags":{"BPS-eng":"192000"}},
                {"index":3,"codec_type":"audio","codec_name":"dts"}]}"#,
        );
        let bitrates: Vec<Option<i64>> = file
            .tracks
            .iter()
            .map(|track| match &track.kind {
                TrackKind::Video(video) => video.bitrate,
                TrackKind::Audio(audio) => audio.bitrate,
                _ => None,
            })
            .collect();
        assert_eq!(
            bitrates,
            vec![Some(7_012_345), Some(640_000), Some(192_000), None],
            "the tag when the stream says nothing, the stream when it does, \
             and nothing rather than a guess"
        );
    }

    #[test]
    fn a_sound_that_starts_after_the_picture_is_measured() {
        let found = lining_up(
            r#"{"streams":[
                {"index":0,"codec_type":"video","codec_name":"h264",
                 "start_time":"0.000000","duration":"7200.000000"},
                {"index":1,"codec_type":"audio","codec_name":"eac3",
                 "start_time":"0.320000","duration":"7200.000000"}]}"#,
        );
        assert_eq!(found.offset(), Some(320));
        assert_eq!(found.drift(), Some(0));
        assert!(
            found.is_worth_saying(),
            "a third of a second is eight frames and plainly visible"
        );
    }

    #[test]
    fn a_sound_that_starts_before_the_picture_is_measured_the_other_way() {
        let found = lining_up(
            r#"{"streams":[
                {"index":0,"codec_type":"video","codec_name":"h264","start_time":"0.500000"},
                {"index":1,"codec_type":"audio","codec_name":"aac","start_time":"0.000000"}]}"#,
        );
        assert_eq!(found.offset(), Some(-500));
    }

    #[test]
    fn two_streams_of_different_lengths_part_company_as_the_film_goes_on() {
        let found = lining_up(
            r#"{"streams":[
                {"index":0,"codec_type":"video","codec_name":"h264",
                 "start_time":"0.000000","duration":"7200.000000"},
                {"index":1,"codec_type":"audio","codec_name":"aac",
                 "start_time":"0.000000","duration":"7205.300000"}]}"#,
        );
        assert_eq!(found.offset(), Some(0));
        assert_eq!(found.drift(), Some(5300));
        assert!(found.is_worth_saying());
    }

    #[test]
    fn a_file_whose_streams_agree_has_nothing_to_say() {
        // Said for every film, it would bury the ones that do.
        let found = lining_up(
            r#"{"streams":[
                {"index":0,"codec_type":"video","codec_name":"h264",
                 "start_time":"0.000000","duration":"7200.000000"},
                {"index":1,"codec_type":"audio","codec_name":"aac",
                 "start_time":"0.010000","duration":"7200.000000"}]}"#,
        );
        assert_eq!(found.offset(), Some(10));
        assert!(
            !found.is_worth_saying(),
            "ten milliseconds is a quarter of a frame and nobody can see it"
        );
    }

    #[test]
    fn a_file_that_says_nothing_about_its_timing_is_not_made_to() {
        // "N/A" and a missing field are both absences, and neither is zero.
        let found = lining_up(
            r#"{"streams":[
                {"index":0,"codec_type":"video","codec_name":"h264","start_time":"N/A"},
                {"index":1,"codec_type":"audio","codec_name":"aac"}]}"#,
        );
        assert_eq!(found.video_starts_at, None);
        assert_eq!(found.audio_starts_at, None);
        assert_eq!(found.offset(), None);
        assert!(!found.is_worth_saying());
    }

    #[test]
    fn a_file_with_no_sound_at_all_has_nothing_to_line_up() {
        let found = lining_up(
            r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"h264",
                "start_time":"0.000000"}]}"#,
        );
        assert_eq!(found.video_starts_at, Some(0));
        assert_eq!(found.audio_starts_at, None);
        assert_eq!(found.offset(), None);
    }

    #[test]
    fn the_perceptual_quantiser_curve_is_recognised_as_wide_gamut() {
        let file = analyse(
            r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"hevc",
                "width":3840,"height":2160,"color_transfer":"smpte2084",
                "color_primaries":"bt2020","pix_fmt":"yuv420p10le"}]}"#,
        );
        let (_, video) = file
            .tracks
            .iter()
            .find_map(|track| match &track.kind {
                TrackKind::Video(details) => Some((track, details)),
                _ => None,
            })
            .expect("a video track");
        assert_eq!(video.hdr, Some(HdrFormat::Hdr10));
        assert!(
            video.needs_tone_mapping(),
            "an unconverted frame from this stream would be washed out"
        );
        assert_eq!(video.color.bit_depth, Some(10));
    }

    #[test]
    fn the_broadcast_curve_is_recognised_too() {
        let file = analyse(
            r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"hevc",
                "color_transfer":"arib-std-b67"}]}"#,
        );
        match &file.tracks[0].kind {
            TrackKind::Video(video) => assert_eq!(video.hdr, Some(HdrFormat::Hlg)),
            other => panic!("expected a video track, got {other:?}"),
        }
    }

    #[test]
    fn dolby_vision_wins_over_the_curve_because_it_needs_stricter_handling() {
        let file = analyse(
            r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"hevc",
                "color_transfer":"smpte2084",
                "side_data_list":[{"side_data_type":"DOVI configuration record","dv_profile":5}]}]}"#,
        );
        match &file.tracks[0].kind {
            TrackKind::Video(video) => {
                assert_eq!(video.hdr, Some(HdrFormat::DolbyVision { profile: Some(5) }));
                assert!(video
                    .hdr
                    .expect("a flavour")
                    .is_incompatible_without_conversion());
            }
            other => panic!("expected a video track, got {other:?}"),
        }
    }

    #[test]
    fn a_film_that_says_part_of_its_frame_is_not_the_picture_is_read_as_the_picture() {
        // The frame and the picture are different shapes, and only one of them
        // is the film. Taking the frame for the picture turns a wide film into
        // a tall one, stretched to fill a shape it never had.
        let file = analyse(
            r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"hevc",
                "width":3840,"height":2160,
                "side_data_list":[{"side_data_type":"Frame Cropping",
                    "crop_top":276,"crop_bottom":276,"crop_left":0,"crop_right":0}]}]}"#,
        );
        match &file.tracks[0].kind {
            TrackKind::Video(video) => {
                assert_eq!(
                    (video.width, video.height),
                    (3840, 2160),
                    "the frame is kept as the film states it"
                );
                assert_eq!(
                    (video.visible_width(), video.visible_height()),
                    (3840, 1608),
                    "the picture is what is left once the margins are off"
                );
            }
            other => panic!("expected a video track, got {other:?}"),
        }
    }

    #[test]
    fn margins_that_take_nothing_off_are_no_margins_at_all() {
        // Some files carry the block with every edge at nought, which says the
        // frame is the picture. Kept as margins it would send every one of
        // those films down a slower path for nothing.
        let file = analyse(
            r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"hevc",
                "width":1920,"height":1080,
                "side_data_list":[{"side_data_type":"Frame Cropping",
                    "crop_top":0,"crop_bottom":0,"crop_left":0,"crop_right":0}]}]}"#,
        );
        match &file.tracks[0].kind {
            TrackKind::Video(video) => {
                assert_eq!(video.margins, None);
                assert_eq!(
                    (video.visible_width(), video.visible_height()),
                    (1920, 1080)
                );
            }
            other => panic!("expected a video track, got {other:?}"),
        }
    }

    #[test]
    fn an_ordinary_stream_carries_no_wide_gamut_flavour() {
        let file = analyse(
            r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"h264",
                "color_transfer":"bt709","pix_fmt":"yuv420p"}]}"#,
        );
        match &file.tracks[0].kind {
            TrackKind::Video(video) => {
                assert_eq!(video.hdr, None);
                assert!(!video.needs_tone_mapping());
                assert_eq!(video.color.bit_depth, Some(8));
            }
            other => panic!("expected a video track, got {other:?}"),
        }
    }

    #[test]
    fn the_sample_depth_is_read_from_the_pixel_format_when_the_field_is_missing() {
        assert_eq!(bit_depth_from_pixel_format(Some("yuv420p10le")), Some(10));
        assert_eq!(bit_depth_from_pixel_format(Some("yuv420p12le")), Some(12));
        // No depth spelled out means the ordinary eight bits.
        assert_eq!(bit_depth_from_pixel_format(Some("yuv420p")), Some(8));
        assert_eq!(bit_depth_from_pixel_format(Some("nv12")), Some(8));
    }

    #[test]
    fn an_unrecognised_pixel_format_admits_it_does_not_know_the_depth() {
        assert_eq!(bit_depth_from_pixel_format(Some("somethingnew")), None);
        assert_eq!(bit_depth_from_pixel_format(None), None);
    }

    #[test]
    fn picture_subtitles_are_told_apart_from_text_ones() {
        assert_eq!(subtitle_layout("subrip"), SubtitleLayout::Text);
        assert_eq!(subtitle_layout("ass"), SubtitleLayout::Text);
        assert_eq!(subtitle_layout("mov_text"), SubtitleLayout::Text);
        assert_eq!(subtitle_layout("hdmv_pgs_subtitle"), SubtitleLayout::Bitmap);
        assert_eq!(subtitle_layout("dvd_subtitle"), SubtitleLayout::Bitmap);
    }

    #[test]
    fn an_unknown_subtitle_codec_is_treated_as_a_picture_which_is_the_safe_side() {
        assert_eq!(subtitle_layout("something_new"), SubtitleLayout::Bitmap);
    }

    #[test]
    fn a_cover_image_is_not_mistaken_for_a_video_track() {
        let file = analyse(
            r#"{"streams":[
                {"index":0,"codec_type":"audio","codec_name":"flac","channels":2},
                {"index":1,"codec_type":"video","codec_name":"mjpeg",
                 "disposition":{"attached_pic":1}}]}"#,
        );
        assert_eq!(file.tracks.len(), 1, "only the audio track is a track");
        assert!(matches!(file.tracks[0].kind, TrackKind::Audio(_)));
    }

    #[test]
    fn language_tags_are_normalised_so_a_picker_never_shows_a_language_twice() {
        assert_eq!(normalise_language("fr"), "fre");
        assert_eq!(normalise_language("fra"), "fre");
        assert_eq!(normalise_language("FRE"), "fre");
        assert_eq!(normalise_language("fr-FR"), "fre");
        assert_eq!(normalise_language("en"), "eng");
        assert_eq!(normalise_language("de"), "ger");
        assert_eq!(normalise_language("deu"), "ger");
    }

    #[test]
    fn an_undetermined_language_reads_as_absent_rather_than_as_a_language() {
        let file = analyse(
            r#"{"streams":[{"index":0,"codec_type":"audio","codec_name":"aac",
                "channels":2,"tags":{"language":"und"}}]}"#,
        );
        assert_eq!(file.tracks[0].language, None);
    }

    #[test]
    fn dispositions_are_carried_over() {
        let file = analyse(
            r#"{"streams":[{"index":2,"codec_type":"subtitle","codec_name":"subrip",
                "disposition":{"default":1,"forced":1,"hearing_impaired":1}}]}"#,
        );
        let track = &file.tracks[0];
        assert!(track.is_default);
        assert!(track.is_forced);
        assert_eq!(track.stream_index, 2);
        match &track.kind {
            TrackKind::Subtitle(subtitle) => assert!(subtitle.is_hearing_impaired),
            other => panic!("expected a subtitle track, got {other:?}"),
        }
    }

    #[test]
    fn an_interlaced_picture_is_flagged_and_a_progressive_one_is_not() {
        assert!(!is_interlaced(Some("progressive")));
        assert!(!is_interlaced(None));
        assert!(is_interlaced(Some("tt")));
        assert!(is_interlaced(Some("bb")));
    }

    #[test]
    fn a_zero_duration_reads_as_unknown_rather_than_as_an_instant_film() {
        let file = analyse(r#"{"format":{"duration":"0.000000"},"streams":[]}"#);
        assert_eq!(file.duration, None);
    }

    #[test]
    fn chapters_are_numbered_from_one_and_carry_their_title() {
        let file = analyse(
            r#"{"streams":[],"chapters":[
                {"start_time":"0.000000","tags":{"title":"Opening"}},
                {"start_time":"300.000000"}]}"#,
        );
        assert_eq!(file.chapters.len(), 2);
        assert_eq!(file.chapters[0].ordinal, 1);
        assert_eq!(file.chapters[0].title.as_deref(), Some("Opening"));
        assert_eq!(file.chapters[1].start, Millis::new(300_000));
        assert_eq!(file.chapters[1].title, None);
    }

    #[tokio::test]
    async fn a_real_file_is_described_end_to_end() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("sample.mkv");

        let status = tokio::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=2:size=640x360:rate=25",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-c:a",
                "ac3",
                "-ac",
                "6",
                "-metadata:s:a:0",
                "language=fra",
                "-shortest",
                "-y",
            ])
            .arg(&media)
            .status()
            .await
            .expect("the tool runs");
        assert!(status.success());

        let tools = melyxar_ffmpeg::ToolPaths::discover(None, None).expect("tools are installed");
        let report = melyxar_ffmpeg::probe::probe(&tools.ffprobe, &media)
            .await
            .expect("the file analyses");
        let file = AnalysedFile::from_report(&report, MediaSourceId::new());

        assert!(file.duration.expect("a duration").get() > 1500);
        assert_eq!(file.tracks.len(), 2);

        let (_, video) = file
            .tracks
            .iter()
            .find_map(|track| match &track.kind {
                TrackKind::Video(details) => Some((track, details)),
                _ => None,
            })
            .expect("a video track");
        assert_eq!(video.width, 640);
        assert_eq!(video.height, 360);
        assert_eq!(video.summary(), "SD H264");

        let (track, audio) = file
            .tracks
            .iter()
            .find_map(|track| match &track.kind {
                TrackKind::Audio(details) => Some((track, details)),
                _ => None,
            })
            .expect("an audio track");
        assert_eq!(audio.channels, 6);
        assert!(audio.is_multichannel(), "this one will need folding");
        assert_eq!(track.language.as_deref(), Some("fre"));
        assert!(
            !audio.loudness.is_measured(),
            "loudness is measured by a separate pass, not during the quick analysis"
        );
    }
}
