//! What the tag manager reads of a song before writing into its file, and
//! what it writes down once a file was renamed.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, MediaSourceId, WorkId};
use sqlx::Row;

use crate::convert::parse_id;
use crate::{Database, Result};

/// The file of a song as the tag manager writes into it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongOnDisk {
    pub source_id: MediaSourceId,
    pub library_id: LibraryId,
    /// The root it is under, and where it is inside that root.
    pub root_path: PathBuf,
    pub relative_path: PathBuf,
}

impl SongOnDisk {
    pub fn path(&self) -> PathBuf {
        self.root_path.join(&self.relative_path)
    }
}

impl Database {
    /// The file a song is written into: the one it is played from.
    pub async fn song_on_disk(&self, song: WorkId) -> Result<Option<SongOnDisk>> {
        let row = sqlx::query(
            "SELECT s.id, w.library_id, r.path AS root_path, s.relative_path
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
            Ok(SongOnDisk {
                source_id: parse_id(&row.try_get::<String, _>("id")?)?,
                library_id: parse_id(&row.try_get::<String, _>("library_id")?)?,
                root_path: PathBuf::from(row.try_get::<String, _>("root_path")?),
                relative_path: PathBuf::from(row.try_get::<String, _>("relative_path")?),
            })
        })
        .transpose()
    }

    /// The songs of an album, in their order on it.
    pub async fn songs_of_album(&self, album: WorkId) -> Result<Vec<WorkId>> {
        sqlx::query(
            "SELECT w.id FROM works w LEFT JOIN music_songs ms ON ms.work_id = w.id
              WHERE w.parent_id = ? AND w.kind = 'song'
              ORDER BY coalesce(ms.disc_number, 1), w.ordinal, w.sort_title, w.id",
        )
        .bind(album.to_db_string())
        .fetch_all(self.reader())
        .await?
        .iter()
        .map(|row| parse_id(&row.try_get::<String, _>("id")?))
        .collect()
    }

    /// A file renamed where it lies: the same file, under its new name, so
    /// everything kept about its song stays with it.
    pub async fn rename_source_file(
        &self,
        source: MediaSourceId,
        relative_path: &std::path::Path,
    ) -> Result<()> {
        sqlx::query("UPDATE media_sources SET relative_path = ? WHERE id = ?")
            .bind(relative_path.to_string_lossy().as_ref())
            .bind(source.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use melyxar_core::media::Loudness;

    use super::*;
    use crate::music_testing::{a_read_song, empty_music_library};

    #[tokio::test]
    async fn a_song_is_found_on_its_disk_and_keeps_itself_when_its_file_is_renamed() {
        let (database, library, root) = empty_music_library().await;
        let file = a_read_song("A/01 track.flac", "Tides", "flac", Loudness::default());
        let source = file.source_id;
        database
            .file_music(library, root, &[file])
            .await
            .expect("filed");
        let song = database.sources_of_root(root).await.expect("read")[0].work_id;

        let on_disk = database
            .song_on_disk(song)
            .await
            .expect("read")
            .expect("a file");
        assert_eq!(on_disk.source_id, source);
        assert_eq!(on_disk.library_id, library);
        assert_eq!(on_disk.path(), PathBuf::from("/mnt/one/A/01 track.flac"));

        database
            .rename_source_file(source, std::path::Path::new("A/01 - Tides.flac"))
            .await
            .expect("renamed");
        let renamed = database
            .song_on_disk(song)
            .await
            .expect("read")
            .expect("a file");
        assert_eq!(renamed.path(), PathBuf::from("/mnt/one/A/01 - Tides.flac"));
        assert_eq!(
            database.sources_of_root(root).await.expect("read")[0].work_id,
            song
        );
        assert_eq!(
            database.song_on_disk(WorkId::new()).await.expect("read"),
            None
        );
    }

    #[tokio::test]
    async fn the_songs_of_an_album_come_in_their_order() {
        let (database, library, _) = crate::music_testing::collection().await;
        let albums = database
            .music_albums(
                library,
                &crate::music_browse::AlbumsWanted::default(),
                crate::music_browse::AlbumOrder::Title,
                false,
                0,
                10,
            )
            .await
            .expect("read")
            .items;
        let lights = albums
            .iter()
            .find(|album| album.title == "Northern Lights")
            .expect("filed");
        let songs = database.songs_of_album(lights.id).await.expect("read");
        let (_, rows) = database
            .music_album(lights.id)
            .await
            .expect("read")
            .expect("there");
        assert_eq!(songs, rows.iter().map(|song| song.id).collect::<Vec<_>>());
        assert_eq!(songs.len(), 2);
    }
}
