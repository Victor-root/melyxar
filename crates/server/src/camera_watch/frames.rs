//! The pictures of a camera, under the `camera-frames` tag: received, shown
//! and dropped every second, and the moments the picture stands still or
//! waits for the camera.

use serde::Deserialize;

use super::{Said, Second};

pub(super) fn write_second(by: &Said, seen: &Second) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        received = seen.received,
        shown = seen.shown,
        dropped = seen.dropped,
        "pictures in the last second"
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct StoodStill {
    seconds: u32,
    /// Pictures received from the camera meanwhile.
    received: i64,
}

pub(super) fn write_stood_still(by: &Said, seen: &StoodStill) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        received = seen.received,
        "the picture stood still for {}s while playing",
        seen.seconds
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct Waiting {
    in_hand_ms: i64,
}

pub(super) fn write_waiting(by: &Said, seen: &Waiting) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        in_hand_ms = seen.in_hand_ms,
        "waiting for the camera"
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct MovingAgain {
    waited_ms: u64,
}

pub(super) fn write_moving_again(by: &Said, seen: &MovingAgain) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        "picture moving again after waiting {}ms",
        seen.waited_ms
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct Dropped {
    frames: u32,
    within_ms: u64,
}

pub(super) fn write_dropped(by: &Said, seen: &Dropped) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        within_ms = seen.within_ms,
        "{} pictures dropped, too late to show",
        seen.frames
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct SoundRanDry {
    last_fed_ms: u64,
}

pub(super) fn write_sound_ran_dry(by: &Said, seen: &SoundRanDry) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        "the sound ran dry, last fed {}ms before",
        seen.last_fed_ms
    );
}
