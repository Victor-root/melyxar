//! Which tab holds the sound of the music, under the `music-tabs` tag: of
//! every tab of a browser, or of the application, one plays and the others
//! follow it.

use serde::Deserialize;

use super::Said;

/// What a tab became.
#[derive(Debug, Deserialize)]
pub(super) struct Tab {
    /// Whether it holds the sound.
    holds: bool,
    /// Whether it shows what another tab plays.
    following: bool,
}

pub(super) fn write(by: &Said, seen: &Tab) {
    let what = match (seen.holds, seen.following) {
        (true, _) => "a tab holds the sound",
        (false, true) => "a tab follows another that holds the sound",
        (false, false) => "a tab holds no sound and follows none",
    };
    tracing::debug!(account = by.account, song = by.song.as_ref().map(tracing::field::display), "{what}");
}
