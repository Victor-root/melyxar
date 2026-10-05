//! What an account watches, as a taste: the genres of the titles it has
//! watched or begun, counted once per title.

use melyxar_core::id::UserId;
use sqlx::Row;

use crate::{Database, Result};

/// One genre of what an account watches, and how many titles of it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchedGenre {
    /// movie or series.
    pub kind: String,
    /// The language the genre is written in: the one its library is
    /// described in.
    pub language: String,
    pub name: String,
    pub titles: i64,
}

impl Database {
    /// The genres of the films and series this account has watched or begun,
    /// most watched first. An episode counts for its series, and a series
    /// counts once however many of its episodes were watched: a taste is
    /// told by how many titles, not by how long one of them ran.
    pub async fn watched_genres(&self, user: UserId) -> Result<Vec<WatchedGenre>> {
        let rows = sqlx::query(
            "WITH watched AS (
                 SELECT DISTINCT
                        CASE w.kind
                            WHEN 'episode' THEN (SELECT s.parent_id FROM works s WHERE s.id = w.parent_id)
                            WHEN 'season' THEN w.parent_id
                            ELSE w.id
                        END AS title_id
                   FROM playback_progress p
                   JOIN works w ON w.id = p.work_id
                  WHERE p.user_id = ?
                    AND p.state IN ('in_progress', 'watched')
                    AND w.kind IN ('movie', 'series', 'season', 'episode')
             )
             SELECT t.kind, l.metadata_language, g.name, COUNT(*) AS titles
               FROM watched
               JOIN works t ON t.id = watched.title_id
               JOIN libraries l ON l.id = t.library_id
               JOIN work_genres wg ON wg.work_id = t.id
               JOIN genres g ON g.id = wg.genre_id
              WHERE t.kind IN ('movie', 'series')
              GROUP BY t.kind, l.metadata_language, g.name
              ORDER BY titles DESC, g.name, t.kind",
        )
        .bind(user.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter()
            .map(|row| {
                Ok(WatchedGenre {
                    kind: row.try_get("kind")?,
                    language: row.try_get("metadata_language")?,
                    name: row.try_get("name")?,
                    titles: row.try_get("titles")?,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use melyxar_core::id::WorkId;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::time::Millis;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::{PlaybackState, WorkKind};

    use super::*;

    async fn with_genres(database: &Database, work: WorkId, names: &[&str]) {
        for name in names {
            let id = format!("genre-{name}");
            sqlx::query("INSERT OR IGNORE INTO genres (id, name) VALUES (?, ?)")
                .bind(&id)
                .bind(name)
                .execute(database.writer())
                .await
                .expect("genre");
            sqlx::query("INSERT INTO work_genres (work_id, genre_id) VALUES (?, ?)")
                .bind(work.to_db_string())
                .bind(&id)
                .execute(database.writer())
                .await
                .expect("linked");
        }
    }

    async fn saw(database: &Database, user: UserId, work: WorkId, state: PlaybackState) {
        database
            .record_playback_progress(user, work, Millis::new(0), state, melyxar_core::time::now())
            .await
            .expect("marked");
    }

    #[tokio::test]
    async fn a_taste_counts_each_title_once_and_follows_an_episode_to_its_series() {
        let database = Database::open_in_memory().await.expect("opens");
        let me = database
            .create_user("me", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let other = database
            .create_user("other", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let library = database
            .create_library("Mixed", LibraryKind::Movies, "fr", &[("disk".to_string(), PathBuf::from("/m"))])
            .await
            .expect("library")
            .id;

        let scary = database
            .create_work(library, WorkKind::Movie, "Scary", "scary", None)
            .await
            .expect("film");
        let ghosts = database
            .create_work(library, WorkKind::Movie, "Ghosts", "ghosts", None)
            .await
            .expect("film");
        let funny = database
            .create_work(library, WorkKind::Movie, "Funny", "funny", None)
            .await
            .expect("film");
        let unseen = database
            .create_work(library, WorkKind::Movie, "Unseen", "unseen", None)
            .await
            .expect("film");
        let series = database
            .create_work(library, WorkKind::Series, "Long", "long", None)
            .await
            .expect("series");
        let season = database
            .create_child_work(library, series.id, 1, WorkKind::Season, "Season", "season")
            .await
            .expect("season");
        let first = database
            .create_child_work(library, season.id, 1, WorkKind::Episode, "One", "one")
            .await
            .expect("episode");
        let second = database
            .create_child_work(library, season.id, 2, WorkKind::Episode, "Two", "two")
            .await
            .expect("episode");

        with_genres(&database, scary.id, &["Horreur"]).await;
        with_genres(&database, ghosts.id, &["Horreur", "Thriller"]).await;
        with_genres(&database, funny.id, &["Comédie"]).await;
        with_genres(&database, unseen.id, &["Western"]).await;
        with_genres(&database, series.id, &["Thriller"]).await;

        saw(&database, me, scary.id, PlaybackState::Watched).await;
        saw(&database, me, ghosts.id, PlaybackState::InProgress).await;
        saw(&database, me, funny.id, PlaybackState::NotStarted).await;
        saw(&database, me, first.id, PlaybackState::Watched).await;
        saw(&database, me, second.id, PlaybackState::Watched).await;
        saw(&database, other, unseen.id, PlaybackState::Watched).await;

        let taste = database.watched_genres(me).await.expect("read");
        let said: Vec<(&str, &str, i64)> = taste
            .iter()
            .map(|genre| (genre.kind.as_str(), genre.name.as_str(), genre.titles))
            .collect();
        assert_eq!(
            said,
            vec![
                ("movie", "Horreur", 2),
                ("movie", "Thriller", 1),
                ("series", "Thriller", 1),
            ],
            "two episodes of a series are one title, a film not begun and another account's films count for nothing"
        );
        assert!(taste.iter().all(|genre| genre.language == "fr"));
        assert!(database.watched_genres(other).await.expect("read").iter().all(|genre| genre.name == "Western"));
    }
}
