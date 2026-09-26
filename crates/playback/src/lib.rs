//! Deciding how a media source reaches a client.
//!
//! Pure logic: no disk, no database, no clock. The same inputs always give the
//! same answer, which is what lets the whole decision tree be covered by tests
//! that run in milliseconds, and what makes the answer reproducible when the
//! maintainer reports something odd.
//!
//! Every answer carries its reasons. Without them, each "why is this
//! transcoding?" turns into a debugging session.

#![forbid(unsafe_code)]

pub mod decision;
pub mod profile;
pub mod subtitles;

pub use decision::{
    decide, PlaybackDecision, PlaybackMethod, PlaybackRequest, Reason, StreamAction,
    SubtitleDelivery,
};
pub use profile::{ClientProfile, VideoCapability};
pub use subtitles::subtitle_by_mode;
