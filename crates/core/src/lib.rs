//! Domain types and business rules shared by every Melyxar crate.
//!
//! This crate has no technical dependency: no database, no HTTP, no process
//! spawning. Everything here is plain data and pure functions, so it can be
//! used by any layer and tested without any setup.

#![forbid(unsafe_code)]

pub mod error;
pub mod id;
pub mod library;
pub mod media;
pub mod privacy;
pub mod time;
pub mod user;
pub mod work;

pub use error::{Error, Result};
