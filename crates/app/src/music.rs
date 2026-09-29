//! Music: what the server does with a library of songs.
//!
//! Kept apart from everything that plays and describes films, as decided in
//! `docs/architecture/06-musique.md`: what goes wrong with music stays with
//! music. Nothing outside this module and its children calls into music, save
//! the scan, which hands a library of music over to it, and the routes of the
//! server that read it.

pub mod browse;
pub(crate) mod pictures;
pub(crate) mod scan;
