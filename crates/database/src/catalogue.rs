//! Works, the files behind them, and what an analysis found inside.
//!
//! Two rules shape everything here. A work outlives the file that revealed it,
//! so replacing a copy with a better one keeps the watch history. And a file
//! that is no longer on disk is marked absent rather than deleted, so that an
//! unplugged disk is a bad evening rather than a lost library.

use std::path::{Path, PathBuf};

use melyxar_core::id::{ChapterId, ExtraVideoId, LibraryId, LibraryRootId, MediaSourceId, WorkId};
use melyxar_core::media::{
    AudioDetails, Chapter, ColorInfo, HdrFormat, Loudness, SubtitleDetails, SubtitleLayout, Track,
    TrackKind, VideoDetails,
};
use melyxar_core::time::{now, Millis, Timestamp};
use melyxar_core::work::{IdentificationNote, IdentificationState, Work, WorkKind};
use sqlx::{Row, Sqlite};

use crate::convert::{
    bool_to_int, int_to_bool, parse_optional_timestamp, parse_timestamp, timestamp_to_text,
};
use crate::{Database, DatabaseError, Result};

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
            library_id,
            kind,
            title,
            sort_title,
            release_year,
        )
        .await
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
            library_id,
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
        let row = sqlx::query(
            "SELECT id, library_id, parent_id, kind, title, sort_title, release_year, runtime_ms,
                    community_rating, age_rating_label, identification, identification_note,
                    dominant_color, added_at, updated_at
             FROM works WHERE id = ?",
        )
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
        let row = sqlx::query(
            "SELECT id, library_id, parent_id, kind, title, sort_title, release_year, runtime_ms,
                    community_rating, age_rating_label, identification, identification_note,
                    dominant_color, added_at, updated_at
             FROM works
             WHERE library_id = ? AND sort_title = ?
               AND (release_year IS ? OR (release_year IS NULL AND ? IS NULL))
             ORDER BY added_at
             LIMIT 1",
        )
        .bind(library_id.to_db_string())
        .bind(sort_title)
        .bind(release_year)
        .bind(release_year)
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| work_from_row(&row)).transpose()
    }

    /// Works of a library, newest first, which is the order the home page uses.
    pub async fn recent_works(&self, library_id: LibraryId, limit: i64) -> Result<Vec<Work>> {
        let rows = sqlx::query(
            "SELECT id, library_id, parent_id, kind, title, sort_title, release_year, runtime_ms,
                    community_rating, age_rating_label, identification, identification_note,
                    dominant_color, added_at, updated_at
             FROM works WHERE library_id = ? ORDER BY added_at DESC LIMIT ?",
        )
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

        Ok(CatalogueSummary {
            works: works.0,
            identified: works.1,
            awaiting_identification: works.2,
            files: files.0,
            missing_files: files.1,
            read_for_key_frames: read.0,
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
    pub async fn sources_without_key_frames(&self, limit: i64) -> Result<Vec<MediaSourceId>> {
        let rows = sqlx::query(
            "SELECT media_sources.id
             FROM media_sources
             LEFT JOIN media_source_key_frames
                    ON media_source_key_frames.source_id = media_sources.id
             WHERE media_sources.analysed_at IS NOT NULL
               AND media_sources.missing_since IS NULL
               AND media_source_key_frames.source_id IS NULL
             ORDER BY media_sources.added_at
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        rows.into_iter()
            .map(|row| {
                let id: String = row.try_get("id")?;
                id.parse().map_err(|_| {
                    DatabaseError::Corrupt("media source identifier is malformed".to_string())
                })
            })
            .collect()
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
            subtitle_layout, is_hearing_impaired, is_external, external_relative_path
         ) VALUES (
            ?, ?, ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?,
            ?, ?, ?, ?, ?, ?,
            ?, ?, ?, ?, ?,
            ?, ?,
            ?, ?, ?,
            ?, ?, ?,
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
    .execute(&mut **transaction)
    .await?;
    Ok(())
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
async fn insert_work<'e, E>(
    executor: E,
    library_id: LibraryId,
    kind: WorkKind,
    title: &str,
    sort_title: &str,
    release_year: Option<i32>,
) -> Result<Work>
where
    E: sqlx::Executor<'e, Database = Sqlite>,
{
    let id = WorkId::new();
    let moment = now();
    let timestamp = timestamp_to_text(moment);

    sqlx::query(
        "INSERT INTO works (id, library_id, kind, title, sort_title, release_year, added_at, updated_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id.to_db_string())
    .bind(library_id.to_db_string())
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
        parent_id: None,
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

fn parse_id<T: std::str::FromStr>(value: &str) -> Result<T> {
    value
        .parse()
        .map_err(|_| DatabaseError::Corrupt(format!("identifier '{value}' is malformed")))
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
                .sources_without_key_frames(10)
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
            database.sources_without_key_frames(10).await.expect("read"),
            vec![source_id]
        );

        database
            .store_key_frames(source_id, &[Millis::ZERO])
            .await
            .expect("kept");
        assert!(
            database
                .sources_without_key_frames(10)
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
            .sources_without_key_frames(10)
            .await
            .expect("read")
            .is_empty());
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
