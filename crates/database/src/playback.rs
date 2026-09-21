//! What playing a file needs from the storage, and where a viewer got to.
//!
//! Two things live here. The file itself, resolved to a path on disk, since a
//! source is stored relative to a root and only the storage knows where that
//! root is. And the position a viewer reached, which carries the instant it
//! was measured: a report arriving late must never make a resume point go
//! backwards, which is exactly what happens when a client reconnects and
//! flushes a copy it kept while it was away.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, MediaSourceId, TrackId, UserId, WorkId};
use melyxar_core::time::{now, Millis, Timestamp};
use melyxar_core::work::{should_accept_position, PlaybackState};
use sqlx::{AssertSqlSafe, Row};

use crate::browse::WHAT_A_CARD_IS;
use crate::convert::{parse_id, parse_optional_timestamp, timestamp_to_text};
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

/// The episode a series is waiting on, and the series it belongs to.
///
/// Distinct from carrying on, and deliberately: what somebody left halfway is
/// not what they have not started. An episode here has never been played, so
/// it shows no progress bar, and what a card leads with is the series rather
/// than the episode, which is the only name anybody remembers.
#[derive(Debug, Clone, PartialEq)]
pub struct UpNext {
    pub card: crate::browse::WorkCard,
    pub series_id: WorkId,
    pub series_title: String,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
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
        within: Option<&[LibraryId]>,
        limit: i64,
    ) -> Result<Vec<WorkToCarryOn>> {
        // Kept inside what this account was granted, like every other read.
        // Progress outlives a grant: somebody who watched half a film in a
        // library that was later taken away from them still has the row, and
        // without this the row would put the title and the poster back on
        // their home page.
        let Some(inside) = crate::browse::kept_inside(within, "w.library_id") else {
            return Ok(Vec::new());
        };
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_CARD_IS},
                    p.position_ms, p.last_played_at
             FROM playback_progress p
             JOIN works w ON w.id = p.work_id
             WHERE p.user_id = ? AND p.state = 'in_progress'{inside}
             ORDER BY p.last_played_at DESC, w.sort_title
             LIMIT ?"
        )))
        .bind(user_id.to_db_string());
        for granted in within.iter().copied().flatten() {
            query = query.bind(granted.to_db_string());
        }
        let rows = query.bind(limit).fetch_all(self.reader()).await?;

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
        // The same card is drawn here as in a grid, so it carries the same
        // marks: one component, one shape of card, one hover.
        self.attach_viewer_state(user_id, &mut cards).await?;
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

    /// The episode each started series is waiting on, the latest first.
    ///
    /// A series is here when this viewer has watched at least one episode of
    /// it and is not partway through another: an episode left halfway belongs
    /// to carrying on, and a series would otherwise stand in both rows at once
    /// saying two different things.
    ///
    /// Read in order rather than from the last one played, for the same reason
    /// a single series is: a series watched out of order has a hole in it, and
    /// the hole is what somebody means by where they are. Episodes with no
    /// file behind them are skipped, because this ends in a button that has to
    /// play something.
    pub async fn up_next(
        &self,
        user_id: UserId,
        within: Option<&[LibraryId]>,
        limit: i64,
    ) -> Result<Vec<UpNext>> {
        // An account granted nothing has nothing waiting for it.
        if within.is_some_and(<[LibraryId]>::is_empty) {
            return Ok(Vec::new());
        }
        let granted = within.unwrap_or_default();
        // Every place in this statement is numbered, and the libraries are
        // numbered on from three. Mixed with plain question marks, the viewer
        // named four times over would shift everything after it and the joins
        // would match nobody, which reads exactly like an account that has
        // watched nothing.
        let inside = match granted.is_empty() {
            true => String::new(),
            false => format!(
                " AND e.library_id IN ({})",
                (3..=granted.len() + 2)
                    .map(|place| format!("?{place}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };

        let mut query = sqlx::query(AssertSqlSafe(format!(
            "WITH begun AS (
                 -- Series this viewer has really started: one watched episode
                 -- is a start, and an unopened series is not up next, it is
                 -- simply there.
                 SELECT DISTINCT s.parent_id AS series_id
                   FROM playback_progress p
                   JOIN works e ON e.id = p.work_id AND e.kind = 'episode'
                   JOIN works s ON s.id = e.parent_id
                  WHERE p.user_id = ?1 AND p.state = 'watched'
                    AND s.parent_id IS NOT NULL
             ),
             partway AS (
                 -- And the ones they are in the middle of, which belong to
                 -- the row above this one instead.
                 SELECT DISTINCT s.parent_id AS series_id
                   FROM playback_progress p
                   JOIN works e ON e.id = p.work_id AND e.kind = 'episode'
                   JOIN works s ON s.id = e.parent_id
                  WHERE p.user_id = ?1 AND p.state = 'in_progress'
                    AND s.parent_id IS NOT NULL
             ),
             waiting AS (
                 SELECT e.id AS episode_id,
                        s.parent_id AS series_id,
                        s.ordinal AS season_number,
                        e.ordinal AS episode_number,
                        row_number() OVER (
                            PARTITION BY s.parent_id
                            ORDER BY s.ordinal, e.ordinal, e.id
                        ) AS place
                   FROM works e
                   JOIN works s ON s.id = e.parent_id
                   JOIN begun ON begun.series_id = s.parent_id
                   LEFT JOIN playback_progress p
                          ON p.work_id = e.id AND p.user_id = ?1
                  WHERE e.kind = 'episode'
                    AND coalesce(p.state, 'not_started') = 'not_started'
                    AND s.parent_id NOT IN (SELECT series_id FROM partway)
                    AND EXISTS (SELECT 1 FROM media_sources m
                                 WHERE m.work_id = e.id AND m.missing_since IS NULL)
                    {inside}
             )
             SELECT {WHAT_A_CARD_IS},
                    waiting.series_id, waiting.season_number, waiting.episode_number,
                    series.title AS series_title,
                    -- When this series was last touched at all, which is the
                    -- order a row of them is read in: what somebody watched
                    -- last night comes before what they watched in March.
                    (SELECT max(q.last_played_at)
                       FROM playback_progress q
                       JOIN works ee ON ee.id = q.work_id AND ee.kind = 'episode'
                       JOIN works ss ON ss.id = ee.parent_id
                      WHERE ss.parent_id = waiting.series_id
                        AND q.user_id = ?1) AS touched_at
               FROM waiting
               JOIN works w ON w.id = waiting.episode_id
               JOIN works series ON series.id = waiting.series_id
              WHERE waiting.place = 1
              ORDER BY touched_at DESC, series.sort_title
              LIMIT ?2"
        )))
        .bind(user_id.to_db_string())
        .bind(limit);
        for library in granted {
            query = query.bind(library.to_db_string());
        }
        let rows = query.fetch_all(self.reader()).await?;

        let mut waiting: Vec<UpNext> = rows
            .iter()
            .map(|row| {
                Ok(UpNext {
                    card: crate::browse::card_from_row(row)?,
                    series_id: parse_id(&row.try_get::<String, _>("series_id")?)?,
                    series_title: row.try_get("series_title")?,
                    season_number: row.try_get("season_number")?,
                    episode_number: row.try_get("episode_number")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        let mut cards: Vec<crate::browse::WorkCard> =
            waiting.iter().map(|entry| entry.card.clone()).collect();
        self.attach_posters(&mut cards).await?;
        self.attach_viewer_state(user_id, &mut cards).await?;
        for (entry, card) in waiting.iter_mut().zip(cards) {
            entry.card = card;
        }
        Ok(waiting)
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

    /// Marks a work watched, or puts it back to unwatched, by hand.
    ///
    /// A film and an episode answer for themselves. A season and a series
    /// answer for their episodes, because a series is not what anybody
    /// watches: marking one without going down would leave a badge saying
    /// twelve episodes left on a series marked watched, which is two screens
    /// contradicting each other.
    ///
    /// The mark is written as a manual one, which is what makes it outlast
    /// whatever a player reports afterwards. Unmarking takes the position with
    /// it: a series put back to unwatched that still offered to carry on
    /// halfway through would be saying two things at once.
    ///
    /// Answers how many works it wrote, so a caller can tell a work that was
    /// marked from a name that means nothing here.
    pub async fn mark_watched(
        &self,
        user_id: UserId,
        work_id: WorkId,
        watched: bool,
    ) -> Result<u64> {
        let state = match watched {
            true => PlaybackState::Watched,
            false => PlaybackState::NotStarted,
        };
        // One statement for the three shapes: the work itself when it is not
        // one that holds episodes, and otherwise every episode below it, at
        // either depth, since a series holds them under its seasons.
        let written = sqlx::query(
            "INSERT INTO playback_progress
                (user_id, work_id, position_ms, state, marked_manually, reported_at,
                 last_played_at)
             SELECT ?1, t.id, 0, ?2, ?3, ?4, ?4
               FROM works t
              WHERE (t.id = ?5 AND t.kind NOT IN ('series', 'season'))
                 OR (t.kind = 'episode'
                     AND (t.parent_id = ?5
                          OR t.parent_id IN (SELECT id FROM works WHERE parent_id = ?5)))
             ON CONFLICT (user_id, work_id) DO UPDATE SET
                state = excluded.state,
                marked_manually = excluded.marked_manually,
                position_ms = CASE WHEN excluded.state = 'watched'
                                   THEN playback_progress.position_ms
                                   ELSE 0 END,
                reported_at = excluded.reported_at,
                last_played_at = excluded.last_played_at",
        )
        .bind(user_id.to_db_string())
        .bind(state.as_str())
        .bind(crate::convert::bool_to_int(watched))
        .bind(timestamp_to_text(now()))
        .bind(work_id.to_db_string())
        .execute(self.writer())
        .await?
        .rows_affected();
        Ok(written)
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

    /// A series of one season and three episodes, with somebody to watch it.
    async fn one_series() -> (Database, UserId, WorkId, WorkId, Vec<WorkId>) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Series",
                LibraryKind::Series,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Series"))],
            )
            .await
            .expect("library created");
        let series = database
            .create_work(
                library.id,
                WorkKind::Series,
                "Amber Field",
                "amber field",
                Some(2021),
            )
            .await
            .expect("series created");
        let season = database
            .create_child_work(
                library.id,
                series.id,
                1,
                WorkKind::Season,
                "Season 1",
                "season 1",
            )
            .await
            .expect("season created");
        let mut episodes = Vec::new();
        for number in 1..=3 {
            episodes.push(
                database
                    .create_child_work(
                        library.id,
                        season.id,
                        number,
                        WorkKind::Episode,
                        &format!("Episode {number}"),
                        &format!("episode {number}"),
                    )
                    .await
                    .expect("episode created")
                    .id,
            );
        }
        let user = database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");
        (database, user.id, series.id, season.id, episodes)
    }

    /// A file behind an episode, so it can be offered by a button that plays.
    async fn a_file_behind(database: &Database, work_id: WorkId, library: LibraryId) {
        let root = database
            .library_roots(library)
            .await
            .expect("roots read")
            .remove(0);
        database
            .insert_source(
                work_id,
                root.id,
                Path::new(&format!("{work_id}.mkv")),
                1_000,
                now(),
            )
            .await
            .expect("file recorded");
    }

    #[tokio::test]
    async fn a_started_series_offers_the_episode_it_is_waiting_on() {
        let (database, user_id, series, _, episodes) = one_series().await;
        let library = database
            .work(series)
            .await
            .expect("series read")
            .expect("series present")
            .library_id;
        for episode in &episodes {
            a_file_behind(&database, *episode, library).await;
        }

        assert!(
            database
                .up_next(user_id, None, 10)
                .await
                .expect("read")
                .is_empty(),
            "a series nobody has opened is not waiting on anything"
        );

        database
            .mark_watched(user_id, episodes[0], true)
            .await
            .expect("marked");

        let waiting = database.up_next(user_id, None, 10).await.expect("read");
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].card.id, episodes[1], "the next one, in order");
        assert_eq!(waiting[0].series_id, series);
        assert_eq!(waiting[0].series_title, "Amber Field");
        assert_eq!(waiting[0].season_number, Some(1));
        assert_eq!(waiting[0].episode_number, Some(2));
    }

    #[tokio::test]
    async fn a_series_being_watched_right_now_is_not_also_waiting() {
        let (database, user_id, series, _, episodes) = one_series().await;
        let library = database
            .work(series)
            .await
            .expect("series read")
            .expect("series present")
            .library_id;
        for episode in &episodes {
            a_file_behind(&database, *episode, library).await;
        }

        database
            .mark_watched(user_id, episodes[0], true)
            .await
            .expect("marked");
        database
            .record_playback_progress(
                user_id,
                episodes[1],
                Millis::new(600_000),
                PlaybackState::InProgress,
                now(),
            )
            .await
            .expect("position recorded");

        assert!(
            database
                .up_next(user_id, None, 10)
                .await
                .expect("read")
                .is_empty(),
            "an episode left halfway belongs to carrying on, not to this row"
        );
    }

    #[tokio::test]
    async fn an_episode_with_no_file_behind_it_is_never_offered() {
        let (database, user_id, series, _, episodes) = one_series().await;
        let library = database
            .work(series)
            .await
            .expect("series read")
            .expect("series present")
            .library_id;
        // Only the third has a file: the second is a gap in the collection.
        a_file_behind(&database, episodes[0], library).await;
        a_file_behind(&database, episodes[2], library).await;

        database
            .mark_watched(user_id, episodes[0], true)
            .await
            .expect("marked");

        let waiting = database.up_next(user_id, None, 10).await.expect("read");
        assert_eq!(
            waiting[0].card.id, episodes[2],
            "this ends in a button that has to play something"
        );
    }

    #[tokio::test]
    async fn neither_row_reaches_past_the_libraries_an_account_was_granted() {
        let (database, user_id, series, _, episodes) = one_series().await;
        let library = database
            .work(series)
            .await
            .expect("series read")
            .expect("series present")
            .library_id;
        for episode in &episodes {
            a_file_behind(&database, *episode, library).await;
        }
        database
            .mark_watched(user_id, episodes[0], true)
            .await
            .expect("marked");
        database
            .record_playback_progress(
                user_id,
                episodes[2],
                Millis::new(600_000),
                PlaybackState::InProgress,
                now(),
            )
            .await
            .expect("position recorded");

        let elsewhere = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-two".to_string(), PathBuf::from("/mnt/two/Films"))],
            )
            .await
            .expect("library created");

        assert!(
            database
                .works_to_carry_on(user_id, Some(&[elsewhere.id]), 10)
                .await
                .expect("read")
                .is_empty(),
            "progress outlives a grant, so the row has to be kept inside it"
        );
        assert!(
            database
                .up_next(user_id, Some(&[elsewhere.id]), 10)
                .await
                .expect("read")
                .is_empty()
        );
        assert!(
            database
                .works_to_carry_on(user_id, Some(&[]), 10)
                .await
                .expect("read")
                .is_empty(),
            "an account granted nothing reads nothing"
        );
        assert!(
            !database
                .works_to_carry_on(user_id, Some(&[library]), 10)
                .await
                .expect("read")
                .is_empty(),
            "and the library it was granted still answers"
        );
    }

    #[tokio::test]
    async fn marking_a_film_watched_by_hand_outlasts_what_a_player_says_next() {
        let (database, user_id, work_id, _) = one_film().await;

        assert_eq!(
            database
                .mark_watched(user_id, work_id, true)
                .await
                .expect("marked"),
            1
        );

        // A player reporting the very beginning afterwards does not undo it.
        database
            .record_playback_progress(
                user_id,
                work_id,
                Millis::new(400),
                PlaybackState::InProgress,
                now(),
            )
            .await
            .expect("position recorded");

        let stored = database
            .playback_progress(user_id, work_id)
            .await
            .expect("progress read")
            .expect("a row was written");
        assert_eq!(stored.state, PlaybackState::Watched);
    }

    #[tokio::test]
    async fn putting_a_film_back_to_unwatched_takes_its_position_with_it() {
        let (database, user_id, work_id, _) = one_film().await;
        database
            .record_playback_progress(
                user_id,
                work_id,
                Millis::new(920_000),
                PlaybackState::InProgress,
                now(),
            )
            .await
            .expect("position recorded");

        database
            .mark_watched(user_id, work_id, false)
            .await
            .expect("marked");

        let stored = database
            .playback_progress(user_id, work_id)
            .await
            .expect("progress read")
            .expect("a row was written");
        assert_eq!(stored.state, PlaybackState::NotStarted);
        assert_eq!(
            stored.position,
            Millis::new(0),
            "a work put back to unwatched must not still offer to carry on"
        );
    }

    #[tokio::test]
    async fn marking_a_series_watched_marks_every_episode_under_it() {
        let (database, user_id, series, _, episodes) = one_series().await;

        assert_eq!(
            database
                .mark_watched(user_id, series, true)
                .await
                .expect("marked"),
            3,
            "a series is not what anybody watches: its episodes are"
        );

        for episode in &episodes {
            let stored = database
                .playback_progress(user_id, *episode)
                .await
                .expect("progress read")
                .expect("a row was written");
            assert_eq!(stored.state, PlaybackState::Watched);
        }
    }

    #[tokio::test]
    async fn marking_a_season_watched_marks_only_its_own_episodes() {
        let (database, user_id, series, season, _) = one_series().await;
        let other = database
            .create_child_work(
                database
                    .work(series)
                    .await
                    .expect("series read")
                    .expect("series present")
                    .library_id,
                series,
                2,
                WorkKind::Season,
                "Season 2",
                "season 2",
            )
            .await
            .expect("season created");
        let apart = database
            .create_child_work(
                other.library_id,
                other.id,
                1,
                WorkKind::Episode,
                "Episode 1",
                "episode 1",
            )
            .await
            .expect("episode created");

        assert_eq!(
            database
                .mark_watched(user_id, season, true)
                .await
                .expect("marked"),
            3
        );
        assert_eq!(
            database
                .playback_progress(user_id, apart.id)
                .await
                .expect("progress read"),
            None,
            "another season's episodes are not this season's to mark"
        );
    }

    #[tokio::test]
    async fn a_name_that_means_nothing_here_marks_nothing() {
        let (database, user_id, _, _) = one_film().await;
        assert_eq!(
            database
                .mark_watched(user_id, WorkId::new(), false)
                .await
                .expect("marked"),
            0
        );
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
            .works_to_carry_on(user_id, None, 20)
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

        let carrying_on = database.works_to_carry_on(user_id, None, 20).await.expect("read");
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
            .works_to_carry_on(user_id, None, 20)
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

        let carrying_on = database.works_to_carry_on(user_id, None, 20).await.expect("read");
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
