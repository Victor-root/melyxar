//! Pictures the server generated and now serves.
//!
//! The rows here describe files in the cache: what they show, what size they
//! are, and the name their content earned. That name is what lets a browser
//! keep a poster for ever and still see a new one the day it changes.

use melyxar_core::id::{ImageId, WorkId};
use melyxar_core::time::now;
use sqlx::{AssertSqlSafe, Row};

use crate::convert::timestamp_to_text;
use crate::{Database, Result};

/// What a picture is, as every reader of one asks for it.
///
/// Written once because four queries hand their rows to the same reader.
pub(crate) const WHAT_A_PICTURE_IS: &str =
    "owner_kind, owner_id, image_kind, relative_path, width, height, fingerprint, dominant_color, \
     framing";

/// One generated picture, as it is stored.
#[derive(Debug, Clone, PartialEq)]
pub struct StoredImage {
    /// work, person, collection, library, server.
    pub owner_kind: String,
    pub owner_id: String,
    /// poster, backdrop, logo, thumbnail, banner.
    pub image_kind: String,
    /// Path inside the image cache.
    pub relative_path: String,
    pub width: Option<i32>,
    pub height: Option<i32>,
    /// Name the content earned, shared by every size of one picture.
    pub fingerprint: String,
    pub dominant_color: Option<String>,
    /// Where a band of it is taken from, between nought and one, when it was
    /// read off the picture. Nothing for a picture prepared before that was
    /// worked out, and for the kinds nothing ever cuts a band out of.
    pub framing: Option<f64>,
}

impl Database {
    /// Replaces every size of one picture of one owner.
    ///
    /// Whole rather than piecemeal: the sizes of a picture belong together, and
    /// a half replaced set would serve one poster in a grid and another on the
    /// page. The paths that are no longer used come back, so their files can be
    /// removed from the cache by the caller that put them there.
    pub async fn replace_images(
        &self,
        owner_kind: &str,
        owner_id: &str,
        image_kind: &str,
        images: &[StoredImage],
    ) -> Result<Vec<String>> {
        let mut transaction = self.begin().await?;

        let previous: Vec<String> = sqlx::query(
            "SELECT relative_path FROM images
             WHERE owner_kind = ? AND owner_id = ? AND image_kind = ?",
        )
        .bind(owner_kind)
        .bind(owner_id)
        .bind(image_kind)
        .fetch_all(&mut *transaction)
        .await?
        .iter()
        .map(|row| row.try_get::<String, _>("relative_path"))
        .collect::<std::result::Result<_, _>>()?;

        sqlx::query("DELETE FROM images WHERE owner_kind = ? AND owner_id = ? AND image_kind = ?")
            .bind(owner_kind)
            .bind(owner_id)
            .bind(image_kind)
            .execute(&mut *transaction)
            .await?;

        let moment = timestamp_to_text(now());
        for image in images {
            sqlx::query(
                "INSERT INTO images
                    (id, owner_kind, owner_id, image_kind, relative_path, width, height,
                     fingerprint, dominant_color, framing, created_at)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(ImageId::new().to_db_string())
            .bind(&image.owner_kind)
            .bind(&image.owner_id)
            .bind(&image.image_kind)
            .bind(&image.relative_path)
            .bind(image.width)
            .bind(image.height)
            .bind(&image.fingerprint)
            .bind(image.dominant_color.as_deref())
            .bind(image.framing)
            .bind(&moment)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;

        let kept: Vec<&str> = images
            .iter()
            .map(|image| image.relative_path.as_str())
            .collect();
        Ok(previous
            .into_iter()
            .filter(|path| !kept.contains(&path.as_str()))
            .collect())
    }

    /// Every picture of one owner, largest first within each kind.
    pub async fn images_of(&self, owner_kind: &str, owner_id: &str) -> Result<Vec<StoredImage>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_PICTURE_IS} FROM images
             WHERE owner_kind = ? AND owner_id = ?
             ORDER BY image_kind, width DESC"
        )))
        .bind(owner_kind)
        .bind(owner_id)
        .fetch_all(self.reader())
        .await?;

        rows.iter().map(image_from_row).collect()
    }

    /// The photos of everyone a work credits, largest first.
    ///
    /// One query for the whole page rather than one per name: a page with
    /// eighteen faces would otherwise cost eighteen round trips. The rows carry
    /// the person they belong to, which is how a caller puts each face next to
    /// its name.
    pub async fn credit_photos_of_work(&self, work_id: WorkId) -> Result<Vec<StoredImage>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_PICTURE_IS} FROM images
             WHERE owner_kind = 'person'
               AND owner_id IN (SELECT person_id FROM credits WHERE work_id = ?)
             ORDER BY width DESC"
        )))
        .bind(work_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter().map(image_from_row).collect()
    }

    /// The pictures of everything hanging under one work, largest first.
    ///
    /// One query for a whole page of seasons or episodes, for the same reason
    /// the faces come back in one: a season of twenty four episodes would
    /// otherwise cost twenty four round trips. The rows carry the work they
    /// belong to, which is how a caller puts each picture on its own card.
    pub async fn pictures_of_children(&self, parent_id: WorkId) -> Result<Vec<StoredImage>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_PICTURE_IS} FROM images
             WHERE owner_kind = 'work'
               AND owner_id IN (SELECT id FROM works WHERE parent_id = ?)
             ORDER BY width DESC"
        )))
        .bind(parent_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter().map(image_from_row).collect()
    }

    /// The name the content of one picture earned, when it is already here.
    ///
    /// This is what stops a refresh from fetching a poster that has not
    /// changed.
    pub async fn image_fingerprint(
        &self,
        owner_kind: &str,
        owner_id: &str,
        image_kind: &str,
    ) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT fingerprint FROM images
             WHERE owner_kind = ? AND owner_id = ? AND image_kind = ? LIMIT 1",
        )
        .bind(owner_kind)
        .bind(owner_id)
        .bind(image_kind)
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| Ok(row.try_get("fingerprint")?)).transpose()
    }

    /// Records the colour a card shows before its picture has arrived.
    pub async fn set_work_dominant_color(&self, work_id: WorkId, colour: &str) -> Result<()> {
        sqlx::query("UPDATE works SET dominant_color = ?, updated_at = ? WHERE id = ?")
            .bind(colour)
            .bind(timestamp_to_text(now()))
            .bind(work_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }
}

