//! The ratings a work carries from elsewhere than its provider, and which
//! works are owed one.
//!
//! A work is rated through its IMDb identifier, which is what both the file of
//! IMDb ratings and OMDb answer to. A row with no value is a work asked about
//! that had none, kept so it is not asked about again before its time.

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::rating::{Rating, RatingSource};
use melyxar_core::time::{Timestamp, now};
use melyxar_core::work::WorkKind;
use sqlx::Row;

use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

/// What a rating is looked up by, and what the answer is written down as.
const IMDB: &str = "imdb";

/// The named films and series of a library that carry an IMDb identifier and
/// were not asked about since a moment.
macro_rules! works_to_rate {
    () => {
        "SELECT w.id, x.external_id AS imdb_id, r.checked_at, w.added_at
       FROM works w
       JOIN work_external_ids x ON x.work_id = w.id AND x.provider = 'imdb'
       LEFT JOIN work_ratings r ON r.work_id = w.id AND r.source = ?
      WHERE w.library_id = ?
        AND w.kind IN ('movie', 'series')
        AND w.identification IN ('identified', 'manual')
        AND (r.checked_at IS NULL OR r.checked_at < ?)"
    };
}

/// The named films and series of a library that carry no IMDb identifier and
/// were not asked for one since a moment. Asking was written down as an IMDb
/// rating with no value.
macro_rules! works_without_an_imdb_id {
    () => {
        "SELECT w.id, w.kind, e.external_id
       FROM works w
       JOIN work_external_ids e ON e.work_id = w.id AND e.provider = ?
       LEFT JOIN work_ratings r ON r.work_id = w.id AND r.source = 'imdb'
      WHERE w.library_id = ?
        AND w.kind IN ('movie', 'series')
        AND w.identification IN ('identified', 'manual')
        AND NOT EXISTS (
            SELECT 1 FROM work_external_ids x
             WHERE x.work_id = w.id AND x.provider = 'imdb')
        AND (r.checked_at IS NULL OR r.checked_at < ?)"
    };
}

/// A work owed a rating, and the identifier it is looked up by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkToRate {
    pub id: WorkId,
    pub imdb_id: String,
}

/// A work that carries no IMDb identifier, and what its provider calls it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkWithoutAnImdbId {
    pub id: WorkId,
    pub kind: WorkKind,
    pub external_id: String,
}

/// What asking about one work came to: its rating and how many voted, or
/// nothing when it has none there.
pub type RatingFound = (WorkId, Option<(f64, Option<i64>)>);

impl Database {
    /// The works of a library owed a rating from this source, never asked
    /// about first, then the longest ago, and at most `limit` of them.
    pub async fn works_to_rate(
        &self,
        library_id: LibraryId,
        source: RatingSource,
        checked_before: Timestamp,
        limit: Option<i64>,
    ) -> Result<Vec<WorkToRate>> {
        let rows = sqlx::query(concat!(
            works_to_rate!(),
            " ORDER BY checked_at IS NOT NULL, checked_at, added_at LIMIT ?"
        ))
        .bind(source.as_str())
        .bind(library_id.to_db_string())
        .bind(timestamp_to_text(checked_before))
        .bind(limit.unwrap_or(-1))
        .fetch_all(self.reader())
        .await?;
        rows.iter()
            .map(|row| {
                Ok(WorkToRate {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    imdb_id: row.try_get("imdb_id")?,
                })
            })
            .collect()
    }

    /// How many works of a library are owed a rating from this source.
    pub async fn count_works_to_rate(
        &self,
        library_id: LibraryId,
        source: RatingSource,
        checked_before: Timestamp,
    ) -> Result<i64> {
        Ok(
            sqlx::query_scalar(concat!("SELECT count(*) FROM (", works_to_rate!(), ")"))
                .bind(source.as_str())
                .bind(library_id.to_db_string())
                .bind(timestamp_to_text(checked_before))
                .fetch_one(self.reader())
                .await?,
        )
    }

    /// The works of a library this provider named that carry no IMDb
    /// identifier and were not asked for one since `checked_before`.
    pub async fn works_without_an_imdb_id(
        &self,
        library_id: LibraryId,
        provider: &str,
        checked_before: Timestamp,
    ) -> Result<Vec<WorkWithoutAnImdbId>> {
        let rows = sqlx::query(works_without_an_imdb_id!())
            .bind(provider)
            .bind(library_id.to_db_string())
            .bind(timestamp_to_text(checked_before))
            .fetch_all(self.reader())
            .await?;
        rows.iter()
            .map(|row| {
                let kind: String = row.try_get("kind")?;
                Ok(WorkWithoutAnImdbId {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    kind: WorkKind::parse(&kind)
                        .ok_or_else(|| DatabaseError::Corrupt(format!("work kind '{kind}'")))?,
                    external_id: row.try_get("external_id")?,
                })
            })
            .collect()
    }

