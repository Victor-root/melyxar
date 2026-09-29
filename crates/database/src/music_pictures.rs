//! The albums and artists of a library of music that could wear a picture,
//! with what it would be made from.
//!
//! The pictures themselves are chosen elsewhere, from the folders the songs
//! sit in: this only says where those folders are, and which picture each
//! album and artist wears today, so that one already made is left alone.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, LibraryRootId, WorkId};
use melyxar_core::work::WorkKind;
use sqlx::Row;

use crate::convert::parse_id;
use crate::{Database, DatabaseError, Result};

/// An album, or an artist, and one song that leads to its folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicToPicture {
    pub work_id: WorkId,
    /// An album or an artist.
    pub kind: WorkKind,
    pub root_id: LibraryRootId,
    /// Where that root is mounted.
    pub root_path: PathBuf,
    /// One of its songs, relative to the root: the first of them by path, so
    /// the same one is asked every time. For an artist, a song of the first
    /// album that is theirs.
    pub song: PathBuf,
    /// Which state of that song a picture taken out of it was made from.
    pub song_made_from: String,
    /// The fingerprint of the picture it wears now, if it wears one.
    pub fingerprint: Option<String>,
}

impl Database {
    /// Every album of a library of music, and every artist whose album one
    /// of them is, each with a song on the disk. A compilation's artist is
    /// nobody's and has no folder of their own, so is not among them.
    pub async fn music_to_picture(&self, library_id: LibraryId) -> Result<Vec<MusicToPicture>> {
        let rows = sqlx::query(
            "WITH first_song AS (
                 SELECT song.parent_id AS album_id, s.id AS source_id,
                        min(s.relative_path) AS relative_path
                   FROM works song
                   JOIN media_sources s ON s.work_id = song.id AND s.missing_since IS NULL
                  WHERE song.library_id = ?1 AND song.kind = 'song' AND song.parent_id IS NOT NULL
                  GROUP BY song.parent_id
             ),
             -- A bare column beside min() is read from the row holding the
             -- minimum, which is how each subject keeps the file its first
             -- path names, and not another root's file of the same path.
             subjects AS (
                 SELECT a.id AS work_id, 'album' AS kind, f.source_id
                   FROM works a JOIN first_song f ON f.album_id = a.id
                  WHERE a.library_id = ?1 AND a.kind = 'album'
                 UNION ALL
                 SELECT artist_id, 'artist', source_id FROM (
                     SELECT c.artist_id, f.source_id, min(f.relative_path)
                       FROM music_credits c
                       JOIN music_albums ma ON ma.work_id = c.work_id AND ma.is_compilation = 0
                       JOIN first_song f ON f.album_id = c.work_id
                      WHERE c.role = 'album_artist'
                      GROUP BY c.artist_id
                 )
             )
             SELECT sub.work_id, sub.kind, s.root_id, r.path AS root_path, s.relative_path,
                    s.size_bytes, s.modified_at,
                    (SELECT i.fingerprint FROM images i
                      WHERE i.owner_kind = 'work' AND i.owner_id = sub.work_id
                        AND i.image_kind = 'poster'
                      LIMIT 1) AS fingerprint
               FROM subjects sub
               JOIN media_sources s ON s.id = sub.source_id
               JOIN library_roots r ON r.id = s.root_id
              ORDER BY sub.kind, sub.work_id",
        )
        .bind(library_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                let kind_text: String = row.try_get("kind")?;
                let relative_path: String = row.try_get("relative_path")?;
                let size_bytes: i64 = row.try_get("size_bytes")?;
                let modified_at: String = row.try_get("modified_at")?;
                Ok(MusicToPicture {
                    work_id: parse_id(&row.try_get::<String, _>("work_id")?)?,
                    kind: WorkKind::parse(&kind_text).ok_or_else(|| {
                        DatabaseError::Corrupt(format!("work kind '{kind_text}'"))
                    })?,
                    root_id: parse_id(&row.try_get::<String, _>("root_id")?)?,
                    root_path: PathBuf::from(row.try_get::<String, _>("root_path")?),
                    song_made_from: format!("{relative_path}|{size_bytes}|{modified_at}"),
                    song: PathBuf::from(relative_path),
                    fingerprint: row.try_get("fingerprint")?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use melyxar_core::id::MediaSourceId;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::music::{AlbumFiling, Named, SongFiling};
    use melyxar_core::time::now;

    use super::*;
    use crate::music::MusicFile;

    fn named(name: &str) -> Named {
        Named {
            name: name.to_string(),
            sort_name: name.to_lowercase(),
        }
    }

    fn file(path: &str, album: &str, artist: &str, compilation: bool) -> MusicFile {
        MusicFile {
            source_id: MediaSourceId::new(),
            song: None,
            relative_path: PathBuf::from(path),
            size_bytes: 10,
            modified_at: now(),
            filing: SongFiling {
                title: named(path),
                artists: vec![named(artist)],
                album: Some(AlbumFiling {
                    title: named(album),
                    artists: vec![named(if compilation {
                        "Various Artists"
                    } else {
                        artist
                    })],
                    is_compilation: compilation,
                }),
                track: None,
                disc: None,
                year: None,
                genres: Vec::new(),
            },
            reading: Err("not read in this test".to_string()),
        }
    }

    #[tokio::test]
    async fn every_album_and_its_artist_are_offered_with_their_first_song() {
        let database = Database::open_in_memory().await.expect("opens");
        let library = database
            .create_library(
                "Music",
                LibraryKind::Music,
                "fr",
                &[
                    ("disk-one".to_string(), PathBuf::from("/mnt/one")),
                    ("disk-two".to_string(), PathBuf::from("/mnt/two")),
                ],
            )
            .await
            .expect("library");
        let (one, two) = (library.roots[0].id, library.roots[1].id);
        database
            .file_music(
                library.id,
                one,
                &[
                    file("Amber Field/Lights/02.flac", "Lights", "Amber Field", false),
                    file("Amber Field/Lights/01.flac", "Lights", "Amber Field", false),
                    file("Hits/01.flac", "Hits", "The Lanterns", true),
                ],
            )
            .await
            .expect("filed");
        // The same path on the other disk, of another album.
        database
            .file_music(
                library.id,
                two,
                &[file("Amber Field/Lights/01.flac", "Other", "Nobody", false)],
            )
            .await
            .expect("filed");

        let offered = database.music_to_picture(library.id).await.expect("read");
        let lights = offered
            .iter()
            .find(|one| {
                one.kind == WorkKind::Album
                    && one.root_id == library.roots[0].id
                    && one.song.ends_with("01.flac")
                    && one.root_path == Path::new("/mnt/one")
            })
            .expect("the album, by its first song on its own disk");
        assert!(lights.fingerprint.is_none(), "no picture yet");
        assert_eq!(
            offered
                .iter()
                .filter(|one| one.kind == WorkKind::Album)
                .count(),
            3
        );
        let artists: Vec<&MusicToPicture> = offered
            .iter()
            .filter(|one| one.kind == WorkKind::Artist)
            .collect();
        assert_eq!(
            artists.len(),
            2,
            "Amber Field and Nobody, never the compilation's"
        );
        assert!(
            artists
                .iter()
                .all(|artist| !artist.song.starts_with("Hits"))
        );
    }
}
