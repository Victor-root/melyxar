//! How far behind the camera the picture is, under the `camera-delay` tag:
//! every few seconds, and every change of speed made to catch up.

use serde::Deserialize;

use super::{Said, Summary};

pub(super) fn write_summary(by: &Said, seen: &Summary) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        state = seen.state.as_word(),
        playing = seen.playing,
        most_behind_ms = seen.most_behind_ms,
        pictures_in_decoder = seen.in_decoder,
        speed = seen.speed,
        fell_behind_since_playing_ms = seen.fell_behind_ms,
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
