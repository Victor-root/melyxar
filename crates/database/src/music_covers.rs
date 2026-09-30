//! The albums whose cover is looked up online: those with none, never
//! looked up before, found or not.

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::time::Timestamp;
use sqlx::Row;

use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, Result};

/// An album that has no cover, as it is looked up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AlbumWithoutCover {
    pub id: WorkId,
    pub title: String,
    /// The first artist the album is credited to.
    pub artist: Option<String>,
}

/// The albums of a library with no cover that were never looked up.
const LEFT: &str = "FROM works w
      WHERE w.library_id = ? AND w.kind = 'album'
        AND NOT EXISTS (SELECT 1 FROM images i WHERE i.owner_kind = 'work'
                         AND i.image_kind = 'poster' AND i.owner_id = w.id)
        AND NOT EXISTS (SELECT 1 FROM music_cover_lookups l WHERE l.album_id = w.id)";

impl Database {
    /// Up to `limit` albums of a library to look a cover up for.
    pub async fn albums_without_cover(
        &self,
        library: LibraryId,
        limit: i64,
    ) -> Result<Vec<AlbumWithoutCover>> {
        sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT w.id, w.title,
                    (SELECT a.title FROM music_credits c JOIN works a ON a.id = c.artist_id
                      WHERE c.work_id = w.id AND c.role = 'album_artist'
                      ORDER BY c.ordinal LIMIT 1) AS artist
             {LEFT} ORDER BY w.id LIMIT ?"
        )))
        .bind(library.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?
        .iter()
        .map(|row| {
            Ok(AlbumWithoutCover {
                id: parse_id(&row.try_get::<String, _>("id")?)?,
                title: row.try_get("title")?,
                artist: row.try_get("artist")?,
            })
        })
        .collect()
    }

    pub async fn count_albums_without_cover(&self, library: LibraryId) -> Result<i64> {
        Ok(
            sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) {LEFT}")))
                .bind(library.to_db_string())
                .fetch_one(self.reader())
                .await?,
        )
    }

    /// An album looked up, found or not: it is not asked about again.
    pub async fn mark_cover_looked_up(&self, album: WorkId, at: Timestamp) -> Result<()> {
        sqlx::query(
            "INSERT INTO music_cover_lookups (album_id, looked_up_at) VALUES (?, ?)
             ON CONFLICT (album_id) DO UPDATE SET looked_up_at = excluded.looked_up_at",
        )
        .bind(album.to_db_string())
        .bind(timestamp_to_text(at))
        .execute(self.writer())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::music_testing::collection;

    #[tokio::test]
    async fn an_album_without_a_cover_is_looked_up_once() {
        let (database, library, _) = collection().await;
        let left = database
            .albums_without_cover(library, 10)
            .await
            .expect("read");
        assert_eq!(left.len(), 4, "no album of the collection has a cover");
        assert_eq!(
            database
                .count_albums_without_cover(library)
                .await
                .expect("read"),
            4
        );
        let road = left
            .iter()
            .find(|album| album.title == "Road")
            .expect("filed");
        assert_eq!(road.artist.as_deref(), Some("The Lanterns"));

        database
            .mark_cover_looked_up(road.id, melyxar_core::time::now())
            .await
            .expect("marked");
        let left = database
            .albums_without_cover(library, 10)
            .await
            .expect("read");
        assert_eq!(left.len(), 3);
        assert!(left.iter().all(|album| album.id != road.id));
    }
}
