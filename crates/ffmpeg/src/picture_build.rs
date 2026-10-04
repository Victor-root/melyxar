//! How a picture carried over untouched is built, read from its own headers.
//!
//! A film copied as it is reaches the browser exactly as its maker wrote it,
//! and two films in the same codec can be built very differently: pictures
//! that stand on their own refreshing everything or only most of it, pictures
//! shown before the one they follow in the file, the description of the
//! picture repeated inside the film. A browser can stumble on any of these
//! where it plays another film of the same codec without a hitch, and nothing
//! in the analysis of a file says which of them a film has. This reads it for
//! the journal, from a few seconds of the film, without decoding anything.

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Stdio;

use melyxar_core::time::Millis;
use tokio::process::Command as TokioCommand;

use crate::{FfmpegError, Result};

/// What a few seconds of a picture are made of.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PictureBuild {
    /// Pictures read.
    pub pictures: u32,
    /// Pictures that stand on their own.
    pub key_pictures: u32,
    /// Of those, the ones that refresh everything (an IDR picture): nothing
    /// after them looks back past them.
    pub full_refresh: u32,
    /// Key pictures followed by pictures shown before them, which look back at
    /// the pictures before the key one (an open group of pictures).
    pub with_leading_pictures: u32,
    /// The most such pictures after one key picture.
    pub most_leading_pictures: u32,
    /// Key pictures that carry the description of the picture again inside
    /// themselves, rather than leaving it to the header of the file.
    pub described_again: u32,
    /// Pictures that end the sequence, after which a decoder starts over.
    pub sequence_ends: u32,
    /// The kinds of message carried with key pictures.
    pub key_messages: BTreeSet<u32>,
    /// The most pictures read after one and shown before it.
    pub deepest_reorder: u32,
    /// Average size of a key picture and of any other, in bytes.
    pub key_bytes: u64,
    pub other_bytes: u64,
}

/// Which numbers mean what, in the codec read.
struct Units {
    full_refresh: &'static [u32],
    description: &'static [u32],
    sequence_end: &'static [u32],
}

const H264: Units = Units {
    full_refresh: &[5],
    description: &[7, 8],
    sequence_end: &[10, 11],
};

const HEVC: Units = Units {
    full_refresh: &[19, 20],
    description: &[32, 33, 34],
    sequence_end: &[36, 37],
};

fn units_of(codec: &str) -> Option<&'static Units> {
    match codec {
        "h264" => Some(&H264),
        "hevc" => Some(&HEVC),
        _ => None,
    }
}

/// One picture as the trace names it.
#[derive(Default)]
struct Picture {
    key: bool,
    bytes: u64,
    pts: Option<i64>,
    units: Vec<u32>,
    messages: Vec<u32>,
}

/// The number at the end of a trace line, `... = 7`.
fn value_of(line: &str) -> Option<u32> {
    line.rsplit_once('=')?.1.trim().parse().ok()
}

/// Reads one picture's own line: `Packet: 8384 bytes, key frame, pts 5, ...`.
fn picture_from(line: &str) -> Picture {
    let after = line.split_once("Packet:").map_or("", |(_, rest)| rest);
    let mut picture = Picture {
        key: after.contains("key frame"),
        ..Picture::default()
    };
    for part in after.split(',') {
        let part = part.trim().trim_end_matches('.');
        if let Some(bytes) = part.strip_suffix(" bytes") {
            picture.bytes = bytes.trim().parse().unwrap_or(0);
        } else if let Some(pts) = part.strip_prefix("pts ") {
            picture.pts = pts.trim().parse().ok();
        }
    }
    picture
}

/// Reads the trace of a few seconds of a picture.
///
/// Whatever comes before the first picture describes the header of the file,
/// and is left out: what matters is what the pictures carry themselves.
pub fn read_trace(trace: &str, codec: &str) -> Option<PictureBuild> {
    let units = units_of(codec)?;
    let mut pictures: Vec<Picture> = Vec::new();
    for line in trace.lines() {
        if line.contains("Packet:") {
            pictures.push(picture_from(line));
        } else if let Some(picture) = pictures.last_mut() {
            if line.contains(" nal_unit_type ") {
                picture.units.extend(value_of(line));
            } else if line.contains(" last_payload_type_byte ") {
                picture.messages.extend(value_of(line));
            }
        }
    }

    let mut build = PictureBuild {
        pictures: pictures.len() as u32,
        ..PictureBuild::default()
    };
    let (mut key_total, mut other_total) = (0, 0);
    for (index, picture) in pictures.iter().enumerate() {
        let has = |wanted: &[u32]| picture.units.iter().any(|unit| wanted.contains(unit));
        if has(units.sequence_end) {
            build.sequence_ends += 1;
        }
        // Read after it and shown before it.
        let reordered = picture.pts.map_or(0, |pts| {
            pictures[index + 1..]
                .iter()
                .take(16)
                .filter(|later| later.pts.is_some_and(|later| later < pts))
                .count() as u32
        });
        build.deepest_reorder = build.deepest_reorder.max(reordered);
        if !picture.key {
            other_total += picture.bytes;
            continue;
        }
        build.key_pictures += 1;
        key_total += picture.bytes;
        if has(units.full_refresh) {
            build.full_refresh += 1;
        }
        if has(units.description) {
            build.described_again += 1;
        }
        build.key_messages.extend(picture.messages.iter().copied());
        let leading = picture.pts.map_or(0, |pts| {
            pictures[index + 1..]
                .iter()
                .take_while(|later| !later.key)
                .filter(|later| later.pts.is_some_and(|later| later < pts))
                .count() as u32
        });
        if leading > 0 {
            build.with_leading_pictures += 1;
            build.most_leading_pictures = build.most_leading_pictures.max(leading);
        }
    }
    let others = build.pictures - build.key_pictures;
    build.key_bytes = key_total / u64::from(build.key_pictures.max(1));
    build.other_bytes = other_total / u64::from(others.max(1));
    Some(build)
}

