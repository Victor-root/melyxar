//! What a music file says about itself.
//!
//! A song is filed by what its file carries: the title, the artists, the album
//! and where on it the song sits. Every one of those is written inside the file
//! by whoever ripped or bought it, so reading the file is how a library of music
//! is put together, the folders only helping where the file says nothing. See
//! `docs/architecture/06-musique.md`.
//!
//! Read here in the process rather than by the analyser the films go through:
//! launching a tool costs tens of milliseconds a file, which on a collection of
//! a hundred thousand songs is an hour of scanning, where reading the tags in
//! place costs a few microseconds. The library that does it stays in this crate
//! and nowhere else, so the rest of the server deals in songs and never in the
//! way one particular library spells an artist.
//!
//! Nothing here depends on another crate of Melyxar, and nothing here knows
//! about a database, a scan or a job: it is handed a path and says what it
//! found.

mod cover;
mod pairs;
mod read;
mod values;

pub use cover::{front_cover, Cover};
pub use pairs::from_pairs;
pub use read::read;

/// Everything read out of one music file.
#[derive(Debug, Clone, PartialEq)]
pub struct AudioFile {
    pub tags: Tags,
    pub sound: Sound,
}

/// What whoever tagged the file wrote in it. Anything they left out is absent,
/// never guessed: guessing from the folders is the filing's job, which sees the
/// whole folder at once.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tags {
    pub title: Option<String>,
    /// How the title is sorted, when the file says so.
    pub title_sort: Option<String>,
    /// Who plays this song, in the order the file gives them. Several when the
    /// file holds several values, or one value with the names separated by a
    /// semicolon.
    pub artists: Vec<String>,
    /// How the first of them is sorted, when the file says so.
    pub artist_sort: Option<String>,
    pub album: Option<String>,
    pub album_sort: Option<String>,
    /// Whose album this is, which is not always who plays each song on it: a
    /// guest on one track does not make the album theirs.
    pub album_artists: Vec<String>,
    pub album_artist_sort: Option<String>,
    pub track: Option<u32>,
    pub track_total: Option<u32>,
    pub disc: Option<u32>,
    pub disc_total: Option<u32>,
    /// The year the song came out, read from a full date when that is what the
    /// file carries.
    pub year: Option<i32>,
    pub genres: Vec<String>,
    /// Whether the file says it belongs to a compilation, an album of songs by
    /// various artists.
    pub compilation: bool,
}

/// How the file sounds and how it is packed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sound {
    /// The container, named as the analyser of the films names it, so a song
    /// read here and one read by that analyser describe themselves the same.
    pub container: &'static str,
    /// The codec, named the same way.
    pub codec: &'static str,
    pub duration_ms: u64,
    /// The whole file, kilobits a second.
    pub overall_bitrate: Option<u32>,
    /// The sound alone, kilobits a second.
    pub audio_bitrate: Option<u32>,
    pub sample_rate: Option<u32>,
    pub channels: Option<u8>,
    pub bit_depth: Option<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// A form this reader does not know, such as WMA. Not a broken file: the
    /// analyser of the films may well read it.
    #[error("this kind of file is not one the tag reader knows")]
    Unsupported,
    /// A form it knows, that it could not make sense of.
    #[error("the file could not be read: {0}")]
    Unreadable(String),
}
