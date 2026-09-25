//! Libraries and their root folders.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, LibraryRootId, MediaSourceId};
use melyxar_core::library::{Library, LibraryKind, LibraryOptions, LibraryRoot, RootAccess};
use melyxar_core::time::{now, Timestamp};
use sqlx::{AssertSqlSafe, Row};

use crate::convert::{parse_id, parse_optional_timestamp, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

/// Pictures whose owner no longer exists.
///
/// Written once: the list is read to know which files to remove from the disk
/// and then run again to delete the rows, and the two drifting apart would
/// leave files behind with nothing left pointing at them.
const NOBODY_OWNS_THEM: &str = "(owner_kind = 'work' AND owner_id NOT IN (SELECT id FROM works))
     OR (owner_kind = 'person' AND owner_id NOT IN (SELECT id FROM people))
     OR (owner_kind = 'collection' AND owner_id NOT IN (SELECT id FROM collections))
     OR (owner_kind = 'library' AND owner_id NOT IN (SELECT id FROM libraries))";

/// What taking a library or one of its folders away would take with it.
///
/// Rows and only rows. No file on the disk is ever touched by any of this, so
/// neither number counts anything anybody could lose off a disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WouldGo {
    /// Films that would disappear, because every copy of them came through
    /// what is being taken away.
    pub works: i64,
    /// Files that would stop being known, and stay exactly where they are.
    pub files: i64,
}

/// What a removal actually took, once everything behind it had gone too.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Removed {
    pub works: i64,
    pub files: i64,
    /// Everything that was only ever there because of those films.
    pub swept: Swept,
}

/// What nothing pointed at any more, once the films were gone.
///
/// A film carries a good deal behind it that is shared rather than its own: the
/// people it credits, the collection it belongs to, its genres and its studios.
/// None of it follows a film out through the schema, because none of it belongs
/// to one film. Left alone, it is what turns years of use into a database full
/// of names nobody can reach and a cache full of faces nobody will see.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Swept {
    /// People nobody credits any more.
    pub people: i64,
    /// Collections that lost their last film. Only ones a provider made: a
    /// collection somebody put together by hand is theirs, empty or not.
    pub collections: i64,
    /// Genres and studios no film carries any more.
    pub genres: i64,
    pub studios: i64,
    /// Pictures that belonged to something now gone, as rows.
    pub pictures: i64,
    /// The files those pictures are, inside the image cache. Deleted by the
    /// caller that wrote them, since nothing below this layer touches a disk.
    pub picture_paths: Vec<String>,
}

/// Throws away everything the films that just went were the last to point at.
///
/// Runs inside the removal's own transaction: a half swept database is one
/// where a page can be opened on a person whose photo has already gone.
///
/// Bounded by what is left rather than by what went, which is what makes it
/// safe to run after any removal: a person credited by one remaining film is
/// not somebody this sweep can reach, whatever else has just been taken away.
pub(crate) async fn sweep_what_nothing_points_at(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
) -> Result<Swept> {
    let people = sqlx::query(
        "DELETE FROM people
          WHERE NOT EXISTS (SELECT 1 FROM credits WHERE person_id = people.id)",
    )
    .execute(&mut **transaction)
    .await?
    .rows_affected();

    let collections = sqlx::query(
        "DELETE FROM collections
          WHERE origin = 'provider'
            AND NOT EXISTS (SELECT 1 FROM collection_items
                             WHERE collection_id = collections.id)",
    )
    .execute(&mut **transaction)
    .await?
    .rows_affected();

    let genres = sqlx::query(
        "DELETE FROM genres
          WHERE NOT EXISTS (SELECT 1 FROM work_genres WHERE genre_id = genres.id)",
    )
    .execute(&mut **transaction)
    .await?
    .rows_affected();

    let studios = sqlx::query(
        "DELETE FROM studios
          WHERE NOT EXISTS (SELECT 1 FROM work_studios WHERE studio_id = studios.id)",
    )
    .execute(&mut **transaction)
    .await?
    .rows_affected();

    // The pictures come last, because what owns them has only just stopped
    // existing. Read before they go: a file cannot be found again once the row
    // that named it has gone, and these are the heaviest thing a removal
    // leaves behind.
    let picture_paths: Vec<String> = sqlx::query_scalar(AssertSqlSafe(format!(
        "SELECT relative_path FROM images WHERE {NOBODY_OWNS_THEM}"
    )))
    .fetch_all(&mut **transaction)
    .await?;

    let pictures = sqlx::query(AssertSqlSafe(format!(
        "DELETE FROM images WHERE {NOBODY_OWNS_THEM}"
    )))
    .execute(&mut **transaction)
    .await?
    .rows_affected();

    Ok(Swept {
        people: people as i64,
        collections: collections as i64,
        genres: genres as i64,
        studios: studios as i64,
        pictures: pictures as i64,
        picture_paths,
    })
}

/// A root along with what the server may actually do with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootWithAccess {
    pub root: LibraryRoot,
    pub access: RootAccess,
    pub checked_at: Option<Timestamp>,
}

