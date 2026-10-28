//! How a camera is reached and how long to its first picture, under the
//! `camera-open` tag: the opening, the streams the camera offers, the
//! decoders made for them, the first picture and the letting go.

use serde::Deserialize;

use super::Said;
use crate::page::cut_to;

/// How much of a codec or a decoder's name is written down.
const ENOUGH_OF_A_NAME: usize = 80;

/// What a stream or a decoder carries.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Kind {
    Video,
    Audio,
    Other,
}

impl Kind {
    pub(super) fn as_word(self) -> &'static str {
        match self {
            Self::Video => "picture",
            Self::Audio => "sound",
            Self::Other => "other",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Opened {
    /// How long the server took to say where the camera is, when it was
    /// asked by the application rather than handed by the page.
    #[serde(default)]
    server_answer_ms: Option<u64>,
}

pub(super) fn write_opened(by: &Said, seen: &Opened) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        server_answer_ms = seen.server_answer_ms,
        "camera opened"
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct Stream {
    kind: Kind,
    #[serde(default)]
    codec: Option<String>,
    #[serde(default)]
    width: Option<u32>,
    #[serde(default)]
    height: Option<u32>,
    #[serde(default)]
    fps: Option<f64>,
    #[serde(default)]
    sample_rate: Option<u32>,
    #[serde(default)]
    channels: Option<u32>,
    supported: bool,
    selected: bool,
}

pub(super) fn write_stream(by: &Said, seen: &Stream) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        codec = seen.codec.as_deref().map(|codec| cut_to(codec, ENOUGH_OF_A_NAME)),
        width = seen.width,
        height = seen.height,
        fps = seen.fps,
        sample_rate = seen.sample_rate,
        channels = seen.channels,
        supported = seen.supported,
        selected = seen.selected,
        "the camera offers a stream of {}",
        seen.kind.as_word()
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct Decoder {
    kind: Kind,
    name: String,
    ready_ms: u64,
}

pub(super) fn write_decoder(by: &Said, seen: &Decoder) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        decoder = cut_to(&seen.name, ENOUGH_OF_A_NAME),
        ready_ms = seen.ready_ms,
        "decoder of the {} ready",
        seen.kind.as_word()
    );
}

/// The first picture, and when each step before it ended, in milliseconds
/// since the camera was asked to open.
#[derive(Debug, Deserialize)]
pub(super) struct FirstPicture {
    #[serde(default)]
    first_data_ms: Option<u64>,
    #[serde(default)]
    streams_ms: Option<u64>,
    #[serde(default)]
    decoder_ms: Option<u64>,
}

pub(super) fn write_first_picture(by: &Said, seen: &FirstPicture) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        first_data_ms = seen.first_data_ms,
        streams_known_ms = seen.streams_ms,
        decoder_ready_ms = seen.decoder_ms,
        "first picture after {}ms",
        by.at_ms
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct LetGo {
    frames: u64,
}

pub(super) fn write_let_go(by: &Said, seen: &LetGo) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        frames = seen.frames,
        "camera let go after {}ms",
        by.at_ms
    );
}
