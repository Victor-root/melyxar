//! Taking one work out of a library, and the files it stands for.
//!
//! Everything a work carries follows it out through the schema: what hangs
//! under it, its files as rows, what was watched of it, its favourites. What
//! is shared rather than its own is swept afterwards, exactly as when a whole
//! library goes. The files on the disk are the caller's to delete or not;
//! nothing here touches a disk.

use std::collections::HashSet;
use std::path::PathBuf;

use melyxar_core::id::{LibraryId, LibraryRootId, MediaSourceId, WorkId};
use melyxar_core::time::now;
use sqlx::Row;

use crate::convert::{parse_id, timestamp_to_text};
use crate::libraries::{sweep_what_nothing_points_at, Removed};
use crate::{Database, Result};

/// Every work under this one and itself, as a statement other statements
/// start from. Bound to the one identifier as `?1`.
const THE_WHOLE_TREE: &str = "WITH RECURSIVE tree(id) AS (
                                  SELECT ?1
                                  UNION ALL
                                  SELECT works.id FROM works JOIN tree ON works.parent_id = tree.id
                              )";

/// What part a file plays for the work it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileRole {
    /// A copy of the work itself.
    Copy,
    /// A subtitle in a file of its own, beside a copy.
    Subtitle,
    /// A trailer or another clip sitting beside the work.
    Extra,
}

/// One file on the disk a work stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOfAWork {
    pub root_id: LibraryRootId,
    /// The folder of the library it sits under, as declared.
    pub root_path: PathBuf,
    /// Where it is under that folder.
    pub relative_path: PathBuf,
    pub role: FileRole,
}

impl FileOfAWork {
    /// Where it is, folder of the library included.
    pub fn path(&self) -> PathBuf {
        self.root_path.join(&self.relative_path)
    }
}

/// What deleting a work would take with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WhatDeletingTakes {
    pub library_id: LibraryId,
    /// The work itself and everything under it: a film is one, a series is
    /// itself, its seasons and their episodes.
    pub works: i64,
    /// Every file on the disk they stand for, copies first. A copy the last
    /// scan did not find is left out: there is nothing of it to delete.
    pub files: Vec<FileOfAWork>,
    /// The copies as rows, whatever became of them on the disk, for whatever
    /// the server keeps beside a file outside the database.
    pub sources: Vec<MediaSourceId>,
}