    /// How many works of a library are owed an IMDb identifier.
    pub async fn count_works_without_an_imdb_id(
        &self,
        library_id: LibraryId,
        provider: &str,
        checked_before: Timestamp,
    ) -> Result<i64> {
        Ok(sqlx::query_scalar(concat!(
            "SELECT count(*) FROM (",
            works_without_an_imdb_id!(),
            ")"
        ))
        .bind(provider)
        .bind(library_id.to_db_string())
        .bind(timestamp_to_text(checked_before))
        .fetch_one(self.reader())
        .await?)
    }

    /// Gives a work the IMDb identifier its provider knows it by, or writes
    /// down that it has none, so it is not asked for one again before its
    /// time.
    pub async fn set_imdb_id(&self, work_id: WorkId, imdb_id: Option<&str>) -> Result<()> {
        match imdb_id {
            Some(imdb_id) => self.set_work_external_id(work_id, IMDB, imdb_id).await,
            None => self
                .write_ratings(RatingSource::Imdb, &[(work_id, None)])
                .await
                .map(|_| ()),
        }
    }

    /// Writes down what asking these works about their rating came to, and
    /// answers how many of them now say something they did not.
    pub async fn write_ratings(
        &self,
        source: RatingSource,
        found: &[RatingFound],
    ) -> Result<usize> {
        let at = timestamp_to_text(now());
        let mut changed = 0;
        let mut transaction = self.begin().await?;
        for (work_id, rating) in found {
            let (value, votes) = rating.map_or((None, None), |(value, votes)| (Some(value), votes));
            let before: Option<(Option<f64>, Option<i64>)> = sqlx::query_as(
                "SELECT value, votes FROM work_ratings WHERE work_id = ? AND source = ?",
            )
            .bind(work_id.to_db_string())
            .bind(source.as_str())
            .fetch_optional(&mut *transaction)
            .await?;
            if before.unwrap_or((None, None)) != (value, votes) {
                changed += 1;
            }
            sqlx::query(
                "INSERT INTO work_ratings (work_id, source, value, votes, checked_at)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (work_id, source) DO UPDATE SET
                    value = excluded.value,
                    votes = excluded.votes,
                    checked_at = excluded.checked_at",
            )
            .bind(work_id.to_db_string())
            .bind(source.as_str())
            .bind(value)
            .bind(votes)
            .bind(&at)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(changed)
    }

    /// How many works were asked about their rating from this source since a
    /// moment, which is how much of a day's allowance is spent.
    pub async fn ratings_checked_since(
        &self,
        source: RatingSource,
        since: Timestamp,
    ) -> Result<i64> {
        Ok(sqlx::query_scalar(
            "SELECT count(*) FROM work_ratings WHERE source = ? AND checked_at >= ?",
        )
        .bind(source.as_str())
        .bind(timestamp_to_text(since))
        .fetch_one(self.reader())
        .await?)
    }

    /// The ratings a work carries, in the order of their sources.
    pub async fn work_ratings(&self, work_id: WorkId) -> Result<Vec<Rating>> {
        let rows = sqlx::query(
            "SELECT source, value, votes FROM work_ratings
              WHERE work_id = ? AND value IS NOT NULL",
        )
        .bind(work_id.to_db_string())
        .fetch_all(self.reader())
        .await?;
        let mut ratings = rows
            .iter()
            .map(|row| {
                let source: String = row.try_get("source")?;
                Ok(Rating {
                    source: RatingSource::parse(&source).ok_or_else(|| {
                        DatabaseError::Corrupt(format!("rating source '{source}'"))
                    })?,
                    value: row.try_get("value")?,
                    votes: row.try_get("votes")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        ratings.sort_by_key(|rating| {
            RatingSource::ALL
                .iter()
                .position(|one| *one == rating.source)
        });
        Ok(ratings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use std::path::PathBuf;
    use time::Duration;

    async fn a_library() -> (Database, LibraryId) {
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
        (database, library.id)
    }

    async fn a_named_work(
        database: &Database,
        library_id: LibraryId,
        kind: WorkKind,
        title: &str,
        imdb_id: Option<&str>,
    ) -> WorkId {
        let work = database
            .create_work(library_id, kind, title, &title.to_lowercase(), None)
            .await
            .expect("work created");
        sqlx::query("UPDATE works SET identification = 'identified' WHERE id = ?")
            .bind(work.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("named");
        database
            .set_work_external_id(work.id, "tmdb", &format!("tmdb-{title}"))
            .await
            .expect("tmdb id");
        if let Some(imdb_id) = imdb_id {
            database
                .set_work_external_id(work.id, "imdb", imdb_id)
                .await
                .expect("imdb id");
        }
        work.id
    }

    #[tokio::test]
    async fn a_work_is_owed_a_rating_until_asked_and_again_once_its_time_has_come() {
        let (database, library_id) = a_library().await;
        let harbour = a_named_work(
            &database,
            library_id,
            WorkKind::Movie,
            "Quiet Harbour",
            Some("tt0000001"),
        )
        .await;
        let lantern = a_named_work(
            &database,
            library_id,
            WorkKind::Series,
            "Lantern Row",
            Some("tt0000002"),
        )
        .await;
        a_named_work(&database, library_id, WorkKind::Movie, "No Number", None).await;

        let later = now() + Duration::seconds(1);
        let owed = database
            .works_to_rate(library_id, RatingSource::Imdb, later, None)
            .await
            .expect("read");
        assert_eq!(
            owed.len(),
            2,
            "a work without an IMDb identifier cannot be rated"
        );
        assert_eq!(
            database
                .count_works_to_rate(library_id, RatingSource::Imdb, later)
                .await
                .expect("count"),
            2
        );

        let changed = database
            .write_ratings(
                RatingSource::Imdb,
                &[(harbour, Some((7.4, Some(1200)))), (lantern, None)],
            )
            .await
            .expect("written");
        assert_eq!(
            changed, 1,
            "a work that had nothing and still has nothing did not change"
        );

        let before = now() - Duration::hours(1);
        assert!(
            database
                .works_to_rate(library_id, RatingSource::Imdb, before, None)
                .await
                .expect("read")
                .is_empty()
        );
        assert_eq!(
            database
                .works_to_rate(library_id, RatingSource::RottenTomatoes, before, Some(1))
                .await
                .expect("read")
                .len(),
            1,
            "each source keeps its own time, and the limit holds"
        );
        assert_eq!(
            database
                .works_to_rate(
                    library_id,
                    RatingSource::Imdb,
                    now() + Duration::seconds(1),
                    None
                )
                .await
                .expect("read")
                .len(),
            2,
            "once its time has come a work is owed a rating again"
        );

        assert_eq!(
            database.work_ratings(harbour).await.expect("read"),
            vec![Rating {
                source: RatingSource::Imdb,
                value: 7.4,
                votes: Some(1200)
            }]
        );
        assert!(
            database
                .work_ratings(lantern)
                .await
                .expect("read")
                .is_empty()
        );
        assert_eq!(
            database
                .ratings_checked_since(RatingSource::Imdb, before)
                .await
                .expect("count"),
            2
        );
        assert_eq!(
            database
                .write_ratings(RatingSource::Imdb, &[(harbour, Some((7.4, Some(1200))))])
                .await
                .expect("written"),
            0,
            "the same rating again is no change"
        );
    }

    #[tokio::test]
    async fn a_work_without_an_imdb_id_is_asked_for_one_once_until_its_time_comes_round() {
        let (database, library_id) = a_library().await;
        let lantern =
            a_named_work(&database, library_id, WorkKind::Series, "Lantern Row", None).await;
        let found = a_named_work(&database, library_id, WorkKind::Series, "Found Row", None).await;
        a_named_work(
            &database,
            library_id,
            WorkKind::Movie,
            "Numbered",
            Some("tt0000003"),
        )
        .await;

        let later = now() + Duration::seconds(1);
        let owed = database
            .works_without_an_imdb_id(library_id, "tmdb", later)
            .await
            .expect("read");
        assert_eq!(owed.len(), 2);
        assert!(owed.iter().all(|work| work.kind == WorkKind::Series));
        assert!(
            owed.iter()
                .any(|work| work.external_id == "tmdb-Lantern Row")
        );

        database.set_imdb_id(lantern, None).await.expect("none");
        database
            .set_imdb_id(found, Some("tt0000004"))
            .await
            .expect("found");

        let before = now() - Duration::hours(1);
        assert_eq!(
            database
                .count_works_without_an_imdb_id(library_id, "tmdb", before)
                .await
                .expect("count"),
            0,
            "one asked about waits its turn, and one found carries its identifier"
        );
        assert_eq!(
            database
                .count_works_without_an_imdb_id(library_id, "tmdb", later)
                .await
                .expect("count"),
            1
        );
        assert_eq!(
            database
                .count_works_to_rate(library_id, RatingSource::Imdb, later)
                .await
                .expect("count"),
            2,
            "the identifier found makes the work one to rate"
        );
    }
}
