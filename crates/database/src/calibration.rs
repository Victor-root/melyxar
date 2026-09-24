//! What one client was really measured decoding.
//!
//! Kept apart from an account on purpose: the same person watches from a
//! laptop and a television, and a calibration is a fact about a machine, not
//! about who is watching it.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, PlaybackClientId};
use melyxar_core::time::{Millis, Timestamp};
use sqlx::{AssertSqlSafe, Row};

use crate::browse::kept_inside;
use crate::convert::{bool_to_int, int_to_bool, parse_timestamp, timestamp_to_text};
use crate::{Database, Result};

/// Where a codec's verdict came from.
///
/// The two are not equal, and one of them is why this exists at all. A test
/// plays a film this server generates, and a generated film cannot be made to
/// cost what a real one costs to decode: measured on real hardware, the same
/// codec at the same size and the same rate lost under one picture in a
/// hundred of the generated film and a third of a real one. What a real film
/// did on this machine is the answer a test was only ever approximating.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FoundBy {
    /// The short test a viewer asks for from the settings.
    Test,
    /// A real film, watched all the way into the part where a decoder that
    /// cannot keep up stops keeping up.
    Watching,
}

impl FoundBy {
    /// How it is written down.
    pub fn as_word(self) -> &'static str {
        match self {
            Self::Test => "test",
            Self::Watching => "watching",
        }
    }

    /// What a stored row says it was. Anything else is read as a test, which
    /// is the weaker of the two and so the safe way to be wrong.
    pub fn from_word(value: &str) -> Self {
        match value {
            "watching" => Self::Watching,
            _ => Self::Test,
        }
    }
}