impl Database {
    /// Creates a library with its roots.
    pub async fn create_library(
        &self,
        name: &str,
        kind: LibraryKind,
        metadata_language: &str,
        roots: &[(String, PathBuf)],
    ) -> Result<Library> {
        let id = LibraryId::new();
        let timestamp = timestamp_to_text(now());

        let mut transaction = self.begin().await?;
        sqlx::query(
            "INSERT INTO libraries (id, name, kind, metadata_language, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(name)
        .bind(kind.as_str())
        .bind(metadata_language)
        .bind(&timestamp)
        .bind(&timestamp)
        .execute(&mut *transaction)
        .await?;

        let mut stored_roots = Vec::with_capacity(roots.len());
        for (label, path) in roots {
            let root_id = LibraryRootId::new();
            sqlx::query(
                "INSERT INTO library_roots (id, library_id, label, path) VALUES (?, ?, ?, ?)",
            )
            .bind(root_id.to_db_string())
            .bind(id.to_db_string())
            .bind(label)
            .bind(path.to_string_lossy().as_ref())
            .execute(&mut *transaction)
            .await?;
            stored_roots.push(LibraryRoot {
                id: root_id,
                library_id: id,
                label: label.clone(),
                path: path.clone(),
            });
        }
        transaction.commit().await?;

        Ok(Library {
            id,
            name: name.to_string(),
            kind,
            metadata_language: metadata_language.to_string(),
            options: LibraryOptions::default(),
            roots: stored_roots,
        })
    }

    /// Changes what a scan of this library does in one sitting.
    ///
    /// Answers whether anything moved, so a caller can tell a real change from
    /// the same value written again: turning a reading on is worth starting
    /// the work that was waiting for the night, and writing it again is not.
    pub async fn set_library_options(
        &self,
        id: LibraryId,
        options: LibraryOptions,
    ) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE libraries
                SET key_frames_during_scan = ?, thumbnails_during_scan = ?,
                    watch_in_real_time = ?, updated_at = ?
              WHERE id = ?
                AND (key_frames_during_scan <> ? OR thumbnails_during_scan <> ?
                     OR watch_in_real_time <> ?)",
        )
        .bind(options.key_frames_during_scan)
        .bind(options.thumbnails_during_scan)
        .bind(options.watch_in_real_time)
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .bind(options.key_frames_during_scan)
        .bind(options.thumbnails_during_scan)
        .bind(options.watch_in_real_time)
        .execute(self.writer())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Calls a library something else.
    ///
    /// The way a name typed wrongly is put right. Nothing else moves: the
    /// films, the folders and everything anybody has watched hang off the
    /// library and not off its name.
    pub async fn rename_library(&self, id: LibraryId, name: &str) -> Result<()> {
        sqlx::query("UPDATE libraries SET name = ?, updated_at = ? WHERE id = ?")
            .bind(name)
            .bind(timestamp_to_text(now()))
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Changes the language a library's films are described in.
    ///
    /// Answers whether anything moved, so a caller can tell a change from a
    /// value written again: only a real change is worth asking a provider
    /// about four hundred films again.
    pub async fn set_metadata_language(&self, id: LibraryId, language: &str) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE libraries SET metadata_language = ?, updated_at = ?
             WHERE id = ? AND metadata_language <> ?",
        )
        .bind(language)
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .bind(language)
        .execute(self.writer())
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Every library with its roots, by kind in the order kinds are met, then
    /// by name.
    pub async fn list_libraries(&self) -> Result<Vec<Library>> {
        let rows = sqlx::query(
            "SELECT id, name, kind, metadata_language,
                    key_frames_during_scan, thumbnails_during_scan, watch_in_real_time
             FROM libraries ORDER BY name COLLATE NOCASE",
        )
        .fetch_all(self.reader())
        .await?;

        let mut libraries = Vec::with_capacity(rows.len());
        for row in rows {
            let id = parse_id::<LibraryId>(&row.try_get::<String, _>("id")?)?;
            let kind_text: String = row.try_get("kind")?;
            let kind = LibraryKind::parse(&kind_text).ok_or_else(|| {
                DatabaseError::Corrupt(format!("library kind '{kind_text}' is unknown"))
            })?;
            libraries.push(Library {
                id,
                name: row.try_get("name")?,
                kind,
                metadata_language: row.try_get("metadata_language")?,
                options: LibraryOptions {
                    key_frames_during_scan: row.try_get("key_frames_during_scan")?,
                    thumbnails_during_scan: row.try_get("thumbnails_during_scan")?,
                    watch_in_real_time: row.try_get("watch_in_real_time")?,
                },
                roots: self.library_roots(id).await?,
            });
        }
        libraries.sort_by_key(|library| {
            LibraryKind::every()
                .iter()
                .position(|kind| *kind == library.kind)
        });
        Ok(libraries)
    }

    /// One library by name, used when reconciling the configuration.
    pub async fn library_by_name(&self, name: &str) -> Result<Option<Library>> {
        Ok(self
            .list_libraries()
            .await?
            .into_iter()
            .find(|library| library.name.eq_ignore_ascii_case(name)))
    }

