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

impl ProbeReport {
    /// Whether this report says anything about a film.
    ///
    /// Empty braces parse perfectly well and mean nothing at all, which is
    /// what an analyser that fell over before reading anything leaves behind.
    /// A report worth keeping names the container and at least one stream.
    pub fn describes_something(&self) -> bool {
        self.format.format_name.is_some() && !self.streams.is_empty()
    }
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
    /// Where this stream starts, in seconds, as the container declares it.
    ///
    /// Two streams that do not start together are the commonest reason a film
    /// plays with the sound ahead of the picture, and nothing about the file
    /// says so anywhere else.
    #[serde(default)]
    pub start_time: Option<String>,
    /// How long this stream runs, in seconds. Two streams of noticeably
    /// different lengths drift apart as the film goes on, which is the other
    /// shape the same complaint takes.
    #[serde(default)]
    pub duration: Option<String>,
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

    /// Where the stream starts, in milliseconds, when the container says.
    pub fn starts_at_ms(&self) -> Option<i64> {
        seconds_to_ms(self.start_time.as_deref()?)
    }

    /// How long the stream runs, in milliseconds, when the container says.
    pub fn runs_for_ms(&self) -> Option<i64> {
        seconds_to_ms(self.duration.as_deref()?)
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

/// Reads a count of seconds the analyser wrote, as milliseconds.
///
/// The analyser writes these as decimal text, and "N/A" when it has nothing to
/// say, which is not a number and must not be read as zero.
fn seconds_to_ms(value: &str) -> Option<i64> {
    let seconds: f64 = value.trim().parse().ok()?;
    seconds
        .is_finite()
        .then(|| (seconds * 1000.0).round() as i64)
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

    let complaint = || {
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .take(5)
            .collect::<Vec<_>>()
            .join(" | ")
    };

    if output.status.success() {
        return parse_report(&String::from_utf8_lossy(&output.stdout));
    }

    // An exit code is not the only thing the analyser says. It complains about
    // one broken track of a film and gives up with a status, having already
    // described every stream it read. Believing the status alone loses the
    // whole film over a subtitle track nobody was going to watch, and loses it
    // again at every scan.
    match parse_report(&String::from_utf8_lossy(&output.stdout)) {
        Ok(report) if report.describes_something() => {
            tracing::warn!(
                status = %output.status,
                complaint = complaint(),
                streams = report.streams.len(),
                "the analyser complained but described the file all the same"
            );
            Ok(report)
        }
        _ => Err(FfmpegError::Failed {
            tool: "analyser",
            status: output.status.to_string(),
            output: complaint(),
        }),
    }
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
    use std::path::PathBuf;
    use tokio::process::Command as TokioCommand;

    /// A stand-in analyser that behaves the way the real one does on a film
    /// with one broken track: it describes the file, complains, and gives up
    /// with a status all the same.
    ///
    /// Written as a script rather than waited for on a real file, because the
    /// films that provoke it are rare, large, and nobody's to put in a test.
    fn analyser_that_complains(directory: &Path, prints: &str, status: i32) -> PathBuf {
        let script = directory.join("analyser");
        std::fs::write(
            &script,
            format!("#!/bin/sh\ncat <<'REPORT'\n{prints}\nREPORT\necho 'Picture size 0x0 is invalid' >&2\nexit {status}\n"),
        )
        .expect("the script is written");
        std::fs::set_permissions(&script, std::os::unix::fs::PermissionsExt::from_mode(0o755))
            .expect("the script can be run");
        script
    }

    const A_REAL_ENOUGH_REPORT: &str = r#"{
        "streams": [
            {"index": 0, "codec_type": "video", "codec_name": "h264", "width": 1920, "height": 1080},
            {"index": 1, "codec_type": "audio", "codec_name": "eac3", "channels": 6},
            {"index": 2, "codec_type": "subtitle", "codec_name": "dvd_subtitle"}
        ],
        "format": {"format_name": "matroska,webm", "duration": "7200.000"}
    }"#;

    #[tokio::test]
    async fn a_film_the_analyser_described_is_kept_even_when_it_gave_up_afterwards() {
        // One broken subtitle track used to lose the whole film, and lose it
        // again at every scan. The description is what matters; the status
        // alone is not the whole of what the analyser said.
        let directory = tempfile::tempdir().expect("temporary directory");
        let analyser = analyser_that_complains(directory.path(), A_REAL_ENOUGH_REPORT, 1);

        let report = probe(&analyser, Path::new("/films/Quiet.Harbour.2019.mkv"))
            .await
            .expect("the film is described");
        assert_eq!(report.streams.len(), 3);
        assert_eq!(report.format.format_name.as_deref(), Some("matroska,webm"));
    }

    #[tokio::test]
    async fn an_analyser_that_gave_up_before_reading_anything_is_still_a_failure() {
        // Empty braces parse perfectly well and mean nothing at all. Accepting
        // them would record a film with no container and no track, which is
        // worse than saying the file could not be read.
        let directory = tempfile::tempdir().expect("temporary directory");
        for prints in ["{}", r#"{"streams":[],"format":{}}"#, "not json at all"] {
            let analyser = analyser_that_complains(directory.path(), prints, 1);
            assert!(
                probe(&analyser, Path::new("/films/Quiet.Harbour.2019.mkv"))
                    .await
                    .is_err(),
                "nothing usable came back: {prints}"
            );
        }
    }

    #[tokio::test]
    async fn a_report_with_no_container_named_is_not_a_film() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let analyser = analyser_that_complains(
            directory.path(),
            r#"{"streams":[{"index":0,"codec_type":"video"}],"format":{}}"#,
            1,
        );
        assert!(probe(&analyser, Path::new("/films/x.mkv")).await.is_err());
    }

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
