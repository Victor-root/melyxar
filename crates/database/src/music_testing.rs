//! A small collection of music every test of music reads, written once.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, LibraryRootId, MediaSourceId, TrackId};
use melyxar_core::library::LibraryKind;
use melyxar_core::media::{AudioDetails, Loudness, Track, TrackKind};
use melyxar_core::music::{AlbumFiling, Named, SongFiling};
use melyxar_core::time::{Millis, now};

use crate::Database;
use crate::catalogue::SourceAnalysis;
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

/// A song on no album whose file was read: one second of sound in `codec`,
/// as loud as `loudness` says.
pub(crate) fn a_read_song(
    relative_path: &str,
    title: &str,
    codec: &str,
    loudness: Loudness,
) -> MusicFile {
    let source_id = MediaSourceId::new();
    MusicFile {
        source_id,
        song: None,
        relative_path: PathBuf::from(relative_path),
        size_bytes: 10,
        modified_at: now(),
        filing: SongFiling {
            title: named(title),
            artists: Vec::new(),
            album: None,
            track: None,
            disc: None,
            year: None,
            genres: Vec::new(),
        },
        reading: Ok((
            SourceAnalysis {
                container: Some("mov,mp4,m4a,3gp,3g2,mj2".to_string()),
                duration: Some(Millis::new(1_000)),
                overall_bitrate: Some(900_000),
            },
            Track {
                id: TrackId::new(),
                source_id,
                stream_index: 0,
                language: None,
                title: None,
                is_default: true,
                is_forced: false,
                kind: TrackKind::Audio(AudioDetails {
                    codec: codec.to_string(),
                    profile: None,
                    channels: 2,
                    channel_layout: None,
                    sample_rate: None,
                    bit_depth: None,
                    bitrate: None,
                    loudness,
                }),
            },
        )),
    }
}

/// An empty library of music on one disk.
pub(crate) async fn empty_music_library() -> (Database, LibraryId, LibraryRootId) {
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
    (database, library.id, library.roots[0].id)
}