    /// Roots of one library.
    pub async fn library_roots(&self, library_id: LibraryId) -> Result<Vec<LibraryRoot>> {
        let rows = sqlx::query(
            "SELECT id, label, path FROM library_roots WHERE library_id = ? ORDER BY label",
        )
        .bind(library_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        let mut roots = Vec::with_capacity(rows.len());
        for row in rows {
            roots.push(LibraryRoot {
                id: parse_id(&row.try_get::<String, _>("id")?)?,
                library_id,
                label: row.try_get("label")?,
                path: PathBuf::from(row.try_get::<String, _>("path")?),
            });
        }
        Ok(roots)
    }

    /// Every root of every library along with its recorded access state.
    ///
    /// This is what the diagnostic command and the libraries screen show, so
    /// that a missing mount or a permission problem is visible before a scan
    /// rather than after it.
    pub async fn roots_with_access(&self) -> Result<Vec<RootWithAccess>> {
        let rows = sqlx::query(
            "SELECT id, library_id, label, path, access_state, access_checked_at
             FROM library_roots ORDER BY label",
        )
        .fetch_all(self.reader())
        .await?;

        let mut roots = Vec::with_capacity(rows.len());
        for row in rows {
            let state_text: String = row.try_get("access_state")?;
            roots.push(RootWithAccess {
                root: LibraryRoot {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    library_id: parse_id(&row.try_get::<String, _>("library_id")?)?,
                    label: row.try_get("label")?,
                    path: PathBuf::from(row.try_get::<String, _>("path")?),
                },
                access: parse_access(&state_text),
                checked_at: parse_optional_timestamp(
                    row.try_get::<Option<String>, _>("access_checked_at")?
                        .as_deref(),
                )?,
            });
        }
        Ok(roots)
    }

    /// How many files each root holds, by root.
    ///
    /// A root that holds none is the question the report exists to answer: the
    /// disk is there and readable, and nothing on it was recognised. Nothing
    /// else says so, and a count of zero next to a label says it at a glance.
    pub async fn file_counts_by_root(&self) -> Result<Vec<(LibraryRootId, i64)>> {
        let rows = sqlx::query(
            "SELECT r.id, count(s.id) AS files
             FROM library_roots r
             LEFT JOIN media_sources s ON s.root_id = r.id
             GROUP BY r.id",
        )
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                Ok((
                    parse_id(&row.try_get::<String, _>("id")?)?,
                    row.try_get("files")?,
                ))
            })
            .collect()
    }

    /// Records what a real access test found for one root.
    pub async fn set_root_access(&self, root_id: LibraryRootId, access: RootAccess) -> Result<()> {
        sqlx::query(
            "UPDATE library_roots SET access_state = ?, access_checked_at = ? WHERE id = ?",
        )
        .bind(access.as_str())
        .bind(timestamp_to_text(now()))
        .bind(root_id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Gives a root the name the configuration now calls it by.
    ///
    /// A root is recognised by its path, which is what it is; the label is
    /// what it is called, and what every log line and every screen shows. So
    /// changing the label in the configuration has to reach the screens, and
    /// mistyping one has to be fixable by fixing the file.
    pub async fn rename_root(&self, root_id: LibraryRootId, label: &str) -> Result<()> {
        sqlx::query("UPDATE library_roots SET label = ? WHERE id = ?")
            .bind(label)
            .bind(root_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Adds a root to an existing library.
    pub async fn add_root(
        &self,
        library_id: LibraryId,
        label: &str,
        path: &std::path::Path,
    ) -> Result<LibraryRoot> {
        let id = LibraryRootId::new();
        sqlx::query("INSERT INTO library_roots (id, library_id, label, path) VALUES (?, ?, ?, ?)")
            .bind(id.to_db_string())
            .bind(library_id.to_db_string())
            .bind(label)
            .bind(path.to_string_lossy().as_ref())
            .execute(self.writer())
            .await?;
        Ok(LibraryRoot {
            id,
            library_id,
            label: label.to_string(),
            path: path.to_path_buf(),
        })
    }

    /// What taking something away would take with it.
    ///
    /// Counted before anything is touched, because it is what the screen says
    /// out loud and what a person answers yes to. Never a file on the disk:
    /// nothing here opens, moves or removes one, and the two numbers below
    /// count rows and nothing else.
    pub async fn what_would_go_with_a_library(&self, id: LibraryId) -> Result<WouldGo> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT
                 (SELECT count(*) FROM works WHERE library_id = ?),
                 (SELECT count(*) FROM media_sources s
                   JOIN library_roots r ON r.id = s.root_id
                  WHERE r.library_id = ?)",
        )
        .bind(id.to_db_string())
        .bind(id.to_db_string())
        .fetch_one(self.reader())
        .await?;
        Ok(WouldGo {
            works: row.0,
            files: row.1,
        })
    }

    /// The same for one folder of a library.
    ///
    /// A film held in two folders is not one of the films that would go: it
    /// stays, and loses that copy. Only a film whose every copy came through
    /// this folder disappears with it, which is exactly what makes the number
    /// worth showing rather than guessing at.
    pub async fn what_would_go_with_a_root(&self, root_id: LibraryRootId) -> Result<WouldGo> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT
                 (SELECT count(*) FROM (
                     SELECT s.work_id FROM media_sources s
                      WHERE s.root_id = ?
                      GROUP BY s.work_id
                     HAVING count(*) = (SELECT count(*) FROM media_sources o
                                         WHERE o.work_id = s.work_id)
                 )),
                 (SELECT count(*) FROM media_sources WHERE root_id = ?)",
        )
        .bind(root_id.to_db_string())
        .bind(root_id.to_db_string())
        .fetch_one(self.reader())
        .await?;
        Ok(WouldGo {
            works: row.0,
            files: row.1,
        })
    }

    /// The files recorded under one folder, by identifier.
    ///
    /// Read before the folder goes, for what is keyed on them outside the
    /// database: the sheets of thumbnails, which are big enough that leaving
    /// them behind would show on a disk.
    pub async fn source_ids_of_root(&self, root_id: LibraryRootId) -> Result<Vec<MediaSourceId>> {
        let rows = sqlx::query("SELECT id FROM media_sources WHERE root_id = ?")
            .bind(root_id.to_db_string())
            .fetch_all(self.reader())
            .await?;
        rows.iter()
            .map(|row| parse_id(&row.try_get::<String, _>("id")?))
            .collect()
    }

    /// The same for every folder of a library.
    pub async fn source_ids_of_library(&self, id: LibraryId) -> Result<Vec<MediaSourceId>> {
        let rows = sqlx::query(
            "SELECT s.id FROM media_sources s
              JOIN library_roots r ON r.id = s.root_id
             WHERE r.library_id = ?",
        )
        .bind(id.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter()
            .map(|row| parse_id(&row.try_get::<String, _>("id")?))
            .collect()
    }

    /// Takes a library away, and everything the server knew about it.
    ///
    /// **No file on the disk is touched.** What goes is what this server
    /// wrote down: the films, their pages, their pictures, what was watched of
    /// them, the files as rows and the folders as declared folders. The
    /// collection itself is not this server's to remove.
    ///
    /// Everything hanging off a library follows it out through the schema
    /// rather than through a list kept here, which is what stops a table added
    /// later from being left behind.
    pub async fn delete_library(&self, id: LibraryId) -> Result<Removed> {
        let going = self.what_would_go_with_a_library(id).await?;

        let mut transaction = self.begin().await?;
        sqlx::query("DELETE FROM libraries WHERE id = ?")
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        let swept = sweep_what_nothing_points_at(&mut transaction).await?;
        transaction.commit().await?;

        Ok(Removed {
            works: going.works,
            files: going.files,
            swept,
        })
    }

    /// Takes one folder away from a library, with the films that were only in
    /// it.
    ///
    /// No file on the disk is touched here either. A film also held in another
    /// folder stays and loses that copy; one whose every copy came through
    /// this folder would otherwise be left as a page with nothing behind it,
    /// which is a film nobody can play and nobody can get rid of.
    pub async fn delete_root(&self, root_id: LibraryRootId) -> Result<Removed> {
        let files = self.what_would_go_with_a_root(root_id).await?.files;
        // Read before the folder goes, because the sweep below is bounded by
        // it: a library elsewhere may be in the middle of a scan, and a series
        // it has written down but not yet filled with episodes looks exactly
        // like a film with nothing behind it.
        let library_id: String =
            sqlx::query_scalar("SELECT library_id FROM library_roots WHERE id = ?")
                .bind(root_id.to_db_string())
                .fetch_one(self.reader())
                .await?;

        let mut transaction = self.begin().await?;
        sqlx::query("DELETE FROM library_roots WHERE id = ?")
            .bind(root_id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        // In the same transaction as the folder: a film left with nothing
        // behind it between the two writes is a film somebody could open.
        let orphans = sqlx::query(
            "DELETE FROM works
              WHERE library_id = ?
                AND NOT EXISTS (SELECT 1 FROM media_sources WHERE work_id = works.id)
                AND NOT EXISTS (SELECT 1 FROM works AS child WHERE child.parent_id = works.id)",
        )
        .bind(&library_id)
        .execute(&mut *transaction)
        .await?
        .rows_affected();
        let swept = sweep_what_nothing_points_at(&mut transaction).await?;
        transaction.commit().await?;

        Ok(Removed {
            works: orphans as i64,
            files,
            swept,
        })
    }

    /// How far along every library taken together is.
    ///
    /// A sum rather than a list, for whoever is asking a question about the
    /// whole catalogue: a sum that has moved is a library that has moved, and
    /// a sum that has not cannot hide two changes cancelling out, since each
    /// of these counters only ever goes up.
    pub async fn every_library_version(&self) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT coalesce(sum(version), 0) FROM libraries")
            .fetch_one(self.reader())
            .await?;
        Ok(row.0)
    }

    /// Current version counter of a library.
    ///
    /// Bumped on every write that changes what a listing would return. Backs
    /// entity tags, client cache invalidation and change events, so a client
    /// can tell in one small read whether anything moved.
    pub async fn library_version(&self, library_id: LibraryId) -> Result<i64> {
        let row: (i64,) = sqlx::query_as("SELECT version FROM libraries WHERE id = ?")
            .bind(library_id.to_db_string())
            .fetch_one(self.reader())
            .await?;
        Ok(row.0)
    }

    /// Bumps the version counter of a library.
    pub async fn bump_library_version(&self, library_id: LibraryId) -> Result<i64> {
        let row: (i64,) = sqlx::query_as(
            "UPDATE libraries SET version = version + 1, updated_at = ? WHERE id = ? RETURNING version",
        )
        .bind(timestamp_to_text(now()))
        .bind(library_id.to_db_string())
        .fetch_one(self.writer())
        .await?;
        Ok(row.0)
    }
}

