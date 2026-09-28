//! Collections: works gathered under one name for the whole server, either
//! by hand or by the provider, which calls them sagas.
//!
//! A collection made by hand keeps the order its works were put in, and is
//! the only kind anybody changes. A saga is ordered by when its films came out
//! and belongs to the provider, which writes it as it names the films: see
//! `apply_identification`.

use melyxar_core::id::{CollectionId, LibraryId, UserId, WorkId};
use melyxar_core::time::now;
use sqlx::{AssertSqlSafe, Row};

use crate::browse::{kept_inside, WorkCard, WHAT_A_CARD_IS};
use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, Result};

/// The fewest films a saga needs here before it counts as a collection: one
/// film on its own gathers nothing.
const FEWEST_IN_A_SAGA: i64 = 2;

/// What decides the order of a collection's works: the order they were put
/// in for one made by hand, when they came out for a saga.
const IN_ITS_ORDER: &str = "CASE WHEN c.origin = 'manual' THEN ci.ordinal ELSE 0 END,
    w.release_year IS NULL, w.release_year, w.sort_title";

/// One collection as a list of them shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionSummary {
    pub id: CollectionId,
    pub name: String,
    pub made_by_hand: bool,
    /// How many of its works this account can reach.
    pub held: i64,
    /// Its first work this account can reach, whose poster it wears.
    pub cover: Option<WorkCard>,
}

/// One collection with its works, in its order.
#[derive(Debug, Clone, PartialEq)]
pub struct CollectionHeld {
    pub id: CollectionId,
    pub name: String,
    pub made_by_hand: bool,
    pub cards: Vec<WorkCard>,
}

impl Database {
    /// Every collection this account can see something of, by name. A saga
    /// needs two films the account can reach; one made by hand needs one,
    /// or none at all for somebody who manages them and has to find the one
    /// they just made.
    pub async fn collections(
        &self,
        viewer: UserId,
        within: Option<&[LibraryId]>,
        with_the_empty: bool,
    ) -> Result<Vec<CollectionSummary>> {
        let inside = kept_inside(within, "w.library_id");
        let rows = match &inside {
            Some(inside) => {
                let mut query = sqlx::query(AssertSqlSafe(format!(
                    "WITH visible AS (
                        SELECT ci.collection_id, ci.work_id, w.release_year, w.sort_title,
                               CASE WHEN c.origin = 'manual' THEN ci.ordinal ELSE 0 END AS placed
                          FROM collection_items ci
                          JOIN collections c ON c.id = ci.collection_id
                          JOIN works w ON w.id = ci.work_id
                         WHERE 1 = 1{inside})
                     SELECT c.id, c.name, c.origin,
                            (SELECT count(*) FROM visible v WHERE v.collection_id = c.id) AS held,
                            (SELECT v.work_id FROM visible v WHERE v.collection_id = c.id
                              ORDER BY v.placed, v.release_year IS NULL, v.release_year,
                                       v.sort_title
                              LIMIT 1) AS cover
                       FROM collections c
                      ORDER BY c.sort_name"
                )));
                for library in within.unwrap_or_default() {
                    query = query.bind(library.to_db_string());
                }
                query.fetch_all(self.reader()).await?
            }
            None => Vec::new(),
        };

        let mut found = Vec::new();
        let mut covers = Vec::new();
        for row in &rows {
            let made_by_hand = row.try_get::<String, _>("origin")? == "manual";
            let held: i64 = row.try_get("held")?;
            let shown = if made_by_hand {
                held > 0 || with_the_empty
            } else {
                held >= FEWEST_IN_A_SAGA
            };
            if !shown {
                continue;
            }
            let cover: Option<String> = row.try_get("cover")?;
            let cover: Option<WorkId> = cover.map(|id| parse_id(&id)).transpose()?;
            covers.extend(cover);
            found.push((
                CollectionSummary {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    name: row.try_get("name")?,
                    made_by_hand,
                    held,
                    cover: None,
                },
                cover,
            ));
        }

