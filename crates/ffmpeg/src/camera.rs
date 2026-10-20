//! A camera relayed to a browser.
//!
//! A browser cannot open a camera's RTSP stream. The tool opens it here and
//! writes it back out as fragmented MP4, in pieces of a few tenths of a
//! second a browser takes as they come: the picture copied as the camera
//! sends it, the sound made AAC whatever the camera sends. No session, no
//! segments, nothing kept on the disk, the tool writing straight into the
//! answer, which stops it when the answer is dropped.
//!
//! The address carries the camera's account: it is never written to the log.

use std::ffi::OsString;
use std::process::Stdio;

use tokio::process::{Child, ChildStdout, Command as TokioCommand};

use crate::{FfmpegError, Result};

/// What the answer is said to be.
pub const CONTENT_TYPE: &str = "video/mp4";

/// How long a piece of the stream is at most, in microseconds: short enough
/// that the picture reaches the browser well under a second behind.
const PIECE_US: u32 = 200_000;

/// A camera on its way, and the tool relaying it. The tool stops when this is
/// dropped, which is what a viewer going away amounts to.
pub struct RelayedCamera {
    pub output: ChildStdout,
    /// Held for as long as the camera is being sent.
    pub process: Child,
}

/// Starts relaying the camera at `address`, its account written in.
///
/// The address is not put through the guard of the formats a file may be
/// read as: it is no file anybody uploaded but an RTSP address the
/// administrator declared, checked as one when it was.
pub fn relay(tools: &crate::ToolPaths, address: &str) -> Result<RelayedCamera> {
    let mut process = TokioCommand::new(&tools.ffmpeg)
        .args(arguments(address))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let output = process.stdout.take().ok_or_else(|| {
        FfmpegError::Spawn(std::io::Error::other("the tool gave no output to read"))
    })?;
    Ok(RelayedCamera { output, process })
}

/// What the tool is told, written as a value so it can be read without
/// running anything.
pub fn arguments(address: &str) -> Vec<OsString> {
    let mut arguments: Vec<OsString> = [
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        // Over the camera's own connection, which no firewall between loses
        // packets of, and taken as it comes rather than held to be sorted.
        "-rtsp_transport",
        "tcp",
        "-fflags",
        "nobuffer",
        "-flags",
        "low_delay",
        "-i",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    arguments.push(address.into());
    let piece = PIECE_US.to_string();
    arguments.extend(
        [
            "-map",
            "0:v:0",
            "-map",
            "0:a:0?",
            "-c:v",
            "copy",
            "-c:a",
            "aac",
            "-b:a",
            "96k",
            "-f",
            "mp4",
            "-movflags",
            "empty_moov+default_base_moof+frag_keyframe",
            "-frag_duration",
            &piece,
            "-flush_packets",
            "1",
            "pipe:1",
        ]
        .map(OsString::from),
    );
    arguments
}

#[cfg(test)]
mod tests {
    use super::*;

    fn written(arguments: &[OsString]) -> String {
        arguments
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn a_camera_is_copied_into_pieces_of_mp4_on_the_way_out() {
        let told = written(&arguments("rtsp://viewer:secret@192.0.2.20:554/stream1"));
        assert!(told.contains("-rtsp_transport tcp"));
        assert!(told.contains("-i rtsp://viewer:secret@192.0.2.20:554/stream1"));
        assert!(told.contains("-c:v copy"));
        assert!(
            told.contains("-map 0:a:0?"),
            "a camera with no sound still plays"
        );
        assert!(told.contains("-c:a aac"));
        assert!(told.contains("-f mp4"));
        assert!(told.contains("empty_moov+default_base_moof+frag_keyframe"));
        assert!(told.contains("-frag_duration 200000"));
        assert!(told.ends_with("pipe:1"));
    }

    #[test]
    fn the_address_is_handed_over_as_one_argument() {
        let address = "rtsp://192.0.2.20/live main";
        let told = arguments(address);
        let at = told
            .iter()
            .position(|value| value == "-i")
            .expect("an input");
        assert_eq!(told[at + 1], OsString::from(address));
    }
}
