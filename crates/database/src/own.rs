//! What people filmed and photographed themselves, kept by folder.
//!
//! A library of home media is in no catalogue. Every file is a work of its
//! own, named by its file, and every folder on the disk is a work holding
//! what was put in it. Both are written down as their own from the start:
//! nothing will ever look them up, so nothing is waiting.

use std::path::PathBuf;

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::time::Millis;
use melyxar_core::work::{IdentificationState, Work, WorkKind};
use sqlx::{AssertSqlSafe, Row};

use crate::catalogue::{insert_work, what_a_work_is, work_from_row, Placed};
use crate::convert::parse_id;
use crate::images::{image_from_row, StoredImage, WHAT_A_PICTURE_IS};
use crate::{Database, DatabaseError, Result};

/// A video or a photo whose card has no picture made the way pictures are
/// made today, with what its picture is made from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OwnFileToPicture {
    pub work_id: WorkId,
    pub kind: WorkKind,
    /// Where the file is, root included.
    pub path: PathBuf,
    /// How long a video runs, which says where its picture is taken.
    pub duration: Option<Millis>,
    /// What says which state of the file the picture was made from: where it
    /// is, how large it is and when it last changed. A file changed on the
    /// disk is pictured again.
    pub made_from: String,
}

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

impl Database {
    /// The picture each of these folders is shown with: every size of the
    /// picture of the first thing inside it that has one.
    ///
    /// First by depth, then in the order the folder shows what it holds: what
    /// sits in the folder itself before what sits in a folder inside it.
    /// Nothing is copied: the folder is shown the same picture, read at the
    /// same moment, so it follows whatever becomes of that picture.
    pub async fn pictures_lent_to_folders(
        &self,
        folders: &[WorkId],
    ) -> Result<Vec<(WorkId, StoredImage)>> {
        if folders.is_empty() {
            return Ok(Vec::new());
        }
        let places = vec!["?"; folders.len()].join(", ");
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "WITH RECURSIVE inside(folder_id, work_id, depth) AS (
                 SELECT parent_id, id, 1 FROM works WHERE parent_id IN ({places})
                 UNION ALL
                 SELECT inside.folder_id, works.id, inside.depth + 1
                   FROM inside JOIN works ON works.parent_id = inside.work_id
             ),
             chosen AS (
                 SELECT folder_id, work_id FROM (
                     SELECT inside.folder_id, inside.work_id,
                            row_number() OVER (PARTITION BY inside.folder_id
                                               ORDER BY inside.depth, w.sort_title, w.id) AS rank
                       FROM inside JOIN works w ON w.id = inside.work_id
                      WHERE EXISTS (SELECT 1 FROM images p
                                     WHERE p.owner_kind = 'work' AND p.owner_id = inside.work_id
                                       AND p.image_kind = 'poster'))
                  WHERE rank = 1
             )
             SELECT chosen.folder_id, {WHAT_A_PICTURE_IS} FROM chosen
               JOIN images ON images.owner_kind = 'work' AND images.owner_id = chosen.work_id
                          AND images.image_kind = 'poster'
              ORDER BY images.width DESC"
        )));
        for folder in folders {
            query = query.bind(folder.to_db_string());
        }
        query
            .fetch_all(self.reader())
            .await?
            .iter()
            .map(|row| {
                Ok((
                    parse_id(&row.try_get::<String, _>("folder_id")?)?,
                    image_from_row(row)?,
                ))
            })
            .collect()
    }

    /// The photos just before and just after this one in the folder it sits
    /// in, in the order the folder shows them, for whoever is looking through
    /// them one by one.
    pub async fn neighbouring_photos(
        &self,
        photo: &Work,
    ) -> Result<(Option<WorkId>, Option<WorkId>)> {
        let mut found = [None, None];
        for (slot, (side, order)) in [("<", "DESC"), (">", "ASC")].iter().enumerate() {
            let row = sqlx::query(AssertSqlSafe(format!(
                "SELECT id FROM works
                  WHERE library_id = ? AND parent_id IS ? AND kind = 'photo'
                    AND (sort_title, id) {side} (?, ?)
                  ORDER BY sort_title {order}, id {order}
                  LIMIT 1"
            )))
            .bind(photo.library_id.to_db_string())
            .bind(photo.parent_id.map(|folder| folder.to_db_string()))
            .bind(&photo.sort_title)
            .bind(photo.id.to_db_string())
            .fetch_optional(self.reader())
            .await?;
            found[slot] = row
                .map(|row| parse_id(&row.try_get::<String, _>("id")?))
                .transpose()?;
        }
        let [before, after] = found;
        Ok((before, after))
    }

    /// The videos and photos of a library of home media whose card has no
    /// picture made by `recipe`, the way pictures are prepared today.
    ///
    /// A video is only offered once it has been analysed, since where its
    /// picture is taken depends on how long it runs. A photo is offered as
    /// soon as it is found. A file that is not on the disk is not offered.
    pub async fn own_files_to_picture(
        &self,
        library_id: LibraryId,
        recipe: &str,
    ) -> Result<Vec<OwnFileToPicture>> {
        let rows = sqlx::query(
            "SELECT w.id, w.kind, r.path AS root_path, s.relative_path, s.duration_ms,
                    s.size_bytes, s.modified_at
               FROM works w
               JOIN media_sources s ON s.work_id = w.id
               JOIN library_roots r ON r.id = s.root_id
              WHERE w.library_id = ?
                AND w.kind IN ('video', 'photo')
                AND s.missing_since IS NULL
                AND (w.kind = 'photo' OR s.analysed_at IS NOT NULL)
                AND NOT EXISTS (
                    SELECT 1 FROM images i
                     WHERE i.owner_kind = 'work' AND i.owner_id = w.id
                       AND i.image_kind = 'poster' AND i.fingerprint LIKE ?)
              ORDER BY w.added_at",
        )
        .bind(library_id.to_db_string())
        .bind(format!("{recipe}-%"))
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                let kind_text: String = row.try_get("kind")?;
                let relative_path: String = row.try_get("relative_path")?;
                let size_bytes: i64 = row.try_get("size_bytes")?;
                let modified_at: String = row.try_get("modified_at")?;
                Ok(OwnFileToPicture {
                    work_id: parse_id(&row.try_get::<String, _>("id")?)?,
                    kind: WorkKind::parse(&kind_text).ok_or_else(|| {
                        DatabaseError::Corrupt(format!("work kind '{kind_text}'"))
                    })?,
                    path: PathBuf::from(row.try_get::<String, _>("root_path")?)
                        .join(&relative_path),
                    duration: row
                        .try_get::<Option<i64>, _>("duration_ms")?
                        .map(Millis::new),
                    made_from: format!("{relative_path}|{size_bytes}|{modified_at}"),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::catalogue::SourceAnalysis;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::time::now;

    use super::*;

    #[tokio::test]
    async fn what_is_left_to_picture_is_each_file_without_a_picture_of_today() {
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
        let root = library.roots[0].id;
        let a_file = |kind: WorkKind, name: &'static str| {
            let database = &database;
            async move {
                let work = database
                    .create_own_work(library.id, None, kind, name, name)
                    .await
                    .expect("written");
                let source = database
                    .insert_source(work.id, root, &PathBuf::from(name), 42, now())
                    .await
                    .expect("recorded");
                (work.id, source)
            }
        };

        let (photo, _) = a_file(WorkKind::Photo, "beach.jpg").await;
        let (clip, clip_source) = a_file(WorkKind::Video, "birthday.mp4").await;
        let (_, gone_source) = a_file(WorkKind::Photo, "gone.jpg").await;
        database
            .mark_source_missing(gone_source)
            .await
            .expect("marked");

        let waiting = |recipe: &'static str| {
            let database = &database;
            async move {
                database
                    .own_files_to_picture(library.id, recipe)
                    .await
                    .expect("read")
                    .into_iter()
                    .map(|file| file.work_id)
                    .collect::<Vec<_>>()
            }
        };
        assert_eq!(
            waiting("b5").await,
            vec![photo],
            "a video waits for its length to be known, and a file gone waits for nothing"
        );

        database
            .store_analysis(
                clip_source,
                &SourceAnalysis {
                    container: Some("mov,mp4".to_string()),
                    duration: Some(Millis::new(60_000)),
                    overall_bitrate: None,
                },
                &[],
                &[],
            )
            .await
            .expect("analysed");
        let listed = database
            .own_files_to_picture(library.id, "b5")
            .await
            .expect("read");
        let video = listed
            .iter()
            .find(|file| file.work_id == clip)
            .expect("the video is offered once analysed");
        assert_eq!(video.duration, Some(Millis::new(60_000)));
        assert_eq!(video.path, PathBuf::from("/mnt/one/Family/birthday.mp4"));
        assert!(video.made_from.starts_with("birthday.mp4|42|"));

        database
            .replace_images(
                "work",
                &photo.to_db_string(),
                "poster",
                &[StoredImage {
                    owner_kind: "work".to_string(),
                    owner_id: photo.to_db_string(),
                    image_kind: "poster".to_string(),
                    relative_path: "works/x/poster-400.webp".to_string(),
                    width: Some(400),
                    height: Some(300),
                    fingerprint: "b5-abc".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("pictured");
        assert_eq!(waiting("b5").await, vec![clip]);
        assert_eq!(
            waiting("b6").await.len(),
            2,
            "a picture made another way is made again"
        );
    }

    #[tokio::test]
    async fn a_folder_is_shown_with_the_first_picture_found_inside_it() {
        let (database, library_id) = library().await;
        let a_picture = |work: WorkId, name: &'static str| {
            let database = &database;
            async move {
                database
                    .replace_images(
                        "work",
                        &work.to_db_string(),
                        "poster",
                        &[StoredImage {
                            owner_kind: "work".to_string(),
                            owner_id: work.to_db_string(),
                            image_kind: "poster".to_string(),
                            relative_path: format!("works/{name}-400.webp"),
                            width: Some(400),
                            height: Some(300),
                            fingerprint: name.to_string(),
                            dominant_color: None,
                        }],
                    )
                    .await
                    .expect("pictured");
            }
        };
        let a_photo = |folder: WorkId, name: &'static str| {
            let database = &database;
            async move {
                database
                    .create_own_work(library_id, Some(folder), WorkKind::Photo, name, name)
                    .await
                    .expect("written")
                    .id
            }
        };

        let summer = database
            .own_folder(library_id, None, "Summer", "summer")
            .await
            .expect("written");
        let deeper = database
            .own_folder(library_id, Some(summer.id), "Beach", "beach")
            .await
            .expect("written");
        let inside = a_photo(deeper.id, "a-sunset").await;
        a_picture(inside, "sunset").await;
        // Nearer the top than the sunset, and further down the alphabet: it
        // still comes first, being in the folder itself.
        let near = a_photo(summer.id, "z-harbour").await;
        a_picture(near, "harbour").await;
        // A photo with no picture yet lends nothing.
        a_photo(summer.id, "b-unpictured").await;

        let only_deep = database
            .own_folder(library_id, None, "Winter", "winter")
            .await
            .expect("written");
        let hidden = database
            .own_folder(library_id, Some(only_deep.id), "Snow", "snow")
            .await
            .expect("written");
        let snowman = a_photo(hidden.id, "snowman").await;
        a_picture(snowman, "snowman").await;

        let page = database
            .browse_works(&crate::browse::BrowseRequest {
                library_id: Some(library_id),
                limit: 20,
                ..Default::default()
            })
            .await
            .expect("read");
        let shown = |title: &str| {
            page.cards
                .iter()
                .find(|card| card.title == title)
                .expect("the folder is at the root")
                .poster
                .iter()
                .map(|picture| picture.fingerprint.clone())
                .collect::<Vec<_>>()
        };
        assert_eq!(shown("Summer"), ["harbour"]);
        assert_eq!(
            shown("Winter"),
            ["snowman"],
            "a folder holding only folders shows what is further in"
        );
    }

    #[tokio::test]
    async fn photos_are_looked_through_in_the_order_their_folder_shows_them() {
        let (database, library_id) = library().await;
        let summer = database
            .own_folder(library_id, None, "Summer", "summer")
            .await
            .expect("written");
        let mut photos = Vec::new();
        for name in ["c", "a", "b"] {
            photos.push(
                database
                    .create_own_work(library_id, Some(summer.id), WorkKind::Photo, name, name)
                    .await
                    .expect("written"),
            );
        }
        // Neither a video nor a photo elsewhere is looked through with them.
        database
            .create_own_work(library_id, Some(summer.id), WorkKind::Video, "ab", "ab")
            .await
            .expect("written");
        database
            .create_own_work(library_id, None, WorkKind::Photo, "b2", "b2")
            .await
            .expect("written");
        database
            .own_folder(library_id, Some(summer.id), "zz", "zz")
            .await
            .expect("written");

        let [c, a, b] = [&photos[0], &photos[1], &photos[2]];
        assert_eq!(
            database.neighbouring_photos(b).await.expect("read"),
            (Some(a.id), Some(c.id))
        );
        assert_eq!(
            database.neighbouring_photos(a).await.expect("read"),
            (None, Some(b.id))
        );
        assert_eq!(
            database.neighbouring_photos(c).await.expect("read"),
            (Some(b.id), None)
        );

        let viewer = database
            .create_user("Viewer", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created");
        let shown: Vec<String> = database
            .children_of(viewer.id, summer.id, "fr")
            .await
            .expect("read")
            .into_iter()
            .map(|child| child.card.title)
            .collect();
        assert_eq!(
            shown,
            ["zz", "a", "ab", "b", "c"],
            "the folders first, then the files by name"
        );
    }

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