        let cards = self.cards_in_order(viewer, &covers).await?;
        Ok(found
            .into_iter()
            .map(|(mut summary, cover)| {
                summary.cover = cover.and_then(|id| cards.iter().find(|card| card.id == id).cloned());
                summary
            })
            .collect())
    }

    /// One collection and the works of it this account can reach, in its
    /// order.
    pub async fn collection(
        &self,
        viewer: UserId,
        id: CollectionId,
        within: Option<&[LibraryId]>,
    ) -> Result<Option<CollectionHeld>> {
        let named: Option<(String, String)> =
            sqlx::query_as("SELECT name, origin FROM collections WHERE id = ?")
                .bind(id.to_db_string())
                .fetch_optional(self.reader())
                .await?;
        let Some((name, origin)) = named else {
            return Ok(None);
        };
        let mut cards = Vec::new();
        if let Some(inside) = kept_inside(within, "w.library_id") {
            let mut query = sqlx::query(AssertSqlSafe(format!(
                "SELECT {WHAT_A_CARD_IS}
                   FROM works w
                   JOIN collection_items ci ON ci.work_id = w.id
                   JOIN collections c ON c.id = ci.collection_id
                  WHERE c.id = ?{inside}
                  ORDER BY {IN_ITS_ORDER}"
            )))
            .bind(id.to_db_string());
            for library in within.unwrap_or_default() {
                query = query.bind(library.to_db_string());
            }
            cards = query
                .fetch_all(self.reader())
                .await?
                .iter()
                .map(crate::browse::card_from_row)
                .collect::<Result<Vec<_>>>()?;
            self.attach_posters(&mut cards).await?;
            self.attach_viewer_state(viewer, &mut cards).await?;
        }
        Ok(Some(CollectionHeld {
            id,
            name,
            made_by_hand: origin == "manual",
            cards,
        }))
    }

    /// Makes a collection by hand, empty, and answers what it is called.
    pub async fn create_collection(&self, name: &str, sort_name: &str) -> Result<CollectionId> {
        let id = CollectionId::new();
        sqlx::query(
            "INSERT INTO collections (id, name, sort_name, origin, created_at)
             VALUES (?, ?, ?, 'manual', ?)",
        )
        .bind(id.to_db_string())
        .bind(name)
        .bind(sort_name)
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(id)
    }

    /// Calls a collection made by hand something else. Answers false for one
    /// that is not there or that the provider made.
    pub async fn rename_collection(
        &self,
        id: CollectionId,
        name: &str,
        sort_name: &str,
    ) -> Result<bool> {
        let done = sqlx::query(
            "UPDATE collections SET name = ?, sort_name = ? WHERE id = ? AND origin = 'manual'",
        )
        .bind(name)
        .bind(sort_name)
        .bind(id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Deletes a collection made by hand, and nothing of the works in it.
    /// Answers false for one that is not there or that the provider made.
    pub async fn delete_collection(&self, id: CollectionId) -> Result<bool> {
        let done = sqlx::query("DELETE FROM collections WHERE id = ? AND origin = 'manual'")
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Puts works at the end of a collection made by hand, in the order
    /// given, leaving where they are the ones already in it. Answers false
    /// for a collection that is not there or that the provider made.
    pub async fn add_to_collection(&self, id: CollectionId, works: &[WorkId]) -> Result<bool> {
        let mut transaction = self.begin().await?;
        let origin: Option<String> =
            sqlx::query_scalar("SELECT origin FROM collections WHERE id = ?")
                .bind(id.to_db_string())
                .fetch_optional(&mut *transaction)
                .await?;
        if origin.as_deref() != Some("manual") {
            return Ok(false);
        }
        for work in works {
            sqlx::query(
                "INSERT INTO collection_items (collection_id, work_id, ordinal)
                 SELECT ?1, ?2, coalesce(max(ordinal) + 1, 0)
                   FROM collection_items WHERE collection_id = ?1
                 ON CONFLICT (collection_id, work_id) DO NOTHING",
            )
            .bind(id.to_db_string())
            .bind(work.to_db_string())
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(true)
    }

    /// Takes works out of a collection made by hand. Answers false for a
    /// collection that is not there or that the provider made.
    pub async fn remove_from_collection(&self, id: CollectionId, works: &[WorkId]) -> Result<bool> {
        let mut transaction = self.begin().await?;
        let origin: Option<String> =
            sqlx::query_scalar("SELECT origin FROM collections WHERE id = ?")
                .bind(id.to_db_string())
                .fetch_optional(&mut *transaction)
                .await?;
        if origin.as_deref() != Some("manual") {
            return Ok(false);
        }
        for work in works {
            sqlx::query("DELETE FROM collection_items WHERE collection_id = ? AND work_id = ?")
                .bind(id.to_db_string())
                .bind(work.to_db_string())
                .execute(&mut *transaction)
                .await?;
        }
        transaction.commit().await?;
        Ok(true)
    }

    /// The collections made by hand a work is in.
    pub async fn hand_made_collections_of(&self, work_id: WorkId) -> Result<Vec<CollectionId>> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT c.id FROM collections c
               JOIN collection_items ci ON ci.collection_id = c.id
              WHERE ci.work_id = ? AND c.origin = 'manual'",
        )
        .bind(work_id.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(|id| parse_id(id)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{CollectionRecord, IdentifiedWork};
    use melyxar_core::library::LibraryKind;
    use melyxar_core::work::WorkKind;
    use std::path::PathBuf;

    async fn two_libraries() -> (Database, LibraryId, LibraryId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let mut made = Vec::new();
        for (name, root) in [("Films", "/mnt/one/Films"), ("Kids", "/mnt/one/Kids")] {
            made.push(
                database
                    .create_library(name, LibraryKind::Movies, "fr", &[(name.to_string(), PathBuf::from(root))])
                    .await
                    .expect("library created")
                    .id,
            );
        }
        (database, made[0], made[1])
    }

    async fn a_film(database: &Database, library: LibraryId, title: &str, year: i32, saga: Option<&str>) -> WorkId {
        let work = database
            .create_work(library, WorkKind::Movie, title, &title.to_lowercase(), None)
            .await
            .expect("work created");
        database
            .apply_identification(
                work.id,
                &IdentifiedWork {
                    provider: "tmdb".to_string(),
                    external_id: format!("tmdb-{title}"),
                    imdb_id: None,
                    language: "fr".to_string(),
                    title: title.to_string(),
                    sort_title: title.to_lowercase(),
                    tagline: None,
                    overview: None,
                    release_year: Some(year),
                    release_date: None,
                    end_date: None,
                    runtime: None,
                    community_rating: None,
                    age_rating_label: None,
                    genres: Vec::new(),
                    studios: Vec::new(),
                    credits: Vec::new(),
                    collection: saga.map(|name| CollectionRecord {
                        external_id: format!("saga-{name}"),
                        name: name.to_string(),
                        sort_name: name.to_lowercase(),
                    }),
                    trailers: Vec::new(),
                },
                false,
            )
            .await
            .expect("named");
        work.id
    }

    #[tokio::test]
    async fn a_collection_made_by_hand_keeps_its_order_and_shows_each_account_what_it_can_reach() {
        let (database, films, kids) = two_libraries().await;
        let viewer = database
            .create_user("somebody", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account")
            .id;
        let late = a_film(&database, films, "Winter Coat", 2020, None).await;
        let early = a_film(&database, kids, "Snow Fort", 1990, None).await;
        let christmas = database.create_collection("Christmas", "christmas").await.expect("made");
        let empty = database.create_collection("Oscars", "oscars").await.expect("made");

        assert!(database.add_to_collection(christmas, &[late, early]).await.expect("added"));
        assert!(database.add_to_collection(christmas, &[late]).await.expect("added again"));
        assert_eq!(database.hand_made_collections_of(late).await.expect("read"), vec![christmas]);

        let held = database.collection(viewer, christmas, None).await.expect("read").expect("there");
        assert!(held.made_by_hand);
        assert_eq!(
            held.cards.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![late, early],
            "made by hand, the order is the one the works were put in"
        );

        let films_only = [films];
        let listed = database.collections(viewer, Some(&films_only), false).await.expect("read");
        assert_eq!(listed.len(), 1, "an empty collection is left out");
        assert_eq!((listed[0].held, listed[0].cover.as_ref().map(|card| card.id)), (1, Some(late)));
        assert_eq!(
            database.collections(viewer, None, true).await.expect("read").len(),
            2,
            "whoever manages them sees the empty one too"
        );
        assert!(database.collections(viewer, Some(&[]), true).await.expect("read").is_empty());

        assert!(database.remove_from_collection(christmas, &[late]).await.expect("taken out"));
        assert!(database.rename_collection(christmas, "Noël", "noël").await.expect("renamed"));
        let held = database.collection(viewer, christmas, None).await.expect("read").expect("there");
        assert_eq!((held.name.as_str(), held.cards.len()), ("Noël", 1));
        assert!(database.delete_collection(empty).await.expect("deleted"));
        assert!(database.collection(viewer, empty, None).await.expect("read").is_none());
    }

    #[tokio::test]
    async fn a_saga_counts_from_two_films_and_is_never_changed_by_hand() {
        let (database, films, _) = two_libraries().await;
        let viewer = database
            .create_user("somebody", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account")
            .id;
        let second = a_film(&database, films, "Harbour Two", 2012, Some("Harbour")).await;
        assert!(
            database.collections(viewer, None, true).await.expect("read").is_empty(),
            "one film gathers nothing"
        );
        let first = a_film(&database, films, "Harbour One", 2008, Some("Harbour")).await;

        let listed = database.collections(viewer, None, false).await.expect("read");
        assert_eq!(listed.len(), 1);
        assert!(!listed[0].made_by_hand);
        let saga = database.collection(viewer, listed[0].id, None).await.expect("read").expect("there");
        assert_eq!(
            saga.cards.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![first, second],
            "a saga is ordered by when its films came out"
        );
        assert!(!database.add_to_collection(saga.id, &[first]).await.expect("refused"));
        assert!(!database.remove_from_collection(saga.id, &[first]).await.expect("refused"));
        assert!(!database.rename_collection(saga.id, "Other", "other").await.expect("refused"));
        assert!(!database.delete_collection(saga.id).await.expect("refused"));
        assert!(database.hand_made_collections_of(first).await.expect("read").is_empty());
    }
}