pub(crate) fn image_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<StoredImage> {
    Ok(StoredImage {
        owner_kind: row.try_get("owner_kind")?,
        owner_id: row.try_get("owner_id")?,
        image_kind: row.try_get("image_kind")?,
        relative_path: row.try_get("relative_path")?,
        width: row.try_get("width")?,
        height: row.try_get("height")?,
        fingerprint: row.try_get("fingerprint")?,
        dominant_color: row.try_get("dominant_color")?,
        framing: row.try_get("framing")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::work::WorkKind;
    use std::path::PathBuf;

    async fn work() -> (Database, WorkId) {
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
        let work = database
            .create_work(
                library.id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        (database, work.id)
    }

    fn poster(work_id: WorkId, fingerprint: &str, width: i32) -> StoredImage {
        StoredImage {
            owner_kind: "work".to_string(),
            owner_id: work_id.to_db_string(),
            image_kind: "poster".to_string(),
            relative_path: format!("works/{work_id}/poster-{fingerprint}-{width}.webp"),
            width: Some(width),
            height: Some(width * 3 / 2),
            fingerprint: fingerprint.to_string(),
            dominant_color: Some("#c81e1e".to_string()),
            framing: None,
        }
    }

    #[tokio::test]
    async fn every_size_of_a_picture_is_stored_and_read_back() {
        let (database, work_id) = work().await;
        let sizes = vec![
            poster(work_id, "abc", 200),
            poster(work_id, "abc", 400),
            poster(work_id, "abc", 800),
        ];

        let removed = database
            .replace_images("work", &work_id.to_db_string(), "poster", &sizes)
            .await
            .expect("images stored");
        assert!(removed.is_empty(), "nothing was there before");

        let stored = database
            .images_of("work", &work_id.to_db_string())
            .await
            .expect("read");
        assert_eq!(stored.len(), 3);
        assert_eq!(stored[0].width, Some(800), "the largest comes first");
        assert!(stored.iter().all(|image| image.fingerprint == "abc"));
    }

    #[tokio::test]
    async fn a_picture_that_changed_says_which_files_are_no_longer_used() {
        let (database, work_id) = work().await;
        database
            .replace_images(
                "work",
                &work_id.to_db_string(),
                "poster",
                &[poster(work_id, "abc", 200), poster(work_id, "abc", 400)],
            )
            .await
            .expect("images stored");

        let removed = database
            .replace_images(
                "work",
                &work_id.to_db_string(),
                "poster",
                &[poster(work_id, "def", 200), poster(work_id, "def", 400)],
            )
            .await
            .expect("images replaced");

        assert_eq!(
            removed.len(),
            2,
            "the files of the old poster are the caller's to remove"
        );
        assert!(removed.iter().all(|path| path.contains("abc")));
        assert_eq!(
            database
                .images_of("work", &work_id.to_db_string())
                .await
                .expect("read")
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn the_same_picture_stored_again_leaves_its_files_alone() {
        let (database, work_id) = work().await;
        let sizes = vec![poster(work_id, "abc", 200), poster(work_id, "abc", 400)];
        database
            .replace_images("work", &work_id.to_db_string(), "poster", &sizes)
            .await
            .expect("images stored");

        let removed = database
            .replace_images("work", &work_id.to_db_string(), "poster", &sizes)
            .await
            .expect("images stored again");
        assert!(
            removed.is_empty(),
            "a picture that did not change must not have its files deleted and written again"
        );
    }

    #[tokio::test]
    async fn one_kind_of_picture_never_disturbs_another() {
        let (database, work_id) = work().await;
        database
            .replace_images(
                "work",
                &work_id.to_db_string(),
                "poster",
                &[poster(work_id, "abc", 200)],
            )
            .await
            .expect("stored");
        database
            .replace_images(
                "work",
                &work_id.to_db_string(),
                "backdrop",
                &[StoredImage {
                    image_kind: "backdrop".to_string(),
                    relative_path: format!("works/{work_id}/backdrop-xyz-1280.webp"),
                    width: Some(1280),
                    height: Some(720),
                    fingerprint: "xyz".to_string(),
                    ..poster(work_id, "xyz", 1280)
                }],
            )
            .await
            .expect("stored");

        assert_eq!(
            database
                .images_of("work", &work_id.to_db_string())
                .await
                .expect("read")
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn the_name_a_picture_earned_is_what_stops_it_being_fetched_again() {
        let (database, work_id) = work().await;
        assert!(database
            .image_fingerprint("work", &work_id.to_db_string(), "poster")
            .await
            .expect("read")
            .is_none());

        database
            .replace_images(
                "work",
                &work_id.to_db_string(),
                "poster",
                &[poster(work_id, "abc", 200)],
            )
            .await
            .expect("stored");

        assert_eq!(
            database
                .image_fingerprint("work", &work_id.to_db_string(), "poster")
                .await
                .expect("read")
                .as_deref(),
            Some("abc")
        );
    }

    /// The smallest identification that credits one actor and one director.
    fn identified_with_a_cast() -> crate::metadata::IdentifiedWork {
        crate::metadata::IdentifiedWork {
            provider: "tmdb".to_string(),
            external_id: "111".to_string(),
            imdb_id: None,
            language: "fr".to_string(),
            title: "Quiet Harbour".to_string(),
            sort_title: "quiet harbour".to_string(),
            tagline: None,
            overview: None,
            release_year: Some(2019),
            runtime: None,
            community_rating: None,
            age_rating_label: None,
            genres: Vec::new(),
            studios: Vec::new(),
            credits: vec![
                crate::metadata::CreditRecord {
                    external_id: "1".to_string(),
                    name: "Alix Moreau".to_string(),
                    sort_name: "alix moreau".to_string(),
                    role: "actor".to_string(),
                    character: Some("Camille".to_string()),
                    ordinal: 0,
                    photo_path: Some("/alix.jpg".to_string()),
                },
                crate::metadata::CreditRecord {
                    external_id: "3".to_string(),
                    name: "Sacha Nord".to_string(),
                    sort_name: "sacha nord".to_string(),
                    role: "director".to_string(),
                    character: None,
                    ordinal: 0,
                    photo_path: None,
                },
            ],
            collection: None,
            trailers: Vec::new(),
        }
    }

    fn face(person_id: &str, width: i32) -> StoredImage {
        StoredImage {
            owner_kind: "person".to_string(),
            owner_id: person_id.to_string(),
            image_kind: "photo".to_string(),
            relative_path: format!("people/{person_id}/photo-abc-{width}.webp"),
            width: Some(width),
            height: Some(width * 3 / 2),
            fingerprint: "abc".to_string(),
            dominant_color: None,
            framing: None,
        }
    }

    #[tokio::test]
    async fn the_faces_of_a_page_come_back_in_one_read() {
        let (database, work_id) = work().await;
        let people = database
            .apply_identification(work_id, &identified_with_a_cast(), false)
            .await
            .expect("identification applied");
        let actor = people
            .iter()
            .find(|person| person.role == "actor")
            .expect("an actor was credited");

        database
            .replace_images(
                "person",
                &actor.person_id.to_db_string(),
                "photo",
                &[
                    face(&actor.person_id.to_db_string(), 96),
                    face(&actor.person_id.to_db_string(), 192),
                ],
            )
            .await
            .expect("photo stored");

        let faces = database.credit_photos_of_work(work_id).await.expect("read");
        assert_eq!(
            faces.len(),
            2,
            "both sizes of the one face that was fetched"
        );
        assert!(faces
            .iter()
            .all(|image| image.owner_id == actor.person_id.to_db_string()));
        assert_eq!(faces[0].width, Some(192), "the largest comes first");
    }

    #[tokio::test]
    async fn a_face_of_someone_this_film_does_not_credit_is_left_out() {
        let (database, work_id) = work().await;
        database
            .apply_identification(work_id, &identified_with_a_cast(), false)
            .await
            .expect("identification applied");

        let stranger = melyxar_core::id::PersonId::new().to_db_string();
        database
            .replace_images("person", &stranger, "photo", &[face(&stranger, 96)])
            .await
            .expect("photo stored");

        assert!(
            database
                .credit_photos_of_work(work_id)
                .await
                .expect("read")
                .is_empty(),
            "a page shows the faces of its own credits and nobody else's"
        );
    }

    #[tokio::test]
    async fn a_card_has_a_colour_to_show_before_its_picture_arrives() {
        let (database, work_id) = work().await;
        database
            .set_work_dominant_color(work_id, "#c81e1e")
            .await
            .expect("colour recorded");

        let stored = database
            .work(work_id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.dominant_color.as_deref(), Some("#c81e1e"));
    }
}
