//! What is read of each song of a library: how loud it is, and how its sound
//! is spread. Which are left to read, and what was read.
//!
//! The loudness is kept on the song's one sound track, where every file keeps
//! it, and the spectrum in a table of its own; what is kept apart from both is
//! which files were read, so that a song is read once, and again only once its
//! file has changed. A song is read once for both, and for only what it still
//! lacks when it has one of them already.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, MediaSourceId, WorkId};
use melyxar_core::media::Loudness;
use melyxar_core::music::Spectrum;
use melyxar_core::time::Timestamp;
use sqlx::Row;

use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, Result};

/// A song with something still to read of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongToAnalyse {
    pub source_id: MediaSourceId,
    /// Where it is, root included.
    pub path: PathBuf,
    /// Its loudness is known neither from its tags nor from a reading.
    pub needs_loudness: bool,
    /// Its spectrum was never read, or its file has changed since.
    pub needs_spectrum: bool,
}

/// Songs of a library lacking a loudness known from neither their tags nor a
/// reading, or a spectrum, as their file stands now.
const LEFT: &str = "FROM (
        SELECT s.id AS id, r.path AS root_path, s.relative_path AS relative_path,
               max(t.loudness_integrated_lufs IS NULL
                   AND NOT EXISTS (SELECT 1 FROM music_loudness_measured m
                                    WHERE m.source_id = s.id AND m.file_modified_at = s.modified_at))
                   AS needs_loudness,
               NOT EXISTS (SELECT 1 FROM music_spectrum p
                            WHERE p.source_id = s.id AND p.file_modified_at = s.modified_at)
                   AS needs_spectrum
          FROM media_sources s
          JOIN works w ON w.id = s.work_id AND w.kind = 'song'
          JOIN library_roots r ON r.id = s.root_id
          JOIN tracks t ON t.source_id = s.id AND t.kind = 'audio'
         WHERE w.library_id = ? AND s.missing_since IS NULL
         GROUP BY s.id, r.path, s.relative_path)
      WHERE needs_loudness OR needs_spectrum";

