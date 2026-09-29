//! How loud each song is: which are left to measure, and what was measured.
//!
//! The measure itself is kept on the song's one sound track, where every
//! file keeps it; what is kept here is only which files were read, so that a
//! song is read once, and again only once its file has changed.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, MediaSourceId};
use melyxar_core::media::Loudness;
use melyxar_core::time::Timestamp;
use sqlx::Row;

use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, Result};

/// A song whose loudness nobody measured yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongToMeasure {
    pub source_id: MediaSourceId,
    /// Where it is, root included.
    pub path: PathBuf,
}

/// Songs of a library whose loudness is known neither from their tags nor
/// from a reading, and whose file was not already tried as it stands.
const LEFT: &str = "FROM media_sources s
       JOIN works w ON w.id = s.work_id AND w.kind = 'song'
       JOIN library_roots r ON r.id = s.root_id
       JOIN tracks t ON t.source_id = s.id AND t.kind = 'audio'
      WHERE w.library_id = ? AND s.missing_since IS NULL
        AND t.loudness_integrated_lufs IS NULL
        AND NOT EXISTS (SELECT 1 FROM music_loudness_measured m
                         WHERE m.source_id = s.id AND m.file_modified_at = s.modified_at)";

impl Database {
    /// Up to `limit` songs of a library still to be measured.
    pub async fn songs_to_measure(
        &self,
        library: LibraryId,
        limit: i64,
    ) -> Result<Vec<SongToMeasure>> {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT DISTINCT s.id, r.path AS root_path, s.relative_path {LEFT}
              ORDER BY s.id LIMIT ?"
        )))
        .bind(library.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?
        .iter()
        .map(|row| {
            Ok(SongToMeasure {
                source_id: parse_id(&row.try_get::<String, _>("id")?)?,
                path: PathBuf::from(row.try_get::<String, _>("root_path")?)
                    .join(row.try_get::<String, _>("relative_path")?),
            })
        })
        .collect()
    }

    /// How many songs of a library are still to be measured.
    pub async fn count_songs_to_measure(&self, library: LibraryId) -> Result<i64> {
        Ok(sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "SELECT count(DISTINCT s.id) {LEFT}"
        )))
        .bind(library.to_db_string())
        .fetch_one(self.reader())
        .await?)
    }

    /// What was measured of a song, or nothing when it could not be read:
    /// either way the file is not read again until it changes.
    pub async fn write_song_loudness(
        &self,
        source: MediaSourceId,
        measured: Option<&Loudness>,
        at: Timestamp,
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        if let Some(loudness) = measured {
            sqlx::query(
                "UPDATE tracks SET loudness_integrated_lufs = ?, loudness_true_peak_dbfs = ?,
                                   loudness_range_lu = ?
                  WHERE source_id = ? AND kind = 'audio'",
            )
            .bind(loudness.integrated_lufs)
            .bind(loudness.true_peak_dbfs)
            .bind(loudness.range_lu)
            .bind(source.to_db_string())
            .execute(&mut *transaction)
            .await?;
        }
        sqlx::query(
            "INSERT INTO music_loudness_measured (source_id, file_modified_at, measured_at)
             SELECT id, modified_at, ? FROM media_sources WHERE id = ?
             ON CONFLICT (source_id) DO UPDATE SET
                file_modified_at = excluded.file_modified_at, measured_at = excluded.measured_at",
        )
        .bind(timestamp_to_text(at))
        .bind(source.to_db_string())
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::music_testing::{a_read_song, empty_music_library};

    #[tokio::test]
    async fn a_song_is_measured_once_unless_its_tags_said_already() {
        let (database, library, root) = empty_music_library().await;
        let tagged = Loudness {
            integrated_lufs: Some(-11.5),
            true_peak_dbfs: Some(0.0),
            range_lu: None,
        };
        let quiet = a_read_song("A/01.flac", "Quiet Harbour", "flac", Loudness::default());
        let loud = a_read_song("A/02.flac", "Tides", "flac", tagged);
        let broken = a_read_song("A/03.flac", "Zebra", "flac", Loudness::default());
        let (quiet_id, broken_id) = (quiet.source_id, broken.source_id);
        database
            .file_music(library, root, &[quiet, loud, broken])
            .await
            .expect("filed");

        let left = database.songs_to_measure(library, 10).await.expect("read");
        let mut ids: Vec<_> = left.iter().map(|song| song.source_id).collect();
        ids.sort();
        let mut expected = vec![quiet_id, broken_id];
        expected.sort();
        assert_eq!(ids, expected, "the tagged one is known already");
        assert_eq!(
            database
                .count_songs_to_measure(library)
                .await
                .expect("read"),
            2
        );
        assert!(
            left.iter()
                .any(|song| song.path == Path::new("/mnt/one/A/01.flac"))
        );

        let measured = Loudness {
            integrated_lufs: Some(-9.0),
            true_peak_dbfs: Some(0.5),
            range_lu: Some(4.0),
        };
        let at = melyxar_core::time::now();
        database
            .write_song_loudness(quiet_id, Some(&measured), at)
            .await
            .expect("written");
        database
            .write_song_loudness(broken_id, None, at)
            .await
            .expect("written");
        assert!(
            database
                .songs_to_measure(library, 10)
                .await
                .expect("read")
                .is_empty()
        );
        assert_eq!(
            database
                .count_songs_to_measure(library)
                .await
                .expect("read"),
            0
        );
    }
}
