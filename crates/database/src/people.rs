//! The page of one person: who they are, and what of theirs this server holds.
//!
//! A person is written down by the identification of every work that credits
//! them, with a name and a face and nothing more. The rest, their life in a
//! few lines, is asked of the provider only when somebody opens their page,
//! and kept here from then on.

use melyxar_core::id::{LibraryId, PersonId, UserId};
use melyxar_core::time::{now, Timestamp};
use sqlx::{AssertSqlSafe, Row};

use crate::browse::{kept_inside, met_on_its_own, WorkCard, WHAT_A_CARD_IS};
use crate::convert::{parse_optional_timestamp, timestamp_to_text};
use crate::{Database, Result};

/// One person, as far as this server knows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub id: PersonId,
    pub name: String,
    pub biography: Option<String>,
    /// Dates as the provider writes them, year, month and day.
    pub born_on: Option<String>,
    pub died_on: Option<String>,
    pub birthplace: Option<String>,
    /// When the provider was last asked about them and answered. Absent means
    /// never, which is what sends the page to ask.
    pub described_at: Option<Timestamp>,
    /// What the provider knows them by, when it knows them at all.
    pub tmdb_id: Option<String>,
}

/// What the provider says of a person's life.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PersonDescription {
    pub biography: Option<String>,
    pub born_on: Option<String>,
    pub died_on: Option<String>,
    pub birthplace: Option<String>,
}