impl Database {
    /// Up to `limit` songs of a library with something still to read.
    pub async fn songs_to_analyse(
        &self,
        library: LibraryId,
        limit: i64,
    ) -> Result<Vec<SongToAnalyse>> {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT id, root_path, relative_path, needs_loudness, needs_spectrum {LEFT}
              ORDER BY id LIMIT ?"
        )))
        .bind(library.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?
        .iter()
        .map(|row| {
            Ok(SongToAnalyse {
                source_id: parse_id(&row.try_get::<String, _>("id")?)?,
                path: PathBuf::from(row.try_get::<String, _>("root_path")?)
                    .join(row.try_get::<String, _>("relative_path")?),
                needs_loudness: row.try_get("needs_loudness")?,
                needs_spectrum: row.try_get("needs_spectrum")?,
            })
        })
        .collect()
    }

    /// How many songs of a library have something still to read.
    pub async fn count_songs_to_analyse(&self, library: LibraryId) -> Result<i64> {
        Ok(
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) {LEFT}")))
                .bind(library.to_db_string())
                .fetch_one(self.reader())
                .await?,
        )
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

    /// How the sound of a song is spread, or nothing when it could not be
    /// read: either way the file is not read again until it changes.
    pub async fn write_song_spectrum(
        &self,
        source: MediaSourceId,
        spectrum: Option<&Spectrum>,
        at: Timestamp,
    ) -> Result<()> {
        let none = Spectrum {
            bands: 0,
            frames_a_second: 0,
            levels: Vec::new(),
        };
        let spectrum = spectrum.unwrap_or(&none);
        sqlx::query(
            "INSERT INTO music_spectrum
                 (source_id, file_modified_at, measured_at, bands, frames_a_second, levels)
             SELECT id, modified_at, ?, ?, ?, ? FROM media_sources WHERE id = ?
             ON CONFLICT (source_id) DO UPDATE SET
                file_modified_at = excluded.file_modified_at, measured_at = excluded.measured_at,
                bands = excluded.bands, frames_a_second = excluded.frames_a_second,
                levels = excluded.levels",
        )
        .bind(timestamp_to_text(at))
        .bind(i64::from(spectrum.bands))
        .bind(i64::from(spectrum.frames_a_second))
        .bind(&spectrum.levels)
        .bind(source.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// The spectrum of a song, as its file stands now, or nothing when it was
    /// never read, could not be, or the file changed since.
    pub async fn song_spectrum(&self, song: WorkId) -> Result<Option<Spectrum>> {
        let row = sqlx::query(
            "SELECT p.bands, p.frames_a_second, p.levels
               FROM media_sources s
               JOIN works w ON w.id = s.work_id AND w.kind = 'song'
               JOIN music_spectrum p
                 ON p.source_id = s.id AND p.file_modified_at = s.modified_at
              WHERE s.work_id = ? AND s.missing_since IS NULL AND length(p.levels) > 0
              ORDER BY s.added_at
              LIMIT 1",
        )
        .bind(song.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        row.map(|row| {
            Ok(Spectrum {
                bands: u8::try_from(row.try_get::<i64, _>("bands")?).unwrap_or(0),
                frames_a_second: u8::try_from(row.try_get::<i64, _>("frames_a_second")?)
                    .unwrap_or(0),
                levels: row.try_get("levels")?,
            })
        })
        .transpose()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use melyxar_core::music::AlbumFiling;

    use super::*;
    use crate::music_browse::SongOrder;
    use crate::music_testing::{a_read_song, empty_music_library, named};

    /// A library with a song quiet and unmeasured, one whose tags say how
    /// loud it is, and one no tool can read.
    async fn three_songs() -> (Database, LibraryId, [MediaSourceId; 3]) {
        let (database, library, root) = empty_music_library().await;
        let tagged = Loudness {
            integrated_lufs: Some(-11.5),
            true_peak_dbfs: Some(0.0),
            range_lu: None,
        };
        let quiet = a_read_song("A/01.flac", "Quiet Harbour", "flac", Loudness::default());
        let loud = a_read_song("A/02.flac", "Tides", "flac", tagged);
        let broken = a_read_song("A/03.flac", "Zebra", "flac", Loudness::default());
        let ids = [quiet.source_id, loud.source_id, broken.source_id];
        database
            .file_music(library, root, &[quiet, loud, broken])
            .await
            .expect("filed");
        (database, library, ids)
    }

    fn spectrum_of_two_readings() -> Spectrum {
        Spectrum {
            bands: 3,
            frames_a_second: 4,
            levels: vec![1, 2, 3, 4, 5, 6],
        }
    }

    #[tokio::test]
    async fn a_song_is_read_once_for_what_it_lacks_and_tags_say_what_they_say() {
        let (database, library, [quiet_id, tagged_id, broken_id]) = three_songs().await;

        let left = database.songs_to_analyse(library, 10).await.expect("read");
        let wants = |id: MediaSourceId| {
            left.iter()
                .find(|song| song.source_id == id)
                .map(|song| (song.needs_loudness, song.needs_spectrum))
        };
        assert_eq!(wants(quiet_id), Some((true, true)));
        assert_eq!(
            wants(tagged_id),
            Some((false, true)),
            "its tags are enough for the loudness"
        );
        assert_eq!(wants(broken_id), Some((true, true)));
        assert_eq!(
            database
                .count_songs_to_analyse(library)
                .await
                .expect("read"),
            3
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
        for (id, loudness, spectrum) in [
            (quiet_id, Some(&measured), Some(spectrum_of_two_readings())),
            (tagged_id, None, Some(spectrum_of_two_readings())),
            (broken_id, None, None),
        ] {
            if id != tagged_id {
                database
                    .write_song_loudness(id, loudness, at)
                    .await
                    .expect("written");
            }
            database
                .write_song_spectrum(id, spectrum.as_ref(), at)
                .await
                .expect("written");
        }
        assert!(
            database
                .songs_to_analyse(library, 10)
                .await
                .expect("read")
                .is_empty()
        );
        assert_eq!(
            database
                .count_songs_to_analyse(library)
                .await
                .expect("read"),
            0
        );
    }

    #[tokio::test]
    async fn a_song_measured_before_the_spectrum_existed_is_only_read_for_its_sound() {
        let (database, library, [quiet_id, ..]) = three_songs().await;
        database
            .write_song_loudness(quiet_id, None, melyxar_core::time::now())
            .await
            .expect("written");
        let left = database.songs_to_analyse(library, 10).await.expect("read");
        let quiet = left
            .iter()
            .find(|song| song.source_id == quiet_id)
            .expect("left");
        assert!(!quiet.needs_loudness && quiet.needs_spectrum);
    }

    #[tokio::test]
    async fn a_changed_file_is_read_again_for_both() {
        let (database, library, [quiet_id, ..]) = three_songs().await;
        let at = melyxar_core::time::now();
        database
            .write_song_loudness(quiet_id, None, at)
            .await
            .expect("written");
        database
            .write_song_spectrum(quiet_id, Some(&spectrum_of_two_readings()), at)
            .await
            .expect("written");
        let left = database.songs_to_analyse(library, 10).await.expect("read");
        assert!(left.iter().all(|song| song.source_id != quiet_id));

        let mut transaction = database.begin().await.expect("begins");
        sqlx::query(
            "UPDATE media_sources SET modified_at = '2099-01-01T00:00:00.000Z' WHERE id = ?",
        )
        .bind(quiet_id.to_db_string())
        .execute(&mut *transaction)
        .await
        .expect("updated");
        transaction.commit().await.expect("committed");

        let left = database.songs_to_analyse(library, 10).await.expect("read");
        let quiet = left
            .iter()
            .find(|song| song.source_id == quiet_id)
            .expect("left again");
        assert!(quiet.needs_loudness && quiet.needs_spectrum);
    }

    #[tokio::test]
    async fn the_spectrum_of_a_song_is_given_back_as_it_was_kept() {
        let (database, _, [quiet_id, _, broken_id]) = three_songs().await;
        let work_of = |id: MediaSourceId| {
            let database = database.clone();
            async move {
                sqlx::query_scalar::<_, String>("SELECT work_id FROM media_sources WHERE id = ?")
                    .bind(id.to_db_string())
                    .fetch_one(database.reader())
                    .await
                    .map(|work| parse_id::<WorkId>(&work).expect("an id"))
                    .expect("a work")
            }
        };
        let (quiet, broken) = (work_of(quiet_id).await, work_of(broken_id).await);
        assert_eq!(database.song_spectrum(quiet).await.expect("read"), None);

        let at = melyxar_core::time::now();
        database
            .write_song_spectrum(quiet_id, Some(&spectrum_of_two_readings()), at)
            .await
            .expect("written");
        database
            .write_song_spectrum(broken_id, None, at)
            .await
            .expect("written");
        assert_eq!(
            database.song_spectrum(quiet).await.expect("read"),
            Some(spectrum_of_two_readings())
        );
        assert_eq!(
            database.song_spectrum(broken).await.expect("read"),
            None,
            "a file that could not be read has no levels to give"
        );
    }

    #[tokio::test]
    async fn a_song_says_how_loud_it_is_and_how_loud_its_album_is() {
        let (database, library, root) = empty_music_library().await;
        let on_road = |path: &str, title: &str, lufs: Option<f64>| {
            let mut song = a_read_song(
                path,
                title,
                "flac",
                Loudness {
                    integrated_lufs: lufs,
                    true_peak_dbfs: lufs.map(|_| -1.0),
                    range_lu: None,
                },
            );
            song.filing.album = Some(AlbumFiling {
                title: named("Road"),
                artists: vec![named("The Lanterns")],
                is_compilation: false,
            });
            song
        };
        database
            .file_music(
                library,
                root,
                &[
                    on_road("R/01.flac", "Dust", Some(-10.0)),
                    on_road("R/02.flac", "Homecoming", Some(-20.0)),
                    on_road("R/03.flac", "Morning", None),
                ],
            )
            .await
            .expect("filed");

        let songs = database
            .music_songs(library, SongOrder::Title, false, 0, 10)
            .await
            .expect("read")
            .items;
        let dust = songs
            .iter()
            .find(|song| song.title == "Dust")
            .expect("filed");
        assert_eq!(dust.lufs, Some(-10.0));
        assert_eq!(dust.peak_dbfs, Some(-1.0));
        let album = dust.album_lufs.expect("measured");
        assert!((album - -12.596).abs() < 0.01, "{album}");
        let morning = songs
            .iter()
            .find(|song| song.title == "Morning")
            .expect("filed");
        assert_eq!(morning.lufs, None);
        assert_eq!(
            morning.album_lufs,
            Some(album),
            "the album is the same album"
        );
    }
}
