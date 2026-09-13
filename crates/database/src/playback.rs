//! What playing a file needs from the storage, and where a viewer got to.
//!
//! Two things live here. The file itself, resolved to a path on disk, since a
//! source is stored relative to a root and only the storage knows where that
//! root is. And the position a viewer reached, which carries the instant it
//! was measured: a report arriving late must never make a resume point go
//! backwards, which is exactly what happens when a client reconnects and
//! flushes a copy it kept while it was away.

use std::path::PathBuf;

use melyxar_core::id::{MediaSourceId, TrackId, UserId, WorkId};
use melyxar_core::time::{now, Millis, Timestamp};
use melyxar_core::work::{should_accept_position, PlaybackState};
use sqlx::Row;

use crate::convert::{parse_optional_timestamp, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

/// One file, with everything needed to hand it to a viewer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayableSource {
    pub id: MediaSourceId,
    pub work_id: WorkId,
    /// Where the file is, root included. Never sent to a client: a viewer is
    /// given an address, not a path on someone's disk.
    pub path: PathBuf,
    pub size_bytes: i64,
    pub container: Option<String>,
    pub duration: Option<Millis>,
    /// True while the file is not on disk. Such a file is refused with an
    /// explanation rather than opened and failing halfway.
    pub missing: bool,
}

/// Where a viewer got to in one work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredProgress {
    pub position: Millis,
    pub state: PlaybackState,
    /// Set when a person said themselves that they had watched it, which no
    /// automatic threshold ever undoes.
    pub marked_manually: bool,
    pub play_count: i64,
    /// Instant the position was measured on the client.
    pub reported_at: Option<Timestamp>,
    pub last_played_at: Option<Timestamp>,
    /// The tracks this viewer chose last time, so the next session starts the
    /// same way rather than back on whatever the file marks as default.
    pub audio_track_id: Option<TrackId>,
    pub subtitle_track_id: Option<TrackId>,
}

impl Database {
    /// The file behind one source, resolved to a path.
    pub async fn playable_source(&self, id: MediaSourceId) -> Result<Option<PlayableSource>> {
        let row = sqlx::query(
            "SELECT media_sources.id, media_sources.work_id, media_sources.relative_path,
                    media_sources.size_bytes, media_sources.container, media_sources.duration_ms,
                    media_sources.missing_since, library_roots.path AS root_path
             FROM media_sources
             JOIN library_roots ON library_roots.id = media_sources.root_id
             WHERE media_sources.id = ?",
        )
        .bind(id.to_db_string())
        .fetch_optional(self.reader())
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };

        let root: String = row.try_get("root_path")?;
        let relative: String = row.try_get("relative_path")?;
        let work_id: String = row.try_get("work_id")?;

