//! What a person wrote about a work themselves.
//!
//! Every field written here can be locked, and a locked field is one a later
//! look up leaves alone: see `apply_identification` next door. The texts are
//! written in the language of the library, which is the one its pages read
//! first.

use melyxar_core::id::WorkId;
use melyxar_core::time::now;
use sqlx::Row;

use crate::convert::timestamp_to_text;
use crate::metadata::{replace_links, GENRES, STUDIOS};
use crate::{Database, Result};

/// The fields a person may write, by the name their lock goes by.
pub const EDITABLE_FIELDS: [&str; 8] = [
    "title",
    "tagline",
    "overview",
    "release_year",
    "community_rating",
    "age_rating",
    "genres",
    "studios",
];

/// What describes a work, as a person reads and writes it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct WrittenDetails {
    pub title: String,
    pub tagline: Option<String>,
    pub overview: Option<String>,
    pub release_year: Option<i32>,
    pub community_rating: Option<f64>,
    pub age_rating_label: Option<String>,
    pub genres: Vec<String>,
    pub studios: Vec<String>,
}

impl Database {
    /// What a work says about itself today, its texts in `language` or, when
    /// that language has none, in English.
    pub async fn written_details(
        &self,
        work_id: WorkId,
        language: &str,
    ) -> Result<Option<WrittenDetails>> {
        let Some(row) = sqlx::query(
            "SELECT w.title, w.release_year, w.community_rating, w.age_rating_label,
                    coalesce(t.tagline, e.tagline) AS tagline,
                    coalesce(t.overview, e.overview) AS overview
             FROM works w
             LEFT JOIN work_translations t ON t.work_id = w.id AND t.language = ?1
             LEFT JOIN work_translations e ON e.work_id = w.id AND e.language = 'en'
             WHERE w.id = ?2",
        )
        .bind(language)
        .bind(work_id.to_db_string())
        .fetch_optional(self.reader())
        .await?
        else {
            return Ok(None);
        };

        Ok(Some(WrittenDetails {
            title: row.try_get("title")?,
            tagline: row.try_get("tagline")?,
            overview: row.try_get("overview")?,
            release_year: row.try_get("release_year")?,
            community_rating: row.try_get("community_rating")?,
            age_rating_label: row.try_get("age_rating_label")?,
            genres: self.work_genres(work_id).await?,
            studios: self.work_studios(work_id).await?,
        }))
    }

