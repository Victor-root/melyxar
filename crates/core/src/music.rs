//! What a song, an album and an artist are, once a library has filed them.
//!
//! Worked out by the filing of a folder, in the crate that walks the disk,
//! and written down by the database as it is: the two meet on these plain
//! values and on nothing of each other.

/// A name as it is shown, and as it is sorted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Named {
    pub name: String,
    pub sort_name: String,
}

/// The album a song goes on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumFiling {
    pub title: Named,
    /// Whose album it is. Empty when nobody could be told, which leaves the
    /// album under no artist rather than under a made up one.
    pub artists: Vec<Named>,
    pub is_compilation: bool,
}

/// Where one song goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongFiling {
    pub title: Named,
    /// Who plays it.
    pub artists: Vec<Named>,
    /// Absent for a song that sits at the top of a library and says nothing
    /// of an album: it is a song on its own, which is found among the songs.
    pub album: Option<AlbumFiling>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub year: Option<i32>,
    pub genres: Vec<String>,
}
