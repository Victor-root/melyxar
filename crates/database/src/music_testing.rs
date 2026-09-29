//! A small collection of music every test of music reads, written once.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, LibraryRootId, MediaSourceId};
use melyxar_core::library::LibraryKind;
use melyxar_core::music::{AlbumFiling, Named, SongFiling};
use melyxar_core::time::now;

use crate::Database;
use crate::music::MusicFile;

pub(crate) fn named(name: &str) -> Named {
    Named {
        name: name.to_string(),
        // Sorted the way the filing sorts them: without capitals and
        // without a leading article.
        sort_name: name.to_lowercase().trim_start_matches("the ").to_string(),
    }
}

struct Song<'a> {
    path: &'a str,
    title: &'a str,
    artists: &'a [&'a str],
    album: &'a str,
    album_artists: &'a [&'a str],
    track: u32,
    year: i32,
    genre: &'a str,
}

fn file(song: Song<'_>) -> MusicFile {
    MusicFile {
        source_id: MediaSourceId::new(),
        song: None,
        relative_path: PathBuf::from(song.path),
        size_bytes: 10,
        modified_at: now(),
        filing: SongFiling {
            title: named(song.title),
            artists: song.artists.iter().map(|a| named(a)).collect(),
            album: Some(AlbumFiling {
                title: named(song.album),
                artists: song.album_artists.iter().map(|a| named(a)).collect(),
                is_compilation: song.album_artists == ["Various Artists"],
            }),
            track: Some(song.track),
            disc: Some(1),
            year: Some(song.year),
            genres: vec![song.genre.to_string()],
        },
        reading: Err("not read in this test".to_string()),
    }
}

/// Two albums of Amber Field, one of The Lanterns on which Amber Field
/// sings once, and a compilation.
pub(crate) async fn collection() -> (Database, LibraryId, LibraryRootId) {
    let database = Database::open_in_memory().await.expect("opens");
    let library = database
        .create_library(
            "Music",
            LibraryKind::Music,
            "fr",
            &[("disk-one".to_string(), PathBuf::from("/mnt/one"))],
        )
        .await
        .expect("library");
    let root = library.roots[0].id;
    let songs = [
        Song {
            path: "A/Lights/02.flac",
            title: "Tides",
            artists: &["Amber Field"],
            album: "Northern Lights",
            album_artists: &["Amber Field"],
            track: 2,
            year: 2019,
            genre: "Folk",
        },
        Song {
            path: "A/Lights/01.flac",
            title: "Quiet Harbour",
            artists: &["Amber Field"],
            album: "Northern Lights",
            album_artists: &["Amber Field"],
            track: 1,
            year: 2019,
            genre: "Folk",
        },
        Song {
            path: "A/Early/01.flac",
            title: "Beginnings",
            artists: &["Amber Field"],
            album: "Early Days",
            album_artists: &["Amber Field"],
            track: 1,
            year: 2011,
            genre: "Pop",
        },
        Song {
            path: "L/Road/01.flac",
            title: "The Long Road",
            artists: &["The Lanterns", "Amber Field"],
            album: "Road",
            album_artists: &["The Lanterns"],
            track: 1,
            year: 2015,
            genre: "Rock",
        },
        Song {
            path: "V/Hits/01.flac",
            title: "Zebra",
            artists: &["Quartz"],
            album: "Summer Hits",
            album_artists: &["Various Artists"],
            track: 1,
            year: 2020,
            genre: "Pop",
        },
    ];
    database
        .file_music(library.id, root, &songs.map(file))
        .await
        .expect("filed");
    (database, library.id, root)
}