        Ok(Some(PlayableSource {
            id,
            work_id: work_id
                .parse()
                .map_err(|_| DatabaseError::Corrupt("work identifier is malformed".to_string()))?,
            path: PathBuf::from(root).join(relative),
            size_bytes: row.try_get("size_bytes")?,
            container: row.try_get("container")?,
            duration: row
                .try_get::<Option<i64>, _>("duration_ms")?
                .map(Millis::new),
            missing: row.try_get::<Option<String>, _>("missing_since")?.is_some(),
        }))
    }

    /// Where someone got to in one work.
    pub async fn playback_progress(
        &self,
        user_id: UserId,
        work_id: WorkId,
    ) -> Result<Option<StoredProgress>> {
        let row = sqlx::query(
            "SELECT position_ms, state, marked_manually, play_count, reported_at, last_played_at,
                    audio_track_id, subtitle_track_id
             FROM playback_progress WHERE user_id = ? AND work_id = ?",
        )
        .bind(user_id.to_db_string())
        .bind(work_id.to_db_string())
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| {
            let state: String = row.try_get("state")?;
            Ok(StoredProgress {
                position: Millis::new(row.try_get("position_ms")?),
                state: PlaybackState::parse(&state).ok_or_else(|| {
                    DatabaseError::Corrupt(format!("playback state '{state}' is unknown"))
                })?,
                marked_manually: crate::convert::int_to_bool(row.try_get("marked_manually")?),
                play_count: row.try_get("play_count")?,
                reported_at: parse_optional_timestamp(
                    row.try_get::<Option<String>, _>("reported_at")?.as_deref(),
                )?,
                last_played_at: parse_optional_timestamp(
                    row.try_get::<Option<String>, _>("last_played_at")?
                        .as_deref(),
                )?,
                audio_track_id: parse_track(row.try_get("audio_track_id")?)?,
                subtitle_track_id: parse_track(row.try_get("subtitle_track_id")?)?,
            })
        })
        .transpose()
    }

    /// Records where a viewer got to, unless a fresher report is already here.
    ///
    /// Answers whether the report was kept. The whole thing runs in one
    /// transaction: reading the instant of the stored report and writing over
    /// it have to be one step, or two clients reporting at once decide the
    /// order between themselves.
    pub async fn record_playback_progress(
        &self,
        user_id: UserId,
        work_id: WorkId,
        position: Millis,
        state: PlaybackState,
        reported_at: Timestamp,
    ) -> Result<bool> {
        let mut transaction = self.begin().await?;

        let stored: Option<(Option<String>, i64)> = sqlx::query_as(
            "SELECT reported_at, marked_manually FROM playback_progress
             WHERE user_id = ? AND work_id = ?",
        )
        .bind(user_id.to_db_string())
        .bind(work_id.to_db_string())
        .fetch_optional(&mut *transaction)
        .await?;

        let (previous_report, marked_manually) = match &stored {
            Some((reported, marked)) => (
                parse_optional_timestamp(reported.as_deref())?,
                crate::convert::int_to_bool(*marked),
            ),
            None => (None, false),
        };

        if !should_accept_position(previous_report, reported_at) {
            transaction.commit().await?;
            return Ok(false);
        }

        // A person who said they had watched it keeps that answer whatever a
        // player reports afterwards.
        let state = match marked_manually {
            true => PlaybackState::Watched,
            false => state,
        };

        sqlx::query(
            "INSERT INTO playback_progress
                (user_id, work_id, position_ms, state, reported_at, last_played_at)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (user_id, work_id) DO UPDATE SET
                position_ms = excluded.position_ms,
                state = excluded.state,
                reported_at = excluded.reported_at,
                last_played_at = excluded.last_played_at",
        )
        .bind(user_id.to_db_string())
        .bind(work_id.to_db_string())
        .bind(position.get())
        .bind(state.as_str())
        .bind(timestamp_to_text(reported_at))
        .bind(timestamp_to_text(now()))
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(true)
    }

    /// Remembers which tracks a viewer chose for one work.
    ///
    /// Written apart from the position: choosing a soundtrack says nothing
    /// about where anyone is, and a viewer who picks a track before pressing
    /// play has chosen nothing to resume from yet.
    pub async fn record_chosen_tracks(
        &self,
        user_id: UserId,
        work_id: WorkId,
        audio_track_id: Option<TrackId>,
        subtitle_track_id: Option<TrackId>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO playback_progress
                (user_id, work_id, audio_track_id, subtitle_track_id)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (user_id, work_id) DO UPDATE SET
                audio_track_id = excluded.audio_track_id,
                subtitle_track_id = excluded.subtitle_track_id",
        )
        .bind(user_id.to_db_string())
        .bind(work_id.to_db_string())
        .bind(audio_track_id.map(|id| id.to_db_string()))
        .bind(subtitle_track_id.map(|id| id.to_db_string()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Counts one more viewing of a work.
    ///
    /// Kept apart from the position: a film watched twice is two viewings and
    /// one position, and conflating them makes both wrong.
    pub async fn count_one_playback(&self, user_id: UserId, work_id: WorkId) -> Result<()> {
        sqlx::query(
            "INSERT INTO playback_progress (user_id, work_id, play_count, last_played_at)
             VALUES (?, ?, 1, ?)
             ON CONFLICT (user_id, work_id) DO UPDATE SET
                play_count = play_count + 1,
                last_played_at = excluded.last_played_at",
        )
        .bind(user_id.to_db_string())
        .bind(work_id.to_db_string())
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }
}

/// Reads back a track identifier, treating a malformed one as none.
///
/// A remembered choice is a comfort, never a reason to refuse to play: a row
/// pointing at a track that no longer parses means the file is played the way
/// it marks itself, which is what would have happened anyway.
fn parse_track(value: Option<String>) -> Result<Option<TrackId>> {
    Ok(value.and_then(|value| value.parse().ok()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::WorkKind;
    use std::path::Path;
    use time::macros::datetime;

    async fn one_film() -> (Database, UserId, WorkId, MediaSourceId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let work = database
            .create_work(
                library.id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        let source = database
            .insert_source(
                work.id,
                library.roots[0].id,
                Path::new("Quiet.Harbour.2019.mkv"),
                1_000,
                now(),
            )
            .await
            .expect("source recorded");
        let user = database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");
        (database, user.id, work.id, source)
    }

    #[tokio::test]
    async fn a_file_is_found_back_under_the_root_it_lives_in() {
        let (database, _, work_id, source_id) = one_film().await;
        let playable = database
            .playable_source(source_id)
            .await
            .expect("read")
            .expect("present");

        assert_eq!(
            playable.path,
            PathBuf::from("/mnt/one/Films/Quiet.Harbour.2019.mkv"),
            "a source is stored relative to its root, and only the storage knows where that is"
        );
        assert_eq!(playable.work_id, work_id);
        assert_eq!(playable.size_bytes, 1_000);
        assert!(!playable.missing);
    }

    #[tokio::test]
    async fn a_file_that_is_not_on_the_disk_says_so_rather_than_being_offered() {
        let (database, _, _, source_id) = one_film().await;
        database
            .mark_source_missing(source_id)
            .await
            .expect("marked");

        assert!(
            database
                .playable_source(source_id)
                .await
                .expect("read")
                .expect("the row is still there")
                .missing
        );
    }

    #[tokio::test]
    async fn a_source_nobody_knows_is_absent_rather_than_an_error() {
        let (database, _, _, _) = one_film().await;
        assert!(database
            .playable_source(MediaSourceId::new())
            .await
            .expect("read")
            .is_none());
    }

    #[tokio::test]
    async fn a_position_is_recorded_and_read_back() {
        let (database, user_id, work_id, _) = one_film().await;
        assert!(database
            .playback_progress(user_id, work_id)
            .await
            .expect("read")
            .is_none());

        assert!(database
            .record_playback_progress(
                user_id,
                work_id,
                Millis::new(1_800_000),
                PlaybackState::InProgress,
                datetime!(2026-01-01 12:00 UTC),
            )
            .await
            .expect("recorded"));

        let stored = database
            .playback_progress(user_id, work_id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.position, Millis::new(1_800_000));
        assert_eq!(stored.state, PlaybackState::InProgress);
        assert!(stored.last_played_at.is_some());
    }

    #[tokio::test]
    async fn a_report_that_arrives_late_never_moves_the_resume_point_backwards() {
        let (database, user_id, work_id, _) = one_film().await;
        database
            .record_playback_progress(
                user_id,
                work_id,
                Millis::new(1_800_000),
                PlaybackState::InProgress,
                datetime!(2026-01-01 12:10 UTC),
            )
            .await
            .expect("recorded");

        // A client that was away flushes what it kept: older, and further back.
        let kept = database
            .record_playback_progress(
                user_id,
                work_id,
                Millis::new(60_000),
                PlaybackState::InProgress,
                datetime!(2026-01-01 12:00 UTC),
            )
            .await
            .expect("read");

        assert!(!kept, "a stale report is refused rather than written");
        assert_eq!(
            database
                .playback_progress(user_id, work_id)
                .await
                .expect("read")
                .expect("present")
                .position,
            Millis::new(1_800_000)
        );
    }

    #[tokio::test]
    async fn what_a_person_marked_watched_stays_watched() {
        let (database, user_id, work_id, _) = one_film().await;
        database
            .record_playback_progress(
                user_id,
                work_id,
                Millis::new(1_000),
                PlaybackState::InProgress,
                datetime!(2026-01-01 12:00 UTC),
            )
            .await
            .expect("recorded");
        sqlx::query("UPDATE playback_progress SET marked_manually = 1, state = 'watched'")
            .execute(database.writer())
            .await
            .expect("marked by hand");

        database
            .record_playback_progress(
                user_id,
                work_id,
                Millis::new(2_000),
                PlaybackState::InProgress,
                datetime!(2026-01-01 12:05 UTC),
            )
            .await
            .expect("recorded");

        let stored = database
            .playback_progress(user_id, work_id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            stored.state,
            PlaybackState::Watched,
            "an answer a person gave is not undone by a player reporting a position"
        );
        assert_eq!(
            stored.position,
            Millis::new(2_000),
            "the position still follows, so resuming works"
        );
    }

    #[tokio::test]
    async fn the_tracks_a_viewer_chose_are_remembered_for_next_time() {
        let (database, user_id, work_id, _) = one_film().await;
        let soundtrack = TrackId::new();
        let caption = TrackId::new();

        database
            .record_chosen_tracks(user_id, work_id, Some(soundtrack), Some(caption))
            .await
            .expect("choice recorded");

        let stored = database
            .playback_progress(user_id, work_id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.audio_track_id, Some(soundtrack));
        assert_eq!(stored.subtitle_track_id, Some(caption));
        assert_eq!(
            stored.position,
            Millis::ZERO,
            "choosing a soundtrack says nothing about where anyone is"
        );

        // Turning subtitles off is a choice too, and it has to stick.
        database
            .record_chosen_tracks(user_id, work_id, Some(soundtrack), None)
            .await
            .expect("choice recorded");
        assert_eq!(
            database
                .playback_progress(user_id, work_id)
                .await
                .expect("read")
                .expect("present")
                .subtitle_track_id,
            None
        );
    }

    #[tokio::test]
    async fn a_remembered_track_that_no_longer_makes_sense_is_forgotten_rather_than_fatal() {
        // A row can outlive the file it points into: an analysis run again
        // gives the tracks new identifiers. Playing the film is what matters.
        let (database, user_id, work_id, _) = one_film().await;
        database
            .record_chosen_tracks(user_id, work_id, Some(TrackId::new()), None)
            .await
            .expect("choice recorded");
        sqlx::query("UPDATE playback_progress SET audio_track_id = 'not an identifier'")
            .execute(database.writer())
            .await
            .expect("value forced");

        let stored = database
            .playback_progress(user_id, work_id)
            .await
            .expect("a malformed choice is not a failure")
            .expect("present");
        assert_eq!(stored.audio_track_id, None);
    }

    #[tokio::test]
    async fn viewings_are_counted_apart_from_the_position() {
        let (database, user_id, work_id, _) = one_film().await;
        database
            .count_one_playback(user_id, work_id)
            .await
            .expect("counted");
        database
            .count_one_playback(user_id, work_id)
            .await
            .expect("counted");

        let stored = database
            .playback_progress(user_id, work_id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.play_count, 2);
        assert_eq!(
            stored.position,
            Millis::ZERO,
            "counting a viewing says nothing about where anyone is"
        );
    }
}
