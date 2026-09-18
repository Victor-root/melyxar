//! Works, the files behind them, and what an analysis found inside.
//!
//! Two rules shape everything here. A work outlives the file that revealed it,
//! so replacing a copy with a better one keeps the watch history. And a file
//! that is no longer on disk is marked absent rather than deleted, so that an
//! unplugged disk is a bad evening rather than a lost library.

use std::path::{Path, PathBuf};

use melyxar_core::id::{
    ChapterId, ExtraVideoId, LibraryId, LibraryRootId, MediaSourceId, UserId, WorkId,
};
use melyxar_core::media::{
    AudioDetails, Chapter, ColorInfo, HdrFormat, Loudness, Margins, SubtitleDetails,
    SubtitleLayout, Track, TrackKind, VideoDetails,
};
use melyxar_core::thumbnails::{Layout, Thumbnails};
use melyxar_core::time::{now, Millis, Timestamp};
use melyxar_core::work::{IdentificationNote, IdentificationState, Work, WorkKind};
use sqlx::{AssertSqlSafe, Row, Sqlite};

use crate::convert::{
    bool_to_int, int_to_bool, parse_id, parse_optional_timestamp, parse_timestamp,
    timestamp_to_text,
};
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
const IN_THE_ONE_ORDER: &str = "ORDER BY ordinal, sort_title";

/// One work hanging under another, stripped to where it sits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RankedChild {
    pub id: WorkId,
    pub ordinal: Option<i32>,
    pub title: String,
}

/// One work hanging under another, with what its card shows.
#[derive(Debug, Clone, PartialEq)]
pub struct ChildWork {
    pub id: WorkId,
    pub kind: WorkKind,
    /// The season number, the episode number.
    pub ordinal: Option<i32>,
    pub title: String,
    /// How long it runs: what the provider said, or failing that the longest
    /// copy on disk, so an episode nobody has looked up still says something.
    pub runtime: Option<Millis>,
    /// How many hang under this one, for a season saying how many episodes.
    pub child_count: i64,
    /// Whether a file of it is on the disk right now.
    pub playable: bool,
    /// Episodes under this one this viewer has not watched. Zero for an
    /// episode, which holds none.
    pub unwatched: i64,
    /// Whether this viewer has watched this one. Only ever true of an episode.
    pub watched: bool,
    /// Where this viewer stopped in it, when they stopped partway.
    pub resume_from: Option<Millis>,
    pub identification: IdentificationState,
    pub dominant_color: Option<String>,
    pub added_at: Timestamp,
}

/// Works one provider says are the same film.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedIdentity {
    /// The one that has been here longest, which the others join.
    pub keep: WorkId,
    pub others: Vec<WorkId>,
}

