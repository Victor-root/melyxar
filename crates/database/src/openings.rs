//! Where a file's opening and closing titles are, and the seasons still
//! waiting to be listened to for them.
//!
//! A season rather than a file is what waits, since an opening is what every
//! episode of a season holds alike.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, MediaSegmentId, MediaSourceId, WorkId};
use melyxar_core::segments::{MediaSegment, SegmentKind, SegmentOrigin};
use melyxar_core::time::{now, Millis};
use sqlx::{Row, Sqlite};

use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

/// A season that still has files nobody has listened to for their openings.
///
/// The season rather than the file, because a file cannot answer this one on
/// its own: what marks an opening is that every episode of the season holds
/// the same one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonToListenTo {
    pub id: WorkId,
    /// The name of the series it belongs to, for the journal to say which.
    pub series: String,
    /// Its number, which a season read off a folder does not always have.
    pub number: Option<i32>,
    /// How many of its files nobody has listened to yet.
    pub waiting: i64,
}

/// One file of one episode of a season, and what listening to it needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EpisodeToListenTo {
    /// The episode this file is a copy of. Two files of one episode are two
    /// copies of the same sound and must never be compared with each other.
    pub work_id: WorkId,
    pub number: Option<i32>,
    pub source_id: MediaSourceId,
    /// Where the file is, root included. The sound is read straight from it,
    /// so asking for it separately would be one question per episode for
    /// something this one already knows.
    pub path: PathBuf,
    pub duration: Option<Millis>,
}

