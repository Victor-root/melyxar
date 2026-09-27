//! Works and the files behind them.
//!
//! Two rules shape everything here. A work outlives the file that revealed it,
//! so replacing a copy with a better one keeps the watch history. And a file
//! that is no longer on disk is marked absent rather than deleted, so that an
//! unplugged disk is a bad evening rather than a lost library.

use std::path::{Path, PathBuf};

use melyxar_core::id::{ExtraVideoId, LibraryId, LibraryRootId, MediaSourceId, UserId, WorkId};
use melyxar_core::time::{now, Millis, Timestamp};
use melyxar_core::work::{IdentificationNote, IdentificationState, Work, WorkKind};
use sqlx::{AssertSqlSafe, Row, Sqlite};

use crate::browse::{card_from_row, WorkCard, WHAT_A_CARD_IS};
use crate::convert::{parse_id, parse_optional_timestamp, parse_timestamp, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

/// Everything a work is, as every reader of one asks for it.
///
/// Written once because five queries hand their rows to the same reader, and a
/// column added to the reader and forgotten in one of them is a field that
/// comes back wrong depending on which question was asked.
///
/// Takes the table it is written about: one of the five joins, and a bare
/// column name is ambiguous the moment a statement carries two tables.
pub(crate) fn what_a_work_is(table: &str) -> String {
    format!(
        "{table}id, {table}library_id, {table}parent_id, {table}ordinal, {table}kind,
         {table}title, {table}sort_title, {table}release_year, {table}runtime_ms,
         {table}community_rating, {table}age_rating_label, {table}identification,
         {table}identification_note, {table}dominant_color, {table}added_at, {table}updated_at"
    )
}

/// How many works can stand between an episode and the top: its season, and
/// the series above that.
const HOW_DEEP_IT_GOES: usize = 2;

/// The one order children are ever read in.
///
/// Written once because two reads answer "what hangs under this", one for a
/// page and one for the look up, and two orders would put an episode in one
/// place on screen and fill in another.
///
/// In a folder of one's own, where nothing has a rank, the folders come
/// before the files, the way every file manager shows them.
const IN_THE_ONE_ORDER: &str = "ORDER BY ordinal, kind <> 'folder', sort_title";

/// A file as `stored_source_from_row` reads it, the disk it lives on included.
const A_STORED_SOURCE: &str = "SELECT s.id, s.work_id, s.relative_path, s.size_bytes, s.modified_at,
        s.missing_since, s.added_at, r.label AS root_label, r.path AS root_path
   FROM media_sources s
   JOIN library_roots r ON r.id = s.root_id";

/// One work hanging under another, stripped to where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedChild {
    pub id: WorkId,
    pub ordinal: Option<i32>,
    pub title: String,
}

/// One work hanging under another: its card, and what its place under the
/// other adds to it.
#[derive(Debug, Clone, PartialEq)]
pub struct ChildWork {
    /// The season number, the episode number.
    pub ordinal: Option<i32>,
    /// How many hang under this one, for a season saying how many episodes
    /// and a folder how many things.
    pub child_count: i64,
    /// What it is about in the language asked for, and failing that in
    /// English, for a list of episodes that has room to say it.
    pub overview: Option<String>,
    /// It as every row draws it, with what this viewer made of it. Its length
    /// is what the provider said, and failing that the longest copy on disk,
    /// so an episode nobody has looked up still says something.
    pub card: WorkCard,
}

/// Works one provider says are the same film.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedIdentity {
    /// The one that has been here longest, which the others join.
    pub keep: WorkId,
    pub others: Vec<WorkId>,
}

/// A file as it is recorded, reduced to what a scan compares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredSource {
    pub id: MediaSourceId,
    pub work_id: WorkId,
    pub relative_path: PathBuf,
    /// The disk this file lives on, by the name the configuration gives it.
    pub root_label: String,
    /// Where that disk is mounted, so the whole path can be shown to whoever
    /// runs the server and has to find the file.
    pub root_path: PathBuf,
    pub size_bytes: i64,
    pub modified_at: Timestamp,
    /// When the scan first saw it, which is not when the file was made.
    pub added_at: Timestamp,
    /// Set while the file is not on disk. A file that comes back keeps its
    /// identifier, and with it everything attached to it.
    pub missing_since: Option<Timestamp>,
}

/// One of the files that weigh the most, with what it is and what it holds.
///
/// Everything the whole file says about itself, in one row. What each of its
/// tracks says is a question of its own, asked of the tracks themselves.
#[derive(Debug, Clone, PartialEq)]
pub struct HeaviestSource {
    pub id: MediaSourceId,
    /// The title of the film this file is a copy of.
    pub title: String,
    pub release_year: Option<i32>,
    /// The name of the file, without the folders leading to it.
    pub file_name: String,
    pub size_bytes: i64,
    pub container: Option<String>,
    pub duration: Option<Millis>,
    /// Everything in the file per second, streams and container alike.
    pub overall_bitrate: Option<i64>,
}

/// What an analysis found about the file as a whole.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SourceAnalysis {
    pub container: Option<String>,
    pub duration: Option<Millis>,
    pub overall_bitrate: Option<i64>,
}

/// What a library holds, counted in one pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CatalogueSummary {
    pub works: i64,
    pub files: i64,
    pub missing_files: i64,
    pub identified: i64,
    pub awaiting_identification: i64,
    /// Files read for where their picture can be started.
    ///
    /// Reading one means reading the whole file through, so this climbs over
    /// several scans and is the only way to tell a pass that is still going
    /// from one that finished.
    pub read_for_key_frames: i64,
    /// Files that have the thumbnails of the playback bar, whatever shape they
    /// were made to. Another whole reading of every file, counted for the same
    /// reason.
    pub with_thumbnails: i64,
}

/// A video that belongs to a work without being the work itself.
///
/// Recorded relative to a root, which is what lets a disk be mounted
/// somewhere else without rewriting a database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalExtraVideo {
    pub kind: String,
    pub name: Option<String>,
    pub root_id: LibraryRootId,
    pub relative_path: PathBuf,
}

/// The same video, read back with its root joined on so it can be opened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlayableExtraVideo {
    pub kind: String,
    pub name: Option<String>,
    /// Where the file is, root included. Never sent to a client: a viewer is
    /// given an address, not a path on someone's disk.
    pub path: PathBuf,
}

impl Database {
    /// Creates a work. Nothing is looked up yet, which is what `pending` says.
    ///
    /// The name it is given is also written down as the name it was filed
    /// under, because that is what it is: what the disk said. A provider will
    /// rename it later, and the scan has to go on finding it under the name
    /// the folder still carries.
    pub async fn create_work(
        &self,
        library_id: LibraryId,
        kind: WorkKind,
        title: &str,
        sort_title: &str,
        release_year: Option<i32>,
    ) -> Result<Work> {
        let mut transaction = self.begin().await?;
        let work = insert_work(
            &mut *transaction,
            Placed::on_its_own(library_id),
            kind,
            title,
            sort_title,
            release_year,
        )
        .await?;
        remember_filing_name(&mut *transaction, work.id, sort_title, release_year).await?;
        transaction.commit().await?;
        Ok(work)
    }

    /// Writes a work that hangs under another: a season under its series, an
    /// episode under its season.
    ///
    /// The parent's count of children is brought up to date in the same
    /// transaction, by counting them rather than by adding one: a count that
    /// is recomputed cannot drift, and a scan that is interrupted halfway
    /// leaves a number that is still true of what is there.
    pub async fn create_child_work(
        &self,
        library_id: LibraryId,
        parent_id: WorkId,
        ordinal: i32,
        kind: WorkKind,
        title: &str,
        sort_title: &str,
    ) -> Result<Work> {
        self.create_child(
            library_id, parent_id, ordinal, kind, title, sort_title, None,
        )
        .await
    }