/// How much of a refusal is worth keeping.
///
/// Long enough to carry what the tool named and where, short enough that a
/// report stays a report.
const LONGEST_FAILURE_REASON: usize = 400;

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
    pub async fn create_work(
        &self,
        library_id: LibraryId,
        kind: WorkKind,
        title: &str,
        sort_title: &str,
        release_year: Option<i32>,
    ) -> Result<Work> {
        insert_work(
            self.writer(),
            Placed::on_its_own(library_id),
            kind,
            title,
            sort_title,
            release_year,
        )
        .await
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
        sqlx::query(
            "UPDATE works
                SET child_count = (SELECT count(*) FROM works AS child WHERE child.parent_id = works.id)
              WHERE id = ?",
        )
        .bind(parent_id.to_db_string())
        .execute(&mut *transaction)
        .await?;
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
    /// A series answers with its seasons, a season with its episodes. Read in
    /// one go, with everything each card shows: whether anything can be
    /// played, how long it runs, how many the one below holds, how many of
    /// those this viewer has left to watch, and where they stopped. A page of
    /// twenty four episodes that asked those questions per episode would be a
    /// hundred and twenty round trips.
    pub async fn children_of(&self, viewer: UserId, parent_id: WorkId) -> Result<Vec<ChildWork>> {
        // Ordered the one way children are ever ordered, named below so the
        // shelf read and this one cannot come back in different orders.
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT w.id, w.kind, w.ordinal, w.title, w.runtime_ms, w.child_count,
                    w.identification, w.dominant_color, w.added_at,
                    (SELECT count(*) FROM media_sources s
                      WHERE s.work_id = w.id AND s.missing_since IS NULL) AS playable,
                    (SELECT max(s.duration_ms) FROM media_sources s WHERE s.work_id = w.id)
                        AS longest_ms,
                    -- What is left to watch under this one. A season answers
                    -- for its episodes; an episode has nothing under it and
                    -- answers nothing.
                    (SELECT count(*) FROM works c
                      LEFT JOIN playback_progress q
                             ON q.work_id = c.id AND q.user_id = ?1
                      WHERE c.parent_id = w.id
                        AND coalesce(q.state, 'not_started') <> 'watched') AS unwatched,
                    coalesce(p.state, 'not_started') AS seen,
                    p.position_ms
             FROM works w
             LEFT JOIN playback_progress p ON p.work_id = w.id AND p.user_id = ?1
             WHERE w.parent_id = ?2
             {IN_THE_ONE_ORDER}"
        )))
        .bind(viewer.to_db_string())
        .bind(parent_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                let kind_text: String = row.try_get("kind")?;
                let identification_text: String = row.try_get("identification")?;
                Ok(ChildWork {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    kind: WorkKind::parse(&kind_text).ok_or_else(|| {
                        DatabaseError::Corrupt(format!("work kind '{kind_text}'"))
                    })?,
                    ordinal: row.try_get("ordinal")?,
                    title: row.try_get("title")?,
                    runtime: row
                        .try_get::<Option<i64>, _>("runtime_ms")?
                        .or(row.try_get::<Option<i64>, _>("longest_ms")?)
                        .map(Millis::new),
                    child_count: row.try_get("child_count")?,
                    playable: row.try_get::<i64, _>("playable")? > 0,
                    unwatched: row.try_get("unwatched")?,
                    watched: row.try_get::<String, _>("seen")? == "watched",
                    // Only where somebody really stopped partway: a position
                    // of nothing is where everybody starts, and a button
                    // offering to carry on from the very beginning is a button
                    // saying the wrong thing.
                    resume_from: row
                        .try_get::<Option<i64>, _>("position_ms")?
                        .filter(|position| *position > 0)
                        .map(Millis::new),
                    identification: IdentificationState::parse(&identification_text).ok_or_else(
                        || DatabaseError::Corrupt(format!("state '{identification_text}'")),
                    )?,
                    dominant_color: row.try_get("dominant_color")?,
                    added_at: parse_timestamp(&row.try_get::<String, _>("added_at")?)?,
                })
            })
            .collect()
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

    /// Where a viewer would pick a series back up: the first episode of it they
    /// have not watched.
    ///
    /// Read in order rather than from the last one played, because a series
    /// watched out of order has a hole in it and the hole is what somebody
    /// means by where they are. Nothing when every episode here has been
    /// watched, which is a series to start again rather than to carry on.
    pub async fn where_to_resume(&self, viewer: UserId, series_id: WorkId) -> Result<Option<Work>> {
        self.an_episode_of(viewer, series_id, None, true).await
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

    /// One episode of a series, read in the order they are watched in.
    ///
    /// Written once because the two questions above are the same question
    /// asked with different bounds: the next one after a place, and the first
    /// one nobody has watched. Both skip an episode with no file behind it,
    /// because both end in a button that has to play something.
    async fn an_episode_of(
        &self,
        viewer: UserId,
        series_id: WorkId,
        after: Option<(i32, i32)>,
        only_unwatched: bool,
    ) -> Result<Option<Work>> {
        let (season, episode) = after.unwrap_or((0, 0));
        let row = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM works e
             JOIN works s ON s.id = e.parent_id
             LEFT JOIN playback_progress p ON p.work_id = e.id AND p.user_id = ?
             WHERE s.parent_id = ? AND e.kind = 'episode'
               AND EXISTS (SELECT 1 FROM media_sources m
                            WHERE m.work_id = e.id AND m.missing_since IS NULL)
               AND (?3 = 0 OR (s.ordinal, e.ordinal) > (?4, ?5))
               AND (?6 = 0 OR coalesce(p.state, 'not_started') <> 'watched')
             ORDER BY s.ordinal, e.ordinal
             LIMIT 1",
            what_a_work_is("e.")
        )))
        .bind(viewer.to_db_string())
        .bind(series_id.to_db_string())
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

    async fn work_named(
        &self,
        library_id: LibraryId,
        sort_title: &str,
        release_year: Option<i32>,
        kind: Option<WorkKind>,
    ) -> Result<Option<Work>> {
        let row = sqlx::query(AssertSqlSafe(format!(
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

        row.map(|row| work_from_row(&row)).transpose()
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
                    sum(identification IN ('identified', 'manual')),
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

    /// Removes a work along with everything hanging from it.
    ///
    /// Only ever called by the caller that created a work and then found no
    /// file to attach to it. A scan never reaches for this.
    pub async fn delete_work(&self, id: WorkId) -> Result<()> {
        sqlx::query("DELETE FROM works WHERE id = ?")
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Every file recorded under one root, in path order.
    ///
    /// This is the side of the comparison a scan starts from, so it stays as
    /// small as the comparison needs.
    pub async fn sources_of_root(&self, root_id: LibraryRootId) -> Result<Vec<StoredSource>> {
        let rows = sqlx::query(
            "SELECT s.id, s.work_id, s.relative_path, s.size_bytes, s.modified_at,
                    s.missing_since, s.added_at, r.label AS root_label, r.path AS root_path
             FROM media_sources s
             JOIN library_roots r ON r.id = s.root_id
             WHERE s.root_id = ? ORDER BY s.relative_path",
        )
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
        let rows = sqlx::query(
            "SELECT s.id, s.work_id, s.relative_path, s.size_bytes, s.modified_at,
                    s.missing_since, s.added_at, r.label AS root_label, r.path AS root_path
             FROM media_sources s
             JOIN library_roots r ON r.id = s.root_id
             WHERE s.work_id = ? ORDER BY s.added_at",
        )
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
        let row = sqlx::query(
            "SELECT s.id, s.work_id, s.relative_path, s.size_bytes, s.modified_at,
                    s.missing_since, s.added_at, r.label AS root_label, r.path AS root_path
             FROM media_sources s
             JOIN library_roots r ON r.id = s.root_id
             WHERE s.id = ?",
        )
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
        let rows = sqlx::query(
            "SELECT s.id, s.work_id, s.relative_path, s.size_bytes, s.modified_at,
                    s.missing_since, s.added_at, r.label AS root_label, r.path AS root_path
             FROM media_sources s
             JOIN library_roots r ON r.id = s.root_id
             WHERE s.root_id = ? AND s.analysed_at IS NULL AND s.missing_since IS NULL
             ORDER BY s.relative_path",
        )
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

    /// Moves every file of one work onto another and drops the empty one.
    ///
    /// Two works turn out to be one film whenever the rules that read file
    /// names improve: several copies whose names differed only by something a
    /// tool stuck on the front now read as the same title. Left alone they are
    /// the same film several times over in a grid, which is the very thing a
    /// version chooser exists to avoid.
    ///
    /// Only the files and the clips attached to them are carried over. The one
    /// that goes describes the same film as the one that stays, by name, by
    /// picture and by cast, so there is nothing there worth keeping twice.
    ///
    /// The paths of the pictures it had come back, so their files can be
    /// removed from the cache by the caller that put them there. Nothing else
    /// points at those rows, so without this they would sit there for ever.
    pub async fn merge_work_into(&self, from: WorkId, into: WorkId) -> Result<Vec<String>> {
        if from == into {
            return Ok(Vec::new());
        }
        let mut transaction = self.begin().await?;
        sqlx::query("UPDATE media_sources SET work_id = ? WHERE work_id = ?")
            .bind(into.to_db_string())
            .bind(from.to_db_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query("UPDATE extra_videos SET work_id = ? WHERE work_id = ?")
            .bind(into.to_db_string())
            .bind(from.to_db_string())
            .execute(&mut *transaction)
            .await?;

        let no_longer_used: Vec<String> = sqlx::query(
            "SELECT relative_path FROM images WHERE owner_kind = 'work' AND owner_id = ?",
        )
        .bind(from.to_db_string())
        .fetch_all(&mut *transaction)
        .await?
        .iter()
        .map(|row| row.try_get::<String, _>("relative_path"))
        .collect::<std::result::Result<_, _>>()?;
        // Pictures are found by owner rather than by a key the engine knows
        // about, so dropping the work does not drop them.
        sqlx::query("DELETE FROM images WHERE owner_kind = 'work' AND owner_id = ?")
            .bind(from.to_db_string())
            .execute(&mut *transaction)
            .await?;

        sqlx::query("DELETE FROM works WHERE id = ?")
            .bind(from.to_db_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(no_longer_used)
    }

    /// Works of one library the provider says are one and the same film.
    ///
    /// Only works met on their own. Joining is about two files of one film,
    /// and a season is never a copy of anything. It also has to be that way:
    /// a provider numbers its films, its series, its seasons and its episodes
    /// on separate counters, so the twelfth season and the twelfth film carry
    /// the same number, and a library holding both would see one work with two
    /// files where there are two works with one each.
    ///
    /// Two copies can carry names nothing could ever match, and be the same
    /// film: only the provider can say so, and it says so by answering the
    /// same identifier for both. Left apart they are the same title, the same
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
            "SELECT e.external_id, e.work_id
             FROM work_external_ids e
             JOIN works w ON w.id = e.work_id
             WHERE w.library_id = ? AND e.provider = ?
               AND w.parent_id IS NULL
             ORDER BY e.external_id, w.added_at",
        )
        .bind(library_id.to_db_string())
        .bind(provider)
        .fetch_all(self.reader())
        .await?;

        let mut groups: Vec<SharedIdentity> = Vec::new();
        let mut current: Option<String> = None;
        for row in &rows {
            let external_id: String = row.try_get("external_id")?;
            let work_id: WorkId = row
                .try_get::<String, _>("work_id")?
                .parse()
                .map_err(|_| DatabaseError::Corrupt("work identifier".to_string()))?;

            // The first of each group is the one that has been here longest,
            // which is the one the others join.
            match current.as_deref() {
                Some(seen) if seen == external_id => {
                    groups
                        .last_mut()
                        .expect("a group was started with this identifier")
                        .others
                        .push(work_id);
                }
                _ => {
                    current = Some(external_id);
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

    /// Writes down why the analyser could not describe a file.
    ///
    /// Kept rather than logged: a reason in a log line is gone by the time
    /// anybody asks, and this is the one thing that says what to do about a
    /// file that has a card in the library and fails when it is played.
    ///
    /// Cut to a length a report can show. What the tool says first is what
    /// says why; the rest is the same complaint again.
    pub async fn record_analysis_failure(
        &self,
        source_id: MediaSourceId,
        reason: &str,
    ) -> Result<()> {
        let reason: String = reason.chars().take(LONGEST_FAILURE_REASON).collect();
        sqlx::query("UPDATE media_sources SET analysis_failure = ? WHERE id = ?")
            .bind(reason)
            .bind(source_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Forgets what the analyser found, so the next scan reads every file
    /// again.
    ///
    /// An analysis is kept once it is done, which is what makes a second scan
    /// cost almost nothing. But what the analyser is asked to read grows: a
    /// collection analysed by an older build carries the gaps that build left,
    /// and nothing would ever look at those files again. This is how somebody
    /// asks for them to be read once more, and it costs one scan.
    ///
    /// The files themselves are untouched, and so is everything attached to
    /// them: only what was read out of them goes.
    ///
    /// All of it goes, not merely the mark saying it was read. A file left
    /// with the container an older reading found and no streams to go with it
    /// describes itself as something it was never shown to be, and whatever
    /// reads that afterwards believes it.
    pub async fn forget_analysis(&self, library_id: LibraryId) -> Result<u64> {
        let done = sqlx::query(
            "UPDATE media_sources
             SET analysed_at = NULL, analysis_failure = NULL, container = NULL,
                 duration_ms = NULL, overall_bitrate = NULL
             WHERE root_id IN (SELECT id FROM library_roots WHERE library_id = ?)",
        )
        .bind(library_id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(done.rows_affected())
    }

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
        let rows = sqlx::query(
            "SELECT media_sources.id
             FROM media_sources
             JOIN library_roots ON library_roots.id = media_sources.root_id
             LEFT JOIN media_source_key_frames
                    ON media_source_key_frames.source_id = media_sources.id
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_source_key_frames.source_id IS NULL
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
        let row: (i64,) = sqlx::query_as(
            "SELECT count(*)
             FROM media_sources
             JOIN library_roots ON library_roots.id = media_sources.root_id
             LEFT JOIN media_source_key_frames
                    ON media_source_key_frames.source_id = media_sources.id
             WHERE library_roots.library_id = ?
               AND media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_source_key_frames.source_id IS NULL",
        )
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
        let rows = sqlx::query(
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
             ORDER BY media_sources.added_at
             LIMIT ?",
        )
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
        let row: (i64,) = sqlx::query_as(
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
               AND media_source_thumbnails.source_id IS NULL",
        )
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

    /// Stores what the analysis found: the file as a whole, its streams and
    /// its chapters, in one transaction so a reader never sees half of it.
    pub async fn store_analysis(
        &self,
        source_id: MediaSourceId,
        analysis: &SourceAnalysis,
        tracks: &[Track],
        chapters: &[Chapter],
    ) -> Result<()> {
        let mut transaction = self.begin().await?;

        sqlx::query(
            "UPDATE media_sources
             SET container = ?, duration_ms = ?, overall_bitrate = ?, analysed_at = ?,
                 analysis_failure = NULL
             WHERE id = ?",
        )
        .bind(analysis.container.as_deref())
        .bind(analysis.duration.map(Millis::get))
        .bind(analysis.overall_bitrate)
        .bind(timestamp_to_text(now()))
        .bind(source_id.to_db_string())
        .execute(&mut *transaction)
        .await?;

        // An analysis owns the streams inside the file and nothing else: the
        // subtitles that live in their own files are not its to replace.
        sqlx::query("DELETE FROM tracks WHERE source_id = ? AND is_external = 0")
            .bind(source_id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM chapters WHERE source_id = ?")
            .bind(source_id.to_db_string())
            .execute(&mut *transaction)
            .await?;

        for track in tracks {
            insert_track(&mut transaction, source_id, track).await?;
        }
        for chapter in chapters {
            sqlx::query(
                "INSERT INTO chapters (id, source_id, ordinal, start_ms, title, thumbnail_path)
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(ChapterId::new().to_db_string())
            .bind(source_id.to_db_string())
            .bind(chapter.ordinal)
            .bind(chapter.start.get())
            .bind(chapter.title.as_deref())
            .bind(
                chapter
                    .thumbnail_path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
            )
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
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

    /// Files of one root that already carry a subtitle of their own.
    ///
    /// A scan asks for this so that it only rewrites what actually changed:
    /// without it, either a removed subtitle would linger for ever or every
    /// file in the library would be written to on every scan.
    pub async fn sources_with_external_subtitles(
        &self,
        root_id: LibraryRootId,
    ) -> Result<Vec<MediaSourceId>> {
        let rows = sqlx::query(
            "SELECT DISTINCT tracks.source_id FROM tracks
             JOIN media_sources ON media_sources.id = tracks.source_id
             WHERE media_sources.root_id = ? AND tracks.is_external = 1",
        )
        .bind(root_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| parse_id(&row.try_get::<String, _>("source_id")?))
            .collect()
    }

    /// Replaces the subtitles that live in their own files next to a source.
    ///
    /// Kept apart from the analysis, which owns the streams inside the file:
    /// each writes what it knows about and leaves the rest alone, so a scan
    /// and an analysis can happen in either order.
    pub async fn store_external_subtitles(
        &self,
        source_id: MediaSourceId,
        tracks: &[Track],
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query("DELETE FROM tracks WHERE source_id = ? AND is_external = 1")
            .bind(source_id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        for track in tracks {
            insert_track(&mut transaction, source_id, track).await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Streams of one source, video first, then audio, then subtitles.
    pub async fn tracks_of_source(&self, source_id: MediaSourceId) -> Result<Vec<Track>> {
        let rows = sqlx::query(
            "SELECT * FROM tracks WHERE source_id = ?
             ORDER BY CASE kind WHEN 'video' THEN 0 WHEN 'audio' THEN 1 ELSE 2 END, stream_index",
        )
        .bind(source_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter().map(track_from_row).collect()
    }

    /// Chapters of one source, in order.
    pub async fn chapters_of_source(&self, source_id: MediaSourceId) -> Result<Vec<Chapter>> {
        let rows = sqlx::query(
            "SELECT ordinal, start_ms, title, thumbnail_path FROM chapters
             WHERE source_id = ? ORDER BY ordinal",
        )
        .bind(source_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        let mut chapters = Vec::with_capacity(rows.len());
        for row in rows {
            chapters.push(Chapter {
                ordinal: row.try_get("ordinal")?,
                start: Millis::new(row.try_get("start_ms")?),
                title: row.try_get("title")?,
                thumbnail_path: row
                    .try_get::<Option<String>, _>("thumbnail_path")?
                    .map(PathBuf::from),
            });
        }
        Ok(chapters)
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

async fn insert_track(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    source_id: MediaSourceId,
    track: &Track,
) -> Result<()> {
    let (codec, profile, level, bitrate) = match &track.kind {
        TrackKind::Video(details) => (
            details.codec.as_str(),
            details.profile.as_deref(),
            details.level,
            details.bitrate,
        ),
        TrackKind::Audio(details) => (
            details.codec.as_str(),
            details.profile.as_deref(),
            None,
            details.bitrate,
        ),
        TrackKind::Subtitle(details) => (details.codec.as_str(), None, None, None),
    };

    let video = match &track.kind {
        TrackKind::Video(details) => Some(details),
        _ => None,
    };
    let audio = match &track.kind {
        TrackKind::Audio(details) => Some(details),
        _ => None,
    };
    let subtitle = match &track.kind {
        TrackKind::Subtitle(details) => Some(details),
        _ => None,
    };

    let (hdr_format, dolby_vision_profile) = match video.and_then(|details| details.hdr) {
        Some(HdrFormat::Hdr10) => (Some("hdr10"), None),
        Some(HdrFormat::Hlg) => (Some("hlg"), None),
        Some(HdrFormat::DolbyVision { profile }) => (Some("dolby_vision"), profile),
        None => (None, None),
    };

    sqlx::query(
        "INSERT INTO tracks (
            id, source_id, stream_index, kind, language, title, is_default, is_forced,
            codec, profile, level, bitrate,
            width, height, aspect_ratio, is_interlaced, frame_rate, pixel_format,
            reference_frames, color_primaries, color_space, color_transfer, bit_depth,
            hdr_format, dolby_vision_profile,
            channels, channel_layout, sample_rate,
            loudness_integrated_lufs, loudness_true_peak_dbfs, loudness_range_lu,
            subtitle_layout, is_hearing_impaired, is_external, external_relative_path,
            margin_top, margin_bottom, margin_left, margin_right
         ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?,
            ?, ?,
            ?, ?, ?,
            ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?
         )",
    )
    .bind(track.id.to_db_string())
    .bind(source_id.to_db_string())
    .bind(track.stream_index)
    .bind(track.kind.as_str())
    .bind(track.language.as_deref())
    .bind(track.title.as_deref())
    .bind(bool_to_int(track.is_default))
    .bind(bool_to_int(track.is_forced))
    .bind(codec)
    .bind(profile)
    .bind(level)
    .bind(bitrate)
    .bind(video.map(|details| details.width))
    .bind(video.map(|details| details.height))
    .bind(video.and_then(|details| details.aspect_ratio.as_deref()))
    .bind(video.map(|details| bool_to_int(details.is_interlaced)))
    .bind(video.and_then(|details| details.frame_rate))
    .bind(video.and_then(|details| details.pixel_format.as_deref()))
    .bind(video.and_then(|details| details.reference_frames))
    .bind(video.and_then(|details| details.color.primaries.as_deref()))
    .bind(video.and_then(|details| details.color.space.as_deref()))
    .bind(video.and_then(|details| details.color.transfer.as_deref()))
    // Sample depth is carried by video colour and by audio alike, and one
    // column holds both because no track is ever of two kinds at once.
    .bind(
        video
            .and_then(|details| details.color.bit_depth)
            .or_else(|| audio.and_then(|details| details.bit_depth)),
    )
    .bind(hdr_format)
    .bind(dolby_vision_profile)
    .bind(audio.map(|details| details.channels))
    .bind(audio.and_then(|details| details.channel_layout.as_deref()))
    .bind(audio.and_then(|details| details.sample_rate))
    .bind(audio.and_then(|details| details.loudness.integrated_lufs))
    .bind(audio.and_then(|details| details.loudness.true_peak_dbfs))
    .bind(audio.and_then(|details| details.loudness.range_lu))
    .bind(subtitle.map(|details| match details.layout {
        SubtitleLayout::Text => "text",
        SubtitleLayout::Bitmap => "bitmap",
    }))
    .bind(subtitle.map(|details| bool_to_int(details.is_hearing_impaired)))
    .bind(bool_to_int(
        subtitle.is_some_and(|details| details.is_external),
    ))
    .bind(subtitle.and_then(|details| {
        details
            .external_relative_path
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned())
    }))
    .bind(video.and_then(|details| details.margins).map(|it| it.top))
    .bind(
        video
            .and_then(|details| details.margins)
            .map(|it| it.bottom),
    )
    .bind(video.and_then(|details| details.margins).map(|it| it.left))
    .bind(video.and_then(|details| details.margins).map(|it| it.right))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Edges a film says are not part of its picture, when it says so.
///
/// Absent unless one of them takes something off: a file described before this
/// was read leaves the four empty, and a file that really has no margins says
/// the same thing, so the two need not be told apart.
fn margins_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Option<Margins>> {
    let edge = |name: &str| -> Result<i32> {
        Ok(row.try_get::<Option<i32>, _>(name)?.unwrap_or_default())
    };
    let margins = Margins {
        top: edge("margin_top")?,
        bottom: edge("margin_bottom")?,
        left: edge("margin_left")?,
        right: edge("margin_right")?,
    };
    Ok((!margins.are_nothing()).then_some(margins))
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
struct Placed {
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

    /// Hung under another work at a given rank.
    fn under(library_id: LibraryId, parent_id: WorkId, ordinal: i32) -> Self {
        Self {
            library_id,
            parent_id: Some(parent_id),
            ordinal: Some(ordinal),
        }
    }
}

async fn insert_work<'e, E>(
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

fn track_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Track> {
    let kind_text: String = row.try_get("kind")?;
    let codec: String = row.try_get("codec")?;

    let kind = match kind_text.as_str() {
        "video" => TrackKind::Video(VideoDetails {
            codec,
            profile: row.try_get("profile")?,
            level: row.try_get("level")?,
            width: row.try_get::<Option<i32>, _>("width")?.unwrap_or_default(),
            height: row.try_get::<Option<i32>, _>("height")?.unwrap_or_default(),
            margins: margins_from_row(row)?,
            aspect_ratio: row.try_get("aspect_ratio")?,
            is_interlaced: row
                .try_get::<Option<i64>, _>("is_interlaced")?
                .map(int_to_bool)
                .unwrap_or_default(),
            frame_rate: row.try_get("frame_rate")?,
            bitrate: row.try_get("bitrate")?,
            pixel_format: row.try_get("pixel_format")?,
            reference_frames: row.try_get("reference_frames")?,
            color: ColorInfo {
                primaries: row.try_get("color_primaries")?,
                space: row.try_get("color_space")?,
                transfer: row.try_get("color_transfer")?,
                bit_depth: row.try_get("bit_depth")?,
            },
            hdr: hdr_from_row(row)?,
        }),
        "audio" => TrackKind::Audio(AudioDetails {
            codec,
            profile: row.try_get("profile")?,
            channels: row
                .try_get::<Option<i32>, _>("channels")?
                .unwrap_or_default(),
            channel_layout: row.try_get("channel_layout")?,
            sample_rate: row.try_get("sample_rate")?,
            bit_depth: row.try_get("bit_depth")?,
            bitrate: row.try_get("bitrate")?,
            loudness: Loudness {
                integrated_lufs: row.try_get("loudness_integrated_lufs")?,
                true_peak_dbfs: row.try_get("loudness_true_peak_dbfs")?,
                range_lu: row.try_get("loudness_range_lu")?,
            },
        }),
        "subtitle" => TrackKind::Subtitle(SubtitleDetails {
            codec,
            // An unknown layout reads as pictures, the cautious side: taking
            // pictures for text shows a viewer nothing at all.
            layout: match row
                .try_get::<Option<String>, _>("subtitle_layout")?
                .as_deref()
            {
                Some("text") => SubtitleLayout::Text,
                _ => SubtitleLayout::Bitmap,
            },
            is_hearing_impaired: row
                .try_get::<Option<i64>, _>("is_hearing_impaired")?
                .map(int_to_bool)
                .unwrap_or_default(),
            is_external: int_to_bool(row.try_get("is_external")?),
            external_relative_path: row
                .try_get::<Option<String>, _>("external_relative_path")?
                .map(PathBuf::from),
        }),
        other => {
            return Err(DatabaseError::Corrupt(format!(
                "track kind '{other}' is unknown"
            )))
        }
    };

    Ok(Track {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        source_id: parse_id(&row.try_get::<String, _>("source_id")?)?,
        stream_index: row.try_get("stream_index")?,
        language: row.try_get("language")?,
        title: row.try_get("title")?,
        is_default: int_to_bool(row.try_get("is_default")?),
        is_forced: int_to_bool(row.try_get("is_forced")?),
        kind,
    })
}

fn hdr_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Option<HdrFormat>> {
    Ok(
        match row.try_get::<Option<String>, _>("hdr_format")?.as_deref() {
            Some("hdr10") => Some(HdrFormat::Hdr10),
            Some("hlg") => Some(HdrFormat::Hlg),
            Some("dolby_vision") => Some(HdrFormat::DolbyVision {
                profile: row.try_get("dolby_vision_profile")?,
            }),
            _ => None,
        },
    )
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
    use melyxar_core::id::TrackId;
    use melyxar_core::library::LibraryKind;

    /// Somebody to answer for, since what a page shows depends on who is
    /// looking at it.
    async fn a_viewer(database: &Database) -> UserId {
        database
            .create_user("Viewer", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created")
            .id
    }

    async fn library() -> (Database, LibraryId, LibraryRootId) {
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

    #[tokio::test]
    async fn a_series_answers_with_its_seasons_and_a_season_with_its_episodes() {
        let (database, library_id, _) = library().await;
        let (series, seasons, _) = a_series(&database, library_id).await;

        let viewer = a_viewer(&database).await;
        let answered = database.children_of(viewer, series).await.expect("read");
        assert_eq!(
            answered
                .iter()
                .map(|child| child.ordinal)
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2)],
            "in the order they are numbered, not the order they were written"
        );
        assert!(answered.iter().all(|child| child.kind == WorkKind::Season));
        assert_eq!(
            answered
                .iter()
                .map(|child| child.child_count)
                .collect::<Vec<_>>(),
            vec![2, 1],
            "a season says how many episodes it holds"
        );

        let first = database
            .children_of(viewer, seasons[0])
            .await
            .expect("read");
        assert_eq!(first.len(), 2);
        assert!(first.iter().all(|child| child.kind == WorkKind::Episode));
        assert_eq!(
            first.iter().map(|child| child.ordinal).collect::<Vec<_>>(),
            vec![Some(1), Some(2)]
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
            .children_of(viewer, film.id)
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

    #[tokio::test]
    async fn a_season_says_how_many_of_it_are_left_to_watch() {
        let (database, library_id, root_id) = library().await;
        let viewer = a_viewer(&database).await;
        let (series, seasons, episodes) = a_series(&database, library_id).await;
        for (rank, episode) in episodes.iter().enumerate() {
            a_file_behind(&database, root_id, *episode, &format!("{rank}.mkv")).await;
        }
        watched(&database, viewer, episodes[0]).await;

        let read = database.children_of(viewer, series).await.expect("read");
        assert_eq!(
            read.iter()
                .map(|season| (season.child_count, season.unwatched))
                .collect::<Vec<_>>(),
            vec![(2, 1), (1, 1)],
            "the first season holds two and one is left; the second holds one"
        );

        let inside = database
            .children_of(viewer, seasons[0])
            .await
            .expect("read");
        assert_eq!(
            inside
                .iter()
                .map(|episode| episode.watched)
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
            .children_of(viewer, seasons[0])
            .await
            .expect("read");
        assert_eq!(
            read.iter().map(|child| child.playable).collect::<Vec<_>>(),
            vec![true, false]
        );
    }

    fn video_track(source_id: MediaSourceId) -> Track {
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

    fn audio_track(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 1,
            language: Some("fre".to_string()),
            title: Some("VFF".to_string()),
            is_default: true,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "eac3".to_string(),
                profile: None,
                channels: 6,
                channel_layout: Some("5.1".to_string()),
                sample_rate: Some(48_000),
                bit_depth: Some(24),
                bitrate: Some(768_000),
                loudness: Loudness {
                    integrated_lufs: Some(-23.4),
                    true_peak_dbfs: Some(-1.2),
                    range_lu: Some(7.5),
                },
            }),
        }
    }

    fn subtitle_track(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 2,
            language: Some("fre".to_string()),
            title: None,
            is_default: false,
            is_forced: true,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "hdmv_pgs_subtitle".to_string(),
                layout: SubtitleLayout::Bitmap,
                is_hearing_impaired: true,
                is_external: false,
                external_relative_path: None,
            }),
        }
    }

    async fn work_with_source(
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
    async fn every_stream_survives_a_round_trip_through_storage() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        let tracks = vec![
            video_track(source_id),
            audio_track(source_id),
            subtitle_track(source_id),
        ];
        database
            .store_analysis(
                source_id,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: Some(48_000_000),
                },
                &tracks,
                &[],
            )
            .await
            .expect("analysis stored");

        let stored = database
            .tracks_of_source(source_id)
            .await
            .expect("tracks read");
        assert_eq!(stored, tracks);
    }

    #[tokio::test]
    async fn the_wide_gamut_flavour_is_kept_because_the_playback_decision_rests_on_it() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
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

        let stored = database
            .tracks_of_source(source_id)
            .await
            .expect("tracks read");
        let TrackKind::Video(details) = &stored[0].kind else {
            panic!("the first stream is the video one");
        };
        assert_eq!(
            details.hdr,
            Some(HdrFormat::DolbyVision { profile: Some(5) })
        );
        assert!(
            details
                .hdr
                .expect("present")
                .is_incompatible_without_conversion(),
            "the profile has to survive storage, it is what forbids playing the file untouched"
        );
        assert_eq!(details.color.transfer.as_deref(), Some("smpte2084"));
        assert_eq!(details.color.bit_depth, Some(10));
    }

    #[tokio::test]
    async fn every_wide_gamut_flavour_is_read_back_as_the_one_that_was_written() {
        // Each one calls for a different answer at playback time, and a
        // flavour that comes back as none would have the file played untouched
        // and shown washed out.
        let (database, library_id, root_id) = library().await;

        for flavour in [
            HdrFormat::Hdr10,
            HdrFormat::Hlg,
            HdrFormat::DolbyVision { profile: Some(8) },
        ] {
            let (_, source_id) = work_with_source(
                &database,
                library_id,
                root_id,
                &format!("Winter.Signal.{flavour:?}.mkv"),
            )
            .await;

            let mut track = video_track(source_id);
            if let TrackKind::Video(details) = &mut track.kind {
                details.hdr = Some(flavour);
            }
            database
                .store_analysis(source_id, &SourceAnalysis::default(), &[track], &[])
                .await
                .expect("analysis stored");

            let stored = database
                .tracks_of_source(source_id)
                .await
                .expect("tracks read");
            let TrackKind::Video(details) = &stored[0].kind else {
                panic!("the first stream is the video one");
            };
            assert_eq!(details.hdr, Some(flavour), "{flavour:?}");
        }
    }

    #[tokio::test]
    async fn an_ordinary_picture_carries_no_wide_gamut_flavour() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Amber.Field.2020.mkv").await;

        let mut track = video_track(source_id);
        if let TrackKind::Video(details) = &mut track.kind {
            details.hdr = None;
        }
        database
            .store_analysis(source_id, &SourceAnalysis::default(), &[track], &[])
            .await
            .expect("analysis stored");

        let stored = database
            .tracks_of_source(source_id)
            .await
            .expect("tracks read");
        let TrackKind::Video(details) = &stored[0].kind else {
            panic!("the first stream is the video one");
        };
        assert_eq!(details.hdr, None);
    }

    #[tokio::test]
    async fn analysing_the_same_file_twice_replaces_its_streams_rather_than_piling_them_up() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        for _ in 0..3 {
            database
                .store_analysis(
                    source_id,
                    &SourceAnalysis::default(),
                    &[video_track(source_id), audio_track(source_id)],
                    &[Chapter {
                        ordinal: 1,
                        start: Millis::new(0),
                        title: Some("Opening".to_string()),
                        thumbnail_path: None,
                    }],
                )
                .await
                .expect("analysis stored");
        }

        assert_eq!(
            database
                .tracks_of_source(source_id)
                .await
                .expect("read")
                .len(),
            2
        );
        assert_eq!(
            database
                .chapters_of_source(source_id)
                .await
                .expect("read")
                .len(),
            1
        );
    }

    fn external_subtitle(source_id: MediaSourceId) -> Track {
        Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
            language: Some("fre".to_string()),
            title: None,
            is_default: false,
            is_forced: true,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "subrip".to_string(),
                layout: SubtitleLayout::Text,
                is_hearing_impaired: false,
                is_external: true,
                external_relative_path: Some(PathBuf::from("Quiet.Harbour.2019.fr.forced.srt")),
            }),
        }
    }

    #[tokio::test]
    async fn a_subtitle_in_its_own_file_survives_the_analysis_of_the_film() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        database
            .store_external_subtitles(source_id, &[external_subtitle(source_id)])
            .await
            .expect("subtitle stored");
        database
            .store_analysis(
                source_id,
                &SourceAnalysis::default(),
                &[video_track(source_id), subtitle_track(source_id)],
                &[],
            )
            .await
            .expect("analysis stored");

        let stored = database
            .tracks_of_source(source_id)
            .await
            .expect("tracks read");
        assert_eq!(stored.len(), 3, "the file next to the film is a track too");
        let external: Vec<&Track> = stored
            .iter()
            .filter(
                |track| matches!(&track.kind, TrackKind::Subtitle(details) if details.is_external),
            )
            .collect();
        assert_eq!(external.len(), 1);
        assert!(external[0].is_forced);
        assert_eq!(external[0].language.as_deref(), Some("fre"));

        // Text or pictures is what decides whether showing this subtitle costs
        // a full rebuild of the picture, so it has to survive storage.
        let layouts: Vec<SubtitleLayout> = stored
            .iter()
            .filter_map(|track| match &track.kind {
                TrackKind::Subtitle(details) => Some(details.layout),
                _ => None,
            })
            .collect();
        assert!(layouts.contains(&SubtitleLayout::Text));
        assert!(layouts.contains(&SubtitleLayout::Bitmap));
    }

    #[tokio::test]
    async fn the_films_carrying_a_subtitle_of_their_own_are_listed_for_the_scan() {
        // A scan asks for this so it rewrites only what changed. An answer of
        // nothing would make every scan rewrite every file in the library.
        let (database, library_id, root_id) = library().await;
        let (_, with_one) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        let (_, without) =
            work_with_source(&database, library_id, root_id, "Amber.Field.2020.mkv").await;

        database
            .store_external_subtitles(with_one, &[external_subtitle(with_one)])
            .await
            .expect("subtitle stored");
        // A subtitle inside the film is not a file beside it.
        database
            .store_analysis(
                without,
                &SourceAnalysis::default(),
                &[subtitle_track(without)],
                &[],
            )
            .await
            .expect("analysis stored");

        assert_eq!(
            database
                .sources_with_external_subtitles(root_id)
                .await
                .expect("read"),
            vec![with_one]
        );
    }

    #[tokio::test]
    async fn subtitles_in_their_own_files_are_replaced_rather_than_piled_up() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;

        for _ in 0..3 {
            database
                .store_external_subtitles(source_id, &[external_subtitle(source_id)])
                .await
                .expect("subtitle stored");
        }
        assert_eq!(
            database
                .tracks_of_source(source_id)
                .await
                .expect("read")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn a_replaced_film_keeps_the_subtitles_that_live_beside_it() {
        let (database, library_id, root_id) = library().await;
        let (_, source_id) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        database
            .store_external_subtitles(source_id, &[external_subtitle(source_id)])
            .await
            .expect("subtitle stored");
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
            .refresh_source_identity(source_id, 2_000, now())
            .await
            .expect("identity refreshed");

        let stored = database.tracks_of_source(source_id).await.expect("read");
        assert_eq!(
            stored.len(),
            1,
            "the streams of the old copy go, the file next to it stays"
        );
        assert!(
            matches!(&stored[0].kind, TrackKind::Subtitle(details) if details.is_external),
            "a subtitle file describes itself, not the copy that was replaced"
        );
    }

    #[tokio::test]
    async fn only_the_files_still_waiting_for_an_analysis_are_handed_to_it() {
        let (database, library_id, root_id) = library().await;
        let (_, waiting) =
            work_with_source(&database, library_id, root_id, "Amber.Field.2020.mkv").await;
        let (_, already_done) =
            work_with_source(&database, library_id, root_id, "Quiet.Harbour.2019.mkv").await;
        let (_, gone) =
            work_with_source(&database, library_id, root_id, "Winter.Signal.2021.mkv").await;

        database
            .store_analysis(
                already_done,
                &SourceAnalysis {
                    container: Some("matroska,webm".to_string()),
                    duration: Some(Millis::new(7_200_000)),
                    overall_bitrate: None,
                },
                &[video_track(already_done)],
                &[],
            )
            .await
            .expect("analysis stored");
        database.mark_source_missing(gone).await.expect("marked");

        let waiting_list = database
            .unanalysed_sources_of_root(root_id)
            .await
            .expect("read");
        assert_eq!(
            waiting_list
                .iter()
                .map(|source| source.id)
                .collect::<Vec<_>>(),
            vec![waiting],
            "a file already looked at is not looked at again, and one that is \
             not on the disk would only fail"
        );
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

        database.delete_work(work_id).await.expect("work removed");

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
