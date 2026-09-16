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
    /// The root the file sits under. Kept because a subtitle in a file of its
    /// own is recorded relative to the root and nowhere else, so without this
    /// there is nothing to join it to.
    pub root: PathBuf,
    pub size_bytes: i64,
    pub container: Option<String>,
    pub duration: Option<Millis>,
    /// How fast the whole file arrives, when the analysis found out.
    ///
    /// Kept because it is the only rate most films in a collection state at
    /// all: the rate of the picture alone is usually absent, and a viewer who
    /// asks for a lighter stream has to be answered on something.
    pub overall_bitrate: Option<i64>,
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

/// A work somebody started and has not finished, and where they got to.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkToCarryOn {
    pub card: crate::browse::WorkCard,
    pub position: Millis,
    /// When it was last played, which is the order a row of them is read in.
    pub last_played_at: Option<Timestamp>,
}

impl Database {
    /// Works this viewer started and has not finished, the latest first.
    ///
    /// A film left halfway is the one thing somebody comes back for, and until
    /// now the only way to it was to remember its title and find it in the
    /// whole library. The position travels with it, so a card can show how far
    /// in it is without asking again per film.
    ///
    /// Only what is really unfinished: a film somebody marked watched, or
    /// played to its end, has nothing to carry on.
    pub async fn works_to_carry_on(
        &self,
        user_id: UserId,
        limit: i64,
    ) -> Result<Vec<WorkToCarryOn>> {
        let rows = sqlx::query(
            "SELECT w.id, w.library_id, w.kind, w.title, w.release_year, w.runtime_ms,
                    w.community_rating, w.identification, w.identification_note,
                    w.dominant_color, w.added_at,
                    p.position_ms, p.last_played_at
             FROM playback_progress p
             JOIN works w ON w.id = p.work_id
             WHERE p.user_id = ? AND p.state = 'in_progress'
             ORDER BY p.last_played_at DESC, w.sort_title
             LIMIT ?",
        )
        .bind(user_id.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        let mut carrying_on: Vec<WorkToCarryOn> = rows
            .iter()
            .map(|row| {
                Ok(WorkToCarryOn {
                    card: crate::browse::card_from_row(row)?,
                    position: Millis::new(row.try_get("position_ms")?),
                    last_played_at: parse_optional_timestamp(
                        row.try_get::<Option<String>, _>("last_played_at")?
                            .as_deref(),
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let mut cards: Vec<crate::browse::WorkCard> =
            carrying_on.iter().map(|entry| entry.card.clone()).collect();
        self.attach_posters(&mut cards).await?;
        for (entry, card) in carrying_on.iter_mut().zip(cards) {
            entry.card = card;
        }
        Ok(carrying_on)
    }

    /// The file behind one source, resolved to a path.
    pub async fn playable_source(&self, id: MediaSourceId) -> Result<Option<PlayableSource>> {
        let row = sqlx::query(
            "SELECT media_sources.id, media_sources.work_id, media_sources.relative_path,
                    media_sources.size_bytes, media_sources.container, media_sources.duration_ms,
                    media_sources.overall_bitrate, media_sources.missing_since,
                    library_roots.path AS root_path
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
            path: PathBuf::from(&root).join(relative),
            root: PathBuf::from(root),
            size_bytes: row.try_get("size_bytes")?,
            container: row.try_get("container")?,
            duration: row
                .try_get::<Option<i64>, _>("duration_ms")?
                .map(Millis::new),
            overall_bitrate: row.try_get("overall_bitrate")?,
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

    /// Whether this viewer has marked a work as one they like.
    pub async fn is_a_favourite(&self, user_id: UserId, work_id: WorkId) -> Result<bool> {
        let row = sqlx::query("SELECT 1 FROM favorites WHERE user_id = ? AND work_id = ?")
            .bind(user_id.to_db_string())
            .bind(work_id.to_db_string())
            .fetch_optional(self.reader())
            .await?;
        Ok(row.is_some())
    }

    /// Marks a work as one this viewer likes, or takes the mark off.
    ///
    /// Says what the answer is now rather than what it was, because that is
    /// what a button is drawn from, and one insistent viewer pressing twice
    /// must not leave the button saying one thing and the row another. Marking
    /// what is already marked keeps the moment it was first marked: it is the
    /// day somebody liked the film, and pressing the button again did not
    /// change that.
    pub async fn set_favourite(
        &self,
        user_id: UserId,
        work_id: WorkId,
        liked: bool,
    ) -> Result<bool> {
        match liked {
            true => {
                sqlx::query(
                    "INSERT INTO favorites (user_id, work_id, created_at) VALUES (?, ?, ?)
                     ON CONFLICT (user_id, work_id) DO NOTHING",
                )
                .bind(user_id.to_db_string())
                .bind(work_id.to_db_string())
                .bind(timestamp_to_text(now()))
                .execute(self.writer())
                .await?;
            }
            false => {
                sqlx::query("DELETE FROM favorites WHERE user_id = ? AND work_id = ?")
                    .bind(user_id.to_db_string())
                    .bind(work_id.to_db_string())
                    .execute(self.writer())
                    .await?;
            }
        }
        Ok(liked)
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
    async fn a_film_somebody_likes_is_marked_and_unmarked_and_says_so_either_way() {
        // Said as it is now rather than as it was, because a button is drawn
        // from the answer: a viewer pressing twice in a second must never end
        // up with a button saying one thing and the row another.
        let (database, user_id, work_id, _) = one_film().await;

        assert!(
            !database
                .is_a_favourite(user_id, work_id)
                .await
                .expect("read"),
            "nobody has said anything about this film"
        );

        assert!(database
            .set_favourite(user_id, work_id, true)
            .await
            .expect("marked"));
        assert!(database
            .is_a_favourite(user_id, work_id)
            .await
            .expect("read"));

        // Marking what is already marked is not an error and not a second row.
        assert!(database
            .set_favourite(user_id, work_id, true)
            .await
            .expect("marked again"));
        assert!(database
            .is_a_favourite(user_id, work_id)
            .await
            .expect("read"));

        assert!(!database
            .set_favourite(user_id, work_id, false)
            .await
            .expect("unmarked"));
        assert!(
            !database
                .is_a_favourite(user_id, work_id)
                .await
                .expect("read"),
            "and taking the mark off leaves nothing behind"
        );

        // Taking off what was never on is the same answer, not a failure.
        assert!(!database
            .set_favourite(user_id, work_id, false)
            .await
            .expect("unmarked again"));
    }

    #[tokio::test]
    async fn how_fast_a_file_arrives_comes_back_with_it() {
        // It is the only rate most films state at all, and a viewer asking for
        // a lighter stream has to be answered on something.
        let (database, _, _, source_id) = one_film().await;
        assert_eq!(
            database
                .playable_source(source_id)
                .await
                .expect("read")
                .expect("present")
                .overall_bitrate,
            None,
            "nothing has looked inside this file yet"
        );

        database
            .store_analysis(
                source_id,
                &crate::catalogue::SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: Some(30_000_000),
                },
                &[],
                &[],
            )
            .await
            .expect("analysis stored");

        assert_eq!(
            database
                .playable_source(source_id)
                .await
                .expect("read")
                .expect("present")
                .overall_bitrate,
            Some(30_000_000)
        );
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
    async fn a_film_left_halfway_is_offered_back_with_where_it_stopped() {
        // The one thing somebody comes back for. Finding it used to mean
        // remembering the title and hunting it down in the whole library.
        let (database, user_id, work_id, _) = one_film().await;
        assert!(database
            .works_to_carry_on(user_id, 20)
            .await
            .expect("read")
            .is_empty());

        database
            .record_playback_progress(
                user_id,
                work_id,
                Millis::new(1_800_000),
                PlaybackState::InProgress,
                datetime!(2026-01-01 12:00 UTC),
            )
            .await
            .expect("recorded");

        let carrying_on = database.works_to_carry_on(user_id, 20).await.expect("read");
        assert_eq!(carrying_on.len(), 1);
        assert_eq!(carrying_on[0].card.id, work_id);
        assert_eq!(carrying_on[0].card.title, "Quiet Harbour");
        assert_eq!(
            carrying_on[0].position,
            Millis::new(1_800_000),
            "where it stopped travels with it, rather than being asked for per film"
        );

        // A film watched to its end has nothing to carry on.
        database
            .record_playback_progress(
                user_id,
                work_id,
                Millis::new(7_000_000),
                PlaybackState::Watched,
                datetime!(2026-01-01 13:00 UTC),
            )
            .await
            .expect("recorded");
        assert!(database
            .works_to_carry_on(user_id, 20)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn what_was_watched_last_is_offered_first() {
        let (database, user_id, first, _) = one_film().await;
        let library = database.list_libraries().await.expect("read")[0].id;
        let second = database
            .create_work(library, WorkKind::Movie, "Amber Field", "amber field", None)
            .await
            .expect("work created");

        database
            .record_playback_progress(
                user_id,
                first,
                Millis::new(600_000),
                PlaybackState::InProgress,
                datetime!(2026-01-01 12:00 UTC),
            )
            .await
            .expect("recorded");
        database
            .record_playback_progress(
                user_id,
                second.id,
                Millis::new(600_000),
                PlaybackState::InProgress,
                datetime!(2026-01-02 21:00 UTC),
            )
            .await
            .expect("recorded");

        let carrying_on = database.works_to_carry_on(user_id, 20).await.expect("read");
        assert_eq!(
            carrying_on
                .iter()
                .map(|entry| entry.card.title.clone())
                .collect::<Vec<_>>(),
            vec!["Amber Field".to_string(), "Quiet Harbour".to_string()],
            "a row is read from the left, and the left is where somebody just was"
        );
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
