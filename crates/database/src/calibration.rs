//! What one client was really measured decoding.
//!
//! Kept apart from an account on purpose: the same person watches from a
//! laptop and a television, and a calibration is a fact about a machine, not
//! about who is watching it.

use melyxar_core::id::PlaybackClientId;
use melyxar_core::time::Timestamp;
use sqlx::Row;

use crate::convert::{bool_to_int, int_to_bool, parse_timestamp, timestamp_to_text};
use crate::{Database, Result};

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
    pub measured_at: Timestamp,
}

impl Database {
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
                (client_id, codec, calibration_version, usable, tested_height, dropped_share, measured_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (client_id, codec) DO UPDATE SET
                calibration_version = excluded.calibration_version,
                usable = excluded.usable,
                tested_height = excluded.tested_height,
                dropped_share = excluded.dropped_share,
                measured_at = excluded.measured_at",
        )
        .bind(client_id.to_db_string())
        .bind(&calibration.codec)
        .bind(calibration.calibration_version)
        .bind(bool_to_int(calibration.usable))
        .bind(calibration.tested_height)
        .bind(calibration.dropped_share)
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
            "SELECT codec, calibration_version, usable, tested_height, dropped_share, measured_at
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

    fn measured(codec: &str, usable: bool, height: i32, dropped: f64) -> CodecCalibration {
        CodecCalibration {
            codec: codec.to_string(),
            calibration_version: 1,
            usable,
            tested_height: height,
            dropped_share: dropped,
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
        assert!(calibrations[0].usable, "the fresh measurement replaced the old one");
        assert_eq!(calibrations[0].tested_height, 2160);
        assert_eq!(calibrations[1].codec, "h264");
        assert!(calibrations[1].usable, "untouched by calibrating another codec");
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

        assert!(database.codec_calibrations_of(client).await.expect("read").is_empty());
        assert_eq!(
            database.codec_calibrations_of(other).await.expect("read").len(),
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
