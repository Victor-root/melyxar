//! Everything that failed around a camera, under the `camera-error` tag,
//! with the chain of its causes, and a crash of the application while a
//! camera was open.

use serde::Deserialize;

use super::Said;
use crate::page::cut_to;

/// How much of an error is written down.
const ENOUGH_OF_AN_ERROR: usize = 600;
const ENOUGH_OF_A_CODE: usize = 80;

/// Where it failed.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum During {
    /// Asking the server where the camera is.
    Server,
    /// The exchange with the camera.
    Rtsp,
    Player,
    Load,
    VideoCodec,
    AudioCodec,
    AudioSink,
    /// The application crashed, a camera open.
    Crash,
}

impl During {
    fn as_word(self) -> &'static str {
        match self {
            Self::Server => "asking the server where the camera is",
            Self::Rtsp => "the exchange with the camera",
            Self::Player => "the player",
            Self::Load => "receiving from the camera",
            Self::VideoCodec => "decoding the picture",
            Self::AudioCodec => "decoding the sound",
            Self::AudioSink => "playing the sound",
            Self::Crash => "the application crashed",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Failed {
    during: During,
    #[serde(default)]
    code: Option<String>,
    causes: String,
}

pub(super) fn write(by: &Said, seen: &Failed) {
    tracing::warn!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        code = seen.code.as_deref().map(|code| cut_to(code, ENOUGH_OF_A_CODE)),
        causes = cut_to(&seen.causes, ENOUGH_OF_AN_ERROR),
        "failed: {}",
        seen.during.as_word()
    );
}
