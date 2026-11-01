//! The presses on the player of music, under the `music-controls` tag.

use serde::Deserialize;

use super::Said;

/// What was pressed, as the player names its commands.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum Command {
    Play,
    Toggle,
    Pause,
    Resume,
    Stop,
    Next,
    Previous,
    Seek,
    SetVolume,
    SetMuted,
    ToggleShuffle,
    CycleRepeat,
    Jump,
    PlayNext,
    PlayLast,
    Remove,
    Move,
}

impl Command {
    fn as_word(self) -> &'static str {
        match self {
            Self::Play => "play these songs",
            Self::Toggle => "play or pause",
            Self::Pause => "pause",
            Self::Resume => "resume",
            Self::Stop => "stop",
            Self::Next => "next",
            Self::Previous => "previous",
            Self::Seek => "move in the song",
            Self::SetVolume => "volume",
            Self::SetMuted => "mute",
            Self::ToggleShuffle => "shuffle",
            Self::CycleRepeat => "repeat",
            Self::Jump => "a song of the queue",
            Self::PlayNext => "play next",
            Self::PlayLast => "play last",
            Self::Remove => "take out of the queue",
            Self::Move => "move in the queue",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Pressed {
    command: Command,
    /// Whether this tab holds the sound, or sends the press to the one that
    /// does.
    holds: bool,
    /// What the press carried when it is a number: a second, a volume, a
    /// place in the queue.
    #[serde(default)]
    value: Option<f64>,
}

pub(super) fn write(by: &Said, seen: &Pressed) {
    tracing::debug!(
        account = by.account,
        song = by.song.as_ref().map(tracing::field::display),
        holds = seen.holds,
        value = seen.value,
        "pressed: {}",
        seen.command.as_word()
    );
}