impl Database {
    /// What deleting this work would take with it, or nothing when there is
    /// no such work.
    pub async fn what_deleting_takes(&self, work_id: WorkId) -> Result<Option<WhatDeletingTakes>> {
        let id = work_id.to_db_string();
        let Some(library_id) =
            sqlx::query_scalar::<_, String>("SELECT library_id FROM works WHERE id = ?")
                .bind(&id)
                .fetch_optional(self.reader())
                .await?
        else {
            return Ok(None);
        };

        let works: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
            "{THE_WHOLE_TREE} SELECT count(*) FROM tree"
        )))
        .bind(&id)
        .fetch_one(self.reader())
        .await?;

        let copies = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{THE_WHOLE_TREE}
             SELECT s.id, s.missing_since, r.id AS root_id, r.path AS root_path, s.relative_path
               FROM media_sources s
               JOIN library_roots r ON r.id = s.root_id
              WHERE s.work_id IN (SELECT id FROM tree)
              ORDER BY s.relative_path"
        )))
        .bind(&id)
        .fetch_all(self.reader())
        .await?;

        let beside = sqlx::query(sqlx::AssertSqlSafe(format!(
            "{THE_WHOLE_TREE}
             SELECT DISTINCT r.id AS root_id, r.path AS root_path,
                    t.external_relative_path AS relative_path, 'subtitle' AS role
               FROM tracks t
               JOIN media_sources s ON s.id = t.source_id
               JOIN library_roots r ON r.id = s.root_id
              WHERE s.work_id IN (SELECT id FROM tree)
                AND t.is_external = 1 AND t.external_relative_path IS NOT NULL
             UNION
             SELECT r.id, r.path, e.relative_path, 'extra'
               FROM extra_videos e
               JOIN library_roots r ON r.id = e.root_id
              WHERE e.work_id IN (SELECT id FROM tree) AND e.relative_path IS NOT NULL
             ORDER BY relative_path"
        )))
        .bind(&id)
        .fetch_all(self.reader())
        .await?;

        let mut files = Vec::new();
        let mut sources = Vec::new();
        for row in &copies {
            sources.push(parse_id(&row.try_get::<String, _>("id")?)?);
            if row.try_get::<Option<String>, _>("missing_since")?.is_none() {
                files.push(file_from_row(row, FileRole::Copy)?);
            }
        }
        for row in &beside {
            let role = match row.try_get::<String, _>("role")?.as_str() {
                "subtitle" => FileRole::Subtitle,
                _ => FileRole::Extra,
            };
            files.push(file_from_row(row, role)?);
        }

        Ok(Some(WhatDeletingTakes {
            library_id: parse_id(&library_id)?,
            works,
            files,
            sources,
        }))
    }

    /// Deletes a work and everything under it, and sweeps what nothing
    /// points at any more.
    ///
    /// With `set_aside`, the copies stay noted as taken out of the library,
    /// so the scan walks past them: they are still on the disk, and the
    /// person who removed them does not want them back. The picture paths in
    /// what comes back are for the caller to take out of the cache.
    pub async fn delete_work(&self, work_id: WorkId, set_aside: bool) -> Result<Removed> {
        let id = work_id.to_db_string();
        let going = self.what_deleting_takes(work_id).await?;
        let (works, files) = going
            .as_ref()
            .map(|going| (going.works, going.sources.len() as i64))
            .unwrap_or_default();

        let mut transaction = self.begin().await?;
        if set_aside {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "{THE_WHOLE_TREE}
                 INSERT OR IGNORE INTO set_aside_files (root_id, relative_path, set_aside_at)
                 SELECT root_id, relative_path, ?2 FROM media_sources
                  WHERE work_id IN (SELECT id FROM tree)"
            )))
            .bind(&id)
            .bind(timestamp_to_text(now()))
            .execute(&mut *transaction)
            .await?;
        }
        let parent: Option<String> = sqlx::query_scalar("SELECT parent_id FROM works WHERE id = ?")
            .bind(&id)
            .fetch_optional(&mut *transaction)
            .await?
            .flatten();
        sqlx::query("DELETE FROM works WHERE id = ?")
            .bind(&id)
            .execute(&mut *transaction)
            .await?;
        if let Some(parent) = parent {
            sqlx::query(
                "UPDATE works
                    SET child_count = (SELECT count(*) FROM works AS child
                                        WHERE child.parent_id = works.id)
                  WHERE id = ?",
            )
            .bind(parent)
            .execute(&mut *transaction)
            .await?;
        }
        let swept = sweep_what_nothing_points_at(&mut transaction).await?;
        transaction.commit().await?;

        Ok(Removed {
            works,
            files,
            swept,
        })
    }

    /// The paths of one folder of a library that were taken out of it and
    /// are to be walked past.
    pub async fn set_aside_paths(&self, root_id: LibraryRootId) -> Result<HashSet<PathBuf>> {
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT relative_path FROM set_aside_files WHERE root_id = ?")
                .bind(root_id.to_db_string())
                .fetch_all(self.reader())
                .await?;
        Ok(rows.into_iter().map(PathBuf::from).collect())
    }

    /// How many files of a library are taken out of it while still on the
    /// disk.
    pub async fn count_set_aside(&self, library_id: LibraryId) -> Result<i64> {
        Ok(sqlx::query_scalar(
            "SELECT count(*) FROM set_aside_files
              WHERE root_id IN (SELECT id FROM library_roots WHERE library_id = ?)",
        )
        .bind(library_id.to_db_string())
        .fetch_one(self.reader())
        .await?)
    }

    /// Forgets that the files of a library were taken out of it, so the next
    /// scan finds them again. Answers how many there were.
    pub async fn take_back_set_aside(&self, library_id: LibraryId) -> Result<u64> {
        Ok(sqlx::query(
            "DELETE FROM set_aside_files
              WHERE root_id IN (SELECT id FROM library_roots WHERE library_id = ?)",
        )
        .bind(library_id.to_db_string())
        .execute(self.writer())
        .await?
        .rows_affected())
    }
}

fn file_from_row(row: &sqlx::sqlite::SqliteRow, role: FileRole) -> Result<FileOfAWork> {
    Ok(FileOfAWork {
        root_id: parse_id(&row.try_get::<String, _>("root_id")?)?,
        root_path: PathBuf::from(row.try_get::<String, _>("root_path")?),
        relative_path: PathBuf::from(row.try_get::<String, _>("relative_path")?),
        role,
    })
}

#[cfg(test)]
mod tests {
    use melyxar_core::library::LibraryKind;
    use melyxar_core::work::WorkKind;

    use super::*;

    struct Series {
        database: Database,
        library_id: LibraryId,
        root_id: LibraryRootId,
        series: WorkId,
        season: WorkId,
        episodes: [WorkId; 2],
    }

