//! Libraries and their root folders.

use std::path::PathBuf;
use std::str::FromStr;

use melyxar_core::id::{LibraryId, LibraryRootId};
use melyxar_core::library::{Library, LibraryKind, LibraryRoot, RootAccess};
use melyxar_core::time::{now, Timestamp};
use sqlx::Row;

use crate::convert::{parse_optional_timestamp, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

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
            roots: stored_roots,
        })
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

    /// Every library with its roots, ordered by name.
    pub async fn list_libraries(&self) -> Result<Vec<Library>> {
        let rows = sqlx::query(
            "SELECT id, name, kind, metadata_language FROM libraries ORDER BY name COLLATE NOCASE",
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
                roots: self.library_roots(id).await?,
            });
        }
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

    /// Removes a root, along with everything found through it.
    pub async fn remove_root(&self, root_id: LibraryRootId) -> Result<()> {
        sqlx::query("DELETE FROM library_roots WHERE id = ?")
            .bind(root_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
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

fn parse_id<T: FromStr>(value: &str) -> Result<T> {
    value
        .parse()
        .map_err(|_| DatabaseError::Corrupt(format!("identifier '{value}' is malformed")))
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

        database.remove_root(added.id).await.expect("root removed");
        assert_eq!(
            database
                .library_roots(library.id)
                .await
                .expect("read")
                .len(),
            2
        );
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