/// The film a calibration measures a client against.
///
/// Chosen by the server, never by a viewer, and on nothing but numbers the
/// analysis already read out of the files: the tallest picture the library
/// holds, and among those the one carrying the most bits. That is the file
/// this collection will ever ask the most of, which is the one worth knowing
/// a machine can play.
#[derive(Debug, Clone, PartialEq)]
pub struct FilmToMeasureAgainst {
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

/// One codec's real, measured verdict for one client.
#[derive(Debug, Clone, PartialEq)]
pub struct CodecCalibration {
    pub codec: String,
    /// Which recipe of the measurement produced this. A row written by an
    /// older version is not trusted the same way a current one is, even
    /// though nothing about the row itself looks wrong.
    pub calibration_version: i32,
    /// Whether this codec, rebuilt at `tested_height`, played without the
    /// decoder itself dropping pictures.
    pub usable: bool,
    pub tested_height: i32,
    /// The share of pictures the decoder dropped during the measurement, kept
    /// for a page that wants to say more than a plain yes or no.
    pub dropped_share: f64,
    /// The share of the pictures the film asked for over the measurement that
    /// ever appeared at all.
    ///
    /// The other half of the same question, and the half that catches a
    /// decoder too slow to produce pictures rather than one throwing them
    /// away: that one drops nothing, because it never made anything to drop.
    pub shown_share: f64,
    /// Which of the two ways this was found out, since one of them outranks
    /// the other.
    pub found_by: FoundBy,
    pub measured_at: Timestamp,
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
            "SELECT library_roots.path AS root_path, media_sources.relative_path,
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
            path: PathBuf::from(root).join(relative),
            duration: Millis::new(row.try_get("duration_ms")?),
            video_index: row.try_get("stream_index")?,
            codec: row.try_get("codec")?,
            height: row.try_get("height")?,
            frame_rate: row.try_get("frame_rate")?,
            wide_gamut: row.try_get::<Option<String>, _>("hdr_format")?.is_some(),
        }))
    }

    /// Records what was measured for one codec, replacing whatever this
    /// client's last calibration of that codec said.
    ///
    /// One codec at a time rather than a whole profile at once: a viewer
    /// asking to try AV1 again must not throw away what was already known
    /// about H.264 and HEVC while that runs.
    pub async fn save_codec_calibration(
        &self,
        client_id: PlaybackClientId,
        calibration: &CodecCalibration,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO client_codec_calibrations
                (client_id, codec, calibration_version, usable, tested_height, dropped_share, shown_share, found_by, measured_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (client_id, codec) DO UPDATE SET
                calibration_version = excluded.calibration_version,
                usable = excluded.usable,
                tested_height = excluded.tested_height,
                dropped_share = excluded.dropped_share,
                shown_share = excluded.shown_share,
                found_by = excluded.found_by,
                measured_at = excluded.measured_at",
        )
        .bind(client_id.to_db_string())
        .bind(&calibration.codec)
        .bind(calibration.calibration_version)
        .bind(bool_to_int(calibration.usable))
        .bind(calibration.tested_height)
        .bind(calibration.dropped_share)
        .bind(calibration.shown_share)
        .bind(calibration.found_by.as_word())
        .bind(timestamp_to_text(calibration.measured_at))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Everything measured for one client, one row per codec it was asked
    /// about. Empty for a client nobody has calibrated, which is every client
    /// until somebody asks for it.
    pub async fn codec_calibrations_of(
        &self,
        client_id: PlaybackClientId,
    ) -> Result<Vec<CodecCalibration>> {
        let rows = sqlx::query(
            "SELECT codec, calibration_version, usable, tested_height, dropped_share, shown_share, found_by, measured_at
             FROM client_codec_calibrations WHERE client_id = ?",
        )
        .bind(client_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.into_iter()
            .map(|row| {
                Ok(CodecCalibration {
                    codec: row.try_get("codec")?,
                    calibration_version: row.try_get("calibration_version")?,
                    usable: int_to_bool(row.try_get("usable")?),
                    tested_height: row.try_get("tested_height")?,
                    dropped_share: row.try_get("dropped_share")?,
                    shown_share: row.try_get("shown_share")?,
                    found_by: FoundBy::from_word(&row.try_get::<String, _>("found_by")?),
                    measured_at: parse_timestamp(&row.try_get::<String, _>("measured_at")?)?,
                })
            })
            .collect()
    }

    /// Forgets everything measured for one client, all codecs at once.
    ///
    /// A client recalibrating one codec on its own already replaces that
    /// row by itself; this is for starting from nothing again, which
    /// somebody testing the calibration itself needs far more often than a
    /// viewer ever will.
    pub async fn forget_codec_calibrations(&self, client_id: PlaybackClientId) -> Result<()> {
        sqlx::query("DELETE FROM client_codec_calibrations WHERE client_id = ?")
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

    fn measured(codec: &str, usable: bool, height: i32, dropped: f64) -> CodecCalibration {
        CodecCalibration {
            codec: codec.to_string(),
            calibration_version: 1,
            usable,
            tested_height: height,
            dropped_share: dropped,
            shown_share: 1.0,
            found_by: FoundBy::Test,
            measured_at: now(),
        }
    }

    #[tokio::test]
    async fn a_client_nobody_has_calibrated_has_nothing_measured() {
        let database = Database::open_in_memory().await.expect("database opens");
        let calibrations = database
            .codec_calibrations_of(PlaybackClientId::new())
            .await
            .expect("read");
        assert!(calibrations.is_empty());
    }

    #[tokio::test]
    async fn what_is_measured_for_one_client_is_read_back_whole() {
        let database = Database::open_in_memory().await.expect("database opens");
        let client = PlaybackClientId::new();

        database
            .save_codec_calibration(client, &measured("av1", false, 1080, 0.42))
            .await
            .expect("saved");
        database
            .save_codec_calibration(client, &measured("hevc", true, 2160, 0.0))
            .await
            .expect("saved");

        let mut calibrations = database.codec_calibrations_of(client).await.expect("read");
        calibrations.sort_by(|a, b| a.codec.cmp(&b.codec));

        assert_eq!(calibrations.len(), 2);
        assert_eq!(calibrations[0].codec, "av1");
        assert!(!calibrations[0].usable);
        assert_eq!(calibrations[0].tested_height, 1080);
        assert_eq!(calibrations[0].dropped_share, 0.42);
        assert_eq!(calibrations[1].codec, "hevc");
        assert!(calibrations[1].usable);
    }

    #[tokio::test]
    async fn a_decoder_that_showed_almost_nothing_is_read_back_saying_so() {
        // The case a dropped share alone never catches: nothing was thrown
        // away because almost nothing was ever produced. A row that lost this
        // number would read as a flawless measurement.
        let database = Database::open_in_memory().await.expect("database opens");
        let client = PlaybackClientId::new();

        database
            .save_codec_calibration(
                client,
                &CodecCalibration {
                    shown_share: 0.125,
                    ..measured("av1", false, 2160, 0.0)
                },
            )
            .await
            .expect("saved");

        let calibrations = database.codec_calibrations_of(client).await.expect("read");
        assert_eq!(calibrations[0].shown_share, 0.125);
        assert_eq!(calibrations[0].dropped_share, 0.0);
        assert!(!calibrations[0].usable);
    }

    #[tokio::test]
    async fn where_a_verdict_came_from_is_read_back_with_it() {
        // The whole reason it is stored: a verdict from a real film is not to
        // be quietly replaced by one from the test.
        let database = Database::open_in_memory().await.expect("database opens");
        let client = PlaybackClientId::new();

        database
            .save_codec_calibration(
                client,
                &CodecCalibration {
                    found_by: FoundBy::Watching,
                    ..measured("av1", false, 1600, 0.31)
                },
            )
            .await
            .expect("saved");

        let calibrations = database.codec_calibrations_of(client).await.expect("read");
        assert_eq!(calibrations[0].found_by, FoundBy::Watching);
    }

    #[tokio::test]
    async fn calibrating_one_codec_again_does_not_disturb_the_others() {
        let database = Database::open_in_memory().await.expect("database opens");
        let client = PlaybackClientId::new();

        database
            .save_codec_calibration(client, &measured("h264", true, 2160, 0.0))
            .await
            .expect("saved");
        database
            .save_codec_calibration(client, &measured("av1", false, 1080, 0.3))
            .await
            .expect("saved");

        // AV1 measured again, this time cleanly: a viewer re-testing one
        // codec must not lose what was already known about h264.
        database
            .save_codec_calibration(client, &measured("av1", true, 2160, 0.0))
            .await
            .expect("saved again");

        let mut calibrations = database.codec_calibrations_of(client).await.expect("read");
        calibrations.sort_by(|a, b| a.codec.cmp(&b.codec));

        assert_eq!(calibrations.len(), 2, "still one row per codec, not two");
        assert_eq!(calibrations[0].codec, "av1");
        assert!(
            calibrations[0].usable,
            "the fresh measurement replaced the old one"
        );
        assert_eq!(calibrations[0].tested_height, 2160);
        assert_eq!(calibrations[1].codec, "h264");
        assert!(
            calibrations[1].usable,
            "untouched by calibrating another codec"
        );
    }

    #[tokio::test]
    async fn forgetting_a_client_clears_every_codec_it_had() {
        let database = Database::open_in_memory().await.expect("database opens");
        let client = PlaybackClientId::new();
        let other = PlaybackClientId::new();

        database
            .save_codec_calibration(client, &measured("h264", true, 2160, 0.0))
            .await
            .expect("saved");
        database
            .save_codec_calibration(client, &measured("av1", false, 1080, 0.3))
            .await
            .expect("saved");
        database
            .save_codec_calibration(other, &measured("hevc", true, 2160, 0.0))
            .await
            .expect("saved");

        database
            .forget_codec_calibrations(client)
            .await
            .expect("forgotten");

        assert!(database
            .codec_calibrations_of(client)
            .await
            .expect("read")
            .is_empty());
        assert_eq!(
            database
                .codec_calibrations_of(other)
                .await
                .expect("read")
                .len(),
            1,
            "forgetting one client must not touch another one's calibration"
        );
    }

    #[tokio::test]
    async fn two_clients_never_share_what_was_measured_for_each_other() {
        let database = Database::open_in_memory().await.expect("database opens");
        let laptop = PlaybackClientId::new();
        let desktop = PlaybackClientId::new();

        database
            .save_codec_calibration(laptop, &measured("av1", true, 2160, 0.0))
            .await
            .expect("saved");

        assert!(
            database
                .codec_calibrations_of(desktop)
                .await
                .expect("read")
                .is_empty(),
            "a machine that has never been calibrated must not inherit another one's answer"
        );
    }
}
