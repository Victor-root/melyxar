//! Scanning folders and making sense of file names.
//!
//! Three jobs, kept apart so each can be tested on its own: finding out what
//! the server may do with a folder, walking that folder, and reading a title
//! out of a file name.
//!
//! Nothing here touches the database. A scan reports what it saw and the
//! orchestration layer decides what to do about it, which is what keeps this
//! crate testable against a temporary folder.

#![forbid(unsafe_code)]

pub mod access;
pub mod companion;
pub mod naming;
pub mod scan;
pub mod sidecar;

pub use access::check as check_root_access;
pub use naming::{parse as parse_file_name, sort_title, ParsedName};
pub use scan::{diff, walk, FoundFile, KnownFile, ScanDiff, ScanError, ScanOutcome};
