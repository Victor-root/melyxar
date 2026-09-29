//! What a song is played from.

use std::path::PathBuf;

use melyxar_core::id::WorkId;
use sqlx::Row;

use crate::{Database, Result};

/// The file a song is played from, and what is in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongFile {
    /// Where it is, root included. Never sent to a browser.
    pub path: PathBuf,
    pub container: Option<String>,
    /// The codec of its sound, once it has been read.
    pub codec: Option<String>,
    /// How heavy it is, in bits a second, once it has been read.
    pub bitrate: Option<i64>,
}

impl Database {
    /// The file a song is played from: the first of its files on a disk.
    /// Nothing for a song whose every file is missing, or for a work that is
    /// not a song.
    pub async fn music_song_file(&self, song: WorkId) -> Result<Option<SongFile>> {
        let row = sqlx::query(
            "SELECT r.path AS root_path, s.relative_path, s.container, s.overall_bitrate,
                    (SELECT t.codec FROM tracks t
                      WHERE t.source_id = s.id AND t.kind = 'audio'
                      ORDER BY t.stream_index LIMIT 1) AS codec
               FROM media_sources s
               JOIN library_roots r ON r.id = s.root_id
               JOIN works w ON w.id = s.work_id AND w.kind = 'song'
              WHERE s.work_id = ? AND s.missing_since IS NULL
              ORDER BY s.added_at
              LIMIT 1",
        )
        .bind(song.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        row.map(|row| {
            Ok(SongFile {
                path: PathBuf::from(row.try_get::<String, _>("root_path")?)
                    .join(row.try_get::<String, _>("relative_path")?),
                container: row.try_get("container")?,
                codec: row.try_get("codec")?,
                bitrate: row.try_get("overall_bitrate")?,
            })
        })
        .transpose()
    }
}

#[cfg(test)]
mod tests {
    use melyxar_core::media::Loudness;

    use super::*;
    use crate::music_testing::{a_read_song, empty_music_library};

    #[tokio::test]
    async fn a_song_is_played_from_its_file_on_the_disk() {
        let (database, library, root) = empty_music_library().await;
        let file = a_read_song("A/01.m4a", "Quiet Harbour", "alac", Loudness::default());
        let source_id = file.source_id;
        database
            .file_music(library, root, &[file])
            .await
            .expect("filed");
        let song = database.sources_of_root(root).await.expect("read")[0].work_id;

        let found = database
            .music_song_file(song)
            .await
            .expect("read")
            .expect("a file");
        assert_eq!(found.path, PathBuf::from("/mnt/one/A/01.m4a"));
        assert_eq!(found.codec.as_deref(), Some("alac"));
        assert_eq!(found.bitrate, Some(900_000));

        database.mark_source_missing(source_id).await.expect("gone");
        assert_eq!(
            database.music_song_file(song).await.expect("read"),
            None,
            "a missing file plays nothing"
        );
        assert_eq!(
            database.music_song_file(WorkId::new()).await.expect("read"),
            None
        );
    }
}
