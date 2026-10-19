//! The only crate in Melyxar that launches the external media tools.
//!
//! Both the analyser and the encoder are ordinary programs started as child
//! processes. Keeping every launch behind this one crate means process
//! supervision, graceful shutdown and capability detection exist once, rather
//! than being reinvented by whoever needs a frame or a stream. It is also the
//! reason an orphaned encoder has a single place to hide.
//!
//! Commands are built as values, never as concatenated strings. A command is a
//! structure that turns itself into arguments at the very end, which makes it
//! testable without running anything and removes the whole family of bugs
//! caused by option order and by quoting.

#![forbid(unsafe_code)]

pub mod calibration;
pub mod capabilities;
pub mod command;
pub mod hardware;
pub mod heard_words;
pub mod images;
pub mod listening;
pub mod painting;
pub mod probe;
pub mod formats;
pub mod process;
mod reading;
pub mod song_analysis;
pub mod song_cutting;
pub mod speech;
pub mod spoken_lines;
pub mod sound;
pub mod subtitles;
pub mod thumbnails;

use std::path::PathBuf;

pub use capabilities::{Capabilities, HardwareAcceleration};
pub use command::{AudioOutput, Command, Input, Output, StreamSelection, VideoOutput};
pub use hardware::{Card, CardPath, CardSearch, ToneMapping, WorkTally};
pub use probe::{ProbeReport, ProbeStream};
pub use process::{AskedToStop, Progress, RunningProcess};

#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    #[error("the media tools were not found: {0}")]
    ToolsMissing(String),
    #[error("the media tool could not be started: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("{tool} exited with status {status}: {output}")]
    Failed {
        tool: &'static str,
        status: String,
        output: String,
    },
    #[error("the analyser returned something unreadable: {0}")]
    MalformedReport(String),
    #[error("the reading was given up on before the tool had finished")]
    GivenUp,
}

impl FfmpegError {
    /// What a tool that would not do what it was asked said about it.
    ///
    /// Written once because it was written seven times, in two shapes that
    /// disagreed: five kept the whole of what the tool complained about and
    /// two kept the first few lines of it. A journal line is a line, and a
    /// tool that fails on every track of a film can say a great deal, so the
    /// shorter rule wins and is now the only one. What is lost is never the
    /// first thing said, which is what names the fault.
    pub fn from_output(tool: &'static str, output: &std::process::Output) -> Self {
        Self::Failed {
            tool,
            status: output.status.to_string(),
            output: Self::what_it_complained_about(output),
        }
    }

    /// How much of a tool's complaint is worth carrying into a journal line.
    pub fn what_it_complained_about(output: &std::process::Output) -> String {
        String::from_utf8_lossy(&output.stderr)
            .lines()
            .filter(|line| !line.trim().is_empty())
            .take(ENOUGH_OF_A_COMPLAINT)
            .collect::<Vec<_>>()
            .join(" | ")
    }
}

/// How many lines of a tool's complaint are kept.
const ENOUGH_OF_A_COMPLAINT: usize = 5;

pub type Result<T> = std::result::Result<T, FfmpegError>;

/// Where the installation script puts ffmpeg-melyxar, the FFmpeg release
/// built with Melyxar's own changes, its analyser beside it.
pub const FFMPEG_MELYXAR: &str = "/opt/melyxar/ffmpeg/bin/ffmpeg";

/// Where the two external tools live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPaths {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    /// Which formats a file the server did not make itself may be read as:
    /// every reader this build of the tool has, less those that open other
    /// inputs. Asked of the tool once, here, so every reading of such a file
    /// can be held to it. See [`crate::formats`].
    allowed_formats: std::sync::Arc<str>,
}

impl ToolPaths {
    /// Which formats an untrusted file may be read as.
    pub fn allowed_formats(&self) -> &str {
        &self.allowed_formats
    }
}

impl ToolPaths {
    /// ffmpeg-melyxar, where the installation script puts it, and nothing
    /// else: no other build is looked for, so no other build can be run by
    /// mistake, and every film is played by the one tool whose behaviour is
    /// known.
    pub fn discover() -> Result<Self> {
        Self::at(std::path::Path::new(FFMPEG_MELYXAR))
    }

    /// The encoder at `ffmpeg` and its analyser beside it, as the two ship.
    pub fn at(ffmpeg: &std::path::Path) -> Result<Self> {
        if !ffmpeg.is_file() {
            return Err(FfmpegError::ToolsMissing(format!(
                "ffmpeg-melyxar is not installed at {}",
                ffmpeg.display()
            )));
        }
        let ffprobe = sibling(ffmpeg, "ffprobe").ok_or_else(|| {
            FfmpegError::ToolsMissing(format!(
                "no analyser beside the encoder at {}",
                ffmpeg.display()
            ))
        })?;

        // Asked once, here, rather than on every reading. A build with no
        // listing to give is held to nothing rather than refused: the tool is
        // there and works, and the guard is one defence among others. Said in
        // the journal, since a defence that is off must never be off quietly.
        let allowed_formats: String = std::process::Command::new(ffmpeg)
            .args(["-hide_banner", "-demuxers"])
            .stdin(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| crate::formats::allowed_from(&String::from_utf8_lossy(&output.stdout)))
            .unwrap_or_default();
        if allowed_formats.is_empty() {
            tracing::warn!(
                encoder = %ffmpeg.display(),
                "the encoder gave no list of the formats it reads, so files are not held to \
                 the formats they may be"
            );
        }

        Ok(Self {
            ffmpeg: ffmpeg.to_path_buf(),
            ffprobe,
            allowed_formats: allowed_formats.into(),
        })
    }
}

/// Looks for a tool next to another one, which is how the two are packaged.
fn sibling(reference: &std::path::Path, name: &str) -> Option<PathBuf> {
    let candidate = reference.parent()?.join(name);
    candidate.exists().then_some(candidate)
}

/// Searches the directories listed in the environment search path.
fn find_on_path(name: &str) -> Option<PathBuf> {
    let search_path = std::env::var_os("PATH")?;
    std::env::split_paths(&search_path)
        .map(|directory| directory.join(name))
        .find(|candidate| candidate.is_file())
}

/// The tools the tests run against.
#[cfg(any(test, feature = "testing"))]
pub mod testing {
    use super::{find_on_path, ToolPaths, FFMPEG_MELYXAR};

    /// ffmpeg-melyxar where it is installed, and on a machine that only
    /// develops Melyxar, the encoder on its search path. The server itself
    /// never looks there.
    pub fn tools() -> ToolPaths {
        let ours = std::path::Path::new(FFMPEG_MELYXAR);
        let ffmpeg = if ours.is_file() {
            ours.to_path_buf()
        } else {
            find_on_path("ffmpeg").expect("an encoder is installed on the machine running the tests")
        };
        ToolPaths::at(&ffmpeg).expect("its analyser is installed beside it")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_analyser_is_taken_from_beside_the_encoder() {
        let tools = testing::tools();
        assert!(tools.ffmpeg.exists());
        assert_eq!(
            tools.ffprobe.parent(),
            tools.ffmpeg.parent(),
            "both tools ship together and must come from the same place"
        );
    }

    #[test]
    fn a_build_that_is_not_there_is_reported_clearly() {
        let error = ToolPaths::at(std::path::Path::new("/nowhere/ffmpeg"))
            .expect_err("a missing tool must be reported");
        assert!(matches!(error, FfmpegError::ToolsMissing(_)));
        assert!(error.to_string().contains("/nowhere/ffmpeg"));
    }
}
