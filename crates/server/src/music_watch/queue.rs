//! The queue of the player of music, under the `music-queue` tag: each time
//! it moves, and why.

use serde::Deserialize;

use super::{song_named, Said};
use crate::error::Result;

/// Why the queue moved.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Why {
    Picked,
    Next,
    Previous,
    Jumped,
    Ended,
    HandedOver,
    CouldNotPlay,
    Stopped,
    Resumed,
    FollowedAnotherTab,
    Shuffle,
    Repeat,
    Added,
    Removed,
    Moved,
    /// Nothing said why: worth chasing when it shows.
    Unknown,
}

impl Why {
    fn as_word(self) -> &'static str {
        match self {
            Self::Picked => "a song was picked",
            Self::Next => "the next one was asked for",
            Self::Previous => "the one before was asked for",
            Self::Jumped => "a song of the queue was picked",
            Self::Ended => "the song ended",
            Self::HandedOver => "the next song took over",
            Self::CouldNotPlay => "the song could not be played",
            Self::Stopped => "the music was stopped",
            Self::Resumed => "the queue of the last visit was taken up",
            Self::FollowedAnotherTab => "another tab said so",
            Self::Shuffle => "the shuffle was turned",
            Self::Repeat => "the repeat was turned",
            Self::Added => "songs were added",
            Self::Removed => "a song was taken out",
            Self::Moved => "a song was moved",
            Self::Unknown => "nothing said why",
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Repeat {
    Off,
    All,
    One,
}

impl Repeat {
    fn as_word(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::All => "all",
            Self::One => "one",
        }
    }
}

/// The queue after it moved.
#[derive(Debug, Deserialize)]
pub(super) struct Moved {
    why: Why,
    /// The song playing before.
    #[serde(default)]
    from_song: Option<String>,
    /// Where it stands now, and how many songs it holds.
    at: u32,
    length: u32,
    shuffle: bool,
    repeat: Repeat,
}

pub(super) fn write(by: &Said, seen: &Moved) -> Result<()> {
    let from_song = song_named(seen.from_song.as_deref())?;
    tracing::debug!(
        account = by.account,
        song = by.song.as_ref().map(tracing::field::display),
        from_song = from_song.as_ref().map(tracing::field::display),
        at = seen.at,
        length = seen.length,
        shuffle = seen.shuffle,
        repeat = seen.repeat.as_word(),
        "the queue moved: {}",
        seen.why.as_word()
    );
    Ok(())
}