impl Database {
    /// Seasons still holding a file nobody has listened to for its openings,
    /// oldest first, a few at a time.
    ///
    /// Asked of seasons rather than of files, because a file cannot answer
    /// this on its own. One file with no row puts its whole season back in the
    /// queue, which is exactly what has to happen the day an episode is added
    /// to a season already done: the newcomer has nothing of its own to be
    /// compared against, so the season is listened to again as a season.
    pub async fn seasons_to_listen_to(
        &self,
        library_id: LibraryId,
        limit: i64,
    ) -> Result<Vec<SeasonToListenTo>> {
        let rows = sqlx::query(
            "SELECT season.id, season.ordinal, series.title AS series,
                    count(*) AS waiting, min(season.added_at) AS added_at
             FROM works season
             JOIN works series ON series.id = season.parent_id
             JOIN works episode ON episode.parent_id = season.id
             JOIN media_sources s ON s.work_id = episode.id
             LEFT JOIN media_source_openings o ON o.source_id = s.id
             WHERE season.library_id = ? AND season.kind = 'season'
               AND s.analysed_at IS NOT NULL AND s.missing_since IS NULL
               AND o.source_id IS NULL
             GROUP BY season.id
             ORDER BY added_at
             LIMIT ?",
        )
        .bind(library_id.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                Ok(SeasonToListenTo {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    series: row.try_get("series")?,
                    number: row.try_get("ordinal")?,
                    waiting: row.try_get("waiting")?,
                })
            })
            .collect()
    }

    /// Seasons of one series, by the name it is filed under, whether or not
    /// anybody has listened to them already.
    ///
    /// The way one series is asked for from a terminal, so that a rule that
    /// has just changed can be tried on one series rather than on a whole
    /// collection. Matched on part of the name and without regard for case,
    /// because whoever types it is typing from memory rather than copying a
    /// row out of the database. No name at all means every season there is.
    ///
    /// What comes back says how many files the season holds, not how many are
    /// waiting: a season asked for by name is about to be forgotten and read
    /// again whole, so every one of its files is waiting the moment it is.
    pub async fn seasons_of_series(
        &self,
        series: Option<&str>,
        only: Option<i32>,
    ) -> Result<Vec<SeasonToListenTo>> {
        let rows = sqlx::query(
            "SELECT season.id, season.ordinal, series.title AS series, count(*) AS waiting
             FROM works season
             JOIN works series ON series.id = season.parent_id
             JOIN works episode ON episode.parent_id = season.id
             JOIN media_sources s ON s.work_id = episode.id
             WHERE season.kind = 'season'
               AND (? IS NULL OR lower(series.title) LIKE '%' || lower(?) || '%')
               AND (? IS NULL OR season.ordinal = ?)
               AND s.analysed_at IS NOT NULL AND s.missing_since IS NULL
             GROUP BY season.id
             ORDER BY series.title, season.ordinal",
        )
        .bind(series)
        .bind(series)
        .bind(only)
        .bind(only)
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                Ok(SeasonToListenTo {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    series: row.try_get("series")?,
                    number: row.try_get("ordinal")?,
                    waiting: row.try_get("waiting")?,
                })
            })
            .collect()
    }

    /// Forgets what listening wrote down about one season, putting every one
    /// of its files back in the queue. Answers how many files that was.
    ///
    /// The note saying a file was listened to goes with the stretches that
    /// note stands for, for the same reason the two are written together: a
    /// file marked as done with nothing to show for it is a file that is never
    /// read again.
    ///
    /// A stretch read off a chapter the file names itself, and a stretch
    /// somebody set by hand, both stay. Neither is this reading's to undo, and
    /// forgetting an answer is not somebody changing their mind about one.
    pub async fn forget_the_listening_of(&self, season_id: WorkId) -> Result<u64> {
        let mut transaction = self.begin().await?;
        sqlx::query(
            "DELETE FROM media_segments
             WHERE origin = 'detected'
               AND source_id IN (
                   SELECT s.id FROM media_sources s
                   JOIN works episode ON episode.id = s.work_id
                   WHERE episode.parent_id = ?)",
        )
        .bind(season_id.to_db_string())
        .execute(&mut *transaction)
        .await?;
        let forgotten = sqlx::query(
            "DELETE FROM media_source_openings
             WHERE source_id IN (
                 SELECT s.id FROM media_sources s
                 JOIN works episode ON episode.id = s.work_id
                 WHERE episode.parent_id = ?)",
        )
        .bind(season_id.to_db_string())
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        transaction.commit().await?;
        Ok(forgotten)
    }

    /// How many seasons of one library are still waiting to be listened to.
    pub async fn count_seasons_to_listen_to(&self, library_id: LibraryId) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT count(DISTINCT season.id)
             FROM works season
             JOIN works episode ON episode.parent_id = season.id
             JOIN media_sources s ON s.work_id = episode.id
             LEFT JOIN media_source_openings o ON o.source_id = s.id
             WHERE season.library_id = ? AND season.kind = 'season'
               AND s.analysed_at IS NOT NULL AND s.missing_since IS NULL
               AND o.source_id IS NULL",
        )
        .bind(library_id.to_db_string())
        .fetch_one(self.reader())
        .await?;
        Ok(row.0)
    }

    /// How many seasons of one library have been listened to right through.
    ///
    /// Counted so a screen says how far the whole reading has got rather than
    /// how far this one run of it has, as the three readings beside it do. A
    /// season counts as done when it holds a file and none of its files is
    /// still waiting, which is also true of a season where nothing was found:
    /// nothing found is an answer and is written down as one.
    pub async fn count_seasons_listened_to(&self, library_id: LibraryId) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT count(*) FROM (
                 SELECT season.id
                 FROM works season
                 JOIN works episode ON episode.parent_id = season.id
                 JOIN media_sources s ON s.work_id = episode.id
                 LEFT JOIN media_source_openings o ON o.source_id = s.id
                 WHERE season.library_id = ? AND season.kind = 'season'
                   AND s.analysed_at IS NOT NULL AND s.missing_since IS NULL
                 GROUP BY season.id
                 HAVING sum(CASE WHEN o.source_id IS NULL THEN 1 ELSE 0 END) = 0
             )",
        )
        .bind(library_id.to_db_string())
        .fetch_one(self.reader())
        .await?;
        Ok(row.0)
    }

    /// Every file of every episode of one season, in the order they are
    /// numbered.
    ///
    /// All of them, including the ones already listened to: the answer for a
    /// newcomer only exists beside its neighbours, so a season is read whole
    /// or not at all.
    pub async fn episodes_to_listen_to(&self, season_id: WorkId) -> Result<Vec<EpisodeToListenTo>> {
        let rows = sqlx::query(
            "SELECT episode.id AS work_id, episode.ordinal, s.id AS source_id,
                    s.relative_path, s.duration_ms, library_roots.path AS root_path
             FROM works episode
             JOIN media_sources s ON s.work_id = episode.id
             JOIN library_roots ON library_roots.id = s.root_id
             WHERE episode.parent_id = ?
               AND s.analysed_at IS NOT NULL AND s.missing_since IS NULL
             ORDER BY episode.ordinal, s.added_at",
        )
        .bind(season_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                let root: String = row.try_get("root_path")?;
                let relative: String = row.try_get("relative_path")?;
                Ok(EpisodeToListenTo {
                    work_id: parse_id(&row.try_get::<String, _>("work_id")?)?,
                    number: row.try_get("ordinal")?,
                    source_id: parse_id(&row.try_get::<String, _>("source_id")?)?,
                    path: PathBuf::from(root).join(relative),
                    duration: row
                        .try_get::<Option<i64>, _>("duration_ms")?
                        .map(Millis::new),
                })
            })
            .collect()
    }

    /// Keeps what listening to one file found, and that it was listened to.
    ///
    /// The two in one step, because a file written down as listened to with
    /// its stretches missing would keep its silence for ever: being written
    /// down is exactly what keeps a season out of the queue.
    ///
    /// Only what an earlier listening found is replaced. A stretch read off a
    /// chapter the file names itself, and a stretch somebody set by hand, are
    /// neither of them this reading's to undo: the file said the one and a
    /// person said the other, and both know better than a comparison does.
    pub async fn store_openings(
        &self,
        source_id: MediaSourceId,
        found: &[MediaSegment],
    ) -> Result<()> {
        let mut transaction = self.begin().await?;

        sqlx::query("DELETE FROM media_segments WHERE source_id = ? AND origin = 'detected'")
            .bind(source_id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        for segment in found {
            insert_segment(&mut *transaction, source_id, segment).await?;
        }

        sqlx::query(
            "INSERT INTO media_source_openings (source_id, found, listened_at)
             VALUES (?, ?, ?)
             ON CONFLICT (source_id) DO UPDATE
             SET found = excluded.found,
                 listened_at = excluded.listened_at",
        )
        .bind(source_id.to_db_string())
        .bind(found.len() as i64)
        .bind(timestamp_to_text(now()))
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(())
    }

    /// The stretches of one file nobody wants to sit through.
    pub async fn segments_of_source(&self, source_id: MediaSourceId) -> Result<Vec<MediaSegment>> {
        let rows = sqlx::query(
            "SELECT kind, start_ms, end_ms, origin FROM media_segments
             WHERE source_id = ? ORDER BY start_ms",
        )
        .bind(source_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                let kind: String = row.try_get("kind")?;
                let origin: String = row.try_get("origin")?;
                Ok(MediaSegment {
                    kind: SegmentKind::parse(&kind)
                        .ok_or_else(|| DatabaseError::Corrupt(format!("segment kind '{kind}'")))?,
                    start: Millis::new(row.try_get("start_ms")?),
                    end: Millis::new(row.try_get("end_ms")?),
                    origin: SegmentOrigin::parse(&origin).ok_or_else(|| {
                        DatabaseError::Corrupt(format!("segment origin '{origin}'"))
                    })?,
                })
            })
            .collect()
    }

    /// Says by hand where one kind of stretch is in one file, over whatever
    /// the file's chapters and the listening said of that kind.
    ///
    /// An empty stretch, starting where it ends, says there is none of that
    /// kind here. One correction per kind: a second replaces the first.
    pub async fn correct_segment(
        &self,
        source_id: MediaSourceId,
        kind: SegmentKind,
        start: Millis,
        end: Millis,
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query(
            "DELETE FROM media_segments WHERE source_id = ? AND kind = ? AND origin = 'manual'",
        )
        .bind(source_id.to_db_string())
        .bind(kind.as_str())
        .execute(&mut *transaction)
        .await?;
        let corrected = MediaSegment {
            kind,
            start,
            end,
            origin: SegmentOrigin::Manual,
        };
        insert_segment(&mut *transaction, source_id, &corrected).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Takes back a correction by hand, so the file's chapters and the
    /// listening speak for that kind again.
    ///
    /// The file goes back in the queue of the listening too: a listening that
    /// ran while the correction stood kept nothing of that kind for it, and
    /// only listening again finds it.
    pub async fn forget_segment_correction(
        &self,
        source_id: MediaSourceId,
        kind: SegmentKind,
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query(
            "DELETE FROM media_segments WHERE source_id = ? AND kind = ? AND origin = 'manual'",
        )
        .bind(source_id.to_db_string())
        .bind(kind.as_str())
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM media_source_openings WHERE source_id = ?")
            .bind(source_id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}

pub(crate) async fn insert_segment<'e, E>(
    executor: E,
    source_id: MediaSourceId,
    segment: &MediaSegment,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT INTO media_segments (id, source_id, kind, start_ms, end_ms, origin, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(MediaSegmentId::new().to_db_string())
    .bind(source_id.to_db_string())
    .bind(segment.kind.as_str())
    .bind(segment.start.get())
    .bind(segment.end.get())
    .bind(segment.origin.as_str())
    .bind(timestamp_to_text(now()))
    .execute(executor)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use melyxar_core::id::LibraryRootId;
    use melyxar_core::media::Chapter;
    use melyxar_core::work::WorkKind;

    use crate::catalogue::tests::{a_series_on_disk, a_viewer, library};
    use crate::catalogue::SourceAnalysis;

    /// A series whose every episode holds one analysed file, which is what
    /// listening asks for: a file nobody has read yet says nothing about the
    /// sound inside it.
    async fn a_series_ready_to_be_listened_to(
        database: &Database,
        library_id: LibraryId,
        root_id: LibraryRootId,
        title: &str,
        seasons: &[(i32, &[i32])],
    ) -> Vec<WorkId> {
        let series_id = a_series_on_disk(database, library_id, root_id, title, seasons).await;
        for source in database
            .sources_of_root(root_id)
            .await
            .expect("the files are there")
        {
            database
                .store_analysis(
                    source.id,
                    &SourceAnalysis {
                        duration: Some(Millis::new(2_400_000)),
                        ..SourceAnalysis::default()
                    },
                    &[],
                    &[],
                )
                .await
                .expect("analysis stored");
        }

        let viewer = a_viewer(database).await;
        database
            .children_of(viewer, series_id, "fr")
            .await
            .expect("read")
            .into_iter()
            .map(|season| season.card.id)
            .collect()
    }

    /// The files of one season, in the order listening reads them.
    async fn files_of(database: &Database, season: WorkId) -> Vec<MediaSourceId> {
        database
            .episodes_to_listen_to(season)
            .await
            .expect("read")
            .into_iter()
            .map(|episode| episode.source_id)
            .collect()
    }

    fn a_found_opening() -> MediaSegment {
        MediaSegment {
            kind: SegmentKind::Intro,
            start: Millis::new(30_000),
            end: Millis::new(90_000),
            origin: SegmentOrigin::Detected,
        }
    }

    #[tokio::test]
    async fn a_series_is_found_by_part_of_its_name_whatever_the_case() {
        let (database, library_id, root_id) = library().await;
        a_series_ready_to_be_listened_to(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1, 2]), (2, &[1])],
        )
        .await;
        a_series_ready_to_be_listened_to(
            &database,
            library_id,
            root_id,
            "Harbour Lights",
            &[(1, &[1, 2])],
        )
        .await;

        let found = database
            .seasons_of_series(Some("distant"), None)
            .await
            .expect("read");
        assert_eq!(
            found
                .iter()
                .map(|season| (season.series.as_str(), season.number, season.waiting))
                .collect::<Vec<_>>(),
            vec![
                ("Distant Signal", Some(1), 2),
                ("Distant Signal", Some(2), 1)
            ],
            "every season of the one series, and how many files each holds"
        );

        let one = database
            .seasons_of_series(Some("DISTANT SIGNAL"), Some(2))
            .await
            .expect("read");
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].number, Some(2));

        let all = database.seasons_of_series(None, None).await.expect("read");
        assert_eq!(
            all.iter()
                .map(|season| (season.series.as_str(), season.number))
                .collect::<Vec<_>>(),
            vec![
                ("Distant Signal", Some(1)),
                ("Distant Signal", Some(2)),
                ("Harbour Lights", Some(1))
            ],
            "no name at all is every season of every series"
        );

        assert!(database
            .seasons_of_series(Some("nothing of the sort"), None)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn a_correction_by_hand_is_one_per_kind_and_taken_back_whole() {
        let (database, library_id, root_id) = library().await;
        let seasons = a_series_ready_to_be_listened_to(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1])],
        )
        .await;
        let file = files_of(&database, seasons[0]).await[0];
        database
            .store_openings(file, &[a_found_opening()])
            .await
            .expect("kept");
        let manual = |segments: Vec<MediaSegment>| {
            segments
                .into_iter()
                .filter(|segment| segment.origin == SegmentOrigin::Manual)
                .map(|segment| (segment.kind, segment.start.get(), segment.end.get()))
                .collect::<Vec<_>>()
        };

        database
            .correct_segment(file, SegmentKind::Intro, Millis::new(28_000), Millis::new(88_000))
            .await
            .expect("corrected");
        database
            .correct_segment(file, SegmentKind::Intro, Millis::new(29_000), Millis::new(89_000))
            .await
            .expect("corrected again");
        database
            .correct_segment(file, SegmentKind::Recap, Millis::ZERO, Millis::ZERO)
            .await
            .expect("none said");
        let said = database.segments_of_source(file).await.expect("read");
        assert_eq!(
            manual(said.clone()),
            vec![(SegmentKind::Recap, 0, 0), (SegmentKind::Intro, 29_000, 89_000)],
            "the second correction replaced the first, and none is a correction too"
        );
        assert!(
            said.contains(&a_found_opening()),
            "what the listening found stays behind the correction"
        );

        database
            .forget_segment_correction(file, SegmentKind::Intro)
            .await
            .expect("forgotten");
        assert_eq!(
            manual(database.segments_of_source(file).await.expect("read")),
            vec![(SegmentKind::Recap, 0, 0)],
            "only that kind is taken back"
        );
        assert_eq!(
            database
                .count_seasons_listened_to(library_id)
                .await
                .expect("read"),
            0,
            "the file is back in the queue of the listening"
        );
    }

    #[tokio::test]
    async fn forgetting_a_seasons_listening_puts_its_files_back_and_spares_the_rest() {
        let (database, library_id, root_id) = library().await;
        let seasons = a_series_ready_to_be_listened_to(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1, 2]), (2, &[1])],
        )
        .await;
        // What a person said by hand about one of those files, which no
        // forgetting is allowed to touch.
        let spoken_for = files_of(&database, seasons[0]).await[0];
        let by_hand = MediaSegment {
            kind: SegmentKind::Outro,
            start: Millis::new(2_300_000),
            end: Millis::new(2_360_000),
            origin: SegmentOrigin::Manual,
        };
        database
            .store_openings(spoken_for, &[by_hand])
            .await
            .expect("kept");
        for season in &seasons {
            for file in files_of(&database, *season).await {
                database
                    .store_openings(file, &[a_found_opening()])
                    .await
                    .expect("kept");
            }
        }
        assert_eq!(
            database
                .count_seasons_listened_to(library_id)
                .await
                .expect("read"),
            2
        );

        let forgotten = database
            .forget_the_listening_of(seasons[0])
            .await
            .expect("forgotten");
        assert_eq!(forgotten, 2, "both files of that season, and only those");
        assert_eq!(
            database
                .count_seasons_to_listen_to(library_id)
                .await
                .expect("read"),
            1,
            "the season is back in the queue"
        );
        assert_eq!(
            database
                .count_seasons_listened_to(library_id)
                .await
                .expect("read"),
            1,
            "and the other season is left exactly where it was"
        );
        assert_eq!(
            database.segments_of_source(spoken_for).await.expect("read"),
            vec![by_hand],
            "what was found is gone and what was said by hand stays"
        );
        assert_eq!(
            database
                .segments_of_source(files_of(&database, seasons[1]).await[0])
                .await
                .expect("read"),
            vec![a_found_opening()],
            "the other season keeps what listening found for it"
        );
    }

    #[tokio::test]
    async fn a_season_nobody_has_listened_to_is_waiting_with_its_series_named() {
        let (database, library_id, root_id) = library().await;
        let seasons = a_series_ready_to_be_listened_to(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1, 2, 3]), (2, &[1])],
        )
        .await;

        let waiting = database
            .seasons_to_listen_to(library_id, 10)
            .await
            .expect("read");
        assert_eq!(waiting.len(), 2, "one row per season, not one per file");
        assert!(waiting
            .iter()
            .all(|season| season.series == "Distant Signal"));
        assert_eq!(
            waiting
                .iter()
                .map(|season| (season.number, season.waiting))
                .collect::<Vec<_>>(),
            vec![(Some(1), 3), (Some(2), 1)],
            "each says how many of its files are still waiting"
        );
        assert_eq!(
            database
                .count_seasons_to_listen_to(library_id)
                .await
                .expect("read"),
            2
        );
        assert_eq!(
            database
                .count_seasons_listened_to(library_id)
                .await
                .expect("read"),
            0
        );
        assert!(
            seasons.contains(&waiting[0].id),
            "the seasons answered are the seasons of that series"
        );
    }

    #[tokio::test]
    async fn a_season_is_done_only_once_every_one_of_its_files_is() {
        let (database, library_id, root_id) = library().await;
        let seasons = a_series_ready_to_be_listened_to(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1, 2])],
        )
        .await;
        let files = files_of(&database, seasons[0]).await;
        assert_eq!(files.len(), 2);

        database
            .store_openings(files[0], &[a_found_opening()])
            .await
            .expect("kept");
        assert_eq!(
            database
                .count_seasons_to_listen_to(library_id)
                .await
                .expect("read"),
            1,
            "one file of two settles nothing: the season is still waiting"
        );
        assert_eq!(
            database
                .count_seasons_listened_to(library_id)
                .await
                .expect("read"),
            0
        );

        database.store_openings(files[1], &[]).await.expect("kept");
        assert!(
            database
                .seasons_to_listen_to(library_id, 10)
                .await
                .expect("read")
                .is_empty(),
            "nothing found is an answer, and it settles that file"
        );
        assert_eq!(
            database
                .count_seasons_listened_to(library_id)
                .await
                .expect("read"),
            1
        );
    }

    #[tokio::test]
    async fn an_episode_added_later_puts_its_whole_season_back_in_the_queue() {
        // A newcomer has nothing of its own to be compared against, so the
        // season is listened to again as a season.
        let (database, library_id, root_id) = library().await;
        let seasons = a_series_ready_to_be_listened_to(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1, 2])],
        )
        .await;
        for file in files_of(&database, seasons[0]).await {
            database.store_openings(file, &[]).await.expect("kept");
        }
        assert_eq!(
            database
                .count_seasons_to_listen_to(library_id)
                .await
                .expect("read"),
            0
        );

        let newcomer = database
            .create_child_work(
                library_id,
                seasons[0],
                3,
                WorkKind::Episode,
                "Episode 3",
                "episode 3",
            )
            .await
            .expect("episode written");
        let arrived = database
            .insert_source(
                newcomer.id,
                root_id,
                Path::new("Distant Signal/1/3.mkv"),
                1_000,
                now(),
            )
            .await
            .expect("file recorded");
        database
            .store_analysis(arrived, &SourceAnalysis::default(), &[], &[])
            .await
            .expect("analysis stored");

        let waiting = database
            .seasons_to_listen_to(library_id, 10)
            .await
            .expect("read");
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].id, seasons[0]);
        assert_eq!(waiting[0].waiting, 1, "one file of it is new");
        assert_eq!(
            files_of(&database, seasons[0]).await.len(),
            3,
            "and the whole season is what comes back to be read"
        );
    }

    #[tokio::test]
    async fn the_files_of_a_season_come_back_with_the_episode_each_is_a_copy_of() {
        // Two copies of one episode hold the same sound from end to end, so
        // whatever compares them has to be able to tell they are the same
        // episode and leave them alone.
        let (database, library_id, root_id) = library().await;
        let seasons = a_series_ready_to_be_listened_to(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1, 2])],
        )
        .await;
        let episodes = database
            .episodes_to_listen_to(seasons[0])
            .await
            .expect("read");
        assert_eq!(
            episodes
                .iter()
                .map(|episode| episode.number)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)],
            "in the order they are numbered"
        );
        assert!(episodes
            .iter()
            .all(|episode| episode.duration == Some(Millis::new(2_400_000))));
        assert_eq!(
            episodes[0].path,
            PathBuf::from("/mnt/one/Films/Distant Signal/1/1.mkv"),
            "the file is named in full, so listening to it asks nothing further"
        );

        let second_copy = database
            .insert_source(
                episodes[0].work_id,
                root_id,
                Path::new("Distant Signal/1/1 other.mkv"),
                2_000,
                now(),
            )
            .await
            .expect("file recorded");
        database
            .store_analysis(second_copy, &SourceAnalysis::default(), &[], &[])
            .await
            .expect("analysis stored");

        let again = database
            .episodes_to_listen_to(seasons[0])
            .await
            .expect("read");
        assert_eq!(again.len(), 3);
        assert_eq!(
            again
                .iter()
                .filter(|episode| episode.work_id == episodes[0].work_id)
                .count(),
            2,
            "both copies say which episode they are a copy of"
        );
    }

    #[tokio::test]
    async fn what_listening_found_replaces_only_what_listening_found() {
        let (database, library_id, root_id) = library().await;
        let seasons = a_series_ready_to_be_listened_to(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1])],
        )
        .await;
        let file = files_of(&database, seasons[0]).await[0];

        // A file that names its own chapters, and somebody who corrected the
        // closing titles by hand. Neither is a comparison's to undo.
        database
            .store_analysis(
                file,
                &SourceAnalysis {
                    duration: Some(Millis::new(1_300_000)),
                    ..SourceAnalysis::default()
                },
                &[],
                &[
                    Chapter {
                        ordinal: 1,
                        start: Millis::ZERO,
                        title: Some("Recap".into()),
                        thumbnail_path: None,
                    },
                    Chapter {
                        ordinal: 2,
                        start: Millis::new(40_000),
                        title: Some("Part One".into()),
                        thumbnail_path: None,
                    },
                ],
            )
            .await
            .expect("analysis stored");
        database
            .store_openings(
                file,
                &[MediaSegment {
                    kind: SegmentKind::Outro,
                    start: Millis::new(1_200_000),
                    end: Millis::new(1_260_000),
                    origin: SegmentOrigin::Manual,
                }],
            )
            .await
            .expect("kept");

        database
            .store_openings(file, &[a_found_opening()])
            .await
            .expect("kept");
        database
            .store_openings(file, &[a_found_opening()])
            .await
            .expect("listening twice says the same thing once");

        let kept = database.segments_of_source(file).await.expect("read");
        assert_eq!(
            kept.iter()
                .map(|segment| (segment.kind, segment.origin))
                .collect::<Vec<_>>(),
            vec![
                (SegmentKind::Recap, SegmentOrigin::Chapter),
                (SegmentKind::Intro, SegmentOrigin::Detected),
                (SegmentKind::Outro, SegmentOrigin::Manual),
            ],
            "the chapter and the hand stay, and listening replaces itself: {kept:?}"
        );
    }

    #[tokio::test]
    async fn reading_a_file_again_makes_its_season_wait_to_be_listened_to_again() {
        // What the analysis throws away, it has to throw away whole. A file
        // written down as listened to whose stretches are gone would keep its
        // silence for ever.
        let (database, library_id, root_id) = library().await;
        let seasons = a_series_ready_to_be_listened_to(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1])],
        )
        .await;
        let file = files_of(&database, seasons[0]).await[0];
        database
            .store_openings(file, &[a_found_opening()])
            .await
            .expect("kept");
        assert_eq!(
            database
                .count_seasons_to_listen_to(library_id)
                .await
                .expect("read"),
            0
        );

        database
            .store_analysis(file, &SourceAnalysis::default(), &[], &[])
            .await
            .expect("read again");

        assert!(
            database
                .segments_of_source(file)
                .await
                .expect("read")
                .is_empty(),
            "reading the file again threw away what listening had found"
        );
        assert_eq!(
            database
                .count_seasons_to_listen_to(library_id)
                .await
                .expect("read"),
            1,
            "so the season is waiting again rather than done and silent"
        );
    }
}
