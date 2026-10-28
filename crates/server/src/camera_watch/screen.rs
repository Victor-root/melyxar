//! The screen around a camera's picture, under the `camera-screen` tag: its
//! life, its sizes, the memory, and the moments the application is held.

use serde::Deserialize;

use super::{Said, Second};

/// What happened to the screen.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum ScreenEvent {
    /// The page asked the phone for the camera.
    Asked,
    /// The page asked with an address that is not a camera's, refused.
    RefusedAddress,
    Made,
    Seen,
    NotSeen,
    Closed,
    /// The television's own button to open the camera again.
    Reconnect,
    /// The screen says the camera cannot be played.
    CannotPlay,
    /// The system asks the application for memory back.
    MemoryAsked,
}

impl ScreenEvent {
    fn as_word(self) -> &'static str {
        match self {
            Self::Asked => "the page asks for the camera",
            Self::RefusedAddress => "the page asked with an address that is not RTSP, refused",
            Self::Made => "screen made",
            Self::Seen => "screen seen, camera opened from now",
            Self::NotSeen => "screen no longer seen, camera stopped",
            Self::Closed => "screen closed",
            Self::Reconnect => "reconnect pressed",
            Self::CannotPlay => "the screen says the camera cannot be played",
            Self::MemoryAsked => "the system asks for memory back",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Screen {
    event: ScreenEvent,
    /// How hard the system asks for memory back, as Android numbers it.
    #[serde(default)]
    level: Option<i32>,
}

pub(super) fn write_screen(by: &Said, seen: &Screen) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        level = seen.level,
        "{}",
        seen.event.as_word()
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct Size {
    width: u32,
    height: u32,
}

pub(super) fn write_size(by: &Said, what: &str, seen: &Size) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        "{what} {}x{}",
        seen.width,
        seen.height
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct Held {
    ms: u64,
}

pub(super) fn write_held(by: &Said, seen: &Held) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        "the application was held {}ms between two of its frames",
        seen.ms
    );
}

pub(super) fn write_memory(by: &Said, seen: &Second) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        most_mb = seen.most_memory_mb,
        "memory in use {}MB",
        seen.memory_mb
    );
}
