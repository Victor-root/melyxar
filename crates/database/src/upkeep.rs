//! What the long passes over each file leave behind: where its picture can be
//! started, the little pictures of its bar, and the subtitles pulled out of it.
//!
//! Each pass asks here for the files still waiting on it and writes down what
//! it found, so a pass cut short picks up where it stopped.

use melyxar_core::id::{LibraryId, MediaSourceId};
use melyxar_core::thumbnails::{Layout, Thumbnails};
use melyxar_core::time::{now, Millis};
use sqlx::Row;

use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, Result};

/// The condition that keeps a photo out of the readings made for what moves:
/// where a picture can be started, and the little pictures of the bar. A
/// photo has neither, and would fail both once per photo at every pass.
///
/// A macro rather than a constant so it can be written into a statement that
/// stays a plain piece of text.
macro_rules! a_moving_picture {
    () => {
        "AND NOT EXISTS (SELECT 1 FROM works
                          WHERE works.id = media_sources.work_id AND works.kind = 'photo')"
    };
}

impl Database {
    /// Keeps where the picture of one file can be started.
    ///
    /// Replaces whatever was there: a file read again has been read again, and
    /// two answers about the same file are one answer too many.
    pub async fn store_key_frames(
        &self,
        source_id: MediaSourceId,
        positions: &[Millis],
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO media_source_key_frames (source_id, positions_ms, counted, read_at)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (source_id) DO UPDATE
             SET positions_ms = excluded.positions_ms,
                 counted = excluded.counted,
                 read_at = excluded.read_at",
        )
        .bind(source_id.to_db_string())
        .bind(write_positions(positions))
        .bind(positions.len() as i64)
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Where the picture of one file can be started, when it has been read.
    pub async fn key_frames_of(&self, source_id: MediaSourceId) -> Result<Option<Vec<Millis>>> {
        let row =
            sqlx::query("SELECT positions_ms FROM media_source_key_frames WHERE source_id = ?")
                .bind(source_id.to_db_string())
                .fetch_optional(self.reader())
                .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let written: String = row.try_get("positions_ms")?;
        Ok(Some(read_positions(&written)))
    }

    /// Files that have been described but never read for where they can be
    /// started, oldest first, a few at a time.
    ///
    /// Reading one means reading the whole file through once, so this is a
    /// background pass bounded like the analysis, and it is picked up again by
    /// the next one rather than run to the end in a single sitting.
    pub async fn sources_without_key_frames(
        &self,
        library_id: LibraryId,
        limit: i64,
    ) -> Result<Vec<MediaSourceId>> {
        let rows = sqlx::query(concat!(
            "SELECT media_sources.id
             FROM media_sources
             JOIN library_roots ON library_roots.id = media_sources.root_id
             LEFT JOIN media_source_key_frames
                    ON media_source_key_frames.source_id = media_sources.id
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_source_key_frames.source_id IS NULL
               ",
            a_moving_picture!(),
            "
             ORDER BY media_sources.added_at
             LIMIT ?",
        ))
        .bind(library_id.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        rows.into_iter()
            .map(|row| parse_id(&row.try_get::<String, _>("id")?))
            .collect()
    }

    /// How many files of one library are still waiting to be read for where
    /// their picture can be started.
    ///
    /// Counted rather than worked out from a batch: the reading is done a few
    /// files at a time so that a library of a hundred thousand never becomes a
    /// hundred thousand rows in memory, and a bar sized on one batch would
    /// reach its end several times over. This is also what the upkeep screen
    /// shows when nothing is running, which is the moment somebody wants to
    /// know whether there is anything left to do at all.
    pub async fn count_awaiting_key_frames(&self, library_id: LibraryId) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(concat!(
            "SELECT count(*)
             FROM media_sources
             JOIN library_roots ON library_roots.id = media_sources.root_id
             LEFT JOIN media_source_key_frames
                    ON media_source_key_frames.source_id = media_sources.id
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_source_key_frames.source_id IS NULL
               ",
            a_moving_picture!()
        ))
        .bind(library_id.to_db_string())
        .fetch_one(self.reader())
        .await?;
        Ok(row.0)
    }

