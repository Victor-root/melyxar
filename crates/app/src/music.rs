//! Music: what the server does with a library of songs.
//!
//! Kept apart from everything that plays and describes films, as decided in
//! `docs/architecture/06-musique.md`: what goes wrong with music stays with
//! music. Nothing outside this module and its children calls into music, save
//! the scan, which hands a library of music over to it, the list of
//! libraries, which counts one in albums, and the routes of the server that
//! read it.

pub mod browse;
pub mod covers;
pub mod listen;
pub mod loudness;
pub mod lyrics;
pub mod marks;
pub mod playlists;
pub mod tag_editing;
pub(crate) mod pictures;
pub mod preferences;
pub(crate) mod scan;
