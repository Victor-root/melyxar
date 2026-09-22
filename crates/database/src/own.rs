//! What people filmed and photographed themselves, kept by folder.
//!
//! A library of home media is in no catalogue. Every file is a work of its
//! own, named by its file, and every folder on the disk is a work holding
//! what was put in it. Both are written down as their own from the start:
//! nothing will ever look them up, so nothing is waiting.

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::work::{IdentificationState, Work, WorkKind};
use sqlx::AssertSqlSafe;

use crate::catalogue::{insert_work, what_a_work_is, work_from_row, Placed};
use crate::{Database, Result};

impl Database {
    /// The folder of that name inside another, or at the root of the
    /// library, written down if it is not there yet.
    ///
    /// Found by its name and where it sits, so one folder name on two disks
    /// of the same library is one folder, the way folders of one name make
    /// one library.
    pub async fn own_folder(
        &self,
        library_id: LibraryId,
        parent: Option<WorkId>,
        name: &str,
        sort_title: &str,
    ) -> Result<Work> {
        let found = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM works
              WHERE library_id = ? AND kind = 'folder' AND parent_id IS ? AND title = ?",
            what_a_work_is("")
        )))
        .bind(library_id.to_db_string())
        .bind(parent.map(|folder| folder.to_db_string()))
        .bind(name)
        .fetch_optional(self.reader())
        .await?;
        if let Some(row) = found {
            return work_from_row(&row);
        }
        self.create_own_work(library_id, parent, WorkKind::Folder, name, sort_title)
            .await
    }

    /// Writes down a folder, a video or a photo of a library of home media.
    ///
    /// The folder it is kept in has its count of what it holds brought up to
    /// date in the same transaction, counted rather than added to, as for a
    /// season.
    pub async fn create_own_work(
        &self,
        library_id: LibraryId,
        folder: Option<WorkId>,
        kind: WorkKind,
        title: &str,
        sort_title: &str,
    ) -> Result<Work> {
        let mut transaction = self.begin().await?;
        let mut work = insert_work(
            &mut *transaction,
            Placed::in_folder(library_id, folder),
            kind,
            title,
            sort_title,
            None,
        )
        .await?;
        sqlx::query("UPDATE works SET identification = ? WHERE id = ?")
            .bind(IdentificationState::Own.as_str())
            .bind(work.id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        work.identification = IdentificationState::Own;
        if let Some(folder) = folder {
            sqlx::query(
                "UPDATE works
                    SET child_count = (SELECT count(*) FROM works AS child
                                        WHERE child.parent_id = works.id)
                  WHERE id = ?",
            )
            .bind(folder.to_db_string())
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(work)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use melyxar_core::library::LibraryKind;

    use super::*;

    async fn library() -> (Database, LibraryId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Family",
                LibraryKind::HomeMedia,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Family"))],
            )
            .await
            .expect("library created");
        (database, library.id)
    }

    #[tokio::test]
    async fn a_folder_is_found_again_by_its_name_and_where_it_sits() {
        let (database, library_id) = library().await;
        let summer = database
            .own_folder(library_id, None, "Summer", "summer")
            .await
            .expect("written");
        let again = database
            .own_folder(library_id, None, "Summer", "summer")
            .await
            .expect("found");
        assert_eq!(summer.id, again.id, "one name at one place is one folder");
        assert_eq!(summer.kind, WorkKind::Folder);
        assert_eq!(summer.identification, IdentificationState::Own);

        let inside = database
            .own_folder(library_id, Some(summer.id), "Summer", "summer")
            .await
            .expect("written");
        assert_ne!(
            inside.id, summer.id,
            "a folder of the same name inside it is another folder"
        );
        assert_eq!(inside.parent_id, Some(summer.id));
    }

    #[tokio::test]
    async fn a_file_is_its_own_work_and_counts_in_its_folder() {
        let (database, library_id) = library().await;
        let summer = database
            .own_folder(library_id, None, "Summer", "summer")
            .await
            .expect("written");
        for name in ["IMG_0001", "IMG_0001"] {
            let photo = database
                .create_own_work(library_id, Some(summer.id), WorkKind::Photo, name, name)
                .await
                .expect("written");
            let stored = database
                .work(photo.id)
                .await
                .expect("read")
                .expect("present");
            assert_eq!(stored.identification, IdentificationState::Own);
            assert_eq!(stored.parent_id, Some(summer.id));
            assert_eq!(stored.ordinal, None);
        }
        let (held,): (i64,) = sqlx::query_as("SELECT child_count FROM works WHERE id = ?")
            .bind(summer.id.to_db_string())
            .fetch_one(database.reader())
            .await
            .expect("read");
        assert_eq!(held, 2, "two files of one name are two photos, never one");
    }
}
