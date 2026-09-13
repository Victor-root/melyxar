//! Playback sessions: the playlist the server owns, and the segments a media
//! tool produces on demand.
//!
//! Two rules shape everything here, both from the architecture notes.
//!
//! The server owns the playlist. It knows how long the film is before any of
//! it has been produced, so it writes the whole list at once and hands out any
//! segment that is asked for, starting the tool at that point if it has to.
//! Letting the tool write a playlist that grows as it goes is the usual
//! shortcut, and it turns moving through a film into a special case.
//!
//! And a session owns a folder and a process. Both are cleaned up: a tool left
//! running after a viewer closed the tab is a core of a machine spent on
//! nobody.

#![forbid(unsafe_code)]

pub mod playlist;

#[derive(Debug, thiserror::Error)]
pub enum StreamingError {
    #[error("the media tool failed: {0}")]
    MediaTool(#[from] melyxar_ffmpeg::FfmpegError),
    #[error("could not prepare the working folder: {0}")]
    Folder(#[from] std::io::Error),
    #[error("no session with that identifier")]
    NoSuchSession,
    #[error("that segment is not part of this film")]
    NoSuchSegment,
    #[error("the segment was not produced in time")]
    TooSlow,
    #[error("too many films are being converted at once")]
    TooManyAtOnce,
}

pub type Result<T> = std::result::Result<T, StreamingError>;
