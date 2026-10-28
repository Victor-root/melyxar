//! The exchange with the camera, under the `camera-net` tag: each step of
//! RTSP and its answer, the transport agreed on, and the reopenings.

use serde::Deserialize;

use super::Said;

/// A step of the exchange with the camera, as RTSP names them.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Step {
    Options,
    Describe,
    Setup,
    Play,
    Pause,
    Teardown,
    GetParameter,
    SetParameter,
    Other,
}

impl Step {
    fn as_word(self) -> &'static str {
        match self {
            Self::Options => "OPTIONS",
            Self::Describe => "DESCRIBE",
            Self::Setup => "SETUP",
            Self::Play => "PLAY",
            Self::Pause => "PAUSE",
            Self::Teardown => "TEARDOWN",
            Self::GetParameter => "GET_PARAMETER",
            Self::SetParameter => "SET_PARAMETER",
            Self::Other => "another step",
        }
    }
}

/// How the pictures travel: in their own packets, which a network can lose,
/// or within the connection the exchange runs on.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Transport {
    Udp,
    Tcp,
}

impl Transport {
    fn as_word(self) -> &'static str {
        match self {
            Self::Udp => "UDP",
            Self::Tcp => "TCP",
        }
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct Exchange {
    step: Step,
    /// The camera's answer, none while it has not answered.
    #[serde(default)]
    status: Option<u16>,
    #[serde(default)]
    answer_ms: Option<u64>,
    #[serde(default)]
    transport: Option<Transport>,
}

pub(super) fn write_exchange(by: &Said, seen: &Exchange) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        status = seen.status,
        answer_ms = seen.answer_ms,
        transport = seen.transport.map(Transport::as_word),
        "{} {}",
        seen.step.as_word(),
        if seen.status.is_some() { "answered" } else { "asked" }
    );
}

#[derive(Debug, Deserialize)]
pub(super) struct ReopenedWithoutSound {
    /// What had come from the camera while the player still waited.
    received_ms: i64,
}

pub(super) fn write_reopened(by: &Said, seen: &ReopenedWithoutSound) {
    tracing::debug!(
        account = by.account,
        camera = by.camera,
        screen = by.screen,
        at_ms = by.at_ms,
        received_ms = seen.received_ms,
        "the sound held the picture, opened again without it"
    );
}
