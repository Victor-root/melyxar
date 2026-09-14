//! Domain types and business rules shared by every Melyxar crate.
//!
//! This crate has no technical dependency: no database, no HTTP, no process
//! spawning. Everything here is plain data and pure functions, so it can be
//! used by any layer and tested without any setup.

#![forbid(unsafe_code)]

pub mod error;
pub mod fingerprint;
pub mod id;
pub mod job;
pub mod journal;
pub mod library;
pub mod media;
pub mod privacy;
pub mod time;
pub mod user;
pub mod work;

pub use error::{Error, Result};

/// Which build of the server this is: the version, and the commit it was made
/// from.
///
/// Carried so that a report says which build produced it. A version number
/// alone changes when somebody remembers to change it, and every account of a
/// problem would otherwise begin by asking whether the fix is even in there.
pub const BUILD: &str = concat!(env!("CARGO_PKG_VERSION"), " (", env!("MELYXAR_BUILD"), ")");