    /// How many files of one library have already been read for where their
    /// picture can be started.
    ///
    /// Counted so that a scan can say how far the whole pass has got rather
    /// than how far this one run of it has. A pass that picks up where it left
    /// off and one that starts again from nothing look exactly alike from a
    /// bar that always begins at zero.
    ///
    /// Over the same films the count of what is waiting is taken over: the two
    /// are added together to make a total, and a file read once and since taken
    /// off the disk was counted by one of them and not the other, so the total
    /// described a set neither of them did.
    pub async fn count_read_for_key_frames(&self, library_id: LibraryId) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT count(*)
             FROM media_source_key_frames
             JOIN media_sources ON media_sources.id = media_source_key_frames.source_id
             JOIN library_roots ON library_roots.id = media_sources.root_id
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL",
        )
        .bind(library_id.to_db_string())
        .fetch_one(self.reader())
        .await?;
        Ok(row.0)
    }

    /// Keeps what a reading of one film gave for the bar somebody drags along.
    ///
    /// Replaces whatever was there: a film read again has been read again.
    /// Nothing counted is kept as well as anything else, because it is an
    /// answer about the file rather than a failure to have one.
    pub async fn store_thumbnails(
        &self,
        source_id: MediaSourceId,
        made: &Thumbnails,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO media_source_thumbnails
                 (source_id, every_ms, thumbnail_width, thumbnail_height,
                  columns_per_sheet, rows_per_sheet, counted, sheets, made_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (source_id) DO UPDATE
             SET every_ms = excluded.every_ms,
                 thumbnail_width = excluded.thumbnail_width,
                 thumbnail_height = excluded.thumbnail_height,
                 columns_per_sheet = excluded.columns_per_sheet,
                 rows_per_sheet = excluded.rows_per_sheet,
                 counted = excluded.counted,
                 sheets = excluded.sheets,
                 made_at = excluded.made_at",
        )
        .bind(source_id.to_db_string())
        .bind(made.every.get())
        .bind(made.width as i64)
        .bind(made.height as i64)
        .bind(made.columns as i64)
        .bind(made.rows as i64)
        .bind(made.counted as i64)
        .bind(made.sheets as i64)
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// What one film has for the bar, when it has been read for it.
    pub async fn thumbnails_of(&self, source_id: MediaSourceId) -> Result<Option<Thumbnails>> {
        let row = sqlx::query(
            "SELECT every_ms, thumbnail_width, thumbnail_height, columns_per_sheet,
                    rows_per_sheet, counted, sheets
             FROM media_source_thumbnails
             WHERE source_id = ?",
        )
        .bind(source_id.to_db_string())
        .fetch_optional(self.reader())
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        Ok(Some(Thumbnails {
            every: Millis::new(row.try_get("every_ms")?),
            width: row.try_get::<i64, _>("thumbnail_width")? as u32,
            height: row.try_get::<i64, _>("thumbnail_height")? as u32,
            columns: row.try_get::<i64, _>("columns_per_sheet")? as u32,
            rows: row.try_get::<i64, _>("rows_per_sheet")? as u32,
            counted: row.try_get::<i64, _>("counted")? as u32,
            sheets: row.try_get::<i64, _>("sheets")? as u32,
        }))
    }

    /// Forgets what a film gave for its bar, so it is read for it again.
    ///
    /// For the one case the table cannot see: a cache emptied by hand. The row
    /// then says a film has thumbnails that are not on the disk, and nothing
    /// would ever put them back, because being written down is exactly what
    /// keeps a film out of the pass that makes them.
    pub async fn forget_thumbnails(&self, source_id: MediaSourceId) -> Result<()> {
        sqlx::query("DELETE FROM media_source_thumbnails WHERE source_id = ?")
            .bind(source_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Files whose thumbnails are missing or were made to another shape,
    /// oldest first, a few at a time.
    ///
    /// The shape is part of the question rather than checked afterwards:
    /// change the interval and every film stops matching, which is exactly
    /// when they all have to be made again.
    pub async fn sources_without_thumbnails(
        &self,
        library_id: LibraryId,
        wanted: Layout,
        limit: i64,
    ) -> Result<Vec<MediaSourceId>> {
        let rows = sqlx::query(concat!(
            "SELECT media_sources.id
             FROM media_sources
             JOIN library_roots ON library_roots.id = media_sources.root_id
             LEFT JOIN media_source_thumbnails
                    ON media_source_thumbnails.source_id = media_sources.id
                   AND media_source_thumbnails.every_ms = ?
                   AND media_source_thumbnails.rows_per_sheet = ?
                   AND media_source_thumbnails.columns_per_sheet = ?
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_source_thumbnails.source_id IS NULL
               ",
            a_moving_picture!(),
            "
             ORDER BY media_sources.added_at
             LIMIT ?",
        ))
        .bind(wanted.every.get())
        .bind(wanted.rows as i64)
        .bind(wanted.columns as i64)
        .bind(library_id.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        rows.into_iter()
            .map(|row| parse_id(&row.try_get::<String, _>("id")?))
            .collect()
    }

    /// How many files of one library are still waiting for thumbnails of this
    /// shape.
    ///
    /// Counted for the same reason as the reading before it, and with the
    /// shape part of the question rather than checked afterwards: change the
    /// interval and every film is waiting again, which is exactly when the
    /// number matters.
    pub async fn count_awaiting_thumbnails(
        &self,
        library_id: LibraryId,
        wanted: Layout,
    ) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(concat!(
            "SELECT count(*)
             FROM media_sources
             JOIN library_roots ON library_roots.id = media_sources.root_id
             LEFT JOIN media_source_thumbnails
                    ON media_source_thumbnails.source_id = media_sources.id
                   AND media_source_thumbnails.every_ms = ?
                   AND media_source_thumbnails.rows_per_sheet = ?
                   AND media_source_thumbnails.columns_per_sheet = ?
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_source_thumbnails.source_id IS NULL
               ",
            a_moving_picture!()
        ))
        .bind(wanted.every.get())
        .bind(wanted.rows as i64)
        .bind(wanted.columns as i64)
        .bind(library_id.to_db_string())
        .fetch_one(self.reader())
        .await?;
        Ok(row.0)
    }

    /// How many files of one library already have thumbnails of this shape.
    ///
    /// Counted so a scan says how far the whole pass has got rather than how
    /// far this one run of it has: a pass picking up where it left off and one
    /// starting again from nothing look alike from a bar that begins at zero.
    pub async fn count_made_thumbnails(
        &self,
        library_id: LibraryId,
        wanted: Layout,
    ) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT count(*)
             FROM media_source_thumbnails
             JOIN media_sources ON media_sources.id = media_source_thumbnails.source_id
             JOIN library_roots ON library_roots.id = media_sources.root_id
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_source_thumbnails.every_ms = ?
               AND media_source_thumbnails.rows_per_sheet = ?
               AND media_source_thumbnails.columns_per_sheet = ?",
        )
        .bind(library_id.to_db_string())
        .bind(wanted.every.get())
        .bind(wanted.rows as i64)
        .bind(wanted.columns as i64)
        .fetch_one(self.reader())
        .await?;
        Ok(row.0)
    }

    /// Keeps that one file has had its subtitles made of words pulled out.
    ///
    /// Replaces whatever was there, for the same reason as the key frames: a
    /// file read again has been read again.
    pub async fn store_pulled_out_subtitles(
        &self,
        source_id: MediaSourceId,
        pulled_out: usize,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO media_source_subtitles (source_id, pulled_out, read_at)
             VALUES (?, ?, ?)
             ON CONFLICT (source_id) DO UPDATE
             SET pulled_out = excluded.pulled_out,
                 read_at = excluded.read_at",
        )
        .bind(source_id.to_db_string())
        .bind(pulled_out as i64)
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Forgets every answer about subtitles pulled out, for every library.
    ///
    /// What was pulled out lives in the cache, which anybody may empty. The
    /// two have to be emptied together: a row left behind says a film is done
    /// when its words are no longer anywhere, and the upkeep would never pull
    /// them out again. Answers how many files went back into the queue.
    pub async fn forget_pulled_out_subtitles(&self) -> Result<u64> {
        let done = sqlx::query("DELETE FROM media_source_subtitles")
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected())
    }

    /// Files carrying subtitles made of words that nobody has pulled out yet,
    /// oldest first, a few at a time.
    ///
    /// Only files that carry such a track inside them. A film with none, and a
    /// film whose only subtitles are pictures or files of their own, has
    /// nothing to pull out and never enters this queue: picture subtitles are
    /// painted into the film at the moment it is watched, and a subtitle in a
    /// file of its own is already the file it would be pulled out into.
    pub async fn sources_without_pulled_out_subtitles(
        &self,
        library_id: LibraryId,
        limit: i64,
    ) -> Result<Vec<MediaSourceId>> {
        let rows = sqlx::query(
            "SELECT media_sources.id
             FROM media_sources
             JOIN library_roots ON library_roots.id = media_sources.root_id
             LEFT JOIN media_source_subtitles
                    ON media_source_subtitles.source_id = media_sources.id
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_source_subtitles.source_id IS NULL
               AND EXISTS (
                   SELECT 1 FROM tracks
                   WHERE tracks.source_id = media_sources.id
                     AND tracks.kind = 'subtitle'
                     AND tracks.subtitle_layout = 'text'
                     AND tracks.is_external = 0
               )
             ORDER BY media_sources.added_at
             LIMIT ?",
        )
        .bind(library_id.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        rows.into_iter()
            .map(|row| parse_id(&row.try_get::<String, _>("id")?))
            .collect()
    }

    /// How many files of one library are still waiting for their words.
    pub async fn count_awaiting_pulled_out_subtitles(&self, library_id: LibraryId) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT count(*)
             FROM media_sources
             JOIN library_roots ON library_roots.id = media_sources.root_id
             LEFT JOIN media_source_subtitles
                    ON media_source_subtitles.source_id = media_sources.id
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_source_subtitles.source_id IS NULL
               AND EXISTS (
                   SELECT 1 FROM tracks
                   WHERE tracks.source_id = media_sources.id
                     AND tracks.kind = 'subtitle'
                     AND tracks.subtitle_layout = 'text'
                     AND tracks.is_external = 0
               )",
        )
        .bind(library_id.to_db_string())
        .fetch_one(self.reader())
        .await?;
        Ok(row.0)
    }

    /// How many files of one library have had their words pulled out already.
    pub async fn count_with_pulled_out_subtitles(&self, library_id: LibraryId) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "SELECT count(*)
             FROM media_source_subtitles
             JOIN media_sources ON media_sources.id = media_source_subtitles.source_id
             JOIN library_roots ON library_roots.id = media_sources.root_id
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL",
        )
        .bind(library_id.to_db_string())
        .fetch_one(self.reader())
        .await?;
        Ok(row.0)
    }
}