/// Reads how a few seconds of a film's picture are built, from `from` on.
///
/// Nothing for a codec this does not know the parts of.
pub async fn picture_build(
    encoder: &Path,
    media: &Path,
    from: Millis,
    seconds: u32,
    codec: &str,
) -> Result<Option<PictureBuild>> {
    if units_of(codec).is_none() {
        return Ok(None);
    }
    let output = TokioCommand::new(encoder)
        .args(["-hide_banner", "-nostdin", "-nostats", "-loglevel", "info", "-ss"])
        .arg(format!("{:.3}", from.as_seconds_f64()))
        .arg("-i")
        .arg(media)
        .args(["-t", &seconds.to_string(), "-map", "0:v:0", "-c", "copy"])
        .args(["-bsf:v", "trace_headers", "-f", "null", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()
        .await?;
    if !output.status.success() {
        return Err(FfmpegError::Failed {
            tool: "encoder",
            status: output.status.to_string(),
            output: FfmpegError::what_it_complained_about(&output),
        });
    }
    Ok(read_trace(&String::from_utf8_lossy(&output.stderr), codec))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two groups of an open film with the description repeated in the first
    /// key picture, as the trace writes them.
    const TRACE: &str = "\
[trace_headers @ 0x1] 3           nal_unit_type                      00111 = 7
[trace_headers @ 0x1] 3           nal_unit_type                      01000 = 8
[trace_headers @ 0x1] Packet: 9000 bytes, key frame, pts 83, dts -83, duration 41.
[trace_headers @ 0x1] 3           nal_unit_type                      00111 = 7
[trace_headers @ 0x1] 3           nal_unit_type                      01000 = 8
[trace_headers @ 0x1] 3           nal_unit_type                      00110 = 6
[trace_headers @ 0x1] 8           last_payload_type_byte          00000110 = 6
[trace_headers @ 0x1] 3           nal_unit_type                      00001 = 1
[trace_headers @ 0x1] Packet: 3000 bytes, pts 0, dts -42, duration 41.
[trace_headers @ 0x1] 3           nal_unit_type                      00001 = 1
[trace_headers @ 0x1] Packet: 3000 bytes, pts 42, dts 0, duration 41.
[trace_headers @ 0x1] 3           nal_unit_type                      00001 = 1
[trace_headers @ 0x1] Packet: 5000 bytes, pts 167, dts 42, duration 41.
[trace_headers @ 0x1] 3           nal_unit_type                      00001 = 1
[trace_headers @ 0x1] Packet: 3000 bytes, pts 125, dts 83, duration 41.
[trace_headers @ 0x1] 3           nal_unit_type                      00001 = 1
[trace_headers @ 0x1] Packet: 7000 bytes, key frame, pts 208, dts 125, duration 41.
[trace_headers @ 0x1] 3           nal_unit_type                      00101 = 5
";

    #[test]
    fn a_trace_says_how_the_picture_is_built() {
        let build = read_trace(TRACE, "h264").expect("a codec this knows");
        assert_eq!(build.pictures, 6);
        assert_eq!(build.key_pictures, 2);
        assert_eq!(build.full_refresh, 1, "only the second refreshes everything");
        assert_eq!(build.with_leading_pictures, 1);
        assert_eq!(build.most_leading_pictures, 2);
        assert_eq!(build.described_again, 1, "the header's own description is not counted");
        assert_eq!(build.key_messages, BTreeSet::from([6]));
        assert_eq!(build.deepest_reorder, 2);
        assert_eq!(build.key_bytes, 8000);
        assert_eq!(build.other_bytes, 3500);
        assert_eq!(build.sequence_ends, 0);
    }

    #[tokio::test]
    async fn a_real_open_film_is_read_as_one() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let tools = crate::ToolPaths::discover(None, None).expect("the tools are installed here");
        let film = directory.path().join("Lantern.Bay.2011.mkv");
        let made = TokioCommand::new(&tools.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi"])
            .args(["-i", "testsrc2=size=160x90:rate=24:duration=4", "-c:v", "libx264"])
            .args(["-preset", "ultrafast", "-x264-params"])
            .arg("keyint=24:min-keyint=24:open-gop=1:bframes=3:scenecut=0")
            .arg(&film)
            .status()
            .await
            .expect("the tool runs");
        assert!(made.success(), "an open film");

        let build = picture_build(&tools.ffmpeg, &film, Millis::ZERO, 3, "h264")
            .await
            .expect("the tool reads it")
            .expect("a codec this knows");
        // Copied as it is, the stretch runs on to the picture that ends it.
        assert!(build.pictures >= 72, "{build:?}");
        assert!(build.key_pictures >= 3, "{build:?}");
        assert_eq!(build.full_refresh, 1, "only the very first refreshes everything: {build:?}");
        assert_eq!(build.with_leading_pictures, build.key_pictures - 1, "{build:?}");
        assert!(build.deepest_reorder >= 1, "{build:?}");
    }

    #[test]
    fn a_codec_this_does_not_know_says_nothing() {
        assert_eq!(read_trace(TRACE, "vp9"), None);
    }
}