    /// Writes an episode whose file numbered it across the whole series
    /// rather than inside a season, and keeps that number with it.
    ///
    /// Kept so the episode can be put back in the right season whenever the
    /// series is described again, however the provider cuts it up that time.
    pub async fn create_episode_numbered_across(
        &self,
        library_id: LibraryId,
        season_id: WorkId,
        ordinal: i32,
        absolute_number: i32,
        title: &str,
        sort_title: &str,
    ) -> Result<Work> {
        self.create_child(
            library_id,
            season_id,
            ordinal,
            WorkKind::Episode,
            title,
            sort_title,
            Some(absolute_number),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn create_child(
        &self,
        library_id: LibraryId,
        parent_id: WorkId,
        ordinal: i32,
        kind: WorkKind,
        title: &str,
        sort_title: &str,
        absolute_number: Option<i32>,
    ) -> Result<Work> {
        let mut transaction = self.begin().await?;
        let work = insert_work(
            &mut *transaction,
            Placed::under(library_id, parent_id, ordinal),
            kind,
            title,
            sort_title,
            None,
        )
        .await?;
        if absolute_number.is_some() {
            sqlx::query("UPDATE works SET absolute_number = ? WHERE id = ?")
                .bind(absolute_number)
                .bind(work.id.to_db_string())
                .execute(&mut *transaction)
                .await?;
        }
        recount_children(&mut *transaction, parent_id).await?;
        transaction.commit().await?;
        Ok(work)
    }

    /// Takes one copy away from the film it sits on and makes it a film of its
    /// own, named after its file and waiting to be looked up.
    ///
    /// The other half of putting copies together. Whatever joins them does so
    /// on its own, and anything that acts on its own has to be undoable by
    /// hand, or a grouping that got it wrong costs a film nobody can get back.
    ///
    /// Refused when the film holds this one copy and no other: there would be
    /// nothing to take it away from, and the film would be left with no file
    /// at all.
    pub async fn detach_source(
        &self,
        source_id: MediaSourceId,
        kind: WorkKind,
        title: &str,
        sort_title: &str,
        release_year: Option<i32>,
    ) -> Result<Option<Work>> {
        let Some(row) = sqlx::query(
            "SELECT w.id AS work_id, w.library_id,
                    (SELECT count(*) FROM media_sources o WHERE o.work_id = w.id) AS copies
             FROM media_sources s
             JOIN works w ON w.id = s.work_id
             WHERE s.id = ?",
        )
        .bind(source_id.to_db_string())
        .fetch_optional(self.reader())
        .await?
        else {
            return Ok(None);
        };
        if row.try_get::<i64, _>("copies")? < 2 {
            return Ok(None);
        }
        let library_id: LibraryId = row
            .try_get::<String, _>("library_id")?
            .parse()
            .map_err(|_| DatabaseError::Corrupt("library identifier".to_string()))?;

        let mut transaction = self.begin().await?;
        let work = insert_work(
            &mut *transaction,
            Placed::on_its_own(library_id),
            kind,
            title,
            sort_title,
            release_year,
        )
        .await?;
        remember_filing_name(&mut *transaction, work.id, sort_title, release_year).await?;
        sqlx::query("UPDATE works SET set_apart_by_hand = 1 WHERE id = ?")
            .bind(work.id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE media_sources SET work_id = ? WHERE id = ?")
            .bind(work.id.to_db_string())
            .bind(source_id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(Some(work))
    }

    /// One work by identifier.
    pub async fn work(&self, id: WorkId) -> Result<Option<Work>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM works WHERE id = ?",
            what_a_work_is("")
        )))
        .bind(id.to_db_string())
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| work_from_row(&row)).transpose()
    }

    /// The work a scan should attach a file to, if it already exists.
    ///
    /// Two copies of one film are two files of the same work, not two works:
    /// that is what puts a version chooser on the page instead of the same
    /// title twice in the grid.
    pub async fn work_by_identity(
        &self,
        library_id: LibraryId,
        sort_title: &str,
        release_year: Option<i32>,
    ) -> Result<Option<Work>> {
        self.work_named(library_id, sort_title, release_year, None)
            .await
    }

    /// The series of this library going by that name, if one is written down.
    ///
    /// Asks the same question as the one above and narrows it to a series on
    /// purpose. A library of series also holds episodes nobody could number,
    /// and one of those going by the name of a series would otherwise be
    /// handed back as the series itself, with seasons hung under a file.
    pub async fn series_by_name(
        &self,
        library_id: LibraryId,
        sort_title: &str,
        release_year: Option<i32>,
    ) -> Result<Option<Work>> {
        self.work_named(library_id, sort_title, release_year, Some(WorkKind::Series))
            .await
    }

    /// Everything hanging under one work, in the order it is numbered.
    ///
    /// A series answers with its seasons, a season with its episodes, each as
    /// the card every row draws: the same pictures, the same marks of this
    /// viewer, and for an episode the same wide picture a row lying its cards
    /// down shows. Read a whole page at a time, like any row of cards: a page
    /// of twenty four episodes that asked per episode would be a hundred round
    /// trips.
    pub async fn children_of(
        &self,
        viewer: UserId,
        parent_id: WorkId,
        language: &str,
    ) -> Result<Vec<ChildWork>> {
        // Ordered the one way children are ever ordered, named below so the
        // shelf read and this one cannot come back in different orders.
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_CARD_IS}, w.ordinal, w.child_count,
                    (SELECT max(s.duration_ms) FROM media_sources s WHERE s.work_id = w.id)
                        AS longest_ms,
                    coalesce(asked.overview, english.overview) AS overview
             FROM works w
             LEFT JOIN work_translations asked
                    ON asked.work_id = w.id AND asked.language = ?2
             LEFT JOIN work_translations english
                    ON english.work_id = w.id AND english.language = 'en'
             WHERE w.parent_id = ?1
             {IN_THE_ONE_ORDER}"
        )))
        .bind(parent_id.to_db_string())
        .bind(language)
        .fetch_all(self.reader())
        .await?;

        let mut cards = Vec::with_capacity(rows.len());
        let mut places = Vec::with_capacity(rows.len());
        for row in &rows {
            let mut card = card_from_row(row)?;
            card.runtime = card
                .runtime
                .or(row.try_get::<Option<i64>, _>("longest_ms")?.map(Millis::new));
            cards.push(card);
            places.push((
                row.try_get("ordinal")?,
                row.try_get("child_count")?,
                row.try_get("overview")?,
            ));
        }
        self.attach_posters(&mut cards).await?;
        self.attach_wide_pictures(&mut cards).await?;
        self.attach_viewer_state(viewer, &mut cards).await?;

        Ok(places
            .into_iter()
            .zip(cards)
            .map(|((ordinal, child_count, overview), card)| ChildWork {
                ordinal,
                child_count,
                overview,
                card,
            })
            .collect())
    }

    /// What hangs under one work, as a shelf rather than as a page.
    ///
    /// The same children in the same order, with nobody's progress in them.
    /// Asked by the look up, which fills a season in and has no viewer to
    /// answer for: handing it one would be inventing a person.
    pub async fn children_ranked(&self, parent_id: WorkId) -> Result<Vec<RankedChild>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT id, ordinal, title FROM works WHERE parent_id = ? {IN_THE_ONE_ORDER}"
        )))
        .bind(parent_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                Ok(RankedChild {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    ordinal: row.try_get("ordinal")?,
                    title: row.try_get("title")?,
                })
            })
            .collect()
    }

    /// The next episode a viewer would watch after this one.
    ///
    /// The one after it in its own season, and failing that the first of the
    /// season after: an episode is numbered inside its season, so "next" only
    /// means anything read across the whole series in order.
    pub async fn next_episode_after(
        &self,
        viewer: UserId,
        episode_id: WorkId,
    ) -> Result<Option<Work>> {
        let Some((series_id, at)) = self.where_an_episode_sits(episode_id).await? else {
            return Ok(None);
        };
        self.an_episode_of(viewer, series_id, Some(at), false).await
    }

    /// The episode before this one, in the order they are watched.
    ///
    /// The one before it in its own season, and failing that the last of the
    /// season before: the same order `next_episode_after` reads, walked the
    /// other way. Nobody's progress is asked about, because stepping back
    /// means the one before this one, watched or not, same as stepping on.
    pub async fn previous_episode_before(&self, episode_id: WorkId) -> Result<Option<Work>> {
        let Some((series_id, (season, episode))) = self.where_an_episode_sits(episode_id).await?
        else {
            return Ok(None);
        };
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM works e
             JOIN works s ON s.id = e.parent_id
             WHERE s.parent_id = ? AND e.kind = 'episode'
               AND EXISTS (SELECT 1 FROM media_sources m
                            WHERE m.work_id = e.id AND m.missing_since IS NULL)
               AND (s.ordinal, e.ordinal) < (?, ?)
             ORDER BY s.ordinal DESC, e.ordinal DESC
             LIMIT 1",
            what_a_work_is("e.")
        )))
        .bind(series_id.to_db_string())
        .bind(season)
        .bind(episode)
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| work_from_row(&row)).transpose()
    }

    /// Where a viewer would pick a series back up, or one season of it.
    ///
    /// An episode left halfway first, the one played last when there are
    /// several: somebody who stopped in the middle of one is in the middle of
    /// that one, whatever else they skipped. Failing that, the first episode
    /// not watched yet, read in order rather than from the last one played,
    /// because a series watched out of order has a hole in it and the hole is
    /// what somebody means by where they are. Nothing when every episode here
    /// has been watched, which is a series to start again rather than to
    /// carry on.
    pub async fn where_to_resume(&self, viewer: UserId, within: WorkId) -> Result<Option<Work>> {
        if let Some(halfway) = self.an_episode_left_halfway(viewer, within).await? {
            return Ok(Some(halfway));
        }
        self.an_episode_of(viewer, within, None, true).await
    }

    /// The episode of a series, or of one season of it, a viewer played last
    /// and stopped in the middle of, when there is one with a file still
    /// behind it.
    async fn an_episode_left_halfway(
        &self,
        viewer: UserId,
        within: WorkId,
    ) -> Result<Option<Work>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM works e
             JOIN works s ON s.id = e.parent_id
             JOIN playback_progress p ON p.work_id = e.id AND p.user_id = ?
             JOIN libraries l ON l.id = e.library_id AND l.keeps_resume_points
             WHERE ?2 IN (s.parent_id, s.id) AND e.kind = 'episode' AND p.position_ms > 0
               AND EXISTS (SELECT 1 FROM media_sources m
                            WHERE m.work_id = e.id AND m.missing_since IS NULL)
             ORDER BY p.last_played_at DESC
             LIMIT 1",
            what_a_work_is("e.")
        )))
        .bind(viewer.to_db_string())
        .bind(within.to_db_string())
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| work_from_row(&row)).transpose()
    }

    /// The series an episode belongs to, and where it sits in it.
    async fn where_an_episode_sits(
        &self,
        episode_id: WorkId,
    ) -> Result<Option<(WorkId, (i32, i32))>> {
        let row = sqlx::query(
            "SELECT s.parent_id AS series_id, s.ordinal AS season, e.ordinal AS episode
             FROM works e
             JOIN works s ON s.id = e.parent_id
             WHERE e.id = ? AND e.kind = 'episode'",
        )
        .bind(episode_id.to_db_string())
        .fetch_optional(self.reader())
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let (Some(series), Some(season), Some(episode)) = (
            row.try_get::<Option<String>, _>("series_id")?,
            row.try_get::<Option<i32>, _>("season")?,
            row.try_get::<Option<i32>, _>("episode")?,
        ) else {
            return Ok(None);
        };
        Ok(Some((parse_id(&series)?, (season, episode))))
    }

    /// One episode of a series, or of one season of it when that is what is
    /// named, read in the order they are watched in.
    ///
    /// Written once because the two questions above are the same question
    /// asked with different bounds: the next one after a place, and the first
    /// one nobody has watched. Both skip an episode with no file behind it,
    /// because both end in a button that has to play something.
    async fn an_episode_of(
        &self,
        viewer: UserId,
        within: WorkId,
        after: Option<(i32, i32)>,
        only_unwatched: bool,
    ) -> Result<Option<Work>> {
        let (season, episode) = after.unwrap_or((0, 0));
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM works e
             JOIN works s ON s.id = e.parent_id
             JOIN libraries l ON l.id = e.library_id
             LEFT JOIN playback_progress p ON p.work_id = e.id AND p.user_id = ?
             WHERE ?2 IN (s.parent_id, s.id) AND e.kind = 'episode'
               AND EXISTS (SELECT 1 FROM media_sources m
                            WHERE m.work_id = e.id AND m.missing_since IS NULL)
               AND (?3 = 0 OR (s.ordinal, e.ordinal) > (?4, ?5))
               AND (?6 = 0 OR NOT l.keeps_watched_marks
                    OR coalesce(p.state, 'not_started') <> 'watched')
             ORDER BY s.ordinal, e.ordinal
             LIMIT 1",
            what_a_work_is("e.")
        )))
        .bind(viewer.to_db_string())
        .bind(within.to_db_string())
        .bind(i64::from(after.is_some()))
        .bind(season)
        .bind(episode)
        .bind(i64::from(only_unwatched))
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| work_from_row(&row)).transpose()
    }

    /// The works one work hangs under, nearest first.
    ///
    /// An episode answers with its season and then its series, a season with
    /// its series, a film with nothing. Two reads at most, because that is how
    /// deep the arrangement goes, and it stops at anything deeper rather than
    /// walking for ever if a row ever pointed at itself.
    pub async fn ancestry_of(&self, work_id: WorkId) -> Result<Vec<Work>> {
        let mut climbed = Vec::new();
        let mut looking = self.work(work_id).await?.and_then(|work| work.parent_id);
        while let Some(parent_id) = looking {
            let Some(parent) = self.work(parent_id).await? else {
                break;
            };
            looking = parent
                .parent_id
                .filter(|_| climbed.len() < HOW_DEEP_IT_GOES);
            climbed.push(parent);
        }
        Ok(climbed)
    }

    /// The child of a work sitting at that number: season two, episode five.
    pub async fn child_by_ordinal(&self, parent_id: WorkId, ordinal: i32) -> Result<Option<Work>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM works WHERE parent_id = ? AND ordinal = ? LIMIT 1",
            what_a_work_is("")
        )))
        .bind(parent_id.to_db_string())
        .bind(ordinal)
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| work_from_row(&row)).transpose()
    }

    /// A work of this library going by that name, by the name it carries now
    /// or by a name it was once filed under.
    ///
    /// The second question is the one that matters after a look-up: a provider
    /// renames a work to its own title and fills in its year, and the folder
    /// on the disk says neither. Asked only by the name it carries, the next
    /// scan would write the same series down a second time, every time a file
    /// is added to it.
    async fn work_named(
        &self,
        library_id: LibraryId,
        sort_title: &str,
        release_year: Option<i32>,
        kind: Option<WorkKind>,
    ) -> Result<Option<Work>> {
        let carried = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM works
             WHERE library_id = ? AND sort_title = ?
               AND (release_year IS ? OR (release_year IS NULL AND ? IS NULL))
               AND (? IS NULL OR kind = ?)
             ORDER BY added_at
             LIMIT 1",
            what_a_work_is("")
        )))
        .bind(library_id.to_db_string())
        .bind(sort_title)
        .bind(release_year)
        .bind(release_year)
        .bind(kind.map(WorkKind::as_str))
        .bind(kind.map(WorkKind::as_str))
        .fetch_optional(self.reader())
        .await?;
        if let Some(row) = carried {
            return work_from_row(&row).map(Some);
        }

        let filed = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM works w
             JOIN work_filing_names f ON f.work_id = w.id
             WHERE w.library_id = ? AND f.sort_title = ?
               AND (f.release_year IS ? OR (f.release_year IS NULL AND ? IS NULL))
               AND (? IS NULL OR w.kind = ?)
             ORDER BY w.added_at
             LIMIT 1",
            what_a_work_is("w.")
        )))
        .bind(library_id.to_db_string())
        .bind(sort_title)
        .bind(release_year)
        .bind(release_year)
        .bind(kind.map(WorkKind::as_str))
        .bind(kind.map(WorkKind::as_str))
        .fetch_optional(self.reader())
        .await?;

        filed.map(|row| work_from_row(&row)).transpose()
    }

    /// Works of a library, newest first, which is the order the home page uses.
    pub async fn recent_works(&self, library_id: LibraryId, limit: i64) -> Result<Vec<Work>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM works WHERE library_id = ? ORDER BY added_at DESC LIMIT ?",
            what_a_work_is("")
        )))
        .bind(library_id.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        rows.iter().map(work_from_row).collect()
    }

    /// What the whole catalogue holds, for the diagnostic.
    pub async fn catalogue_summary(&self) -> Result<CatalogueSummary> {
        let works: (i64, i64, i64) = sqlx::query_as(
            "SELECT count(*),
                    sum(identification IN ('identified', 'manual', 'own')),
                    sum(identification IN ('pending', 'unidentified'))
             FROM works",
        )
        .fetch_one(self.reader())
        .await?;
        let files: (i64, i64) =
            sqlx::query_as("SELECT count(*), sum(missing_since IS NOT NULL) FROM media_sources")
                .fetch_one(self.reader())
                .await?;

        let read: (i64,) = sqlx::query_as("SELECT count(*) FROM media_source_key_frames")
            .fetch_one(self.reader())
            .await?;
        let with_thumbnails: (i64,) =
            sqlx::query_as("SELECT count(*) FROM media_source_thumbnails")
                .fetch_one(self.reader())
                .await?;

        Ok(CatalogueSummary {
            works: works.0,
            identified: works.1,
            awaiting_identification: works.2,
            files: files.0,
            missing_files: files.1,
            read_for_key_frames: read.0,
            with_thumbnails: with_thumbnails.0,
        })
    }

    /// How many works a library holds.
    pub async fn count_works(&self, library_id: LibraryId) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT count(*) FROM works WHERE library_id = ?")
            .bind(library_id.to_db_string())
            .fetch_one(self.reader())
            .await?;
        Ok(row.0)
    }

    /// Every file recorded under one root, in path order.
    ///
    /// This is the side of the comparison a scan starts from, so it stays as
    /// small as the comparison needs.
    pub async fn sources_of_root(&self, root_id: LibraryRootId) -> Result<Vec<StoredSource>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "{A_STORED_SOURCE}
             WHERE s.root_id = ? ORDER BY s.relative_path"
        )))
        .bind(root_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter().map(stored_source_from_row).collect()
    }

    /// The name of every file a library holds.
    ///
    /// Read for what the names have in common rather than for any one of them:
    /// the word whoever named these files signs with can only be seen across
    /// the whole set.
    pub async fn source_names_of_library(&self, library_id: LibraryId) -> Result<Vec<String>> {
        let rows: Vec<(String,)> = sqlx::query_as(
            "SELECT s.relative_path
             FROM media_sources s
             JOIN library_roots r ON r.id = s.root_id
             WHERE r.library_id = ?",
        )
        .bind(library_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(path,)| {
                Path::new(&path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or(&path)
                    .to_string()
            })
            .collect())
    }

    /// Every file behind one work, newest first.
    ///
    /// A work can have several: the same film in two definitions is two files
    /// of one work, and the page offers a choice between them.
    pub async fn sources_of_work(&self, work_id: WorkId) -> Result<Vec<StoredSource>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "{A_STORED_SOURCE}
             WHERE s.work_id = ? ORDER BY s.added_at"
        )))
        .bind(work_id.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(stored_source_from_row).collect()
    }

    /// One file, with the disk it lives on.
    ///
    /// For the things that are done to one file rather than to a library: the
    /// whole path has to be built, and only the root knows where the disk is
    /// mounted.
    pub async fn source_by_id(&self, source_id: MediaSourceId) -> Result<Option<StoredSource>> {
        let row = sqlx::query(AssertSqlSafe(format!(
            "{A_STORED_SOURCE}
             WHERE s.id = ?"
        )))
        .bind(source_id.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        row.as_ref().map(stored_source_from_row).transpose()
    }

    /// The name of one file and the library it belongs to.
    ///
    /// What is needed to read a file name again by the rules of its own
    /// library, without reading everything that library holds.
    pub async fn where_a_source_lives(
        &self,
        source_id: MediaSourceId,
    ) -> Result<Option<(PathBuf, LibraryId)>> {
        let row = sqlx::query(
            "SELECT s.relative_path, r.library_id
             FROM media_sources s
             JOIN library_roots r ON r.id = s.root_id
             WHERE s.id = ?",
        )
        .bind(source_id.to_db_string())
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| {
            Ok((
                PathBuf::from(row.try_get::<String, _>("relative_path")?),
                row.try_get::<String, _>("library_id")?
                    .parse()
                    .map_err(|_| DatabaseError::Corrupt("library identifier".to_string()))?,
            ))
        })
        .transpose()
    }

    /// What one file is, beyond what identifies it.
    pub async fn source_details(
        &self,
        source_id: MediaSourceId,
    ) -> Result<Option<(SourceAnalysis, Option<Timestamp>)>> {
        let row = sqlx::query(
            "SELECT container, duration_ms, overall_bitrate, analysed_at
             FROM media_sources WHERE id = ?",
        )
        .bind(source_id.to_db_string())
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| {
            Ok((
                SourceAnalysis {
                    container: row.try_get("container")?,
                    duration: row
                        .try_get::<Option<i64>, _>("duration_ms")?
                        .map(Millis::new),
                    overall_bitrate: row.try_get("overall_bitrate")?,
                },
                parse_optional_timestamp(
                    row.try_get::<Option<String>, _>("analysed_at")?.as_deref(),
                )?,
            ))
        })
        .transpose()
    }

    /// The files that weigh the most, heaviest first.
    ///
    /// For the diagnostic, and for one question in particular: which films in
    /// this library are the hardest thing it will ever be asked to play. A
    /// server is not tested by its ordinary films but by its worst ones, and
    /// the worst ones are not known until they are named.
    ///
    /// Weight rather than anything cleverer, because weight is the one measure
    /// that needs nothing read and no rule agreed on: a file is big because of
    /// what is in it. Files no longer on disk are left out, since a film
    /// nobody can play is not a test of anything.
    pub async fn heaviest_sources(&self, limit: i64) -> Result<Vec<HeaviestSource>> {
        let rows = sqlx::query(
            "SELECT s.id, s.relative_path, s.size_bytes, s.container, s.duration_ms,
                    s.overall_bitrate, w.title, w.release_year
             FROM media_sources s
             JOIN works w ON w.id = s.work_id
             WHERE s.missing_since IS NULL
             ORDER BY s.size_bytes DESC, s.relative_path
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                let path: String = row.try_get("relative_path")?;
                Ok(HeaviestSource {
                    id: row
                        .try_get::<String, _>("id")?
                        .parse()
                        .map_err(|_| DatabaseError::Corrupt("media source id".to_string()))?,
                    title: row.try_get("title")?,
                    release_year: row.try_get("release_year")?,
                    file_name: Path::new(&path)
                        .file_name()
                        .map_or(path.clone(), |name| name.to_string_lossy().into_owned()),
                    size_bytes: row.try_get("size_bytes")?,
                    container: row.try_get("container")?,
                    duration: row
                        .try_get::<Option<i64>, _>("duration_ms")?
                        .map(Millis::new),
                    overall_bitrate: row.try_get("overall_bitrate")?,
                })
            })
            .collect()
    }

    /// Files of one root that carry no analysis yet.
    ///
    /// A file absent from disk is left out: analysing it would fail, and
    /// failing on purpose is not a diagnosis.
    pub async fn unanalysed_sources_of_root(
        &self,
        root_id: LibraryRootId,
    ) -> Result<Vec<StoredSource>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "{A_STORED_SOURCE}
             WHERE s.root_id = ? AND s.analysed_at IS NULL AND s.missing_since IS NULL
             ORDER BY s.relative_path"
        )))
        .bind(root_id.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(stored_source_from_row).collect()
    }

    /// Records a file found by a scan.
    pub async fn insert_source(
        &self,
        work_id: WorkId,
        root_id: LibraryRootId,
        relative_path: &Path,
        size_bytes: i64,
        modified_at: Timestamp,
    ) -> Result<MediaSourceId> {
        let id = MediaSourceId::new();
        sqlx::query(
            "INSERT INTO media_sources
                (id, work_id, root_id, relative_path, size_bytes, modified_at, added_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(work_id.to_db_string())
        .bind(root_id.to_db_string())
        .bind(relative_path.to_string_lossy().as_ref())
        .bind(size_bytes)
        .bind(timestamp_to_text(modified_at))
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(id)
    }

    /// Records that a file changed on disk, which also clears any absence.
    ///
    /// The analysis it carried is dropped at the same time: it describes a
    /// file that no longer exists, and keeping it would mean serving the
    /// tracks of one copy while playing another.
    pub async fn refresh_source_identity(
        &self,
        id: MediaSourceId,
        size_bytes: i64,
        modified_at: Timestamp,
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query(
            "UPDATE media_sources
             SET size_bytes = ?, modified_at = ?, missing_since = NULL, analysed_at = NULL,
                 container = NULL, duration_ms = NULL, overall_bitrate = NULL,
                 analysis_failure = NULL
             WHERE id = ?",
        )
        .bind(size_bytes)
        .bind(timestamp_to_text(modified_at))
        .bind(id.to_db_string())
        .execute(&mut *transaction)
        .await?;
        // The streams inside the file go, since they described a copy that is
        // gone. A subtitle in its own file describes itself and stays.
        sqlx::query("DELETE FROM tracks WHERE source_id = ? AND is_external = 0")
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM chapters WHERE source_id = ?")
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        // Where the picture could be started and the thumbnails of the bar
        // were both read out of the copy that is gone. Left behind, they would
        // cut a film at positions belonging to another one and show a viewer
        // pictures of it.
        sqlx::query("DELETE FROM media_source_key_frames WHERE source_id = ?")
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM media_source_thumbnails WHERE source_id = ?")
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Moves everything one work holds onto another and drops the empty one.
    ///
    /// Two works turn out to be one whenever the rules that read file names
    /// improve, or whenever the provider answers one identifier for two names
    /// nothing could ever have matched: several copies of one film, or one
    /// series written two ways because it arrived on two disks. Left apart
    /// they are the same title, the same poster and the same synopsis twice in
    /// a grid, which is the very thing a version chooser exists to avoid.
    ///
    /// A series is not one row, so joining one is not one move: each season
    /// meets the season of the same number, each episode the episode of the
    /// same number, and only there do the files change hands. A season or an
    /// episode the other side does not have changes parent instead. Nothing is
    /// dropped until every level has been walked, because dropping a series
    /// still holding its seasons would take its episodes and their files down
    /// with it.
    ///
    /// What the ones that go described is not carried over: they name the same
    /// work as the ones that stay, by title, by picture and by cast, so there
    /// is nothing there worth keeping twice.
    ///
    /// The paths of the pictures they had come back, so their files can be
    /// removed from the cache by the caller that put them there. Nothing else
    /// points at those rows, so without this they would sit there for ever.
    pub async fn merge_work_into(&self, from: WorkId, into: WorkId) -> Result<Vec<String>> {
        if from == into {
            return Ok(Vec::new());
        }
        let mut transaction = self.begin().await?;
        let no_longer_used = merge_within(&mut transaction, from, into).await?;
        transaction.commit().await?;
        Ok(no_longer_used)
    }

    /// Works of one library the provider says are one and the same work.
    ///
    /// Only works met on their own: a season and an episode are placed by the
    /// series that holds them, never by what a provider numbers them. It also
    /// has to be that way, because a provider numbers its films, its series,
    /// its seasons and its episodes on separate counters, so the twelfth
    /// season and the twelfth film carry the same number. For the same reason
    /// a film never meets a series here, whatever number they share: a group
    /// is an identifier and a kind together.
    ///
    /// Two copies of one film can carry names nothing could ever match, and a
    /// series arriving on a second disk can be written another way there. Only
    /// the provider can say they are one, and it says so by answering the same
    /// identifier for both. Left apart they are the same title, the same
    /// poster and the same synopsis twice in a grid.
    ///
    /// One provider is asked about at a time, and a work carries one
    /// identifier per provider, so the groups never overlap.
    pub async fn works_sharing_an_identity(
        &self,
        library_id: LibraryId,
        provider: &str,
    ) -> Result<Vec<SharedIdentity>> {
        let rows = sqlx::query(
            "SELECT e.external_id, w.kind, e.work_id
             FROM work_external_ids e
             JOIN works w ON w.id = e.work_id
             WHERE w.library_id = ? AND e.provider = ?
               AND w.parent_id IS NULL
             ORDER BY e.external_id, w.kind, w.added_at",
        )
        .bind(library_id.to_db_string())
        .bind(provider)
        .fetch_all(self.reader())
        .await?;

        let mut groups: Vec<SharedIdentity> = Vec::new();
        let mut current: Option<(String, String)> = None;
        for row in &rows {
            let named = (row.try_get("external_id")?, row.try_get("kind")?);
            let work_id: WorkId = row
                .try_get::<String, _>("work_id")?
                .parse()
                .map_err(|_| DatabaseError::Corrupt("work identifier".to_string()))?;

            // The first of each group is the one that has been here longest,
            // which is the one the others join.
            match &current {
                Some(seen) if seen == &named => {
                    groups
                        .last_mut()
                        .expect("a group was started with this identifier")
                        .others
                        .push(work_id);
                }
                _ => {
                    current = Some(named);
                    groups.push(SharedIdentity {
                        keep: work_id,
                        others: Vec::new(),
                    });
                }
            }
        }

        groups.retain(|group| !group.others.is_empty());
        Ok(groups)
    }

    /// Marks a file absent. Never a deletion: a disconnected disk must not
    /// cost a library.
    pub async fn mark_source_missing(&self, id: MediaSourceId) -> Result<()> {
        sqlx::query(
            "UPDATE media_sources SET missing_since = ? WHERE id = ? AND missing_since IS NULL",
        )
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Marks a file present again, keeping its identifier and its history.
    pub async fn mark_source_present(&self, id: MediaSourceId) -> Result<()> {
        sqlx::query("UPDATE media_sources SET missing_since = NULL WHERE id = ?")
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Records what a work is called at an external provider.
    ///
    /// Replaces the identifier already held for that provider, since a work
    /// has exactly one at each of them and a second would be a contradiction
    /// rather than an addition.
    pub async fn set_work_external_id(
        &self,
        work_id: WorkId,
        provider: &str,
        external_id: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO work_external_ids (work_id, provider, external_id)
             VALUES (?, ?, ?)
             ON CONFLICT (work_id, provider) DO UPDATE SET external_id = excluded.external_id",
        )
        .bind(work_id.to_db_string())
        .bind(provider)
        .bind(external_id)
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// What a work is called at each provider that knows it.
    pub async fn work_external_ids(&self, work_id: WorkId) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query(
            "SELECT provider, external_id FROM work_external_ids WHERE work_id = ? ORDER BY provider",
        )
        .bind(work_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| Ok((row.try_get("provider")?, row.try_get("external_id")?)))
            .collect()
    }

    /// Attaches a video sitting next to the work, such as a trailer.
    ///
    /// Replaces the entry at the same path rather than piling copies up, so a
    /// scan can run as often as it likes.
    pub async fn store_local_extra_video(
        &self,
        work_id: WorkId,
        extra: &LocalExtraVideo,
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query(
            "DELETE FROM extra_videos WHERE work_id = ? AND root_id = ? AND relative_path = ?",
        )
        .bind(work_id.to_db_string())
        .bind(extra.root_id.to_db_string())
        .bind(extra.relative_path.to_string_lossy().as_ref())
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "INSERT INTO extra_videos (id, work_id, kind, name, root_id, relative_path, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(ExtraVideoId::new().to_db_string())
        .bind(work_id.to_db_string())
        .bind(&extra.kind)
        .bind(extra.name.as_deref())
        .bind(extra.root_id.to_db_string())
        .bind(extra.relative_path.to_string_lossy().as_ref())
        .bind(timestamp_to_text(now()))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Videos attached to a work, such as its trailers.
    pub async fn extra_videos_of_work(&self, work_id: WorkId) -> Result<Vec<PlayableExtraVideo>> {
        let rows = sqlx::query(
            "SELECT extra_videos.kind, extra_videos.name, extra_videos.relative_path,
                    library_roots.path AS root_path
             FROM extra_videos
             JOIN library_roots ON library_roots.id = extra_videos.root_id
             WHERE extra_videos.work_id = ? AND extra_videos.relative_path IS NOT NULL
             ORDER BY extra_videos.kind, extra_videos.relative_path",
        )
        .bind(work_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        let mut extras = Vec::with_capacity(rows.len());
        for row in rows {
            let root: String = row.try_get("root_path")?;
            let relative: String = row.try_get("relative_path")?;
            extras.push(PlayableExtraVideo {
                kind: row.try_get("kind")?,
                name: row.try_get("name")?,
                path: PathBuf::from(root).join(relative),
            });
        }
        Ok(extras)
    }
}

fn stored_source_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<StoredSource> {
    Ok(StoredSource {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        work_id: parse_id(&row.try_get::<String, _>("work_id")?)?,
        relative_path: PathBuf::from(row.try_get::<String, _>("relative_path")?),
        root_label: row.try_get("root_label")?,
        root_path: PathBuf::from(row.try_get::<String, _>("root_path")?),
        size_bytes: row.try_get("size_bytes")?,
        modified_at: parse_timestamp(&row.try_get::<String, _>("modified_at")?)?,
        added_at: parse_timestamp(&row.try_get::<String, _>("added_at")?)?,
        missing_since: parse_optional_timestamp(
            row.try_get::<Option<String>, _>("missing_since")?
                .as_deref(),
        )?,
    })
}

/// Writes a new work, wherever the caller is writing: on the pool for a plain
/// creation, inside a transaction when something else has to land with it.
/// Where a work sits the moment it is first written down.
///
/// The three together rather than one by one: a work met on its own has no
/// parent and no rank, and a season or an episode has both. Passing them apart
/// is how one of them ends up written and the other forgotten.
pub(crate) struct Placed {
    library_id: LibraryId,
    parent_id: Option<WorkId>,
    ordinal: Option<i32>,
}

impl Placed {
    /// Met on its own: a film, a series, an album.
    fn on_its_own(library_id: LibraryId) -> Self {
        Self {
            library_id,
            parent_id: None,
            ordinal: None,
        }
    }

    /// Kept in a folder of a library of home media, or at its root: under a
    /// parent when there is one, and at no rank, since what orders a folder
    /// is the names of what is in it.
    pub(crate) fn in_folder(library_id: LibraryId, folder: Option<WorkId>) -> Self {
        Self {
            library_id,
            parent_id: folder,
            ordinal: None,
        }
    }

    /// Hung under another work at a given rank.
    pub(crate) fn under(library_id: LibraryId, parent_id: WorkId, ordinal: i32) -> Self {
        Self {
            library_id,
            parent_id: Some(parent_id),
            ordinal: Some(ordinal),
        }
    }
}

/// Writes down a name a work was filed under, if it is not written down yet.
pub(crate) async fn remember_filing_name<'e, E>(
    executor: E,
    work_id: WorkId,
    sort_title: &str,
    release_year: Option<i32>,
) -> Result<()>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "INSERT OR IGNORE INTO work_filing_names (work_id, sort_title, release_year)
         VALUES (?, ?, ?)",
    )
    .bind(work_id.to_db_string())
    .bind(sort_title)
    .bind(release_year)
    .execute(executor)
    .await?;
    Ok(())
}

/// Everything [`Database::merge_work_into`] does, inside a transaction somebody
/// else holds.
pub(crate) async fn merge_within(
    transaction: &mut sqlx::Transaction<'_, Sqlite>,
    from: WorkId,
    into: WorkId,
) -> Result<Vec<String>> {
    let mut no_longer_used = Vec::new();
    let mut going = Vec::new();
    let mut receiving = Vec::new();
    let mut pairs = vec![(from, into)];

    while let Some((from, into)) = pairs.pop() {
        for (child, ordinal) in children_with_their_ordinal(&mut **transaction, from).await? {
            let met = match ordinal {
                Some(ordinal) => child_at(&mut **transaction, into, ordinal).await?,
                None => None,
            };
            match met {
                Some(met) => pairs.push((child, met)),
                None => {
                    sqlx::query("UPDATE works SET parent_id = ? WHERE id = ?")
                        .bind(into.to_db_string())
                        .bind(child.to_db_string())
                        .execute(&mut **transaction)
                        .await?;
                }
            }
        }

        sqlx::query("UPDATE media_sources SET work_id = ? WHERE work_id = ?")
            .bind(into.to_db_string())
            .bind(from.to_db_string())
            .execute(&mut **transaction)
            .await?;
        sqlx::query("UPDATE extra_videos SET work_id = ? WHERE work_id = ?")
            .bind(into.to_db_string())
            .bind(from.to_db_string())
            .execute(&mut **transaction)
            .await?;

        // What each person had of the one that goes stays theirs: where they
        // were, what they watched, what they kept. Where they already have
        // something of the one that stays, that one is kept.
        for carried in [
            "INSERT OR IGNORE INTO playback_progress
                (user_id, work_id, position_ms, state, marked_manually, play_count,
                 audio_track_id, subtitle_track_id, subtitles_off, reported_at, last_played_at)
             SELECT user_id, ?, position_ms, state, marked_manually, play_count,
                    audio_track_id, subtitle_track_id, subtitles_off, reported_at, last_played_at
             FROM playback_progress WHERE work_id = ?",
            "INSERT OR IGNORE INTO favorites (user_id, work_id, created_at)
             SELECT user_id, ?, created_at FROM favorites WHERE work_id = ?",
            "INSERT OR IGNORE INTO watchlist (user_id, work_id, created_at)
             SELECT user_id, ?, created_at FROM watchlist WHERE work_id = ?",
        ] {
            sqlx::query(carried)
                .bind(into.to_db_string())
                .bind(from.to_db_string())
                .execute(&mut **transaction)
                .await?;
        }

        let pictures: Vec<String> = sqlx::query(
            "SELECT relative_path FROM images WHERE owner_kind = 'work' AND owner_id = ?",
        )
        .bind(from.to_db_string())
        .fetch_all(&mut **transaction)
        .await?
        .iter()
        .map(|row| row.try_get::<String, _>("relative_path"))
        .collect::<std::result::Result<_, _>>()?;
        no_longer_used.extend(pictures);
        // Pictures are found by owner rather than by a key the engine
        // knows about, so dropping the work does not drop them.
        sqlx::query("DELETE FROM images WHERE owner_kind = 'work' AND owner_id = ?")
            .bind(from.to_db_string())
            .execute(&mut **transaction)
            .await?;

        going.push(from);
        receiving.push(into);
    }

    // The name the one that goes was filed under is the one its folder still
    // carries. Dropped here, the next file added under that folder would
    // write the same work down again, and the two would have to be put
    // together all over again. Only the two at the top are filed under a
    // name of their own; a season and an episode are placed by number.
    sqlx::query(
        "INSERT OR IGNORE INTO work_filing_names (work_id, sort_title, release_year)
         SELECT ?, sort_title, release_year FROM work_filing_names WHERE work_id = ?",
    )
    .bind(into.to_db_string())
    .bind(from.to_db_string())
    .execute(&mut **transaction)
    .await?;

    for work in going {
        sqlx::query("DELETE FROM works WHERE id = ?")
            .bind(work.to_db_string())
            .execute(&mut **transaction)
            .await?;
    }
    for work in receiving {
        recount_children(&mut **transaction, work).await?;
    }

    Ok(no_longer_used)
}

/// What hangs under a work, each with the number it is ranked at.
async fn children_with_their_ordinal<'e, E>(
    executor: E,
    parent_id: WorkId,
) -> Result<Vec<(WorkId, Option<i32>)>>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    sqlx::query("SELECT id, ordinal FROM works WHERE parent_id = ?")
        .bind(parent_id.to_db_string())
        .fetch_all(executor)
        .await?
        .iter()
        .map(|row| {
            let id: WorkId = row
                .try_get::<String, _>("id")?
                .parse()
                .map_err(|_| DatabaseError::Corrupt("work identifier".to_string()))?;
            Ok((id, row.try_get("ordinal")?))
        })
        .collect()
}