    /// Writes what a person gave a work, and locks exactly the fields in
    /// `locked` among those a person may write.
    ///
    /// Every field is written, locked or not: an unlocked one simply goes back
    /// to what the provider says at the next look up. The locks of pictures
    /// are not this one's to touch.
    pub async fn write_details(
        &self,
        work_id: WorkId,
        language: &str,
        details: &WrittenDetails,
        sort_title: &str,
        locked: &[&str],
    ) -> Result<()> {
        let mut transaction = self.begin().await?;
        let moment = timestamp_to_text(now());

        sqlx::query(
            "UPDATE works SET title = ?, sort_title = ?, release_year = ?,
                              community_rating = ?, age_rating_label = ?, updated_at = ?
             WHERE id = ?",
        )
        .bind(&details.title)
        .bind(sort_title)
        .bind(details.release_year)
        .bind(details.community_rating)
        .bind(details.age_rating_label.as_deref())
        .bind(&moment)
        .bind(work_id.to_db_string())
        .execute(&mut *transaction)
        .await?;

        sqlx::query(
            "INSERT INTO work_translations (work_id, language, title, tagline, overview)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (work_id, language) DO UPDATE SET
                title = excluded.title,
                tagline = excluded.tagline,
                overview = excluded.overview",
        )
        .bind(work_id.to_db_string())
        .bind(language)
        .bind(&details.title)
        .bind(details.tagline.as_deref())
        .bind(details.overview.as_deref())
        .execute(&mut *transaction)
        .await?;

        replace_links(&mut transaction, work_id, &GENRES, &details.genres).await?;
        replace_links(&mut transaction, work_id, &STUDIOS, &details.studios).await?;

        for field in EDITABLE_FIELDS {
            let query = match locked.contains(&field) {
                true => {
                    "INSERT INTO work_locked_fields (work_id, field, locked_at) VALUES (?, ?, ?)
                     ON CONFLICT (work_id, field) DO NOTHING"
                }
                false => "DELETE FROM work_locked_fields WHERE work_id = ? AND field = ?",
            };
            let mut asked = sqlx::query(query).bind(work_id.to_db_string()).bind(field);
            if locked.contains(&field) {
                asked = asked.bind(&moment);
            }
            asked.execute(&mut *transaction).await?;
        }

        transaction.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::IdentifiedWork;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::work::WorkKind;
    use std::path::PathBuf;

    async fn a_film() -> (Database, WorkId) {
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
            .create_work(library.id, WorkKind::Movie, "Quiet Harbour", "quiet harbour", None)
            .await
            .expect("work created");
        (database, work.id)
    }

    fn what_the_provider_says() -> IdentifiedWork {
        IdentifiedWork {
            provider: "tmdb".to_string(),
            external_id: "111".to_string(),
            imdb_id: None,
            language: "fr".to_string(),
            title: "Quiet Harbour".to_string(),
            sort_title: "quiet harbour".to_string(),
            tagline: Some("La mer ne rend rien.".to_string()),
            overview: Some("Un port, une nuit.".to_string()),
            release_year: Some(2019),
            runtime: None,
            community_rating: Some(7.4),
            age_rating_label: Some("12".to_string()),
            genres: vec!["Drame".to_string()],
            studios: vec!["Invented Pictures".to_string()],
            credits: Vec::new(),
            collection: None,
            trailers: Vec::new(),
        }
    }

    fn written() -> WrittenDetails {
        WrittenDetails {
            title: "Le Port tranquille".to_string(),
            tagline: Some("Rien ne revient.".to_string()),
            overview: Some("Écrit à la main.".to_string()),
            release_year: Some(2020),
            community_rating: Some(9.0),
            age_rating_label: Some("16".to_string()),
            genres: vec!["Mystère".to_string(), "Drame".to_string()],
            studios: vec!["Studio Imaginaire".to_string()],
        }
    }

    #[tokio::test]
    async fn what_a_person_writes_is_read_back_whole() {
        let (database, work) = a_film().await;
        database
            .apply_identification(work, &what_the_provider_says(), false)
            .await
            .expect("identified");

        database
            .write_details(work, "fr", &written(), "port tranquille", &["title"])
            .await
            .expect("written");

        let read = database
            .written_details(work, "fr")
            .await
            .expect("read")
            .expect("present");
        let mut expected = written();
        expected.genres.sort();
        assert_eq!(read, expected);
        assert_eq!(database.locked_fields(work).await.expect("read"), vec!["title"]);
        assert_eq!(
            database.work(work).await.expect("read").expect("present").sort_title,
            "port tranquille"
        );
    }

    #[tokio::test]
    async fn every_locked_field_survives_a_later_lookup_and_the_others_follow_it() {
        let (database, work) = a_film().await;
        database
            .write_details(work, "fr", &written(), "port tranquille", &EDITABLE_FIELDS)
            .await
            .expect("written");

        database
            .apply_identification(work, &what_the_provider_says(), false)
            .await
            .expect("identified");
        let mut expected = written();
        expected.genres.sort();
        assert_eq!(
            database.written_details(work, "fr").await.expect("read").expect("present"),
            expected,
            "a refresh that undoes what someone typed is a refresh nobody dares run"
        );

        database
            .write_details(work, "fr", &written(), "port tranquille", &[])
            .await
            .expect("unlocked");
        database
            .apply_identification(work, &what_the_provider_says(), false)
            .await
            .expect("identified");
        let followed = database
            .written_details(work, "fr")
            .await
            .expect("read")
            .expect("present");
        assert_eq!(followed.title, "Quiet Harbour");
        assert_eq!(followed.overview.as_deref(), Some("Un port, une nuit."));
        assert_eq!(followed.genres, vec!["Drame"]);
        assert_eq!(followed.age_rating_label.as_deref(), Some("12"));
    }

    #[tokio::test]
    async fn the_lock_of_a_picture_is_left_alone() {
        let (database, work) = a_film().await;
        database.lock_field(work, "poster").await.expect("locked");
        database
            .write_details(work, "fr", &written(), "port tranquille", &[])
            .await
            .expect("written");
        assert_eq!(database.locked_fields(work).await.expect("read"), vec!["poster"]);
    }

    #[tokio::test]
    async fn texts_missing_in_the_library_language_are_read_in_english() {
        let (database, work) = a_film().await;
        let mut english = what_the_provider_says();
        english.language = "en".to_string();
        english.overview = Some("A harbour, one night.".to_string());
        database
            .apply_identification(work, &english, false)
            .await
            .expect("identified");
        let read = database
            .written_details(work, "fr")
            .await
            .expect("read")
            .expect("present");
        assert_eq!(read.overview.as_deref(), Some("A harbour, one night."));
    }
}
