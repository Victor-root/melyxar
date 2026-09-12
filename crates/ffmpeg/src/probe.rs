//! Running the analyser and reading its report.
//!
//! This module only runs the tool and parses its answer into plain data. It
//! deliberately does not decide what any of it means: turning a report into
//! domain tracks, deciding whether a stream carries wide gamut colour, and
//! choosing what to do about it all belong further up.

use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;

use serde::Deserialize;
use tokio::process::Command as TokioCommand;

use crate::{FfmpegError, Result};

/// Everything the analyser says about one file.
#[derive(Debug, Clone, Deserialize)]
pub struct ProbeReport {
    #[serde(default)]
    pub format: ProbeFormat,
    #[serde(default)]
    pub streams: Vec<ProbeStream>,
    #[serde(default)]
    pub chapters: Vec<ProbeChapter>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ProbeFormat {
    #[serde(default)]
    pub format_name: Option<String>,
    #[serde(default)]
    pub duration: Option<String>,
    #[serde(default)]
    pub bit_rate: Option<String>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProbeStream {
    pub index: i32,
    #[serde(default)]
    pub codec_name: Option<String>,
    #[serde(default)]
    pub codec_type: Option<String>,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub level: Option<i32>,
    #[serde(default)]
    pub width: Option<i32>,
    #[serde(default)]
    pub height: Option<i32>,
    #[serde(default)]
    pub display_aspect_ratio: Option<String>,
    #[serde(default)]
    pub field_order: Option<String>,
    #[serde(default)]
    pub r_frame_rate: Option<String>,
    #[serde(default)]
    pub avg_frame_rate: Option<String>,
    #[serde(default)]
    pub bit_rate: Option<String>,
    #[serde(default)]
    pub pix_fmt: Option<String>,
    #[serde(default)]
    pub refs: Option<i32>,
    // The four colour fields. They decide whether a stream, and any still
    // image taken from it, has to be converted to standard range.
    #[serde(default)]
    pub color_primaries: Option<String>,
    #[serde(default)]
    pub color_space: Option<String>,
    #[serde(default)]
    pub color_transfer: Option<String>,
    #[serde(default)]
    pub bits_per_raw_sample: Option<String>,
    #[serde(default)]
    pub channels: Option<i32>,
    #[serde(default)]
    pub channel_layout: Option<String>,
    #[serde(default)]
    pub sample_rate: Option<String>,
    #[serde(default)]
    pub bits_per_sample: Option<i32>,
    #[serde(default)]
    pub disposition: HashMap<String, i32>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
    /// Extra blocks carried alongside the stream. Dolby Vision announces
    /// itself here rather than through the colour fields.
    #[serde(default)]
    pub side_data_list: Vec<HashMap<String, serde_json::Value>>,
}

impl ProbeStream {
    pub fn is_video(&self) -> bool {
        self.codec_type.as_deref() == Some("video")
    }

    pub fn is_audio(&self) -> bool {
        self.codec_type.as_deref() == Some("audio")
    }

    pub fn is_subtitle(&self) -> bool {
        self.codec_type.as_deref() == Some("subtitle")
    }

    /// Reads one of the disposition markers, such as default or forced.
    pub fn has_disposition(&self, name: &str) -> bool {
        self.disposition.get(name).copied().unwrap_or(0) != 0
    }

    /// Reads a tag, ignoring how it was capitalised. Containers disagree on
    /// that, and a language silently lost is a track a viewer cannot find.
    pub fn tag(&self, name: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.as_str())
    }

    /// Dolby Vision profile, when the stream announces one.
    pub fn dolby_vision_profile(&self) -> Option<i32> {
        self.side_data_list.iter().find_map(|block| {
            // The analyser labels this block with the format's short name
            // rather than its marketing name, so both spellings are accepted.
            let is_dolby_vision = block
                .get("side_data_type")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| {
                    let lowered = value.to_lowercase();
                    lowered.contains("dovi") || lowered.contains("dolby vision")
                });
            if !is_dolby_vision {
                return None;
            }
            block
                .get("dv_profile")
                .and_then(serde_json::Value::as_i64)
                .map(|value| value as i32)
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProbeChapter {
    #[serde(default)]
    pub start_time: Option<String>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

impl ProbeChapter {
    pub fn title(&self) -> Option<&str> {
        self.tags
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("title"))
            .map(|(_, value)| value.as_str())
    }
}

/// Runs the analyser on a file and parses its report.
///
/// The path is handed over as a path rather than as text, so that a name
/// beginning with a dash cannot be read as an option.
pub async fn probe(analyser: &Path, media: &Path) -> Result<ProbeReport> {
    let output = TokioCommand::new(analyser)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
            "-show_chapters",
        ])
        .arg(media)
        .stdin(Stdio::null())
        .output()
        .await?;

    if !output.status.success() {
        return Err(FfmpegError::Failed {
            tool: "analyser",
            status: output.status.to_string(),
            output: String::from_utf8_lossy(&output.stderr)
                .lines()
                .take(5)
                .collect::<Vec<_>>()
                .join(" | "),
        });
    }

    parse_report(&String::from_utf8_lossy(&output.stdout))
}

/// Parses a report that was already captured, which is what tests use.
pub fn parse_report(text: &str) -> Result<ProbeReport> {
    serde_json::from_str(text).map_err(|error| FfmpegError::MalformedReport(error.to_string()))
}

/// Reads a rate written as a fraction, the form the analyser uses.
///
/// A zero denominator appears on streams with no meaningful rate, so it is
/// treated as absent rather than as an error.
pub fn parse_rational(value: &str) -> Option<f64> {
    let (numerator, denominator) = value.split_once('/')?;
    let numerator: f64 = numerator.trim().parse().ok()?;
    let denominator: f64 = denominator.trim().parse().ok()?;
    (denominator != 0.0).then_some(numerator / denominator)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolPaths;
    use tokio::process::Command as TokioCommand;

    #[test]
    fn a_rate_written_as_a_fraction_is_read_back() {
        assert_eq!(parse_rational("24000/1001"), Some(24000.0 / 1001.0));
        assert_eq!(parse_rational("25/1"), Some(25.0));
    }

    #[test]
    fn a_rate_with_no_denominator_reads_as_absent_rather_than_as_an_error() {
        assert_eq!(parse_rational("0/0"), None);
        assert_eq!(parse_rational("nonsense"), None);
    }

    #[test]
    fn a_tag_is_found_whatever_its_capitalisation() {
        let report = parse_report(
            r#"{"streams":[{"index":0,"codec_type":"audio","tags":{"LANGUAGE":"fre"}}]}"#,
        )
        .expect("the report parses");
        assert_eq!(report.streams[0].tag("language"), Some("fre"));
    }

    #[test]
    fn dolby_vision_announces_itself_in_the_side_blocks_not_in_the_colour_fields() {
        let report = parse_report(
            r#"{"streams":[{"index":0,"codec_type":"video",
                "side_data_list":[{"side_data_type":"DOVI configuration record","dv_profile":5}]}]}"#,
        )
        .expect("the report parses");
        assert_eq!(report.streams[0].dolby_vision_profile(), Some(5));
    }

    #[test]
    fn a_stream_without_side_blocks_carries_no_dolby_vision_profile() {
        let report =
            parse_report(r#"{"streams":[{"index":0,"codec_type":"video"}]}"#).expect("parses");
        assert_eq!(report.streams[0].dolby_vision_profile(), None);
    }

    #[test]
    fn a_malformed_report_is_reported_rather_than_silently_empty() {
        let error = parse_report("{ not json").expect_err("must fail");
        assert!(matches!(error, FfmpegError::MalformedReport(_)));
    }

    #[test]
    fn an_empty_report_parses_into_empty_collections() {
        let report = parse_report("{}").expect("an empty report is still a report");
        assert!(report.streams.is_empty());
        assert!(report.chapters.is_empty());
    }

    /// Builds a clip carrying the shapes the analyser has to describe.
    async fn make_clip(path: &Path) {
        let status = TokioCommand::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=2:size=320x240:rate=25",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:duration=2",
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-c:a",
                "aac",
                "-ac",
                "6",
                "-metadata:s:a:0",
                "language=fre",
                "-shortest",
                "-y",
            ])
            .arg(path)
            .status()
            .await
            .expect("the tool runs");
        assert!(status.success());
    }

    #[tokio::test]
    async fn a_real_file_is_described_down_to_its_tracks() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("sample.mp4");
        make_clip(&media).await;

        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let report = probe(&tools.ffprobe, &media)
            .await
            .expect("the file analyses");

        let duration: f64 = report
            .format
            .duration
            .as_deref()
            .expect("a duration is reported")
            .parse()
            .expect("the duration is a number");
        assert!((duration - 2.0).abs() < 0.5, "duration was {duration}");

        let video = report
            .streams
            .iter()
            .find(|stream| stream.is_video())
            .expect("a video track is present");
        assert_eq!(video.width, Some(320));
        assert_eq!(video.height, Some(240));
        assert_eq!(video.codec_name.as_deref(), Some("h264"));
        assert!(parse_rational(video.r_frame_rate.as_deref().unwrap_or("0/0")).is_some());

        let audio = report
            .streams
            .iter()
            .find(|stream| stream.is_audio())
            .expect("an audio track is present");
        assert_eq!(audio.channels, Some(6), "the multichannel layout survived");
        assert_eq!(audio.tag("language"), Some("fre"));
    }

    #[tokio::test]
    async fn analysing_a_missing_file_fails_with_a_reason() {
        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let error = probe(&tools.ffprobe, Path::new("/nowhere/missing.mkv"))
            .await
            .expect_err("a missing file must fail");
        match error {
            FfmpegError::Failed { output, .. } => assert!(!output.is_empty()),
            other => panic!("expected a failure carrying its output, got {other:?}"),
        }
    }
}
