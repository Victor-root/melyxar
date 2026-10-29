//! What an Emby or a Jellyfin server kept of the people who used it, read from
//! its own files: its accounts, what each one watched and listened to, where
//! they left off, what they liked, their playlists and the collections made
//! by hand.
//!
//! Read from a copy, never from the files themselves: a server still running
//! keeps writing them, and opening them even to read takes locks and leaves
//! files behind. Nothing here knows about Melyxar's database or its rules: it
//! says what the other server kept, in its own words, and the server decides
//! what is taken up and where it goes.
//!
//! What is read is recognised by the tables found, never by a version number,
//! and a backup older than what is read here is refused with the version to
//! upgrade to rather than read wrongly.

mod copy;
mod emby;
mod guid;
pub mod matching;

use std::path::Path;

use serde::Serialize;
use time::OffsetDateTime;

/// The server a backup came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Server {
    Emby,
    Jellyfin,
}

impl Server {
    /// The oldest version whose files are read here.
    pub fn oldest_read(self) -> &'static str {
        match self {
            Self::Emby => "4.8",
            Self::Jellyfin => "10.9.11",
        }
    }
}

/// Everything a backup holds that can be taken up.
#[derive(Debug, Clone)]
pub struct Backup {
    pub server: Server,
    pub accounts: Vec<Account>,
    pub libraries: Vec<Library>,
    pub items: Vec<Item>,
    pub marks: Vec<Mark>,
    pub playlists: Vec<Playlist>,
    pub collections: Vec<Collection>,
}

/// One account of the other server. Its password is never read.
#[derive(Debug, Clone, PartialEq)]
pub struct Account {
    /// What the other server calls it, for the rest of the backup to name it.
    pub key: String,
    pub name: String,
    pub administrator: bool,
    pub disabled: bool,
    pub may_delete: bool,
    pub may_download: bool,
    /// The keys of the libraries it may see, or none for all of them.
    pub libraries: Option<Vec<String>>,
    /// As the other server wrote them, three letters most of the time.
    pub audio_language: Option<String>,
    pub subtitle_language: Option<String>,
}

/// A library of the other server, and the folders it was made of.
#[derive(Debug, Clone, PartialEq)]
pub struct Library {
    pub key: String,
    pub name: String,
    pub folders: Vec<String>,
}

/// What an item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Movie,
    Series,
    Season,
    Episode,
    /// A video that is none of the above: a home video, a recording.
    Video,
    Song,
    Album,
    Artist,
}

/// One film, series, episode, song, album or artist the other server knew,
/// with everything it can be found again by.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub key: String,
    pub kind: Kind,
    pub name: String,
    pub year: Option<i32>,
    /// The file, as the other server saw it: on another machine, or the same
    /// disks mounted elsewhere.
    pub path: Option<String>,
    /// The identifiers at providers, the provider in lower case
    /// (`tmdb`, `imdb`, `tvdb`, `musicbrainzalbum`...).
    pub ids: Vec<(String, String)>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    /// The series of an episode or a season, the album of a song.
    pub parent: Option<String>,
    /// The artists an album or a song is filed under.
    pub artists: Vec<String>,
}

/// What one account did with one item.
#[derive(Debug, Clone, PartialEq)]
pub struct Mark {
    pub account: String,
    pub item: String,
    pub played: bool,
    pub play_count: u32,
    /// Where it was left, in milliseconds; nought when not left partway.
    pub position_ms: i64,
    pub last_played: Option<OffsetDateTime>,
    pub favorite: bool,
}

/// A playlist, its items in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Playlist {
    pub name: String,
    /// The account it belongs to, or none for one shared with everybody.
    pub owner: Option<String>,
    pub items: Vec<String>,
}

/// A collection, its items in order.
#[derive(Debug, Clone, PartialEq)]
pub struct Collection {
    pub name: String,
    pub items: Vec<String>,
    /// Made by the other server from a provider rather than by somebody:
    /// `None` when the server keeps no way to tell.
    pub automatic: Option<bool>,
}

/// Why a folder could not be read.
#[derive(Debug, thiserror::Error)]
pub enum Refusal {
    #[error("nothing at {0}")]
    NotFound(String),
    #[error("no Emby or Jellyfin data in this folder")]
    NotAServer,
    #[error("{server:?} older than {}", server.oldest_read())]
    TooOld { server: Server },
    #[error("unreadable: {0}")]
    Unreadable(String),
}

impl From<sqlx::Error> for Refusal {
    fn from(error: sqlx::Error) -> Self {
        Self::Unreadable(error.to_string())
    }
}

impl From<std::io::Error> for Refusal {
    fn from(error: std::io::Error) -> Self {
        Self::Unreadable(error.to_string())
    }
}

/// Reads the data folder of an Emby or Jellyfin server, or a copy of it,
/// copying what it needs into `work` first, which is left for the caller to
/// clear.
pub async fn read(from: &Path, work: &Path) -> Result<Backup, Refusal> {
    if !from.exists() {
        return Err(Refusal::NotFound(from.display().to_string()));
    }
    if let Some(found) = emby::Found::at(from) {
        return emby::read(&found, work).await;
    }
    Err(Refusal::NotAServer)
}