/// Writes a list of positions the way the column holds them.
///
/// Ascending milliseconds separated by commas. Plain text rather than packed
/// bytes: a thousand of them is ten kilobytes, they are only ever read whole,
/// and a column somebody can read with their eyes is a column that can be
/// looked at when something goes wrong.
fn write_positions(positions: &[Millis]) -> String {
    positions
        .iter()
        .map(|position| position.get().to_string())
        .collect::<Vec<_>>()
        .join(",")
}

/// Reads that list back, leaving out anything that is not a number.
///
/// A position nobody can read is left out rather than guessed at: a boundary
/// invented here is a segment beginning where there is no picture.
fn read_positions(written: &str) -> Vec<Millis> {
    written
        .split(',')
        .filter_map(|value| value.trim().parse().ok())
        .map(Millis::new)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use melyxar_core::id::{LibraryRootId, TrackId};
    use melyxar_core::library::LibraryKind;
    use melyxar_core::media::{SubtitleDetails, SubtitleLayout, Track, TrackKind};

    use crate::catalogue::tests::{library, video_track, work_with_source};
    use crate::catalogue::SourceAnalysis;

    #[tokio::test]
    async fn where_a_film_can_be_started_is_kept_and_read_back() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        assert_eq!(
            database.key_frames_of(source_id).await.expect("read"),
            None,
            "nothing has read this file for where it can be started"
        );

        let found = vec![Millis::new(0), Millis::new(10_010), Millis::new(20_020)];
        database
            .store_key_frames(source_id, &found)
            .await
            .expect("kept");
        assert_eq!(
            database.key_frames_of(source_id).await.expect("read"),
            Some(found)
        );

        // Read again is read again: two answers about one file is one answer
        // too many.
        let again = vec![Millis::new(0), Millis::new(4_004)];
        database
            .store_key_frames(source_id, &again)
            .await
            .expect("kept");
        assert_eq!(
            database.key_frames_of(source_id).await.expect("read"),
            Some(again)
        );
    }

    /// A film that has been described, which is what both long passes wait
    /// for.
    async fn a_described_film(
        database: &Database,
        library_id: LibraryId,
        root_id: LibraryRootId,
        name: &str,
    ) -> MediaSourceId {
        let (_, source_id) = work_with_source(database, library_id, root_id, name).await;
        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &[],
                &[],
            )
            .await
            .expect("analysis stored");
        source_id
    }

    fn every_ten_seconds() -> Layout {
        Layout {
            every: Millis::new(10_000),
            height: 180,
            columns: 10,
            rows: 10,
        }
    }

    fn made(counted: u32) -> Thumbnails {
        Thumbnails {
            every: Millis::new(10_000),
            width: 320,
            height: 180,
            columns: 10,
            rows: 10,
            counted,
            sheets: counted.div_ceil(100),
        }
    }

    #[tokio::test]
    async fn the_thumbnails_of_a_film_are_kept_and_read_back_whole() {
        let (database, library_id, root_id) = library().await;
        let source_id =
            a_described_film(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        assert_eq!(
            database.thumbnails_of(source_id).await.expect("read"),
            None,
            "nothing has read this film for the bar"
        );

        let first = made(720);
        database
            .store_thumbnails(source_id, &first)
            .await
            .expect("kept");
        assert_eq!(
            database.thumbnails_of(source_id).await.expect("read"),
            Some(first)
        );

        // Read again is read again: two answers about one file is one answer
        // too many.
        let again = made(430);
        database
            .store_thumbnails(source_id, &again)
            .await
            .expect("kept");
        assert_eq!(
            database.thumbnails_of(source_id).await.expect("read"),
            Some(again)
        );
    }

    #[tokio::test]
    async fn a_film_without_thumbnails_is_offered_up_once() {
        let (database, library_id, root_id) = library().await;
        let (_, undescribed) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        assert!(
            database
                .sources_without_thumbnails(library_id, every_ten_seconds(), 10)
                .await
                .expect("read")
                .is_empty(),
            "nothing has described this file yet, so there is nothing to read it for"
        );
        assert_eq!(
            database
                .count_made_thumbnails(library_id, every_ten_seconds())
                .await
                .expect("read"),
            0
        );

        let source_id =
            a_described_film(&database, library_id, root_id, "The.Long.Wait.2004.mkv").await;
        assert_eq!(
            database
                .sources_without_thumbnails(library_id, every_ten_seconds(), 10)
                .await
                .expect("read"),
            vec![source_id],
            "only the one that has been described"
        );
        assert_ne!(source_id, undescribed);

        database
            .store_thumbnails(source_id, &made(720))
            .await
            .expect("kept");
        assert!(
            database
                .sources_without_thumbnails(library_id, every_ten_seconds(), 10)
                .await
                .expect("read")
                .is_empty(),
            "a film read once is not read again"
        );
        assert_eq!(
            database
                .count_made_thumbnails(library_id, every_ten_seconds())
                .await
                .expect("read"),
            1
        );
    }

    #[tokio::test]
    async fn a_cache_emptied_by_hand_puts_the_film_back_in_the_queue() {
        // Being written down is exactly what keeps a film out of the pass that
        // makes them, so a row left behind by a cache emptied by hand is a
        // film whose bar stays bare for ever.
        let (database, library_id, root_id) = library().await;
        let source_id =
            a_described_film(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        database
            .store_thumbnails(source_id, &made(720))
            .await
            .expect("kept");
        assert!(database
            .sources_without_thumbnails(library_id, every_ten_seconds(), 10)
            .await
            .expect("read")
            .is_empty());

        database
            .forget_thumbnails(source_id)
            .await
            .expect("forgotten");
        assert_eq!(database.thumbnails_of(source_id).await.expect("read"), None);
        assert_eq!(
            database
                .sources_without_thumbnails(library_id, every_ten_seconds(), 10)
                .await
                .expect("read"),
            vec![source_id]
        );
    }

    #[tokio::test]
    async fn a_film_that_gave_up_nothing_is_never_read_through_again() {
        // There are files in a film folder that hold no picture. Nought is an
        // answer about the file, and reading a whole file again to be told the
        // same nothing is the most expensive way to learn it.
        let (database, library_id, root_id) = library().await;
        let source_id =
            a_described_film(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        database
            .store_thumbnails(source_id, &made(0))
            .await
            .expect("kept");
        assert!(database
            .sources_without_thumbnails(library_id, every_ten_seconds(), 10)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn films_made_to_another_shape_are_offered_up_again() {
        // Change the interval and every film stops matching, which is exactly
        // when they all have to be made again.
        let (database, library_id, root_id) = library().await;
        let source_id =
            a_described_film(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        database
            .store_thumbnails(source_id, &made(720))
            .await
            .expect("kept");

        let closer = Layout {
            every: Millis::new(5_000),
            ..every_ten_seconds()
        };
        assert_eq!(
            database
                .sources_without_thumbnails(library_id, closer, 10)
                .await
                .expect("read"),
            vec![source_id]
        );
        assert_eq!(
            database
                .count_made_thumbnails(library_id, closer)
                .await
                .expect("read"),
            0,
            "none of them were made to this shape"
        );

        let wider = Layout {
            columns: 5,
            ..every_ten_seconds()
        };
        assert_eq!(
            database
                .sources_without_thumbnails(library_id, wider, 10)
                .await
                .expect("read"),
            vec![source_id]
        );
    }

    #[tokio::test]
    async fn thumbnails_stay_inside_the_library_being_scanned() {
        let (database, films, films_root) = library().await;
        let series = database
            .create_library(
                "Series",
                LibraryKind::Series,
                "fr",
                &[("disk-two".to_string(), PathBuf::from("/mnt/two/Series"))],
            )
            .await
            .expect("library created");
        let in_films =
            a_described_film(&database, films, films_root, "Quiet.Harbour.2019.mkv").await;
        let in_series = a_described_film(
            &database,
            series.id,
            series.roots[0].id,
            "Amber.Field.S01E01.mkv",
        )
        .await;

        assert_eq!(
            database
                .sources_without_thumbnails(films, every_ten_seconds(), 10)
                .await
                .expect("read"),
            vec![in_films]
        );

        database
            .store_thumbnails(in_series, &made(720))
            .await
            .expect("kept");
        assert_eq!(
            database
                .count_made_thumbnails(films, every_ten_seconds())
                .await
                .expect("read"),
            0,
            "a film of another library counts for nothing here"
        );
    }

    #[tokio::test]
    async fn a_file_that_changed_underneath_loses_what_was_read_out_of_it() {
        // Both were read out of the copy that is gone. Left behind, they would
        // cut a film at positions belonging to another one and show a viewer
        // pictures of it.
        let (database, library_id, root_id) = library().await;
        let source_id =
            a_described_film(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        database
            .store_key_frames(source_id, &[Millis::ZERO, Millis::new(4_004)])
            .await
            .expect("kept");
        database
            .store_thumbnails(source_id, &made(720))
            .await
            .expect("kept");

        database
            .refresh_source_identity(source_id, 12_345, now())
            .await
            .expect("the file changed on disk");

        assert_eq!(database.key_frames_of(source_id).await.expect("read"), None);
        assert_eq!(database.thumbnails_of(source_id).await.expect("read"), None);
    }

    #[tokio::test]
    async fn a_film_with_nowhere_to_start_is_written_down_as_having_nowhere() {
        // A reading that finished and gave nothing is an answer about the
        // file. Written down, the film is never read through again for the
        // same nothing; left out, it costs a whole reading at every scan.
        let (database, library_id, root_id) = library().await;
        let source_id =
            a_described_film(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        database
            .store_key_frames(source_id, &[])
            .await
            .expect("kept");
        assert_eq!(
            database.key_frames_of(source_id).await.expect("read"),
            Some(Vec::new()),
            "read back as nowhere rather than as never read"
        );
        assert!(
            database
                .sources_without_key_frames(library_id, 10)
                .await
                .expect("read")
                .is_empty(),
            "and never offered up again"
        );
    }

    #[tokio::test]
    async fn a_film_nobody_has_read_for_its_key_frames_is_offered_up_once() {
        // Reading one means reading the whole file through, so this is a
        // background pass that picks up where it left off rather than one that
        // runs to the end in a single sitting.
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        assert!(
            database
                .sources_without_key_frames(library_id, 10)
                .await
                .expect("read")
                .is_empty(),
            "nothing has described this file yet, so there is nothing to read it for"
        );

        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &[],
                &[],
            )
            .await
            .expect("analysis stored");
        assert_eq!(
            database
                .sources_without_key_frames(library_id, 10)
                .await
                .expect("read"),
            vec![source_id]
        );

        database
            .store_key_frames(source_id, &[Millis::ZERO])
            .await
            .expect("kept");
        assert!(
            database
                .sources_without_key_frames(library_id, 10)
                .await
                .expect("read")
                .is_empty(),
            "read once is read"
        );

        // A file off the disk is not a file to read.
        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &[],
                &[],
            )
            .await
            .expect("analysis stored");
        database
            .mark_source_missing(source_id)
            .await
            .expect("marked");
        assert!(database
            .sources_without_key_frames(library_id, 10)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn reading_for_key_frames_stays_inside_the_library_being_scanned() {
        // Scanning the films must not set the tool reading through the series,
        // and the count a scan shows its progress against must not be the
        // whole server's.
        let (database, films, films_root) = library().await;
        let series = database
            .create_library(
                "Series",
                LibraryKind::Series,
                "fr",
                &[("disk-two".to_string(), PathBuf::from("/mnt/two/Series"))],
            )
            .await
            .expect("library created");
        let series_root = series.roots[0].id;

        let analysis = SourceAnalysis {
            container: Some("matroska,webm".to_string()),
            duration: Some(Millis::new(7_200_000)),
            overall_bitrate: None,
        };
        let (_, in_films) =
            work_with_source(&database, films, films_root, "Quiet.Harbour.2019.mkv").await;
        let (_, in_series) =
            work_with_source(&database, series.id, series_root, "Amber.Field.S01E01.mkv").await;
        for source in [in_films, in_series] {
            database
                .store_analysis(source, &analysis, &[], &[])
                .await
                .expect("analysis stored");
        }

        assert_eq!(
            database
                .sources_without_key_frames(films, 10)
                .await
                .expect("read"),
            vec![in_films],
            "a scan of the films reads the films and nothing else"
        );
        assert_eq!(
            database
                .count_read_for_key_frames(films)
                .await
                .expect("read"),
            0
        );

        database
            .store_key_frames(in_films, &[Millis::ZERO])
            .await
            .expect("kept");
        database
            .store_key_frames(in_series, &[Millis::ZERO])
            .await
            .expect("kept");

        assert_eq!(
            database
                .count_read_for_key_frames(films)
                .await
                .expect("read"),
            1,
            "one film read, and the episode belongs to another scan's count"
        );
        assert_eq!(
            database
                .count_read_for_key_frames(series.id)
                .await
                .expect("read"),
            1
        );
    }

    #[tokio::test]
    async fn a_photo_is_never_read_for_what_only_moving_pictures_have() {
        let (database, library_id, root_id) = library().await;
        let clip = a_described_film(&database, library_id, root_id, "clip.mp4").await;
        let photo = a_described_film(&database, library_id, root_id, "beach.jpg").await;
        sqlx::query(
            "UPDATE works SET kind = 'photo'
              WHERE id = (SELECT work_id FROM media_sources WHERE id = ?)",
        )
        .bind(photo.to_db_string())
        .execute(database.writer())
        .await
        .expect("made a photo");

        assert_eq!(
            database
                .sources_without_key_frames(library_id, 10)
                .await
                .expect("read"),
            vec![clip]
        );
        assert_eq!(
            database
                .count_awaiting_key_frames(library_id)
                .await
                .expect("read"),
            1
        );
        assert_eq!(
            database
                .sources_without_thumbnails(library_id, every_ten_seconds(), 10)
                .await
                .expect("read"),
            vec![clip]
        );
        assert_eq!(
            database
                .count_awaiting_thumbnails(library_id, every_ten_seconds())
                .await
                .expect("read"),
            1,
            "a photo has no bar to put little pictures on"
        );
    }

    #[tokio::test]
    async fn what_is_left_to_read_is_counted_rather_than_guessed_from_one_batch() {
        // The readings are done a few files at a time, so a bar sized on one
        // batch reaches its end once per batch and says nothing true about the
        // work. This is also the number the upkeep screen shows when nothing
        // is running, which is when somebody wants to know whether there is
        // anything left to do at all.
        let (database, library_id, root_id) = library().await;
        let first =
            a_described_film(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        let second = a_described_film(&database, library_id, root_id, "Amber.Field.2021.mkv").await;

        assert_eq!(
            database
                .count_awaiting_key_frames(library_id)
                .await
                .expect("read"),
            2
        );
        assert_eq!(
            database
                .count_awaiting_thumbnails(library_id, every_ten_seconds())
                .await
                .expect("read"),
            2
        );

        database
            .store_key_frames(first, &[Millis::ZERO])
            .await
            .expect("kept");
        database
            .store_thumbnails(second, &made(720))
            .await
            .expect("kept");

        assert_eq!(
            database
                .count_awaiting_key_frames(library_id)
                .await
                .expect("read"),
            1
        );
        assert_eq!(
            database
                .count_awaiting_thumbnails(library_id, every_ten_seconds())
                .await
                .expect("read"),
            1
        );

        // A file off the disk is not a file to read, here as everywhere else.
        database.mark_source_missing(second).await.expect("marked");
        assert_eq!(
            database
                .count_awaiting_key_frames(library_id)
                .await
                .expect("read"),
            0
        );

        // Move the shape and every film is waiting again, which is exactly
        // when the number has to say so.
        let closer = Layout {
            every: Millis::new(5_000),
            ..every_ten_seconds()
        };
        assert_eq!(
            database
                .count_awaiting_thumbnails(library_id, closer)
                .await
                .expect("read"),
            1,
            "the one still on the disk, whatever was made for it in another shape"
        );
    }

    /// A subtitle track of whatever shape, for the queue that only wants one
    /// of them.
    fn subtitle_of(
        source_id: MediaSourceId,
        stream_index: i32,
        codec: &str,
        layout: SubtitleLayout,
        is_external: bool,
    ) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index,
            language: Some("fre".to_string()),
            title: None,
            is_default: false,
            is_forced: false,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: codec.to_string(),
                layout,
                is_hearing_impaired: false,
                is_external,
                external_relative_path: is_external.then(|| PathBuf::from("Quiet.Harbour.fr.srt")),
            }),
        }
    }

    /// Records a film carrying exactly the subtitle tracks given.
    async fn a_film_with_subtitles(
        database: &Database,
        library_id: LibraryId,
        root_id: LibraryRootId,
        name: &str,
        tracks: impl Fn(MediaSourceId) -> Vec<Track>,
    ) -> MediaSourceId {
        let (_, source_id) = work_with_source(database, library_id, root_id, name).await;
        let carried = tracks(source_id);
        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &carried,
                &[],
            )
            .await
            .expect("analysis stored");
        source_id
    }

    #[tokio::test]
    async fn what_is_done_and_what_is_waiting_are_counted_over_the_same_films() {
        // The two are added together to make the total of a progress bar, so
        // they have to describe the same set. A film read once and since taken
        // off the disk was counted as done and not as waiting, so the total
        // named a set neither number described, and the report subtracted one
        // from the other and could go below nought.
        let (database, library_id, root_id) = library().await;
        let here = a_described_film(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        let gone =
            a_described_film(&database, library_id, root_id, "Distant.Signal.2021.mkv").await;
        for film in [here, gone] {
            database
                .store_key_frames(film, &[Millis::ZERO])
                .await
                .expect("kept");
        }
        assert_eq!(
            database
                .count_read_for_key_frames(library_id)
                .await
                .expect("read"),
            2
        );

        database
            .mark_source_missing(gone)
            .await
            .expect("marked absent");
        assert_eq!(
            database
                .count_read_for_key_frames(library_id)
                .await
                .expect("read"),
            1,
            "a film off the disk is not waiting, so it is not done either"
        );
        assert_eq!(
            database
                .count_awaiting_key_frames(library_id)
                .await
                .expect("read"),
            0
        );

        // And it comes back on both sides the day the disk does.
        database
            .mark_source_present(gone)
            .await
            .expect("marked present");
        assert_eq!(
            database
                .count_read_for_key_frames(library_id)
                .await
                .expect("read"),
            2
        );
    }

    #[tokio::test]
    async fn only_a_film_carrying_words_of_its_own_waits_for_them_to_be_pulled_out() {
        // Pulling the words out of a film means reading the whole file
        // through, so the queue must hold exactly the files that would give
        // something for it. Pictures are painted into the film at the moment
        // it is watched and are never pulled out; a subtitle in a file of its
        // own is already the file it would be pulled out into; and a film with
        // no subtitle at all has nothing to read for.
        let (database, library_id, root_id) = library().await;

        let carries_words = a_film_with_subtitles(
            &database,
            library_id,
            root_id,
            "Quiet.Harbour.2019.mkv",
            |source_id| {
                vec![
                    video_track(source_id),
                    subtitle_of(source_id, 2, "subrip", SubtitleLayout::Text, false),
                ]
            },
        )
        .await;
        a_film_with_subtitles(
            &database,
            library_id,
            root_id,
            "Distant.Signal.2021.mkv",
            |source_id| {
                vec![
                    video_track(source_id),
                    subtitle_of(
                        source_id,
                        2,
                        "hdmv_pgs_subtitle",
                        SubtitleLayout::Bitmap,
                        false,
                    ),
                ]
            },
        )
        .await;
        a_film_with_subtitles(
            &database,
            library_id,
            root_id,
            "Paper.Lanterns.2018.mkv",
            |source_id| {
                vec![
                    video_track(source_id),
                    subtitle_of(source_id, 2, "subrip", SubtitleLayout::Text, true),
                ]
            },
        )
        .await;
        a_described_film(&database, library_id, root_id, "Broken.Compass.2020.mkv").await;

        assert_eq!(
            database
                .sources_without_pulled_out_subtitles(library_id, 10)
                .await
                .expect("read"),
            vec![carries_words],
            "one of the four, and it is the one carrying words inside it"
        );
        assert_eq!(
            database
                .count_awaiting_pulled_out_subtitles(library_id)
                .await
                .expect("read"),
            1
        );
        assert_eq!(
            database
                .count_with_pulled_out_subtitles(library_id)
                .await
                .expect("read"),
            0
        );
    }

    #[tokio::test]
    async fn a_film_whose_words_are_pulled_out_is_never_offered_up_again() {
        // Nought is an answer here as it is everywhere else: a film whose
        // every track was already in the cache gave nothing to this reading
        // and is still done with. Left out, it would cost a whole reading at
        // every run of the upkeep, for ever.
        let (database, library_id, root_id) = library().await;
        let source_id = a_film_with_subtitles(
            &database,
            library_id,
            root_id,
            "Quiet.Harbour.2019.mkv",
            |source_id| {
                vec![
                    video_track(source_id),
                    subtitle_of(source_id, 2, "subrip", SubtitleLayout::Text, false),
                ]
            },
        )
        .await;

        database
            .store_pulled_out_subtitles(source_id, 0)
            .await
            .expect("kept");
        assert!(
            database
                .sources_without_pulled_out_subtitles(library_id, 10)
                .await
                .expect("read")
                .is_empty(),
            "read once is read, whatever the reading gave"
        );
        assert_eq!(
            database
                .count_awaiting_pulled_out_subtitles(library_id)
                .await
                .expect("read"),
            0
        );
        assert_eq!(
            database
                .count_with_pulled_out_subtitles(library_id)
                .await
                .expect("read"),
            1
        );

        // Read again is read again: one answer per file, never two.
        database
            .store_pulled_out_subtitles(source_id, 3)
            .await
            .expect("kept");
        assert_eq!(
            database
                .count_with_pulled_out_subtitles(library_id)
                .await
                .expect("read"),
            1
        );

        // Emptying the cache puts the film back in the queue. A row left
        // behind would say it is done when its words are nowhere any more.
        assert_eq!(
            database
                .forget_pulled_out_subtitles()
                .await
                .expect("forgotten"),
            1
        );
        assert_eq!(
            database
                .sources_without_pulled_out_subtitles(library_id, 10)
                .await
                .expect("read"),
            vec![source_id]
        );
    }

    #[tokio::test]
    async fn a_film_off_the_disk_or_never_described_is_not_a_film_to_pull_words_out_of() {
        let (database, library_id, root_id) = library().await;
        let (_, never_described) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        assert!(
            database
                .sources_without_pulled_out_subtitles(library_id, 10)
                .await
                .expect("read")
                .is_empty(),
            "nothing has described this file yet, so nothing knows it carries words"
        );

        let gone = a_film_with_subtitles(
            &database,
            library_id,
            root_id,
            "Distant.Signal.2021.mkv",
            |source_id| {
                vec![
                    video_track(source_id),
                    subtitle_of(source_id, 2, "subrip", SubtitleLayout::Text, false),
                ]
            },
        )
        .await;
        database
            .mark_source_missing(gone)
            .await
            .expect("marked absent");
        assert!(
            database
                .sources_without_pulled_out_subtitles(library_id, 10)
                .await
                .expect("read")
                .is_empty(),
            "a file off the disk cannot be read"
        );
        assert_eq!(
            database
                .count_awaiting_pulled_out_subtitles(library_id)
                .await
                .expect("read"),
            0
        );

        // And it is offered again the day the disk comes back.
        database
            .mark_source_present(gone)
            .await
            .expect("marked present");
        assert_eq!(
            database
                .sources_without_pulled_out_subtitles(library_id, 10)
                .await
                .expect("read"),
            vec![gone]
        );
        assert_ne!(gone, never_described);
    }

    #[tokio::test]
    async fn one_library_s_words_are_not_counted_against_another_s() {
        let (database, films, films_root) = library().await;
        let series = database
            .create_library(
                "Series",
                LibraryKind::Series,
                "fr",
                &[("disk-two".to_string(), PathBuf::from("/mnt/two/Series"))],
            )
            .await
            .expect("library created");

        let carries = |source_id: MediaSourceId| {
            vec![
                video_track(source_id),
                subtitle_of(source_id, 2, "subrip", SubtitleLayout::Text, false),
            ]
        };
        let in_films = a_film_with_subtitles(
            &database,
            films,
            films_root,
            "Quiet.Harbour.2019.mkv",
            carries,
        )
        .await;
        a_film_with_subtitles(
            &database,
            series.id,
            series.roots[0].id,
            "Distant.Signal.S01E01.mkv",
            carries,
        )
        .await;

        assert_eq!(
            database
                .sources_without_pulled_out_subtitles(films, 10)
                .await
                .expect("read"),
            vec![in_films]
        );
        database
            .store_pulled_out_subtitles(in_films, 2)
            .await
            .expect("kept");
        assert_eq!(
            database
                .count_with_pulled_out_subtitles(films)
                .await
                .expect("read"),
            1
        );
        assert_eq!(
            database
                .count_with_pulled_out_subtitles(series.id)
                .await
                .expect("read"),
            0
        );
        assert_eq!(
            database
                .count_awaiting_pulled_out_subtitles(series.id)
                .await
                .expect("read"),
            1
        );
    }

    #[test]
    fn a_position_nobody_can_read_is_left_out_rather_than_guessed_at() {
        // A boundary invented here is a segment beginning where there is no
        // picture, which is worse than a boundary missing.
        assert_eq!(
            read_positions("0,4004,,rubbish,8008"),
            vec![Millis::new(0), Millis::new(4004), Millis::new(8008)]
        );
        assert_eq!(read_positions(""), Vec::<Millis>::new());
        assert_eq!(
            write_positions(&[Millis::new(0), Millis::new(4004)]),
            "0,4004"
        );
        assert_eq!(write_positions(&[]), "");
    }
}
