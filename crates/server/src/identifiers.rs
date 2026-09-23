//! Identifiers read off a request.
//!
//! Every route takes its identifiers as text and has to say the same thing
//! when one of them is not an identifier at all. Written once per kind here,
//! so the wording a client sees does not depend on which route it knocked at.

use melyxar_core::id::{DeviceId, LibraryId, MediaSourceId, PlaybackClientId, TrackId, WorkId};

use crate::error::Result;
use crate::ServerError;

/// Reads one identifier, naming what was expected when it does not read.
fn parse<T: std::str::FromStr>(value: &str, what: &str) -> Result<T> {
    value
        .parse()
        .map_err(|_| ServerError::invalid_input(format!("the {what} identifier is malformed")))
}

pub(crate) fn parse_work(value: &str) -> Result<WorkId> {
    parse(value, "work")
}

pub(crate) fn parse_library(value: &str) -> Result<LibraryId> {
    parse(value, "library")
}

pub(crate) fn parse_source(value: &str) -> Result<MediaSourceId> {
    parse(value, "file")
}

pub(crate) fn parse_track(value: &str) -> Result<TrackId> {
    parse(value, "track")
}

pub(crate) fn parse_client(value: &str) -> Result<PlaybackClientId> {
    parse(value, "client")
}

pub(crate) fn parse_device(value: &str) -> Result<DeviceId> {
    parse(value, "device")
}