/// Counts again what hangs under a work, a count kept on the work so that a
/// page never has to make it.
pub(crate) async fn recount_children<'e, E>(executor: E, parent_id: WorkId) -> Result<()>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    sqlx::query(
        "UPDATE works
            SET child_count = (SELECT count(*) FROM works AS child WHERE child.parent_id = works.id)
          WHERE id = ?",
    )
    .bind(parent_id.to_db_string())
    .execute(executor)
    .await?;
    Ok(())
}

/// The work hanging under another at that number, if there is one.
pub(crate) async fn child_at<'e, E>(
    executor: E,
    parent_id: WorkId,
    ordinal: i32,
) -> Result<Option<WorkId>>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    let row = sqlx::query("SELECT id FROM works WHERE parent_id = ? AND ordinal = ? LIMIT 1")
        .bind(parent_id.to_db_string())
        .bind(ordinal)
        .fetch_optional(executor)
        .await?;

    row.map(|row| {
        row.try_get::<String, _>("id")?
            .parse()
            .map_err(|_| DatabaseError::Corrupt("work identifier".to_string()))
    })
    .transpose()
}

pub(crate) async fn insert_work<'e, E>(
    executor: E,
    placed: Placed,
    kind: WorkKind,
    title: &str,
    sort_title: &str,
    release_year: Option<i32>,
) -> Result<Work>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    let Placed {
        library_id,
        parent_id,
        ordinal,
    } = placed;
    let id = WorkId::new();
    let moment = now();
    let timestamp = timestamp_to_text(moment);

    sqlx::query(
        "INSERT INTO works (id, library_id, parent_id, ordinal, kind, title, sort_title,
                            release_year, added_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_db_string())
    .bind(library_id.to_db_string())
    .bind(parent_id.map(|parent| parent.to_db_string()))
    .bind(ordinal)
    .bind(kind.as_str())
    .bind(title)
    .bind(sort_title)
    .bind(release_year)
    .bind(&timestamp)
    .bind(&timestamp)
    .execute(executor)
    .await?;

    Ok(Work {
        id,
        library_id,
        parent_id,
        ordinal,
        kind,
        title: title.to_string(),
        sort_title: sort_title.to_string(),
        release_year,
        runtime: None,
        community_rating: None,
        age_rating_label: None,
        identification: IdentificationState::Pending,
        identification_note: None,
        dominant_color: None,
        added_at: moment,
        updated_at: moment,
    })
}

