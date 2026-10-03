//! What one device was really measured decoding, and the film it is measured
//! against.
//!
//! Kept apart from an account on purpose: the same person watches from a
//! laptop and a television, and a calibration is a fact about a machine, not
//! about who is watching it.
//!
//! A calibration is kept whole or not at all. A run that stopped halfway was
//! never a calibration, and keeping the half that finished is how a device
//! ended up judged on a part of what it can do.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, PlaybackClientId};
use melyxar_core::time::{Millis, Timestamp};
use sqlx::{AssertSqlSafe, Row};

use crate::browse::kept_inside;
use crate::convert::{parse_timestamp, timestamp_to_text};
use crate::{Database, Result};

/// The film a calibration measures a client against.
///
/// Chosen by the server, never by a viewer, and on nothing but numbers the
/// analysis already read out of the files: the tallest picture the library
/// holds, and among those the one carrying the most bits. That is the file
/// this collection will ever ask the most of, which is the one worth knowing
/// a machine can play.
#[derive(Debug, Clone, PartialEq)]
pub struct FilmToMeasureAgainst {
    /// Which source this is, which is what the clips cut from it are kept
    /// under: another film chosen later is another set of clips.
    pub source_id: String,
    /// Where the file is. Never sent to a client, which is given an address.
    pub path: PathBuf,
    pub duration: Millis,
    /// The picture's stream inside the file.
    pub video_index: i32,
    /// What the picture is held in, which decides whether the card can read
    /// the file for itself the way a real playback of it would.
    pub codec: String,
    pub height: i32,
    /// How many pictures a second it runs at, when the analysis read one.
    /// What "as fast as it plays" is counted against, so a guess here would
    /// be a verdict built on a guess.
    pub frame_rate: Option<f64>,
    /// Whether the picture is wide gamut, and so has to be converted on the
    /// way out exactly as a real playback of it would.
    pub wide_gamut: bool,
}

/// One clip of one codec at one height, as the device played it.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Measurement {
    pub height: i32,
    /// Whether it played cleanly: kept time, showed what it had to, and
    /// threw almost nothing away.
    pub passed: bool,
    /// The share of the pictures the decoder made that it threw away.
    pub dropped_share: f64,
    /// The share of the pictures the clip asked for that ever appeared.
    pub shown_share: f64,
}

/// What a device was found to do with one codec.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CodecResult {
    pub codec: String,
    /// The tallest picture it played cleanly. Absent when it played none of
    /// them, or could not open the codec at all.
    pub smooth_height: Option<i32>,
    /// Every height it was asked to play, tallest first, kept for a page that
    /// wants to say more than the answer.
    pub measurements: Vec<Measurement>,
}

/// One whole calibration of one device.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceCalibration {
    /// Which recipe of the measurement this was made by. Read back only
    /// when it is the one the server makes now.
    pub calibration_version: i32,
    pub measured_at: Timestamp,
    /// One entry for every codec the server offered, never fewer.
    pub codecs: Vec<CodecResult>,
}

/// How long a film has to run to be worth measuring against.
///
/// Long enough to hold a passage in its middle that is really the film rather
/// than its opening: a minute rules out the odds and ends a folder collects
/// without ruling out anything anybody watches.
const LONG_ENOUGH_TO_MEASURE_AGAINST: i64 = 60_000;