impl Database {
    /// One person, or nothing when there is nobody by that identifier.
    pub async fn person(&self, id: PersonId) -> Result<Option<Person>> {
        let row = sqlx::query(
            "SELECT p.name, p.biography, p.born_on, p.died_on, p.birthplace, p.described_at,
                    x.external_id AS tmdb_id
               FROM people p
               LEFT JOIN person_external_ids x ON x.person_id = p.id AND x.provider = 'tmdb'
              WHERE p.id = ?",
        )
        .bind(id.to_db_string())
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| {
            Ok(Person {
                id,
                name: row.try_get("name")?,
                biography: row.try_get("biography")?,
                born_on: row.try_get("born_on")?,
                died_on: row.try_get("died_on")?,
                birthplace: row.try_get("birthplace")?,
                described_at: parse_optional_timestamp(
                    row.try_get::<Option<String>, _>("described_at")?.as_deref(),
                )?,
                tmdb_id: row.try_get("tmdb_id")?,
            })
        })
        .transpose()
    }

    /// Keeps what the provider said of a person, and when it said it.
    pub async fn describe_person(
        &self,
        id: PersonId,
        description: &PersonDescription,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE people
                SET biography = ?, born_on = ?, died_on = ?, birthplace = ?, described_at = ?
              WHERE id = ?",
        )
        .bind(description.biography.as_deref())
        .bind(description.born_on.as_deref())
        .bind(description.died_on.as_deref())
        .bind(description.birthplace.as_deref())
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Every work of this server that credits a person, newest first, as
    /// cards for the one looking.
    ///
    /// Credits hang on films and series, never on a season or an episode, so
    /// what is met on its own is exactly what is asked for. A person credited
    /// twice on one film, acting and directing, is one card.
    pub async fn works_crediting(
        &self,
        viewer: UserId,
        person: PersonId,
        within: Option<&[LibraryId]>,
    ) -> Result<Vec<WorkCard>> {
        let Some(inside) = kept_inside(within, "w.library_id") else {
            return Ok(Vec::new());
        };
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_CARD_IS}
               FROM works w
              WHERE w.id IN (SELECT work_id FROM credits WHERE person_id = ?)
                AND {}{inside}
              ORDER BY w.release_year IS NULL, w.release_year DESC, w.sort_title",
            met_on_its_own("w.")
        )))
        .bind(person.to_db_string());
        for granted in within.iter().copied().flatten() {
            query = query.bind(granted.to_db_string());
        }
        let rows = query.fetch_all(self.reader()).await?;

        let mut cards = rows
            .iter()
            .map(crate::browse::card_from_row)
            .collect::<Result<Vec<_>>>()?;
        self.attach_posters(&mut cards).await?;
        self.attach_viewer_state(viewer, &mut cards).await?;
        Ok(cards)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{CreditRecord, IdentifiedWork};
    use melyxar_core::library::LibraryKind;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::WorkKind;
    use std::path::PathBuf;

    fn crediting(title: &str, year: i32, people: &[(&str, &str)]) -> IdentifiedWork {
        IdentifiedWork {
            provider: "tmdb".to_string(),
            external_id: format!("film-{title}"),
            imdb_id: None,
            language: "fr".to_string(),
            title: title.to_string(),
            sort_title: title.to_lowercase(),
            tagline: None,
            overview: None,
            release_year: Some(year),
            runtime: None,
            community_rating: None,
            age_rating_label: None,
            genres: Vec::new(),
            studios: Vec::new(),
            credits: people
                .iter()
                .enumerate()
                .map(|(ordinal, (name, role))| CreditRecord {
                    external_id: format!("person-{name}"),
                    name: name.to_string(),
                    sort_name: name.to_lowercase(),
                    role: role.to_string(),
                    character: None,
                    ordinal: ordinal as i32,
                    photo_path: None,
                })
                .collect(),
            collection: None,
            trailers: Vec::new(),
        }
    }

    /// Two libraries, three films, and one person credited on two of them.
    async fn a_career() -> (Database, UserId, LibraryId, PersonId, Vec<melyxar_core::id::WorkId>) {
        let database = Database::open_in_memory().await.expect("database opens");
        let films = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let more = database
            .create_library(
                "More",
                LibraryKind::Movies,
                "fr",
                &[("disk-two".to_string(), PathBuf::from("/mnt/two/Films"))],
            )
            .await
            .expect("library created");

        let mut written = Vec::new();
        for (library, title, year, people) in [
            (films.id, "Salt Road", 2011, vec![("Ada Moss", "actor"), ("Ada Moss", "director")]),
            (films.id, "Paper Moon Harbour", 2019, vec![("Ada Moss", "actor")]),
            (films.id, "Night Ferry", 2015, vec![("Ben Oak", "actor")]),
            (more.id, "Glass Orchard", 2021, vec![("Ada Moss", "writer")]),
        ] {
            let work = database
                .create_work(library, WorkKind::Movie, title, &title.to_lowercase(), Some(year))
                .await
                .expect("work created");
            database
                .apply_identification(work.id, &crediting(title, year, &people), false)
                .await
                .expect("identified");
            written.push(work.id);
        }

        let viewer = database
            .create_user("vera", None, &Permissions::viewer())
            .await
            .expect("account created")
            .id;
        let ada = database
            .work_credits(written[0])
            .await
            .expect("credits read")
            .into_iter()
            .find(|credit| credit.name == "Ada Moss")
            .expect("credited")
            .person_id;
        (database, viewer, films.id, ada, written)
    }

    #[tokio::test]
    async fn a_person_comes_back_with_what_the_provider_calls_them() {
        let (database, _, _, ada, _) = a_career().await;
        let person = database.person(ada).await.expect("read").expect("present");
        assert_eq!(person.name, "Ada Moss");
        assert_eq!(person.tmdb_id.as_deref(), Some("person-Ada Moss"));
        assert_eq!(person.described_at, None, "nobody has asked about her yet");
    }

    #[tokio::test]
    async fn what_the_provider_said_is_kept_and_dated() {
        let (database, _, _, ada, _) = a_career().await;
        database
            .describe_person(
                ada,
                &PersonDescription {
                    biography: Some("Born by the sea.".to_string()),
                    born_on: Some("1970-04-02".to_string()),
                    died_on: None,
                    birthplace: Some("Harbourtown".to_string()),
                },
            )
            .await
            .expect("described");

        let person = database.person(ada).await.expect("read").expect("present");
        assert_eq!(person.biography.as_deref(), Some("Born by the sea."));
        assert_eq!(person.born_on.as_deref(), Some("1970-04-02"));
        assert_eq!(person.birthplace.as_deref(), Some("Harbourtown"));
        assert!(person.described_at.is_some(), "the answer is dated");
    }

    #[tokio::test]
    async fn a_career_is_newest_first_once_per_work_and_inside_what_was_granted() {
        let (database, viewer, films, ada, written) = a_career().await;

        let everything = database
            .works_crediting(viewer, ada, None)
            .await
            .expect("read");
        assert_eq!(
            everything.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![written[3], written[1], written[0]],
            "newest first, the film she both acted in and directed only once"
        );

        let granted = database
            .works_crediting(viewer, ada, Some(&[films]))
            .await
            .expect("read");
        assert_eq!(
            granted.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![written[1], written[0]],
            "a library that was not granted is not there"
        );

        assert!(
            database
                .works_crediting(viewer, ada, Some(&[]))
                .await
                .expect("read")
                .is_empty(),
            "an account granted nothing reads nothing"
        );
    }
}