pub(crate) fn work_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Work> {
    let kind_text: String = row.try_get("kind")?;
    let identification_text: String = row.try_get("identification")?;
    Ok(Work {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        library_id: parse_id(&row.try_get::<String, _>("library_id")?)?,
        ordinal: row.try_get("ordinal")?,
        parent_id: row
            .try_get::<Option<String>, _>("parent_id")?
            .map(|value| parse_id(&value))
            .transpose()?,
        kind: WorkKind::parse(&kind_text)
            .ok_or_else(|| DatabaseError::Corrupt(format!("work kind '{kind_text}' is unknown")))?,
        title: row.try_get("title")?,
        sort_title: row.try_get("sort_title")?,
        release_year: row.try_get("release_year")?,
        runtime: row
            .try_get::<Option<i64>, _>("runtime_ms")?
            .map(Millis::new),
        community_rating: row.try_get("community_rating")?,
        age_rating_label: row.try_get("age_rating_label")?,
        identification: IdentificationState::parse(&identification_text).ok_or_else(|| {
            DatabaseError::Corrupt(format!(
                "identification state '{identification_text}' is unknown"
            ))
        })?,
        identification_note: identification_note_from_row(row)?,
        dominant_color: row.try_get("dominant_color")?,
        added_at: parse_timestamp(&row.try_get::<String, _>("added_at")?)?,
        updated_at: parse_timestamp(&row.try_get::<String, _>("updated_at")?)?,
    })
}