impl Database {
    /// The film this library would ask the most of, when it holds one.
    ///
    /// Nothing here is a matter of taste: the tallest picture, then the one
    /// carrying the most bits, then the oldest identifier so that the same
    /// library always answers the same way. Dolby Vision is left out, being
    /// the one colour a real playback may itself refuse, and a calibration
    /// must never fail over the film it picked rather than over the codec it
    /// is asking about.
    ///
    /// Only among the libraries given, when some are: the film is played to
    /// whoever asked, so it is one they could have opened themselves.
    ///
    /// Empty for a library nobody has scanned yet, and for an account
    /// granted nothing, which is what the generated film is still there for.
    pub async fn film_to_measure_against(
        &self,
        within: Option<&[LibraryId]>,
    ) -> Result<Option<FilmToMeasureAgainst>> {
        let Some(inside) = kept_inside(within, "works.library_id") else {
            return Ok(None);
        };
        // Nothing assembled here but a row of question marks, one per
        // library identifier the caller was handed by this crate.
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT media_sources.id AS source_id, library_roots.path AS root_path,
                    media_sources.relative_path,
                    media_sources.duration_ms, tracks.stream_index, tracks.codec,
                    tracks.height, tracks.frame_rate, tracks.hdr_format
             FROM tracks
             JOIN media_sources ON media_sources.id = tracks.source_id
             JOIN library_roots ON library_roots.id = media_sources.root_id
             JOIN works ON works.id = media_sources.work_id
             WHERE tracks.kind = 'video'
               AND tracks.height IS NOT NULL
               AND tracks.is_external = 0
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_sources.duration_ms >= ?
               AND (tracks.hdr_format IS NULL OR tracks.hdr_format <> 'dolby_vision')
               {inside}
             ORDER BY tracks.height DESC,
                      COALESCE(tracks.bitrate, media_sources.overall_bitrate, 0) DESC,
                      media_sources.id
             LIMIT 1"
        )))
        .bind(LONG_ENOUGH_TO_MEASURE_AGAINST);
        for library in within.unwrap_or_default() {
            query = query.bind(library.to_db_string());
        }
        let row = query.fetch_optional(self.reader()).await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let root: String = row.try_get("root_path")?;
        let relative: String = row.try_get("relative_path")?;
        Ok(Some(FilmToMeasureAgainst {
            source_id: row.try_get("source_id")?,
            path: PathBuf::from(root).join(relative),
            duration: Millis::new(row.try_get("duration_ms")?),
            video_index: row.try_get("stream_index")?,
            codec: row.try_get("codec")?,
            height: row.try_get("height")?,
            frame_rate: row.try_get("frame_rate")?,
            wide_gamut: row.try_get::<Option<String>, _>("hdr_format")?.is_some(),
        }))
    }

    /// Keeps one device's whole calibration, replacing the one before.
    pub async fn save_device_calibration(
        &self,
        client_id: PlaybackClientId,
        calibration: &DeviceCalibration,
    ) -> Result<()> {
        let results = serde_json::to_string(&calibration.codecs)
            .map_err(|error| crate::DatabaseError::Corrupt(error.to_string()))?;
        sqlx::query(
            "INSERT INTO device_calibrations (client_id, calibration_version, measured_at, results)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (client_id) DO UPDATE SET
                calibration_version = excluded.calibration_version,
                measured_at = excluded.measured_at,
                results = excluded.results",
        )
        .bind(client_id.to_db_string())
        .bind(calibration.calibration_version)
        .bind(timestamp_to_text(calibration.measured_at))
        .bind(results)
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// One device's calibration, when it has one made by this recipe.
    ///
    /// One made by another recipe measured something else, and is read as
    /// no calibration at all.
    pub async fn device_calibration(
        &self,
        client_id: PlaybackClientId,
        calibration_version: i32,
    ) -> Result<Option<DeviceCalibration>> {
        let row = sqlx::query(
            "SELECT calibration_version, measured_at, results
             FROM device_calibrations WHERE client_id = ? AND calibration_version = ?",
        )
        .bind(client_id.to_db_string())
        .bind(calibration_version)
        .fetch_optional(self.reader())
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let results: String = row.try_get("results")?;
        Ok(Some(DeviceCalibration {
            calibration_version: row.try_get("calibration_version")?,
            measured_at: parse_timestamp(&row.try_get::<String, _>("measured_at")?)?,
            codecs: serde_json::from_str(&results)
                .map_err(|error| crate::DatabaseError::Corrupt(error.to_string()))?,
        }))
    }

    /// Forgets one device's calibration.
    pub async fn forget_device_calibration(&self, client_id: PlaybackClientId) -> Result<()> {
        sqlx::query("DELETE FROM device_calibrations WHERE client_id = ?")
            .bind(client_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::time::now;

    /// One analysed film in a library, described only by what deciding
    /// between films actually reads. Titles are invented and paths are
    /// nowhere: nothing of anybody's collection belongs in a test.
    struct AFilm {
        name: &'static str,
        height: i32,
        bitrate: Option<i64>,
        duration_ms: i64,
        hdr: Option<&'static str>,
        analysed: bool,
        missing: bool,
    }

    impl AFilm {
        fn plain(name: &'static str, height: i32, bitrate: i64) -> Self {
            Self {
                name,
                height,
                bitrate: Some(bitrate),
                duration_ms: 7_200_000,
                hdr: None,
                analysed: true,
                missing: false,
            }
        }
    }

    /// The library the films below are put in.
    fn the_library() -> LibraryId {
        "01890a5d-ac96-774b-bcce-b302099a8057"
            .parse()
            .expect("an identifier")
    }

    /// Puts a library holding these films into a migrated database.
    async fn a_library_holding(database: &Database, films: &[AFilm]) {
        sqlx::query(
            "INSERT INTO libraries (id, name, kind, metadata_language, created_at, updated_at)
             VALUES (?, 'Films', 'movies', 'fr', ?, ?)",
        )
        .bind(the_library().to_db_string())
        .bind(timestamp_to_text(now()))
        .bind(timestamp_to_text(now()))
        .execute(database.writer())
        .await
        .expect("a library");
        sqlx::query("INSERT INTO library_roots (id, library_id, label, path) VALUES ('root', ?, 'disk-one', '/films')")
            .bind(the_library().to_db_string())
            .execute(database.writer())
            .await
            .expect("a root");

        for (index, film) in films.iter().enumerate() {
            let work = format!("work-{index}");
            let source = format!("source-{index}");
            sqlx::query(
                "INSERT INTO works (id, library_id, kind, title, sort_title, added_at, updated_at)
                 VALUES (?, ?, 'movie', ?, ?, ?, ?)",
            )
            .bind(&work)
            .bind(the_library().to_db_string())
            .bind(film.name)
            .bind(film.name)
            .bind(timestamp_to_text(now()))
            .bind(timestamp_to_text(now()))
            .execute(database.writer())
            .await
            .expect("a work");
            sqlx::query(
                "INSERT INTO media_sources
                    (id, work_id, root_id, relative_path, size_bytes, modified_at, added_at,
                     duration_ms, analysed_at, missing_since)
                 VALUES (?, ?, 'root', ?, 1, ?, ?, ?, ?, ?)",
            )
            .bind(&source)
            .bind(&work)
            .bind(film.name)
            .bind(timestamp_to_text(now()))
            .bind(timestamp_to_text(now()))
            .bind(film.duration_ms)
            .bind(film.analysed.then(|| timestamp_to_text(now())))
            .bind(film.missing.then(|| timestamp_to_text(now())))
            .execute(database.writer())
            .await
            .expect("a source");
            sqlx::query(
                "INSERT INTO tracks (id, source_id, stream_index, kind, codec, height, width, bitrate, frame_rate, hdr_format)
                 VALUES (?, ?, 0, 'video', 'hevc', ?, 3840, ?, 23.976, ?)",
            )
            .bind(format!("track-{index}"))
            .bind(&source)
            .bind(film.height)
            .bind(film.bitrate)
            .bind(film.hdr)
            .execute(database.writer())
            .await
            .expect("a picture");
        }
    }

    #[tokio::test]
    async fn a_library_nobody_has_scanned_offers_no_film_to_measure_against() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert!(database
            .film_to_measure_against(None)
            .await
            .expect("asked")
            .is_none());
    }

    #[tokio::test]
    async fn only_a_film_of_a_library_granted_is_measured_against() {
        // The film is played to whoever asked for the calibration.
        let database = Database::open_in_memory().await.expect("database opens");
        a_library_holding(&database, &[AFilm::plain("a-film", 2160, 30_000_000)]).await;

        let granted = [the_library()];
        assert!(database
            .film_to_measure_against(Some(&granted))
            .await
            .expect("asked")
            .is_some());
        let elsewhere = [LibraryId::new()];
        assert!(
            database
                .film_to_measure_against(Some(&elsewhere))
                .await
                .expect("asked")
                .is_none(),
            "a film of a library not granted is never handed over"
        );
        assert!(database
            .film_to_measure_against(Some(&[]))
            .await
            .expect("asked")
            .is_none());
    }

    #[tokio::test]
    async fn the_tallest_picture_is_the_one_measured_against() {
        let database = Database::open_in_memory().await.expect("database opens");
        a_library_holding(
            &database,
            &[
                AFilm::plain("a-tall-one", 2160, 30_000_000),
                AFilm::plain("a-small-one", 1080, 90_000_000),
            ],
        )
        .await;

        let chosen = database
            .film_to_measure_against(None)
            .await
            .expect("asked")
            .expect("a film");
        assert_eq!(chosen.height, 2160);
        assert_eq!(chosen.path, PathBuf::from("/films/a-tall-one"));
    }

    #[tokio::test]
    async fn among_pictures_of_one_height_the_heaviest_is_measured_against() {
        let database = Database::open_in_memory().await.expect("database opens");
        a_library_holding(
            &database,
            &[
                AFilm::plain("a-light-one", 2160, 20_000_000),
                AFilm::plain("a-heavy-one", 2160, 80_000_000),
            ],
        )
        .await;

        let chosen = database
            .film_to_measure_against(None)
            .await
            .expect("asked")
            .expect("a film");
        assert_eq!(chosen.path, PathBuf::from("/films/a-heavy-one"));
    }

    #[tokio::test]
    async fn a_film_a_calibration_could_not_play_is_never_the_one_chosen() {
        let database = Database::open_in_memory().await.expect("database opens");
        a_library_holding(
            &database,
            &[
                // Every one of these is the tallest and the heaviest, and
                // every one of them would fail for a reason having nothing
                // to do with the codec being asked about.
                AFilm {
                    hdr: Some("dolby_vision"),
                    ..AFilm::plain("a-colour-that-may-be-refused", 4320, 200_000_000)
                },
                AFilm {
                    missing: true,
                    ..AFilm::plain("a-file-off-its-disk", 4320, 190_000_000)
                },
                AFilm {
                    analysed: false,
                    ..AFilm::plain("a-file-nobody-has-read", 4320, 180_000_000)
                },
                AFilm {
                    duration_ms: 20_000,
                    ..AFilm::plain("something-far-too-short", 4320, 170_000_000)
                },
                AFilm::plain("the-one-left-standing", 2160, 30_000_000),
            ],
        )
        .await;

        let chosen = database
            .film_to_measure_against(None)
            .await
            .expect("asked")
            .expect("a film");
        assert_eq!(chosen.path, PathBuf::from("/films/the-one-left-standing"));
    }

    #[tokio::test]
    async fn a_wide_gamut_film_says_so_that_it_may_be_converted_like_any_other() {
        let database = Database::open_in_memory().await.expect("database opens");
        a_library_holding(
            &database,
            &[AFilm {
                hdr: Some("hdr10"),
                ..AFilm::plain("a-wide-gamut-one", 2160, 60_000_000)
            }],
        )
        .await;

        let chosen = database
            .film_to_measure_against(None)
            .await
            .expect("asked")
            .expect("a film");
        assert!(
            chosen.wide_gamut,
            "a picture handed over unconverted is a picture nobody can watch"
        );
        assert_eq!(chosen.duration, Millis::new(7_200_000));
    }

    fn calibrated(version: i32, h264: Option<i32>) -> DeviceCalibration {
        DeviceCalibration {
            calibration_version: version,
            measured_at: now(),
            codecs: vec![
                CodecResult {
                    codec: "h264".to_string(),
                    smooth_height: h264,
                    measurements: vec![Measurement {
                        height: 2160,
                        passed: h264 == Some(2160),
                        dropped_share: 0.01,
                        shown_share: 0.99,
                    }],
                },
                CodecResult {
                    codec: "av1".to_string(),
                    smooth_height: None,
                    measurements: Vec::new(),
                },
            ],
        }
    }

    #[tokio::test]
    async fn a_device_nobody_has_calibrated_has_no_calibration() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert!(database
            .device_calibration(PlaybackClientId::new(), 1)
            .await
            .expect("read")
            .is_none());
    }

    #[tokio::test]
    async fn a_calibration_is_read_back_whole() {
        let database = Database::open_in_memory().await.expect("database opens");
        let client = PlaybackClientId::new();
        let kept = calibrated(1, Some(2160));
        database
            .save_device_calibration(client, &kept)
            .await
            .expect("saved");

        let read = database
            .device_calibration(client, 1)
            .await
            .expect("read")
            .expect("there is one");
        assert_eq!(read.codecs, kept.codecs);
        assert_eq!(read.calibration_version, 1);
    }

    #[tokio::test]
    async fn a_new_calibration_replaces_the_one_before_whole() {
        let database = Database::open_in_memory().await.expect("database opens");
        let client = PlaybackClientId::new();
        database
            .save_device_calibration(client, &calibrated(1, Some(2160)))
            .await
            .expect("saved");
        database
            .save_device_calibration(client, &calibrated(1, Some(1080)))
            .await
            .expect("saved");

        let read = database.device_calibration(client, 1).await.expect("read");
        assert_eq!(
            read.expect("there is one").codecs[0].smooth_height,
            Some(1080)
        );
    }

    #[tokio::test]
    async fn a_calibration_made_another_way_reads_as_none() {
        let database = Database::open_in_memory().await.expect("database opens");
        let client = PlaybackClientId::new();
        database
            .save_device_calibration(client, &calibrated(1, Some(2160)))
            .await
            .expect("saved");
        assert!(database
            .device_calibration(client, 2)
            .await
            .expect("read")
            .is_none());
    }

    #[tokio::test]
    async fn forgetting_a_device_leaves_the_others_alone() {
        let database = Database::open_in_memory().await.expect("database opens");
        let (one, other) = (PlaybackClientId::new(), PlaybackClientId::new());
        for client in [one, other] {
            database
                .save_device_calibration(client, &calibrated(1, Some(1080)))
                .await
                .expect("saved");
        }
        database
            .forget_device_calibration(one)
            .await
            .expect("forgotten");
        assert!(database
            .device_calibration(one, 1)
            .await
            .expect("read")
            .is_none());
        assert!(database
            .device_calibration(other, 1)
            .await
            .expect("read")
            .is_some());
    }
}