    /// A series of one season and two episodes, each with its file, the
    /// first with a subtitle beside it and the series with a trailer.
    async fn a_series() -> Series {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Shows",
                LibraryKind::Series,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Series"))],
            )
            .await
            .expect("library created");
        let (library_id, root_id) = (library.id, library.roots[0].id);
        let series = database
            .create_work(library_id, WorkKind::Series, "Signal", "signal", None)
            .await
            .expect("series")
            .id;
        let season = database
            .create_child_work(library_id, series, 1, WorkKind::Season, "Saison 1", "1")
            .await
            .expect("season")
            .id;
        let mut episodes = Vec::new();
        for number in 1..=2 {
            let episode = database
                .create_child_work(library_id, season, number, WorkKind::Episode, "e", "e")
                .await
                .expect("episode")
                .id;
            database
                .insert_source(
                    episode,
                    root_id,
                    &PathBuf::from(format!("Signal/Saison 1/Signal S01E0{number}.mkv")),
                    10,
                    now(),
                )
                .await
                .expect("file");
            episodes.push(episode);
        }
        let first_source: String =
            sqlx::query_scalar("SELECT id FROM media_sources WHERE work_id = ?")
                .bind(episodes[0].to_db_string())
                .fetch_one(database.reader())
                .await
                .expect("read");
        sqlx::query(
            "INSERT INTO tracks (id, source_id, stream_index, kind, codec, is_default, is_forced,
                                 is_external, external_relative_path)
             VALUES ('t1', ?, 0, 'subtitle', 'subrip', 0, 0, 1, 'Signal/Saison 1/Signal S01E01.fr.srt')",
        )
        .bind(&first_source)
        .execute(database.writer())
        .await
        .expect("subtitle");
        sqlx::query(
            "INSERT INTO extra_videos (id, work_id, kind, root_id, relative_path, created_at)
             VALUES ('x1', ?, 'trailer', ?, 'Signal/Signal-trailer.mkv', '2026-01-01T00:00:00Z')",
        )
        .bind(series.to_db_string())
        .bind(root_id.to_db_string())
        .execute(database.writer())
        .await
        .expect("trailer");

        Series {
            database,
            library_id,
            root_id,
            series,
            season,
            episodes: [episodes[0], episodes[1]],
        }
    }

    #[tokio::test]
    async fn a_series_takes_its_seasons_its_episodes_and_every_file_beside_them() {
        let world = a_series().await;
        let going = world
            .database
            .what_deleting_takes(world.series)
            .await
            .expect("read")
            .expect("the series exists");
        assert_eq!(going.library_id, world.library_id);
        assert_eq!(going.works, 4, "the series, its season and two episodes");
        assert_eq!(going.sources.len(), 2);
        let files: Vec<(String, FileRole)> = going
            .files
            .iter()
            .map(|file| (file.path().to_string_lossy().into_owned(), file.role))
            .collect();
        assert_eq!(
            files,
            [
                (
                    "/mnt/one/Series/Signal/Saison 1/Signal S01E01.mkv".to_string(),
                    FileRole::Copy
                ),
                (
                    "/mnt/one/Series/Signal/Saison 1/Signal S01E02.mkv".to_string(),
                    FileRole::Copy
                ),
                (
                    "/mnt/one/Series/Signal/Saison 1/Signal S01E01.fr.srt".to_string(),
                    FileRole::Subtitle
                ),
                (
                    "/mnt/one/Series/Signal/Signal-trailer.mkv".to_string(),
                    FileRole::Extra
                ),
            ]
        );

        // A copy the last scan did not find has nothing left to delete.
        let missing: String = sqlx::query_scalar("SELECT id FROM media_sources WHERE work_id = ?")
            .bind(world.episodes[1].to_db_string())
            .fetch_one(world.database.reader())
            .await
            .expect("read");
        world
            .database
            .mark_source_missing(parse_id(&missing).expect("identifier"))
            .await
            .expect("marked");
        let going = world
            .database
            .what_deleting_takes(world.episodes[1])
            .await
            .expect("read")
            .expect("the episode exists");
        assert_eq!(going.works, 1);
        assert!(going.files.is_empty());
        assert_eq!(going.sources.len(), 1, "the row goes all the same");

        assert!(world
            .database
            .what_deleting_takes(WorkId::new())
            .await
            .expect("read")
            .is_none());
    }

    #[tokio::test]
    async fn a_work_taken_out_of_the_library_leaves_its_files_set_aside() {
        let world = a_series().await;
        let removed = world
            .database
            .delete_work(world.episodes[0], true)
            .await
            .expect("deleted");
        assert_eq!((removed.works, removed.files), (1, 1));
        assert!(world
            .database
            .work(world.episodes[0])
            .await
            .expect("read")
            .is_none());
        let (left,): (i64,) = sqlx::query_as("SELECT child_count FROM works WHERE id = ?")
            .bind(world.season.to_db_string())
            .fetch_one(world.database.reader())
            .await
            .expect("read");
        assert_eq!(left, 1, "the season counts what it still holds");

        assert_eq!(
            world
                .database
                .set_aside_paths(world.root_id)
                .await
                .expect("read"),
            HashSet::from([PathBuf::from("Signal/Saison 1/Signal S01E01.mkv")])
        );
        assert_eq!(
            world
                .database
                .count_set_aside(world.library_id)
                .await
                .expect("read"),
            1
        );

        // Deleted from the disk as well, nothing is set aside: there is
        // nothing left there to walk past.
        world
            .database
            .delete_work(world.series, false)
            .await
            .expect("deleted");
        assert!(world
            .database
            .work(world.season)
            .await
            .expect("read")
            .is_none());
        assert_eq!(
            world
                .database
                .count_set_aside(world.library_id)
                .await
                .expect("read"),
            1
        );

        assert_eq!(
            world
                .database
                .take_back_set_aside(world.library_id)
                .await
                .expect("taken back"),
            1
        );
        assert!(world
            .database
            .set_aside_paths(world.root_id)
            .await
            .expect("read")
            .is_empty());
    }
}