/// Reads the reason the last look up failed.
///
/// A word this version does not know is read as no reason rather than as a
/// corrupt row: the note explains a film, it does not decide anything, and a
/// page that refuses to open because of an explanation would be absurd.
pub(crate) fn identification_note_from_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<Option<IdentificationNote>> {
    Ok(row
        .try_get::<Option<String>, _>("identification_note")?
        .as_deref()
        .and_then(IdentificationNote::parse))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use melyxar_core::id::TrackId;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::media::{ColorInfo, HdrFormat, Track, TrackKind, VideoDetails};

    /// Somebody to answer for, since what a page shows depends on who is
    /// looking at it.
    ///
    /// The same one every time it is asked for, so that a test laying out two
    /// series is not refused a second account under a name it already used.
    pub(crate) async fn a_viewer(database: &Database) -> UserId {
        if let Some((already, _)) = database.user_by_name("Viewer").await.expect("read") {
            return already.id;
        }
        database
            .create_user("Viewer", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created")
            .id
    }

    /// What the viewer made of one child, which a read for somebody always
    /// carries.
    fn state_of(child: &ChildWork) -> &crate::browse::CardState {
        child.card.state.as_ref().expect("read for somebody")
    }

    /// The copy a play button on one child would start.
    fn source_of(child: &ChildWork) -> Option<MediaSourceId> {
        state_of(child).source_id
    }

    pub(crate) async fn library() -> (Database, LibraryId, LibraryRootId) {
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
        let root_id = library.roots[0].id;
        (database, library.id, root_id)
    }

    /// A series with two seasons, the first holding two episodes and the
    /// second one, built the way a scan builds it.
    async fn a_series(
        database: &Database,
        library_id: LibraryId,
    ) -> (WorkId, Vec<WorkId>, Vec<WorkId>) {
        let series = database
            .create_work(
                library_id,
                WorkKind::Series,
                "Distant Signal",
                "distant signal",
                Some(2019),
            )
            .await
            .expect("series written");

        let mut seasons = Vec::new();
        let mut episodes = Vec::new();
        for (season, count) in [(1, 2), (2, 1)] {
            let written = database
                .create_child_work(
                    library_id,
                    series.id,
                    season,
                    WorkKind::Season,
                    &format!("Season {season}"),
                    &format!("season {season}"),
                )
                .await
                .expect("season written");
            for number in 1..=count {
                episodes.push(
                    database
                        .create_child_work(
                            library_id,
                            written.id,
                            number,
                            WorkKind::Episode,
                            &format!("Episode {number}"),
                            &format!("episode {number}"),
                        )
                        .await
                        .expect("episode written")
                        .id,
                );
            }
            seasons.push(written.id);
        }
        (series.id, seasons, episodes)
    }

    /// A series whose every episode holds a file, seasons and their episodes
    /// given by number, built the way a scan builds one.
    pub(crate) async fn a_series_on_disk(
        database: &Database,
        library_id: LibraryId,
        root_id: LibraryRootId,
        title: &str,
        seasons: &[(i32, &[i32])],
    ) -> WorkId {
        let series = database
            .create_work(
                library_id,
                WorkKind::Series,
                title,
                &title.to_lowercase(),
                Some(2019),
            )
            .await
            .expect("series written");
        for (season, episodes) in seasons {
            let written = database
                .create_child_work(
                    library_id,
                    series.id,
                    *season,
                    WorkKind::Season,
                    &format!("Season {season}"),
                    &format!("season {season}"),
                )
                .await
                .expect("season written");
            for number in *episodes {
                let episode = database
                    .create_child_work(
                        library_id,
                        written.id,
                        *number,
                        WorkKind::Episode,
                        &format!("Episode {number}"),
                        &format!("episode {number}"),
                    )
                    .await
                    .expect("episode written");
                let path = format!("{title}/{season}/{number}.mkv");
                database
                    .insert_source(episode.id, root_id, Path::new(&path), 1_000, now())
                    .await
                    .expect("file recorded");
            }
        }
        series.id
    }

    #[tokio::test]
    async fn a_series_answers_with_its_seasons_and_a_season_with_its_episodes() {
        let (database, library_id, _) = library().await;
        let (series, seasons, _) = a_series(&database, library_id).await;

        let viewer = a_viewer(&database).await;
        let answered = database.children_of(viewer, series, "fr").await.expect("read");
        assert_eq!(
            answered
                .iter()
                .map(|child| child.ordinal)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)],
            "in the order they are numbered, not the order they were written"
        );
        assert!(answered.iter().all(|child| child.card.kind == WorkKind::Season));
        assert_eq!(
            answered
                .iter()
                .map(|child| child.child_count)
                .collect::<Vec<_>>(),
            vec![2, 1],
            "a season says how many episodes it holds"
        );

        let first = database
            .children_of(viewer, seasons[0], "fr")
            .await
            .expect("read");
        assert_eq!(first.len(), 2);
        assert!(first.iter().all(|child| child.card.kind == WorkKind::Episode));
        assert_eq!(
            first.iter().map(|child| child.ordinal).collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
        );
    }

    #[tokio::test]
    async fn a_childs_row_names_the_biggest_copy_of_it_on_disk() {
        // The "up next" row of a player wants to start any episode it shows
        // straight from its own row, exactly the copy `next_episode_after`
        // would offer for the same episode.
        let (database, library_id, root_id) = library().await;
        let (_, seasons, episodes) = a_series(&database, library_id).await;
        let viewer = a_viewer(&database).await;

        // One episode with no file at all.
        let untouched = database
            .children_of(viewer, seasons[0], "fr")
            .await
            .expect("read")
            .into_iter()
            .find(|child| child.card.id == episodes[0])
            .expect("the episode is among the children");
        assert_eq!(source_of(&untouched), None, "nothing to play means nothing to name");

        // The other with two copies, one heavier than the other.
        let smaller = database
            .insert_source(episodes[1], root_id, Path::new("small.mkv"), 1_000, now())
            .await
            .expect("file recorded");
        let biggest = database
            .insert_source(episodes[1], root_id, Path::new("biggest.mkv"), 9_000, now())
            .await
            .expect("file recorded");

        let named = database
            .children_of(viewer, seasons[0], "fr")
            .await
            .expect("read")
            .into_iter()
            .find(|child| child.card.id == episodes[1])
            .expect("the episode is among the children");
        assert_eq!(source_of(&named), Some(biggest));
        assert_ne!(source_of(&named), Some(smaller));
    }

    #[tokio::test]
    async fn an_episode_in_a_list_says_what_it_is_about_and_how_long_it_runs() {
        // A list of episodes has room for a few lines on each one, in the
        // language of the library, and for a length even before anybody has
        // looked the episode up.
        let (database, library_id, root_id) = library().await;
        let (_, seasons, episodes) = a_series(&database, library_id).await;
        database
            .set_work_synopsis(episodes[0], "fr", None, "Un phare, une tempête.")
            .await
            .expect("synopsis written");
        database
            .set_work_synopsis(episodes[0], "en", None, "A lighthouse, a storm.")
            .await
            .expect("synopsis written");
        database
            .set_work_synopsis(episodes[1], "en", None, "The harbour at dawn.")
            .await
            .expect("synopsis written");
        let source = database
            .insert_source(episodes[1], root_id, Path::new("two.mkv"), 1_000, now())
            .await
            .expect("file recorded");
        database
            .store_analysis(
                source,
                &SourceAnalysis {
                    duration: Some(Millis::new(2_700_000)),
                    ..SourceAnalysis::default()
                },
                &[],
                &[],
            )
            .await
            .expect("analysis stored");

        let viewer = a_viewer(&database).await;
        let read = database
            .children_of(viewer, seasons[0], "fr")
            .await
            .expect("read");
        assert_eq!(
            read.iter()
                .map(|child| child.overview.as_deref())
                .collect::<Vec<_>>(),
            vec![Some("Un phare, une tempête."), Some("The harbour at dawn.")],
            "the language asked for, and English where it has nothing"
        );
        assert_eq!(
            read.iter().map(|child| child.card.runtime).collect::<Vec<_>>(),
            vec![None, Some(Millis::new(2_700_000))],
            "nobody said how long the second runs, and its file does"
        );
    }

    #[tokio::test]
    async fn a_film_has_nothing_hanging_under_it_and_nothing_above_it() {
        let (database, library_id, _) = library().await;
        let film = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("film written");

        let viewer = a_viewer(&database).await;
        assert!(database
            .children_of(viewer, film.id, "fr")
            .await
            .expect("read")
            .is_empty());
        assert!(database
            .ancestry_of(film.id)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn an_episode_knows_its_season_and_then_its_series() {
        // The way back up is drawn before anything else on the page, so it
        // travels with the page rather than being asked for afterwards.
        let (database, library_id, _) = library().await;
        let (series, seasons, episodes) = a_series(&database, library_id).await;

        let climbed = database.ancestry_of(episodes[0]).await.expect("read");
        assert_eq!(
            climbed.iter().map(|work| work.id).collect::<Vec<_>>(),
            vec![seasons[0], series],
            "nearest first"
        );
        assert_eq!(climbed[0].kind, WorkKind::Season);
        assert_eq!(climbed[1].kind, WorkKind::Series);

        assert_eq!(
            database
                .ancestry_of(seasons[1])
                .await
                .expect("read")
                .iter()
                .map(|work| work.id)
                .collect::<Vec<_>>(),
            vec![series]
        );
    }

    /// Puts a file behind an episode, so it can be offered.
    async fn a_file_behind(database: &Database, root_id: LibraryRootId, work: WorkId, name: &str) {
        database
            .insert_source(
                work,
                root_id,
                &PathBuf::from(name),
                12,
                melyxar_core::time::now(),
            )
            .await
            .expect("file written down");
    }

    /// Says somebody watched one.
    async fn watched(database: &Database, viewer: UserId, work: WorkId) {
        database
            .record_playback_progress(
                viewer,
                work,
                Millis::new(0),
                melyxar_core::work::PlaybackState::Watched,
                melyxar_core::time::now(),
            )
            .await
            .expect("marked");
    }

    #[tokio::test]
    async fn the_next_episode_is_the_next_one_of_the_whole_series() {
        // An episode is numbered inside its own season, so the one after the
        // last of a season is the first of the season after it, not nothing.
        let (database, library_id, root_id) = library().await;
        let viewer = a_viewer(&database).await;
        let (_, seasons, episodes) = a_series(&database, library_id).await;
        for (rank, episode) in episodes.iter().enumerate() {
            a_file_behind(&database, root_id, *episode, &format!("{rank}.mkv")).await;
        }

        // Season one holds two, season two holds one.
        let after_first = database
            .next_episode_after(viewer, episodes[0])
            .await
            .expect("read")
            .expect("there is one after it");
        assert_eq!(after_first.id, episodes[1]);

        let across = database
            .next_episode_after(viewer, episodes[1])
            .await
            .expect("read")
            .expect("the first of the next season");
        assert_eq!(across.id, episodes[2]);
        assert_eq!(across.parent_id, Some(seasons[1]));

        assert_eq!(
            database
                .next_episode_after(viewer, episodes[2])
                .await
                .expect("read"),
            None,
            "and nothing at all after the last one of the last season"
        );
    }

    #[tokio::test]
    async fn the_previous_episode_is_the_one_before_it_across_the_series() {
        // Symmetric to the one above: the episode before the first of a
        // season is the last of the season before it, not nothing.
        let (database, library_id, root_id) = library().await;
        let (_, seasons, episodes) = a_series(&database, library_id).await;
        for (rank, episode) in episodes.iter().enumerate() {
            a_file_behind(&database, root_id, *episode, &format!("{rank}.mkv")).await;
        }

        let before_last = database
            .previous_episode_before(episodes[2])
            .await
            .expect("read")
            .expect("the last of the season before it");
        assert_eq!(before_last.id, episodes[1]);
        assert_eq!(before_last.parent_id, Some(seasons[0]));

        let before_second = database
            .previous_episode_before(episodes[1])
            .await
            .expect("read")
            .expect("there is one before it");
        assert_eq!(before_second.id, episodes[0]);

        assert_eq!(
            database
                .previous_episode_before(episodes[0])
                .await
                .expect("read"),
            None,
            "and nothing at all before the first one of the first season"
        );
    }

    #[tokio::test]
    async fn an_episode_with_no_file_behind_it_is_never_offered_as_the_next_one() {
        // The button it ends in has to play something.
        let (database, library_id, root_id) = library().await;
        let viewer = a_viewer(&database).await;
        let (_, _, episodes) = a_series(&database, library_id).await;
        a_file_behind(&database, root_id, episodes[0], "one.mkv").await;
        // Nothing behind the second; the third is there.
        a_file_behind(&database, root_id, episodes[2], "three.mkv").await;

        assert_eq!(
            database
                .next_episode_after(viewer, episodes[0])
                .await
                .expect("read")
                .map(|found| found.id),
            Some(episodes[2]),
            "the one nobody has is stepped over"
        );
    }

    #[tokio::test]
    async fn a_series_is_picked_back_up_at_the_first_episode_left_unwatched() {
        // Read in order rather than from the last one played: a series watched
        // out of order has a hole in it, and the hole is where somebody is.
        let (database, library_id, root_id) = library().await;
        let viewer = a_viewer(&database).await;
        let (series, _, episodes) = a_series(&database, library_id).await;
        for (rank, episode) in episodes.iter().enumerate() {
            a_file_behind(&database, root_id, *episode, &format!("{rank}.mkv")).await;
        }

        assert_eq!(
            database
                .where_to_resume(viewer, series)
                .await
                .expect("read")
                .map(|found| found.id),
            Some(episodes[0]),
            "nothing watched yet, so the very first"
        );

        // The first and the last watched, the middle one not.
        watched(&database, viewer, episodes[0]).await;
        watched(&database, viewer, episodes[2]).await;
        assert_eq!(
            database
                .where_to_resume(viewer, series)
                .await
                .expect("read")
                .map(|found| found.id),
            Some(episodes[1]),
            "the hole, not the one after the last one played"
        );

        watched(&database, viewer, episodes[1]).await;
        assert_eq!(
            database
                .where_to_resume(viewer, series)
                .await
                .expect("read"),
            None,
            "and nothing once it has all been watched, which is a series to \
             start again rather than to carry on"
        );
    }

    async fn left_halfway(
        database: &Database,
        viewer: UserId,
        work: WorkId,
        at: melyxar_core::time::Timestamp,
    ) {
        database
            .record_playback_progress(
                viewer,
                work,
                Millis::new(600_000),
                melyxar_core::work::PlaybackState::InProgress,
                at,
            )
            .await
            .expect("recorded");
    }

    #[tokio::test]
    async fn a_series_is_picked_back_up_only_by_what_its_library_keeps() {
        let (database, library_id, root_id) = library().await;
        let viewer = a_viewer(&database).await;
        let (series, _, episodes) = a_series(&database, library_id).await;
        for (rank, episode) in episodes.iter().enumerate() {
            a_file_behind(&database, root_id, *episode, &format!("{rank}.mkv")).await;
        }
        watched(&database, viewer, episodes[0]).await;
        left_halfway(&database, viewer, episodes[2], melyxar_core::time::now()).await;
        let picked_up = || async {
            database
                .where_to_resume(viewer, series)
                .await
                .expect("read")
                .map(|found| found.id)
        };
        let keeping = |keeps_resume_points, keeps_watched_marks| {
            melyxar_core::library::LibraryOptions {
                keeps_resume_points,
                keeps_watched_marks,
                ..Default::default()
            }
        };
        assert_eq!(picked_up().await, Some(episodes[2]), "the one left halfway");

        database
            .set_library_options(library_id, keeping(false, true))
            .await
            .expect("switched");
        assert_eq!(
            picked_up().await,
            Some(episodes[1]),
            "no place kept, so the first one not watched"
        );

        database
            .set_library_options(library_id, keeping(false, false))
            .await
            .expect("switched");
        assert_eq!(
            picked_up().await,
            Some(episodes[0]),
            "no mark kept either, so the very first"
        );
    }

    #[tokio::test]
    async fn an_episode_left_halfway_is_where_a_series_is_picked_back_up() {
        // Somebody in the middle of an episode is in the middle of that one,
        // even past a hole left earlier in the series.
        let (database, library_id, root_id) = library().await;
        let viewer = a_viewer(&database).await;
        let (series, _, episodes) = a_series(&database, library_id).await;
        for (rank, episode) in episodes.iter().enumerate() {
            a_file_behind(&database, root_id, *episode, &format!("{rank}.mkv")).await;
        }
        let two_days_ago = melyxar_core::time::now() - time::Duration::days(2);
        left_halfway(&database, viewer, episodes[2], two_days_ago).await;
        assert_eq!(
            database
                .where_to_resume(viewer, series)
                .await
                .expect("read")
                .map(|found| found.id),
            Some(episodes[2]),
            "the one left halfway, not the first one nobody has watched"
        );

        // Two left halfway: the one played last.
        left_halfway(&database, viewer, episodes[1], two_days_ago + time::Duration::DAY).await;
        assert_eq!(
            database
                .where_to_resume(viewer, series)
                .await
                .expect("read")
                .map(|found| found.id),
            Some(episodes[1])
        );

        // Finished, it gives way to the other one still halfway.
        watched(&database, viewer, episodes[1]).await;
        assert_eq!(
            database
                .where_to_resume(viewer, series)
                .await
                .expect("read")
                .map(|found| found.id),
            Some(episodes[2])
        );
    }

    #[tokio::test]
    async fn a_season_is_picked_back_up_inside_itself() {
        // Somebody on the page of a later season means that season, not the
        // first episode of a series they never started from the beginning.
        let (database, library_id, root_id) = library().await;
        let viewer = a_viewer(&database).await;
        let (series, seasons, episodes) = a_series(&database, library_id).await;
        for (rank, episode) in episodes.iter().enumerate() {
            a_file_behind(&database, root_id, *episode, &format!("{rank}.mkv")).await;
        }
        let resumed = |within: WorkId| {
            let database = &database;
            async move {
                database
                    .where_to_resume(viewer, within)
                    .await
                    .expect("read")
                    .map(|found| found.id)
            }
        };

        assert_eq!(resumed(seasons[1]).await, Some(episodes[2]), "its own first episode");
        assert_eq!(resumed(series).await, Some(episodes[0]), "the series still starts at the start");

        left_halfway(&database, viewer, episodes[1], melyxar_core::time::now()).await;
        assert_eq!(
            resumed(seasons[1]).await,
            Some(episodes[2]),
            "an episode left halfway in another season is not this season's"
        );
        assert_eq!(resumed(seasons[0]).await, Some(episodes[1]));

        watched(&database, viewer, episodes[2]).await;
        assert_eq!(resumed(seasons[1]).await, None, "a season watched through has nothing left");
    }

    #[tokio::test]
    async fn a_season_says_how_many_of_it_are_left_to_watch() {
        let (database, library_id, root_id) = library().await;
        let viewer = a_viewer(&database).await;
        let (series, seasons, episodes) = a_series(&database, library_id).await;
        for (rank, episode) in episodes.iter().enumerate() {
            a_file_behind(&database, root_id, *episode, &format!("{rank}.mkv")).await;
        }
        watched(&database, viewer, episodes[0]).await;

        let read = database.children_of(viewer, series, "fr").await.expect("read");
        assert_eq!(
            read.iter()
                .map(|season| (season.child_count, state_of(season).unwatched))
                .collect::<Vec<_>>(),
            vec![(2, 1), (1, 1)],
            "the first season holds two and one is left; the second holds one"
        );

        let inside = database
            .children_of(viewer, seasons[0], "fr")
            .await
            .expect("read");
        assert_eq!(
            inside
                .iter()
                .map(|episode| state_of(episode).seen == melyxar_core::work::PlaybackState::Watched)
                .collect::<Vec<_>>(),
            vec![true, false]
        );
    }

    #[tokio::test]
    async fn a_page_answers_for_the_person_looking_at_it_and_nobody_else() {
        // Two accounts watching the same series are two different pages.
        let (database, library_id, root_id) = library().await;
        let viewer = a_viewer(&database).await;
        let other = database
            .create_user(
                "Somebody else",
                None,
                &melyxar_core::user::Permissions::viewer(),
            )
            .await
            .expect("account created")
            .id;
        let (series, _, episodes) = a_series(&database, library_id).await;
        for (rank, episode) in episodes.iter().enumerate() {
            a_file_behind(&database, root_id, *episode, &format!("{rank}.mkv")).await;
        }
        watched(&database, viewer, episodes[0]).await;

        assert_eq!(
            database
                .where_to_resume(viewer, series)
                .await
                .expect("read")
                .map(|found| found.id),
            Some(episodes[1])
        );
        assert_eq!(
            database
                .where_to_resume(other, series)
                .await
                .expect("read")
                .map(|found| found.id),
            Some(episodes[0]),
            "who has watched nothing starts at the first"
        );
    }

    #[tokio::test]
    async fn an_episode_says_whether_anything_of_it_is_on_the_disk() {
        // A page that offers an episode with no file behind it offers a button
        // that fails when it is pressed.
        let (database, library_id, root_id) = library().await;
        let (_, seasons, episodes) = a_series(&database, library_id).await;

        database
            .insert_source(
                episodes[0],
                root_id,
                &PathBuf::from("Distant Signal/Saison 1/one.mkv"),
                12,
                melyxar_core::time::now(),
            )
            .await
            .expect("file written down");

        let viewer = a_viewer(&database).await;
        let read = database
            .children_of(viewer, seasons[0], "fr")
            .await
            .expect("read");
        assert_eq!(
            read.iter()
                .map(|child| source_of(child).is_some())
                .collect::<Vec<_>>(),
            vec![true, false]
        );
    }

    pub(crate) fn video_track(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Video(VideoDetails {
                codec: "hevc".to_string(),
                profile: Some("Main 10".to_string()),
                level: Some(153),
                width: 3840,
                height: 2160,
                margins: None,
                aspect_ratio: Some("16:9".to_string()),
                is_interlaced: false,
                frame_rate: Some(23.976),
                bitrate: Some(45_000_000),
                pixel_format: Some("yuv420p10le".to_string()),
                reference_frames: Some(4),
                color: ColorInfo {
                    primaries: Some("bt2020".to_string()),
                    space: Some("bt2020nc".to_string()),
                    transfer: Some("smpte2084".to_string()),
                    bit_depth: Some(10),
                },
                hdr: Some(HdrFormat::DolbyVision { profile: Some(5) }),
            }),
        }
    }

    pub(crate) async fn work_with_source(
        database: &Database,
        library_id: LibraryId,
        root_id: LibraryRootId,
        relative: &str,
    ) -> (WorkId, MediaSourceId) {
        let work = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        let source = database
            .insert_source(work.id, root_id, Path::new(relative), 1_000, now())
            .await
            .expect("source recorded");
        (work.id, source)
    }

    #[tokio::test]
    async fn the_heaviest_files_come_back_heaviest_first_and_leave_out_what_is_gone() {
        // Which films are the hardest thing this library will ever be asked to
        // play, which is what a server is tested on. Heaviest first, and a
        // file no longer on disk left out: it cannot be played, so it is not a
        // test of anything.
        let (database, library_id, root_id) = library().await;
        let mut recorded = Vec::new();
        for (name, size) in [
            ("Quiet.Harbour.2019.mkv", 3_000),
            ("Amber.Field.2020.mkv", 90_000),
            ("Silent.Coast.2015.mkv", 40_000),
            ("Paper.Lantern.2012.mkv", 70_000),
        ] {
            let work = database
                .create_work(library_id, WorkKind::Movie, name, name, Some(2019))
                .await
                .expect("work created");
            let source = database
                .insert_source(work.id, root_id, Path::new(name), size, now())
                .await
                .expect("source recorded");
            recorded.push((name, source));
        }

        // The second heaviest is gone from the disk, so it drops out entirely.
        let gone = recorded
            .iter()
            .find(|(name, _)| *name == "Paper.Lantern.2012.mkv")
            .expect("it was recorded")
            .1;
        database.mark_source_missing(gone).await.expect("marked");

        database
            .store_analysis(
                recorded[1].1,
                &SourceAnalysis {
                    container: Some("matroska,webm".into()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: Some(80_000_000),
                },
                &[],
                &[],
            )
            .await
            .expect("analysis stored");

        let heaviest = database.heaviest_sources(2).await.expect("read");
        assert_eq!(
            heaviest.len(),
            2,
            "no more than it was asked for: {heaviest:?}"
        );
        assert_eq!(heaviest[0].file_name, "Amber.Field.2020.mkv");
        assert_eq!(heaviest[0].size_bytes, 90_000);
        assert_eq!(heaviest[0].container.as_deref(), Some("matroska,webm"));
        assert_eq!(heaviest[0].duration, Some(Millis::new(7_200_000)));
        assert_eq!(heaviest[0].overall_bitrate, Some(80_000_000));
        assert_eq!(
            heaviest[1].file_name, "Silent.Coast.2015.mkv",
            "the heavier one between them is gone from the disk"
        );
        assert_eq!(
            heaviest[1].container, None,
            "a file nothing has read yet says nothing about itself, rather than failing"
        );
    }

    #[tokio::test]
    async fn a_file_can_be_looked_up_on_its_own_with_the_disk_it_lives_on() {
        // Everything done to one file rather than to a library needs the whole
        // path, and only the root knows where the disk is mounted.
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        let found = database
            .source_by_id(source_id)
            .await
            .expect("read")
            .expect("that file is there");
        assert_eq!(found.id, source_id);
        assert_eq!(found.relative_path, PathBuf::from("Quiet.Harbour.2019.mkv"));
        assert_eq!(found.root_label, "disk-one");
        assert_eq!(found.root_path, PathBuf::from("/mnt/one/Films"));

        assert!(
            database
                .source_by_id(MediaSourceId::new())
                .await
                .expect("read")
                .is_none(),
            "a file nobody has is nothing, not a failure"
        );
    }

    #[tokio::test]
    async fn a_work_starts_out_waiting_to_be_identified() {
        let (database, library_id, _) = library().await;
        let work = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");

        assert_eq!(work.identification, IdentificationState::Pending);
        assert!(work.identification.may_be_looked_up_again());

        let reloaded = database
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(reloaded, work);
        assert_eq!(reloaded.kind, WorkKind::Movie);
        assert!(reloaded.kind.is_playable());
    }

    #[tokio::test]
    async fn a_file_that_is_gone_is_marked_absent_and_comes_back_with_its_history() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        database
            .mark_source_missing(source_id)
            .await
            .expect("marked");
        let stored = database.sources_of_root(root_id).await.expect("read");
        assert_eq!(stored.len(), 1, "a scan never deletes a row");
        assert!(stored[0].missing_since.is_some());

        database
            .mark_source_present(source_id)
            .await
            .expect("marked");
        let back = database.sources_of_root(root_id).await.expect("read");
        assert_eq!(
            back[0].id, source_id,
            "the identifier is what carries the history"
        );
        assert!(back[0].missing_since.is_none());
    }

    #[tokio::test]
    async fn a_replaced_file_drops_the_analysis_of_the_copy_that_is_gone() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &[video_track(source_id)],
                &[],
            )
            .await
            .expect("analysis stored");

        database
            .refresh_source_identity(source_id, 2_000, now())
            .await
            .expect("identity refreshed");

        assert!(
            database
                .tracks_of_source(source_id)
                .await
                .expect("read")
                .is_empty(),
            "serving the streams of one copy while playing another is a defect, not a shortcut"
        );
        let stored = database.sources_of_root(root_id).await.expect("read");
        assert_eq!(stored[0].size_bytes, 2_000);
    }

    #[tokio::test]
    async fn a_work_gives_up_every_file_behind_it() {
        let (database, library_id, root_id) = library().await;
        let work = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");

        for name in [
            "Quiet.Harbour.2019.1080p.mkv",
            "Quiet.Harbour.2019.2160p.mkv",
        ] {
            database
                .insert_source(work.id, root_id, Path::new(name), 1_000, now())
                .await
                .expect("source recorded");
        }

        let sources = database.sources_of_work(work.id).await.expect("read");
        assert_eq!(
            sources.len(),
            2,
            "the same film in two definitions is two files of one work"
        );
    }

    #[tokio::test]
    async fn what_a_file_turned_out_to_be_is_read_back() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        let (before, analysed_at) = database
            .source_details(source_id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(before, SourceAnalysis::default());
        assert!(analysed_at.is_none(), "nothing has looked at it yet");

        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: Some(48_000_000),
                },
                &[],
                &[],
            )
            .await
            .expect("analysis stored");

        let (after, analysed_at) = database
            .source_details(source_id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(after.container.as_deref(), Some("matroska,webm"));
        assert_eq!(after.duration, Some(Millis::new(7_200_000)));
        assert!(analysed_at.is_some());
    }

    #[tokio::test]
    async fn a_second_copy_of_one_film_finds_the_work_that_is_already_there() {
        let (database, library_id, _) = library().await;
        database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");

        assert!(database
            .work_by_identity(library_id, "quiet harbour", Some(2019))
            .await
            .expect("read")
            .is_some());
        assert!(
            database
                .work_by_identity(library_id, "quiet harbour", Some(2024))
                .await
                .expect("read")
                .is_none(),
            "a remake is another film, not another copy"
        );
        assert!(database
            .work_by_identity(library_id, "quiet harbour", None)
            .await
            .expect("read")
            .is_none());
    }

    #[tokio::test]
    async fn a_film_whose_name_carries_no_year_is_still_found_back() {
        let (database, library_id, _) = library().await;
        database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Amber Field",
                "amber field",
                None,
            )
            .await
            .expect("work created");

        assert!(database
            .work_by_identity(library_id, "amber field", None)
            .await
            .expect("read")
            .is_some());
    }

    #[tokio::test]
    async fn stored_files_come_back_in_path_order_so_two_runs_can_be_compared() {
        let (database, library_id, root_id) = library().await;
        for name in ["c.mkv", "a.mkv", "b.mkv"] {
            work_with_source(&database, library_id, root_id, name).await;
        }
        let stored = database.sources_of_root(root_id).await.expect("read");
        let paths: Vec<String> = stored
            .iter()
            .map(|source| source.relative_path.to_string_lossy().into_owned())
            .collect();
        assert_eq!(paths, vec!["a.mkv", "b.mkv", "c.mkv"]);
    }

    #[tokio::test]
    async fn two_works_that_turn_out_to_be_one_film_become_one_work_with_two_files() {
        let (database, library_id, root_id) = library().await;
        let (kept, _) =
            work_with_source(&database, library_id, root_id, "Quiet Harbour 1080p.mkv").await;
        let (gone, moved) = work_with_source(
            &database,
            library_id,
            root_id,
            "zz12Quiet Harbour 1080p.mkv",
        )
        .await;
        database
            .store_local_extra_video(
                gone,
                &LocalExtraVideo {
                    kind: "trailer".to_string(),
                    name: None,
                    root_id,
                    relative_path: PathBuf::from("zz12Quiet Harbour 1080p-trailer.mkv"),
                },
            )
            .await
            .expect("trailer attached");

        database
            .merge_work_into(gone, kept)
            .await
            .expect("the two are one film");

        assert!(
            database.work(gone).await.expect("read").is_none(),
            "the same film twice in a grid is the defect this exists to avoid"
        );
        let sources = database.sources_of_root(root_id).await.expect("read");
        assert_eq!(sources.len(), 2, "no file is lost in the move");
        assert!(
            sources.iter().all(|source| source.work_id == kept),
            "both copies belong to the film that stayed"
        );
        assert!(
            sources.iter().any(|source| source.id == moved),
            "a file keeps its identifier, and with it everything attached to it"
        );
        assert_eq!(
            database
                .extra_videos_of_work(kept)
                .await
                .expect("read")
                .len(),
            1,
            "what was attached to the copy follows it"
        );
    }

    #[tokio::test]
    async fn the_pictures_of_a_work_that_goes_are_named_so_their_files_can_go_too() {
        // Pictures are found by owner rather than by a key the engine knows
        // about, so dropping a work leaves its rows and its files behind.
        let (database, library_id, root_id) = library().await;
        let (kept, _) =
            work_with_source(&database, library_id, root_id, "Quiet Harbour 1080p.mkv").await;
        let (gone, _) = work_with_source(
            &database,
            library_id,
            root_id,
            "zz12Quiet Harbour 1080p.mkv",
        )
        .await;
        database
            .replace_images(
                "work",
                &gone.to_db_string(),
                "poster",
                &[crate::images::StoredImage {
                    owner_kind: "work".to_string(),
                    owner_id: gone.to_db_string(),
                    image_kind: "poster".to_string(),
                    relative_path: format!("works/{gone}/poster-abc-200.webp"),
                    width: Some(200),
                    height: Some(300),
                    fingerprint: "abc".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("poster stored");

        let no_longer_used = database
            .merge_work_into(gone, kept)
            .await
            .expect("the two are one film");

        assert_eq!(no_longer_used.len(), 1);
        assert!(no_longer_used[0].contains("poster-abc-200"));
        assert!(
            database
                .images_of("work", &gone.to_db_string())
                .await
                .expect("read")
                .is_empty(),
            "a row pointing at a work that no longer exists is a row nobody will ever clear"
        );
    }

    #[tokio::test]
    async fn a_series_joining_another_brings_its_seasons_its_episodes_and_their_files() {
        // The same series on two disks, its folder written one way on one and
        // another way on the other. Dropping the one that goes while it still
        // held its seasons would take every episode and every file under it
        // down with it, and the library would quietly lose a whole run.
        let (database, library_id, root_id) = library().await;
        let kept = a_series_on_disk(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1, 2])],
        )
        .await;
        let gone = a_series_on_disk(
            &database,
            library_id,
            root_id,
            "Signal Lointain",
            &[(1, &[2, 3]), (2, &[1])],
        )
        .await;

        database
            .merge_work_into(gone, kept)
            .await
            .expect("the two are one series");

        assert!(
            database.work(gone).await.expect("read").is_none(),
            "the same series twice in a grid is the defect this exists to avoid"
        );
        assert_eq!(
            database.sources_of_root(root_id).await.expect("read").len(),
            5,
            "no episode loses its file when its series joins another"
        );

        let viewer = a_viewer(&database).await;
        let seasons = database.children_of(viewer, kept, "fr").await.expect("read");
        assert_eq!(
            seasons
                .iter()
                .map(|season| (season.ordinal, season.child_count))
                .collect::<Vec<_>>(),
            vec![(Some(1), 3), (Some(2), 1)],
            "the episodes joined the season they share, and the season nobody had changed parent"
        );

        let first = database
            .child_by_ordinal(kept, 1)
            .await
            .expect("read")
            .expect("season one stayed");
        let held_twice = database
            .child_by_ordinal(first.id, 2)
            .await
            .expect("read")
            .expect("the episode both sides had");
        assert_eq!(
            database
                .sources_of_work(held_twice.id)
                .await
                .expect("read")
                .len(),
            2,
            "one episode on two disks is one episode with two files"
        );
    }

    #[tokio::test]
    async fn the_name_a_series_that_goes_was_filed_under_leads_to_the_one_that_stays() {
        // Otherwise the next file dropped in that folder writes the series
        // down again, and the two have to be put together all over again.
        let (database, library_id, root_id) = library().await;
        let kept = a_series_on_disk(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1])],
        )
        .await;
        let gone = a_series_on_disk(
            &database,
            library_id,
            root_id,
            "Signal Lointain",
            &[(1, &[1])],
        )
        .await;

        database
            .merge_work_into(gone, kept)
            .await
            .expect("the two are one series");

        assert_eq!(
            database
                .series_by_name(library_id, "signal lointain", Some(2019))
                .await
                .expect("read")
                .expect("the folder of the one that went leads to the one that stayed")
                .id,
            kept
        );
    }

    #[tokio::test]
    async fn the_pictures_of_an_episode_that_goes_are_named_so_their_files_can_go_too() {
        let (database, library_id, root_id) = library().await;
        let kept = a_series_on_disk(
            &database,
            library_id,
            root_id,
            "Distant Signal",
            &[(1, &[1])],
        )
        .await;
        let gone = a_series_on_disk(
            &database,
            library_id,
            root_id,
            "Signal Lointain",
            &[(1, &[1])],
        )
        .await;
        let season = database
            .child_by_ordinal(gone, 1)
            .await
            .expect("read")
            .expect("season one");
        let episode = database
            .child_by_ordinal(season.id, 1)
            .await
            .expect("read")
            .expect("episode one");
        database
            .replace_images(
                "work",
                &episode.id.to_db_string(),
                "thumb",
                &[crate::images::StoredImage {
                    owner_kind: "work".to_string(),
                    owner_id: episode.id.to_db_string(),
                    image_kind: "thumb".to_string(),
                    relative_path: format!("works/{}/thumb-abc-200.webp", episode.id),
                    width: Some(200),
                    height: Some(300),
                    fingerprint: "abc".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("picture stored");

        let no_longer_used = database
            .merge_work_into(gone, kept)
            .await
            .expect("the two are one series");

        assert_eq!(
            no_longer_used.len(),
            1,
            "a picture two levels down is still a file nobody else will ever clear"
        );
        assert!(no_longer_used[0].contains("thumb-abc-200"));
    }

    #[tokio::test]
    async fn a_film_and_a_series_the_provider_numbers_alike_are_not_put_together() {
        // A provider numbers its films and its series on separate counters, so
        // the same number names one of each.
        let (database, library_id, root_id) = library().await;
        let (film, _) =
            work_with_source(&database, library_id, root_id, "Quiet Harbour 1080p.mkv").await;
        let series = a_series_on_disk(
            &database,
            library_id,
            root_id,
            "Quiet Harbour",
            &[(1, &[1])],
        )
        .await;
        for work_id in [film, series] {
            database
                .set_work_external_id(work_id, "tmdb", "111")
                .await
                .expect("identifier written");
        }

        assert!(database
            .works_sharing_an_identity(library_id, "tmdb")
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn works_the_provider_gives_one_identifier_are_listed_together() {
        let (database, library_id, root_id) = library().await;
        let (first, _) =
            work_with_source(&database, library_id, root_id, "Quiet Harbour 1080p.mkv").await;
        let (second, _) =
            work_with_source(&database, library_id, root_id, "Port Tranquille 1080p.mkv").await;
        let (apart, _) =
            work_with_source(&database, library_id, root_id, "Amber Field 1080p.mkv").await;

        for (work_id, external_id) in [(first, "111"), (second, "111"), (apart, "222")] {
            database
                .set_work_external_id(work_id, "tmdb", external_id)
                .await
                .expect("identifier written");
        }
        // Another provider naming the same film is not a second grouping.
        database
            .set_work_external_id(first, "imdb", "tt111")
            .await
            .expect("identifier written");

        let shared = database
            .works_sharing_an_identity(library_id, "tmdb")
            .await
            .expect("read");
        assert_eq!(shared.len(), 1, "one film is here twice, and one once");
        assert_eq!(
            shared[0].keep, first,
            "the one that has been here longest is the one the others join"
        );
        assert_eq!(shared[0].others, vec![second]);
    }

    #[tokio::test]
    async fn a_film_that_is_here_once_is_not_listed_as_sharing_anything() {
        let (database, library_id, root_id) = library().await;
        let (work_id, _) =
            work_with_source(&database, library_id, root_id, "Quiet Harbour 1080p.mkv").await;
        database
            .set_work_external_id(work_id, "tmdb", "111")
            .await
            .expect("identifier written");

        assert!(database
            .works_sharing_an_identity(library_id, "tmdb")
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn a_copy_that_is_another_film_can_be_taken_away_as_one() {
        // The other half of putting copies together. What acts on its own has
        // to be undoable by hand, or a grouping that got it wrong costs a film
        // nobody can get back.
        let (database, library_id, root_id) = library().await;
        let (held, _) =
            work_with_source(&database, library_id, root_id, "Quiet Harbour 1080p.mkv").await;
        let leaving = database
            .insert_source(
                held,
                root_id,
                Path::new("Amber Field 1080p.mkv"),
                2_000,
                now(),
            )
            .await
            .expect("source recorded");

        let detached = database
            .detach_source(
                leaving,
                WorkKind::Movie,
                "Amber Field",
                "amber field",
                Some(2020),
            )
            .await
            .expect("read")
            .expect("a film held twice can give one of them up");

        assert_eq!(detached.title, "Amber Field");
        assert_eq!(detached.library_id, library_id);
        assert_eq!(
            detached.identification,
            IdentificationState::Pending,
            "it waits to be looked up like any film a scan has just found"
        );
        assert_eq!(
            database
                .sources_of_work(detached.id)
                .await
                .expect("read")
                .len(),
            1
        );
        assert_eq!(
            database.sources_of_work(held).await.expect("read").len(),
            1,
            "the film it left keeps the copy that really is it"
        );
    }

    #[tokio::test]
    async fn the_only_copy_of_a_film_is_never_taken_away_from_it() {
        // There would be nothing to take it away from, and the film it left
        // would be a card with no file behind it.
        let (database, library_id, root_id) = library().await;
        let (_, only_copy) =
            work_with_source(&database, library_id, root_id, "Quiet Harbour 1080p.mkv").await;

        assert!(database
            .detach_source(
                only_copy,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019)
            )
            .await
            .expect("read")
            .is_none());
        assert_eq!(
            database.count_works(library_id).await.expect("read"),
            1,
            "and no empty film is left behind by the attempt"
        );
    }

    #[tokio::test]
    async fn a_film_is_never_merged_into_itself() {
        let (database, library_id, root_id) = library().await;
        let (work_id, _) =
            work_with_source(&database, library_id, root_id, "Quiet Harbour 1080p.mkv").await;

        database
            .merge_work_into(work_id, work_id)
            .await
            .expect("nothing to do");

        assert!(database.work(work_id).await.expect("read").is_some());
        assert_eq!(
            database.sources_of_root(root_id).await.expect("read").len(),
            1
        );
    }

    #[tokio::test]
    async fn a_trailer_next_to_a_film_is_attached_to_it_without_piling_up() {
        let (database, library_id, root_id) = library().await;
        let (work_id, _) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        let extra = LocalExtraVideo {
            kind: "trailer".to_string(),
            name: None,
            root_id,
            relative_path: PathBuf::from("Quiet.Harbour.2019-trailer.mkv"),
        };
        for _ in 0..2 {
            database
                .store_local_extra_video(work_id, &extra)
                .await
                .expect("trailer attached");
        }

        let stored = database.extra_videos_of_work(work_id).await.expect("read");
        assert_eq!(
            stored,
            vec![PlayableExtraVideo {
                kind: "trailer".to_string(),
                name: None,
                // Read back with its root joined on: the database holds the
                // path relative to the root so a disk can be mounted somewhere
                // else, and nothing can be opened until the two are put back
                // together.
                path: PathBuf::from("/mnt/one/Films/Quiet.Harbour.2019-trailer.mkv"),
            }]
        );
    }

    #[tokio::test]
    async fn a_work_holds_one_identifier_per_provider_and_the_latest_wins() {
        let (database, library_id, _) = library().await;
        let work = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");

        database
            .set_work_external_id(work.id, "tmdb", "12345")
            .await
            .expect("identifier stored");
        database
            .set_work_external_id(work.id, "imdb", "tt7654321")
            .await
            .expect("identifier stored");
        database
            .set_work_external_id(work.id, "tmdb", "54321")
            .await
            .expect("identifier corrected");

        assert_eq!(
            database.work_external_ids(work.id).await.expect("read"),
            vec![
                ("imdb".to_string(), "tt7654321".to_string()),
                ("tmdb".to_string(), "54321".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn the_summary_says_what_the_library_holds_and_what_is_missing() {
        let (database, library_id, root_id) = library().await;
        let (_, first) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        work_with_source(&database, library_id, root_id, "Amber.Field.2020.mkv").await;
        database.mark_source_missing(first).await.expect("marked");

        let summary = database.catalogue_summary().await.expect("read");
        assert_eq!(summary.works, 2);
        assert_eq!(summary.files, 2);
        assert_eq!(summary.missing_files, 1);
        assert_eq!(summary.identified, 0);
        assert_eq!(summary.awaiting_identification, 2);
    }

    #[tokio::test]
    async fn a_summary_of_nothing_is_a_row_of_zeros_rather_than_a_failure() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(
            database.catalogue_summary().await.expect("read"),
            CatalogueSummary::default()
        );
    }

    #[tokio::test]
    async fn works_are_counted_and_listed_newest_first() {
        let (database, library_id, root_id) = library().await;
        for name in ["a.mkv", "b.mkv", "c.mkv"] {
            work_with_source(&database, library_id, root_id, name).await;
        }
        assert_eq!(database.count_works(library_id).await.expect("read"), 3);

        let recent = database.recent_works(library_id, 2).await.expect("read");
        assert_eq!(recent.len(), 2);
        assert!(recent[0].added_at >= recent[1].added_at);
    }

    #[tokio::test]
    async fn a_malformed_stored_kind_is_reported_rather_than_guessed() {
        let (database, library_id, _) = library().await;
        let work = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                None,
            )
            .await
            .expect("work created");

        sqlx::query("UPDATE works SET kind = 'something_new' WHERE id = ?")
            .bind(work.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("value forced");

        assert!(matches!(
            database.work(work.id).await,
            Err(DatabaseError::Corrupt(_))
        ));
    }

    #[tokio::test]
    async fn removing_a_work_takes_its_files_and_streams_with_it() {
        let (database, library_id, root_id) = library().await;
        let (work_id, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        database
            .store_analysis(
                source_id,
                &SourceAnalysis::default(),
                &[video_track(source_id)],
                &[],
            )
            .await
            .expect("analysis stored");

        database
            .delete_works(&[work_id], false)
            .await
            .expect("work removed");

        assert!(database
            .sources_of_root(root_id)
            .await
            .expect("read")
            .is_empty());
        assert!(database
            .tracks_of_source(source_id)
            .await
            .expect("read")
            .is_empty());
    }

}
