//! The two decks of the player of music, under the `music-decks` tag: what
//! each one did, and what the page asked of it.

use serde::Deserialize;

use super::{song_named, Said};
use crate::error::Result;
use crate::page::cut_to;

/// How much of why a deck could not play is written down.
const ENOUGH_OF_AN_ERROR: usize = 160;

/// What a deck said, named as an element of sound names it.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum EventName {
    Loadedmetadata,
    Canplay,
    Play,
    Playing,
    Waiting,
    Pause,
    Ended,
    Error,
    Emptied,
}

impl EventName {
    fn as_word(self) -> &'static str {
        match self {
            Self::Loadedmetadata => "its length known",
            Self::Canplay => "ready to play",
            Self::Play => "asked to play",
            Self::Playing => "sounding",
            Self::Waiting => "waiting for the sound",
            Self::Pause => "paused",
            Self::Ended => "ended",
            Self::Error => "could not play",
            Self::Emptied => "let go of its song",
        }
    }
}

/// What one deck said.
#[derive(Debug, Deserialize)]
pub(super) struct Event {
    deck: u8,
    live_deck: u8,
    event: EventName,
    /// Where the deck is in what it plays, and its length when known.
    second: f64,
    #[serde(default)]
    length: Option<f64>,
    paused: bool,
    /// The song the deck holds.
    #[serde(default)]
    deck_song: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

pub(super) fn write_event(by: &Said, seen: &Event) -> Result<()> {
    let deck_song = song_named(seen.deck_song.as_deref())?;
    tracing::debug!(
        account = by.account,
        song = by.song.as_ref().map(tracing::field::display),
        deck = seen.deck,
        live = seen.deck == seen.live_deck,
        second = seen.second,
        length = seen.length,
        paused = seen.paused,
        deck_song = deck_song.as_ref().map(tracing::field::display),
        error = seen.error.as_deref().map(|error| cut_to(error, ENOUGH_OF_AN_ERROR)),
        "a deck {}",
        seen.event.as_word()
    );
    Ok(())
}

/// What the page asked of a deck, or did with it.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Action {
    /// A song asked for in the deck playing.
    Load,
    /// The next song made ready in the other deck.
    Prepare,
    Play,
    Pause,
    Seek,
    /// The song the deck held let go of.
    LetGo,
    /// The next song asked to take over at a moment of this deck's clock.
    HandOverSet,
    /// That called off.
    HandOverCalledOff,
    /// The deck made ready taking over.
    HandOverFollowed,
    /// The deck handed over from let go of once its fade was done.
    LetGoAfterTheFade,
    /// Kept instead, having been given another song meanwhile.
    KeptAfterTheFade,
}

impl Action {
    fn as_word(self) -> &'static str {
        match self {
            Self::Load => "asked for a song",
            Self::Prepare => "made the next song ready",
            Self::Play => "asked to play",
            Self::Pause => "asked to pause",
            Self::Seek => "asked to move",
            Self::LetGo => "let go of its song",
            Self::HandOverSet => "set to take over",
            Self::HandOverCalledOff => "taking over called off",
            Self::HandOverFollowed => "took over",
            Self::LetGoAfterTheFade => "let go of once the fade was done",
            Self::KeptAfterTheFade => "kept after the fade, holding another song since",
        }
    }
}

/// One thing asked of a deck.
#[derive(Debug, Deserialize)]
pub(super) struct Asked {
    deck: u8,
    action: Action,
    /// The song it concerns.
    #[serde(default)]
    deck_song: Option<String>,
    /// Where on the deck's clock: the start asked for, the moment of a hand
    /// over, where it is moved to.
    #[serde(default)]
    at_second: Option<f64>,
    #[serde(default)]
    fade_seconds: Option<f64>,
    #[serde(default)]
    and_play: Option<bool>,
}

pub(super) fn write_asked(by: &Said, seen: &Asked) -> Result<()> {
    let deck_song = song_named(seen.deck_song.as_deref())?;
    tracing::debug!(
        account = by.account,
        song = by.song.as_ref().map(tracing::field::display),
        deck = seen.deck,
        deck_song = deck_song.as_ref().map(tracing::field::display),
        at_second = seen.at_second,
        fade_seconds = seen.fade_seconds,
        and_play = seen.and_play,
        "a deck {}",
        seen.action.as_word()
    );
    Ok(())
}