/// Reads a stored access state.
///
/// An unknown value reads as missing, the most cautious of the four: it stops
/// a scan rather than letting it walk a root nobody vouched for.
fn parse_access(value: &str) -> RootAccess {
    match value {
        "read_write" => RootAccess::ReadWrite,
        "read_only" => RootAccess::ReadOnly,
        "unreadable" => RootAccess::Unreadable,
        _ => RootAccess::Missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn database() -> Database {
        Database::open_in_memory().await.expect("database opens")
    }

    fn roots() -> Vec<(String, PathBuf)> {
        vec![
            ("disk-one".to_string(), PathBuf::from("/mnt/one/Films")),
            ("disk-two".to_string(), PathBuf::from("/mnt/two/Films")),
        ]
    }

    /// One root of its own, for a second library alongside the first.
    fn roots_under(name: &str) -> Vec<(String, PathBuf)> {
        vec![(name.to_string(), PathBuf::from(format!("/mnt/one/{name}")))]
    }

    #[tokio::test]
    async fn how_far_along_every_library_is_moves_whatever_moved() {
        // What a question about the whole catalogue is read by. It has to move
        // when a library grows and when one goes away altogether, or an answer
        // counted once would stand for ever.
        let database = database().await;
        assert_eq!(
            database.every_library_version().await.expect("read"),
            0,
            "no library at all is a sum of nothing"
        );

        let first = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");
        let second = database
            .create_library("Séries", LibraryKind::Series, "fr", &roots_under("series"))
            .await
            .expect("library created");
        let both = database.every_library_version().await.expect("read");
        assert_eq!(both, 2, "each library starts at one");

        database
            .bump_library_version(first.id)
            .await
            .expect("bumped");
        assert_eq!(database.every_library_version().await.expect("read"), 3);

        database.delete_library(second.id).await.expect("removed");
        assert!(
            database.every_library_version().await.expect("read") < 3,
            "a library taken away has to move the sum too"
        );
    }

    #[tokio::test]
    async fn the_language_of_a_library_can_be_changed_afterwards() {
        // Read once at creation and never again is what left an installation
        // describing its films in a language nobody there speaks, with no way
        // out short of editing the database by hand.
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");

        assert!(
            database
                .set_metadata_language(library.id, "en")
                .await
                .expect("written"),
            "a real change is a change"
        );
        assert_eq!(
            database.list_libraries().await.expect("listed")[0].metadata_language,
            "en"
        );

        assert!(
            !database
                .set_metadata_language(library.id, "en")
                .await
                .expect("written"),
            "writing the same value again is not a change, and must not send a \
             provider four hundred films to describe once more"
        );
    }

    #[tokio::test]
    async fn what_a_scan_of_a_library_does_is_kept_and_read_back() {
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");
        assert_eq!(
            library.options,
            LibraryOptions::default(),
            "a library nobody has configured leaves the heavy readings to the night"
        );

        let both = LibraryOptions {
            key_frames_during_scan: true,
            thumbnails_during_scan: true,
            watch_in_real_time: false,
        };
        assert!(
            database
                .set_library_options(library.id, both)
                .await
                .expect("written"),
            "a real change is a change"
        );
        assert_eq!(
            database.list_libraries().await.expect("listed")[0].options,
            both
        );

        assert!(
            !database
                .set_library_options(library.id, both)
                .await
                .expect("written"),
            "the same value written again is not a change, and must not set a \
             reading of every film going for nothing"
        );

        let only_one = LibraryOptions {
            key_frames_during_scan: true,
            thumbnails_during_scan: false,
            watch_in_real_time: false,
        };
        assert!(database
            .set_library_options(library.id, only_one)
            .await
            .expect("written"));
        assert_eq!(
            database.list_libraries().await.expect("listed")[0].options,
            only_one,
            "the two switches are two answers, not one"
        );

        let watched = LibraryOptions {
            watch_in_real_time: true,
            ..only_one
        };
        assert!(
            database
                .set_library_options(library.id, watched)
                .await
                .expect("written"),
            "watching a library's folders is a change of its own"
        );
        assert_eq!(
            database.list_libraries().await.expect("listed")[0].options,
            watched
        );
    }

    #[tokio::test]
    async fn a_library_holds_several_roots_which_is_the_ordinary_case() {
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");

        assert_eq!(library.roots.len(), 2);
        let reloaded = database.list_libraries().await.expect("listed");
        assert_eq!(reloaded.len(), 1);
        assert_eq!(reloaded[0].roots.len(), 2);
        assert_eq!(reloaded[0].kind, LibraryKind::Movies);
    }

    #[tokio::test]
    async fn libraries_are_listed_by_kind_and_then_by_name() {
        let database = database().await;
        for (name, kind) in [
            ("Anime", LibraryKind::Anime),
            ("Zoo films", LibraryKind::Movies),
            ("Holidays", LibraryKind::HomeMedia),
            ("Classic films", LibraryKind::Movies),
            ("Shows", LibraryKind::Shows),
            ("Series", LibraryKind::Series),
        ] {
            let folder = PathBuf::from(format!("/media/{name}"));
            database
                .create_library(name, kind, "en", &[(name.to_string(), folder)])
                .await
                .expect("library created");
        }

        let names: Vec<String> = database
            .list_libraries()
            .await
            .expect("listed")
            .into_iter()
            .map(|library| library.name)
            .collect();
        assert_eq!(
            names,
            ["Classic films", "Zoo films", "Series", "Anime", "Holidays", "Shows"]
        );
    }

    #[tokio::test]
    async fn a_root_that_holds_nothing_is_counted_as_holding_nothing() {
        // A disk added and left out of every count looks exactly like a disk
        // that worked, which is the whole reason this is counted at all.
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");
        let work = database
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        database
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new("Quiet Harbour 1080p.mkv"),
                1_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");

        let counts: std::collections::HashMap<_, _> = database
            .file_counts_by_root()
            .await
            .expect("read")
            .into_iter()
            .collect();

        assert_eq!(
            counts.len(),
            2,
            "every root is counted, empty ones included"
        );
        assert_eq!(counts.get(&library.roots[0].id), Some(&1));
        assert_eq!(counts.get(&library.roots[1].id), Some(&0));
    }

    #[tokio::test]
    async fn root_access_starts_unknown_and_is_recorded_after_a_real_test() {
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");

        let before = database.roots_with_access().await.expect("roots readable");
        assert!(before
            .iter()
            .all(|entry| entry.access == RootAccess::Missing));
        assert!(before.iter().all(|entry| entry.checked_at.is_none()));

        database
            .set_root_access(library.roots[0].id, RootAccess::ReadOnly)
            .await
            .expect("access recorded");

        let after = database.roots_with_access().await.expect("roots readable");
        let updated = after
            .iter()
            .find(|entry| entry.root.id == library.roots[0].id)
            .expect("root still present");
        assert_eq!(updated.access, RootAccess::ReadOnly);
        assert!(updated.checked_at.is_some());
        assert!(updated.access.is_usable());
        assert!(!updated.access.allows_writing());
    }

    #[tokio::test]
    async fn every_state_a_root_can_be_in_survives_storage() {
        // The diagnostic and the interface both read this back, and a state
        // that returned as something else would either hide an unmounted disk
        // or announce a perfectly good one as gone.
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");
        let root_id = library.roots[0].id;

        for state in [
            RootAccess::ReadWrite,
            RootAccess::ReadOnly,
            RootAccess::Unreadable,
            RootAccess::Missing,
        ] {
            database
                .set_root_access(root_id, state)
                .await
                .expect("access recorded");
            let stored = database
                .roots_with_access()
                .await
                .expect("roots readable")
                .into_iter()
                .find(|entry| entry.root.id == root_id)
                .expect("root still present");
            assert_eq!(stored.access, state, "{state:?}");
        }
    }

    #[tokio::test]
    async fn an_unknown_stored_access_state_reads_as_the_most_cautious_one() {
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");

        sqlx::query("UPDATE library_roots SET access_state = 'something_new' WHERE id = ?")
            .bind(library.roots[0].id.to_db_string())
            .execute(database.writer())
            .await
            .expect("value forced");

        let entries = database.roots_with_access().await.expect("roots readable");
        let updated = entries
            .iter()
            .find(|entry| entry.root.id == library.roots[0].id)
            .expect("root present");
        assert_eq!(updated.access, RootAccess::Missing);
        assert!(
            !updated.access.is_usable(),
            "an unvouched root must never be walked"
        );
    }

    #[tokio::test]
    async fn the_version_counter_starts_at_one_and_moves_on_demand() {
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");

        assert_eq!(database.library_version(library.id).await.expect("read"), 1);
        let bumped = database
            .bump_library_version(library.id)
            .await
            .expect("bumped");
        assert_eq!(bumped, 2);
        assert_eq!(database.library_version(library.id).await.expect("read"), 2);
    }

    #[tokio::test]
    async fn roots_can_be_added_and_removed_after_the_library_exists() {
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");

        let added = database
            .add_root(
                library.id,
                "disk-three",
                std::path::Path::new("/mnt/three/Films"),
            )
            .await
            .expect("root added");
        assert_eq!(
            database
                .library_roots(library.id)
                .await
                .expect("read")
                .len(),
            3
        );

        database.delete_root(added.id).await.expect("root removed");
        assert_eq!(
            database
                .library_roots(library.id)
                .await
                .expect("read")
                .len(),
            2
        );
    }

    /// A library holding one film with one copy in each of its two roots.
    async fn library_with_a_film_on_both_disks() -> (
        Database,
        Library,
        melyxar_core::id::WorkId,
        Vec<MediaSourceId>,
    ) {
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");
        let work = database
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");

        let mut sources = Vec::new();
        for root in &library.roots {
            sources.push(
                database
                    .insert_source(
                        work.id,
                        root.id,
                        std::path::Path::new("Quiet Harbour 1080p.mkv"),
                        1_000,
                        melyxar_core::time::now(),
                    )
                    .await
                    .expect("source recorded"),
            );
        }
        (database, library, work.id, sources)
    }

    #[tokio::test]
    async fn taking_a_library_away_takes_what_the_server_knew_and_says_how_much() {
        let (database, library, work, _) = library_with_a_film_on_both_disks().await;

        let would = database
            .what_would_go_with_a_library(library.id)
            .await
            .expect("counted");
        assert_eq!(
            (would.works, would.files),
            (1, 2),
            "one film, held twice: the numbers somebody answers yes to"
        );

        let went = database.delete_library(library.id).await.expect("removed");
        assert_eq!(
            (went.works, went.files),
            (would.works, would.files),
            "what was counted is what went"
        );
        assert!(database.list_libraries().await.expect("read").is_empty());
        assert!(
            database.work(work).await.expect("read").is_none(),
            "the film goes with the library that held it"
        );
        assert!(
            database.roots_with_access().await.expect("read").is_empty(),
            "and so do its folders"
        );
    }

    #[tokio::test]
    async fn taking_one_folder_away_keeps_a_film_that_is_also_held_elsewhere() {
        // The whole reason the count is worth showing rather than guessing at:
        // a film in two folders loses a copy, it does not disappear.
        let (database, library, work, sources) = library_with_a_film_on_both_disks().await;

        let would = database
            .what_would_go_with_a_root(library.roots[0].id)
            .await
            .expect("counted");
        assert_eq!(
            (would.works, would.files),
            (0, 1),
            "one copy goes, the film stays"
        );

        let went = database
            .delete_root(library.roots[0].id)
            .await
            .expect("removed");
        assert_eq!((went.works, went.files), (would.works, would.files));
        assert!(
            database.work(work).await.expect("read").is_some(),
            "the film is still held by the other folder"
        );
        assert!(database
            .source_by_id(sources[0])
            .await
            .expect("read")
            .is_none());
        assert!(database
            .source_by_id(sources[1])
            .await
            .expect("read")
            .is_some());

        // The second folder is the last one holding it, so this time the film
        // goes: a page with nothing behind it is a film nobody can play and
        // nobody can get rid of.
        let last = database
            .what_would_go_with_a_root(library.roots[1].id)
            .await
            .expect("counted");
        assert_eq!((last.works, last.files), (1, 1));
        let went = database
            .delete_root(library.roots[1].id)
            .await
            .expect("removed");
        assert_eq!((went.works, went.files), (last.works, last.files));
        assert!(database.work(work).await.expect("read").is_none());
        assert_eq!(
            database.list_libraries().await.expect("read").len(),
            1,
            "the library itself stays, with nowhere left to look"
        );
    }

    /// An identification carrying everything a film drags behind it.
    fn identified(title: &str, person: &str, genre: &str) -> crate::metadata::IdentifiedWork {
        crate::metadata::IdentifiedWork {
            provider: "tmdb".to_string(),
            external_id: title.to_string(),
            imdb_id: None,
            language: "fr".to_string(),
            title: title.to_string(),
            sort_title: title.to_lowercase(),
            tagline: None,
            overview: None,
            release_year: Some(2019),
            runtime: None,
            community_rating: None,
            age_rating_label: None,
            genres: vec![genre.to_string()],
            studios: vec!["Atelier Nord".to_string()],
            credits: vec![crate::metadata::CreditRecord {
                external_id: person.to_string(),
                name: person.to_string(),
                sort_name: person.to_lowercase(),
                role: "actor".to_string(),
                character: None,
                ordinal: 0,
                photo_path: Some(format!("/{person}.jpg")),
            }],
            collection: None,
            trailers: Vec::new(),
        }
    }

    #[tokio::test]
    async fn a_removal_takes_the_names_and_pictures_nothing_points_at_any_more() {
        // The whole reason this matters: none of it hangs off a library, so
        // none of it follows one out. Left alone, years of use fill the
        // database with names nobody can reach and the cache with faces
        // nobody will see.
        let database = database().await;
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");
        let work = database
            .create_work(
                library.id,
                melyxar_core::work::WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        database
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new("Quiet Harbour 1080p.mkv"),
                1_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");
        let credited = database
            .apply_identification(
                work.id,
                &identified("Quiet Harbour", "Alix Moreau", "Drama"),
                false,
            )
            .await
            .expect("identified");

        // A poster for the film and a face for the one name it credits.
        database
            .replace_images(
                "work",
                &work.id.to_db_string(),
                "poster",
                &[crate::images::StoredImage {
                    owner_kind: "work".to_string(),
                    owner_id: work.id.to_db_string(),
                    image_kind: "poster".to_string(),
                    relative_path: format!("works/{}/poster-abc-400.webp", work.id),
                    width: Some(400),
                    height: Some(600),
                    fingerprint: "abc".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("poster stored");
        let person = credited[0].person_id;
        database
            .replace_images(
                "person",
                &person.to_db_string(),
                "photo",
                &[crate::images::StoredImage {
                    owner_kind: "person".to_string(),
                    owner_id: person.to_db_string(),
                    image_kind: "photo".to_string(),
                    relative_path: format!("people/{person}/photo-abc-192.webp"),
                    width: Some(192),
                    height: Some(288),
                    fingerprint: "abc".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("face stored");

        let went = database.delete_library(library.id).await.expect("removed");

        assert_eq!(went.swept.people, 1, "the one name nothing credits now");
        assert_eq!(went.swept.genres, 1);
        assert_eq!(went.swept.studios, 1);
        assert_eq!(
            went.swept.pictures, 2,
            "the poster of the film and the face of its cast"
        );
        assert_eq!(
            went.swept.picture_paths.len(),
            2,
            "and their files are named, so the cache can be swept too"
        );

        // Nothing at all is left behind in any of the shared tables: this is
        // the assertion the whole thing exists for.
        let left: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM works),
                    (SELECT count(*) FROM credits),
                    (SELECT count(*) FROM people),
                    (SELECT count(*) FROM genres),
                    (SELECT count(*) FROM studios),
                    (SELECT count(*) FROM images)",
        )
        .fetch_one(database.reader())
        .await
        .expect("counted");
        assert_eq!(
            left,
            (0, 0, 0, 0, 0, 0),
            "works, credits, people, genres, studios and images: not one row \
             nothing can reach is left behind"
        );
    }

    #[tokio::test]
    async fn a_removal_never_takes_a_name_another_film_still_credits() {
        // The other half of the promise: the sweep is bounded by what is left,
        // so an actor who is also in a film of another library keeps their row
        // and their face.
        let database = database().await;
        let mut kept_person = None;
        let mut libraries = Vec::new();
        for (name, title) in [("Films", "Quiet Harbour"), ("Anime", "Amber Field")] {
            let library = database
                .create_library(name, LibraryKind::Movies, "fr", &roots_under(name))
                .await
                .expect("library created");
            let work = database
                .create_work(
                    library.id,
                    melyxar_core::work::WorkKind::Movie,
                    title,
                    &title.to_lowercase(),
                    Some(2019),
                )
                .await
                .expect("work created");
            database
                .insert_source(
                    work.id,
                    library.roots[0].id,
                    std::path::Path::new("film.mkv"),
                    1_000,
                    melyxar_core::time::now(),
                )
                .await
                .expect("source recorded");
            let credited = database
                .apply_identification(work.id, &identified(title, "Alix Moreau", "Drama"), false)
                .await
                .expect("identified");
            kept_person = Some(credited[0].person_id);
            libraries.push(library);
        }
        let person = kept_person.expect("both films credit the same name");

        let went = database
            .delete_library(libraries[0].id)
            .await
            .expect("removed");
        assert_eq!(
            went.swept.people, 0,
            "the other film still credits them, so they stay"
        );
        assert_eq!(went.swept.genres, 0, "and so does the genre they share");

        // And once the second library goes too, they finally do.
        let last = database
            .delete_library(libraries[1].id)
            .await
            .expect("removed");
        assert_eq!(last.swept.people, 1);
        assert!(database
            .credit_photos_of_work(melyxar_core::id::WorkId::new())
            .await
            .expect("read")
            .is_empty());
        let _ = person;
    }

    #[tokio::test]
    async fn taking_a_folder_away_never_reaches_into_another_library() {
        // A series written down before its episodes have been found looks
        // exactly like a film with nothing behind it. One being scanned in
        // another library must not be swept away by a folder going here.
        let (database, library, _, _) = library_with_a_film_on_both_disks().await;
        let elsewhere = database
            .create_library("Series", LibraryKind::Series, "fr", &roots())
            .await
            .expect("library created");
        let half_scanned = database
            .create_work(
                elsewhere.id,
                melyxar_core::work::WorkKind::Series,
                "Lantern Road",
                "lantern road",
                None,
            )
            .await
            .expect("work created");

        database
            .delete_root(library.roots[0].id)
            .await
            .expect("removed");
        assert!(
            database
                .work(half_scanned.id)
                .await
                .expect("read")
                .is_some(),
            "the other library's work is none of this removal's business"
        );
    }

    #[tokio::test]
    async fn a_library_that_is_gone_can_be_declared_again_from_nothing() {
        // What somebody does straight after taking one away by mistake. The
        // files never moved, so it comes back whole on the next scan.
        let (database, library, _, _) = library_with_a_film_on_both_disks().await;
        database.delete_library(library.id).await.expect("removed");

        let again = database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("declared again");
        assert_eq!(again.roots.len(), 2);
        assert_ne!(again.id, library.id);
    }

    #[tokio::test]
    async fn a_library_is_found_back_by_name_whatever_the_case() {
        let database = database().await;
        database
            .create_library("Films", LibraryKind::Movies, "fr", &roots())
            .await
            .expect("library created");
        assert!(database
            .library_by_name("films")
            .await
            .expect("lookup works")
            .is_some());
        assert!(database
            .library_by_name("Séries")
            .await
            .expect("lookup works")
            .is_none());
    }

    #[tokio::test]
    async fn every_library_kind_survives_a_round_trip_through_storage() {
        let database = database().await;
        for (name, kind) in [
            ("Films", LibraryKind::Movies),
            ("Series", LibraryKind::Series),
            ("Anime", LibraryKind::Anime),
            ("Shows", LibraryKind::Shows),
            ("Music", LibraryKind::Music),
        ] {
            database
                .create_library(name, kind, "fr", &roots())
                .await
                .expect("library created");
        }
        let stored = database.list_libraries().await.expect("listed");
        assert_eq!(stored.len(), 5);
        assert!(stored
            .iter()
            .any(|library| library.kind == LibraryKind::Music));
        assert!(stored
            .iter()
            .any(|library| library.kind == LibraryKind::Shows));
    }
}
