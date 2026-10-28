//! How far behind the camera the picture is, under the `camera-delay` tag:
//! every second, and every change of speed made to catch up.

use serde::Deserialize;

use super::{Said, Second};

pub(super) fn write_second(by: &Said, seen: &Second) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        state = seen.state.as_word(),
        playing = seen.playing,
        speed = seen.speed,
        lost_standing_still_ms = seen.lost_ms,
        "{}ms behind the camera",
        seen.behind_ms
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct CatchingUp {
    speed: f64,
    behind_ms: i64,
}

pub(super) fn write_catching_up(by: &Said, seen: &CatchingUp) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        behind_ms = seen.behind_ms,
        "played at {} to catch up",
        seen.speed
    );
}
