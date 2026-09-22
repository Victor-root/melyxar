//! Files that left one place in a library and turned up in another.
//!
//! A file that is not on the disk any more is marked absent and never
//! forgotten, so that a disk left unplugged costs an evening and not a
//! library. A file that was moved is not absent, though: it is right there
//! under another folder, and the scan has already written it down again as
//! something new. What is here recognises the two as one file and keeps only
//! where it is now.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, MediaSourceId, WorkId};
use sqlx::Row;

use crate::catalogue::merge_within;
use crate::convert::parse_id;
use crate::{Database, Result};

/// A file marked absent from one place and present in another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MovedFile {
    /// The row of the place it left.
    pub gone: MediaSourceId,
    pub gone_work: WorkId,
    /// Where it is now, and the work that place gave it.
    pub here: PathBuf,
    pub here_work: WorkId,
}

impl Database {
    /// The files of a library marked absent that are present somewhere else
    /// in it.
    ///
    /// Recognised by what a move keeps and a different file would not all
    /// share: the same name, the same size and the same date. A file with two
    /// such twins is left alone, since nothing says which of them it became.
    pub async fn moved_files(&self, library_id: LibraryId) -> Result<Vec<MovedFile>> {
        let rows = sqlx::query(
            "SELECT gone.id AS gone_id, gone.work_id AS gone_work,
                    gone.relative_path AS gone_path,
                    here.work_id AS here_work, here.relative_path AS here_path
             FROM media_sources gone
             JOIN library_roots gone_root ON gone_root.id = gone.root_id
             JOIN media_sources here ON here.size_bytes = gone.size_bytes
                                    AND here.modified_at = gone.modified_at
                                    AND here.missing_since IS NULL
             JOIN library_roots here_root ON here_root.id = here.root_id
                                         AND here_root.library_id = gone_root.library_id
             WHERE gone_root.library_id = ? AND gone.missing_since IS NOT NULL",
        )
        .bind(library_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        let mut twins: Vec<(MediaSourceId, MovedFile)> = Vec::new();
        for row in &rows {
            let gone_path = PathBuf::from(row.try_get::<String, _>("gone_path")?);
            let here = PathBuf::from(row.try_get::<String, _>("here_path")?);
            if gone_path.file_name() != here.file_name() {
                continue;
            }
            let gone: MediaSourceId = parse_id(&row.try_get::<String, _>("gone_id")?)?;
            twins.push((
                gone,
                MovedFile {
                    gone,
                    gone_work: parse_id(&row.try_get::<String, _>("gone_work")?)?,
                    here,
                    here_work: parse_id(&row.try_get::<String, _>("here_work")?)?,
                },
            ));
        }

        Ok(twins
            .iter()
            .filter(|(gone, _)| twins.iter().filter(|(other, _)| other == gone).count() == 1)
            .map(|(_, moved)| moved.clone())
            .collect())
    }

    /// Forgets the place a file left, and hands what it carried to where it
    /// went.
    ///
    /// The row of the old place goes. The work it stood for goes too when that
    /// row was all it held, and it goes by joining the work of the new place,
    /// which is what carries over where everybody was in it and what they
    /// watched. A work still holding another copy or anything under it stays.
    ///
    /// The paths of the pictures that went with it come back, for the caller
    /// to take out of the cache.
    pub async fn follow_moved_file(&self, moved: &MovedFile) -> Result<Vec<String>> {
        let mut transaction = self.begin().await?;
        sqlx::query("DELETE FROM media_sources WHERE id = ?")
            .bind(moved.gone.to_db_string())
            .execute(&mut *transaction)
            .await?;

        let left_empty: bool = sqlx::query_scalar(
            "SELECT NOT EXISTS (SELECT 1 FROM media_sources WHERE work_id = ?1)
                AND NOT EXISTS (SELECT 1 FROM works WHERE parent_id = ?1)",
        )
        .bind(moved.gone_work.to_db_string())
        .fetch_one(&mut *transaction)
        .await?;

        let no_longer_used = if left_empty && moved.gone_work != moved.here_work {
            merge_within(&mut transaction, moved.gone_work, moved.here_work).await?
        } else {
            Vec::new()
        };
        transaction.commit().await?;
        Ok(no_longer_used)
    }
}

#[cfg(test)]
mod tests {
    use melyxar_core::id::{LibraryRootId, UserId};
    use melyxar_core::library::LibraryKind;
    use melyxar_core::time::{now, Millis, Timestamp};
    use melyxar_core::work::{PlaybackState, WorkKind};

