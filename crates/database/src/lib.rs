//! The only crate in Melyxar that talks SQL.
//!
//! Everything else asks this crate for domain values and never sees a row.
//! That boundary is what keeps the schema free to change without breaking the
//! rest of the server, and what keeps the database out of the API surface.
//!
//! Three rules hold here and are not negotiable, because they are what lets a
//! library stay browsable while a scan is running:
//!
//! * the journal runs in write-ahead mode, so readers never wait on a writer;
//! * exactly one connection may write, which removes lock contention entirely;
//! * write transactions are short and never wrap an external call.

#![forbid(unsafe_code)]

pub mod browse;
pub mod calibration;
pub mod catalogue;
pub mod images;
pub mod jobs;
pub mod libraries;
pub mod metadata;
pub mod playback;
pub mod settings;
pub mod synthetic;
pub mod users;

mod connection;
mod convert;

pub use connection::{Database, DatabaseError, Result};
pub use convert::{parse_optional_timestamp, parse_timestamp, timestamp_to_text};

/// Migrations shipped with the binary, applied at start-up.
pub static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");
