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
pub mod images;
pub mod probe;
pub mod process;
pub mod subtitles;
pub mod thumbnails;

use std::path::PathBuf;

pub use capabilities::{Capabilities, HardwareAcceleration};
pub use command::{AudioOutput, Command, Input, Output, StreamSelection, VideoOutput};
pub use hardware::{Card, CardSearch};
pub use probe::{ProbeReport, ProbeStream};
pub use process::{Progress, RunningProcess};

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

/// Where the two external tools live.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolPaths {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
}

impl ToolPaths {
    /// Locates the tools, honouring an explicit configuration first.
    ///
    /// The analyser is derived from the encoder when it is not given
    /// separately: the two ship together, and a maintainer who points at a
    /// specific build almost certainly wants both from the same place. A build
    /// maintained for media servers usually carries hardware support that a
    /// distribution package lacks, which is why the path is configurable at
    /// all.
    pub fn discover(
        configured_ffmpeg: Option<&std::path::Path>,
        configured_ffprobe: Option<&std::path::Path>,
    ) -> Result<Self> {
        let ffmpeg = match configured_ffmpeg {
            Some(path) => {
                if !path.exists() {
                    return Err(FfmpegError::ToolsMissing(format!(
                        "the configured encoder path does not exist: {}",
                        path.display()
                    )));
                }
                path.to_path_buf()
            }
            None => find_on_path("ffmpeg").ok_or_else(|| {
                FfmpegError::ToolsMissing(
                    "no encoder found; install one or set its path in the configuration".into(),
                )
            })?,
        };

        let ffprobe = match configured_ffprobe {
            Some(path) => {
                if !path.exists() {
                    return Err(FfmpegError::ToolsMissing(format!(
                        "the configured analyser path does not exist: {}",
                        path.display()
                    )));
                }
                path.to_path_buf()
            }
            None => sibling(&ffmpeg, "ffprobe")
                .or_else(|| find_on_path("ffprobe"))
                .ok_or_else(|| {
                    FfmpegError::ToolsMissing(
                        "no analyser found next to the encoder nor on the search path".into(),
                    )
                })?,
        };

        Ok(Self { ffmpeg, ffprobe })
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tools_are_found_on_the_search_path() {
        let paths = ToolPaths::discover(None, None).expect("the tools are installed here");
        assert!(paths.ffmpeg.exists());
        assert!(paths.ffprobe.exists());
    }

    #[test]
    fn a_configured_path_that_does_not_exist_is_reported_clearly() {
        let error = ToolPaths::discover(Some(std::path::Path::new("/nowhere/ffmpeg")), None)
            .expect_err("a missing tool must be reported");
        assert!(matches!(error, FfmpegError::ToolsMissing(_)));
        assert!(error.to_string().contains("/nowhere/ffmpeg"));
    }

    #[test]
    fn the_analyser_is_taken_from_beside_the_configured_encoder() {
        let discovered = ToolPaths::discover(None, None).expect("the tools are installed here");
        let paths = ToolPaths::discover(Some(&discovered.ffmpeg), None).expect("tools resolve");
        assert_eq!(
            paths.ffprobe.parent(),
            discovered.ffmpeg.parent(),
            "both tools ship together and must come from the same place"
        );
    }
}