    use super::*;

    async fn library() -> (Database, LibraryId, LibraryRootId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Anime",
                LibraryKind::Anime,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Anime"))],
            )
            .await
            .expect("library created");
        let root_id = library.roots[0].id;
        (database, library.id, root_id)
    }

    async fn a_work(database: &Database, library_id: LibraryId, title: &str) -> WorkId {
        database
            .create_work(library_id, WorkKind::Episode, title, title, None)
            .await
            .expect("work written")
            .id
    }

    async fn a_file(
        database: &Database,
        root_id: LibraryRootId,
        work: WorkId,
        path: &str,
        modified_at: Timestamp,
    ) -> MediaSourceId {
        database
            .insert_source(work, root_id, &PathBuf::from(path), 12, modified_at)
            .await
            .expect("file written down")
    }

    async fn a_viewer(database: &Database) -> UserId {
        database
            .create_user("Viewer", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created")
            .id
    }

    #[tokio::test]
    async fn a_file_that_moved_is_found_by_its_twin_and_followed() {
        let (database, library_id, root_id) = library().await;
        let moment = now();
        let old = a_work(&database, library_id, "16 - Wild Run").await;
        let new = a_work(&database, library_id, "Episode 16").await;
        let gone = a_file(&database, root_id, old, "Road/16 - Wild Run.mp4", moment).await;
        a_file(
            &database,
            root_id,
            new,
            "Road/Saison 1/16 - Wild Run.mp4",
            moment,
        )
        .await;
        database.mark_source_missing(gone).await.expect("marked");

        let viewer = a_viewer(&database).await;
        database
            .record_playback_progress(viewer, old, Millis::new(0), PlaybackState::Watched, now())
            .await
            .expect("watched");

        let moved = database.moved_files(library_id).await.expect("read");
        assert_eq!(moved.len(), 1);
        assert_eq!(moved[0].here_work, new);

        database
            .follow_moved_file(&moved[0])
            .await
            .expect("followed");

        assert!(database.work(old).await.expect("read").is_none());
        assert_eq!(
            database.sources_of_root(root_id).await.expect("read").len(),
            1
        );
        assert_eq!(
            database
                .playback_progress(viewer, new)
                .await
                .expect("read")
                .map(|progress| progress.state),
            Some(PlaybackState::Watched),
            "what was watched goes where the file went"
        );
        assert!(database
            .moved_files(library_id)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn a_different_file_or_one_with_two_twins_is_left_alone() {
        let (database, library_id, root_id) = library().await;
        let moment = now();
        let later = moment + time::Duration::seconds(5);
        let old = a_work(&database, library_id, "Wild Run").await;
        let gone = a_file(&database, root_id, old, "Road/Wild Run.mp4", moment).await;
        database.mark_source_missing(gone).await.expect("marked");

        // Same name, another date: another file.
        let other = a_work(&database, library_id, "Other").await;
        a_file(&database, root_id, other, "Elsewhere/Wild Run.mp4", later).await;
        assert!(database
            .moved_files(library_id)
            .await
            .expect("read")
            .is_empty());

        // Two exact twins: nothing says which one it became.
        a_file(&database, root_id, other, "One/Wild Run.mp4", moment).await;
        a_file(&database, root_id, other, "Two/Wild Run.mp4", moment).await;
        assert!(database
            .moved_files(library_id)
            .await
            .expect("read")
            .is_empty());
    }
}
