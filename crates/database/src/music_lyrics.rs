//! What was found online for the lyrics of a song, kept so it is asked once,
//! and what a song is asked by.

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::time::Timestamp;
use sqlx::Row;

use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, Result};

/// What LRCLIB answered for a song. Nothing in either form, and not a song
/// without words, is an answer too: the song it knows nothing of.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LookedUpLyrics {
    pub plain: Option<String>,
    pub synced: Option<String>,
    pub instrumental: bool,
}

/// What a song is looked up by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongToLookUp {
    pub library_id: LibraryId,
    pub title: String,
    /// The first artist credited on it.
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<i64>,
}

impl Database {
    /// What was found for a song, if it was ever looked up.
    pub async fn looked_up_lyrics(&self, song: WorkId) -> Result<Option<LookedUpLyrics>> {
        let row =
            sqlx::query("SELECT plain, synced, instrumental FROM music_lyrics WHERE song_id = ?")
                .bind(song.to_db_string())
                .fetch_optional(self.reader())
                .await?;
        row.map(|row| {
            Ok(LookedUpLyrics {
                plain: row.try_get("plain")?,
                synced: row.try_get("synced")?,
                instrumental: row.try_get::<i64, _>("instrumental")? != 0,
            })
        })
        .transpose()
    }

    pub async fn keep_looked_up_lyrics(
        &self,
        song: WorkId,
        found: &LookedUpLyrics,
        at: Timestamp,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO music_lyrics (song_id, plain, synced, instrumental, looked_up_at)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (song_id) DO UPDATE SET
                plain = excluded.plain, synced = excluded.synced,
                instrumental = excluded.instrumental, looked_up_at = excluded.looked_up_at",
        )
        .bind(song.to_db_string())
        .bind(&found.plain)
        .bind(&found.synced)
        .bind(found.instrumental)
        .bind(timestamp_to_text(at))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// A song as it is looked up, or nothing for a work that is not one.
    pub async fn song_to_look_up(&self, song: WorkId) -> Result<Option<SongToLookUp>> {
        let row = sqlx::query(
            "SELECT w.title, w.library_id, al.title AS album,
                    (SELECT a.title FROM music_credits c JOIN works a ON a.id = c.artist_id
                      WHERE c.work_id = w.id AND c.role = 'artist'
                      ORDER BY c.ordinal LIMIT 1) AS artist,
                    (SELECT max(s.duration_ms) FROM media_sources s WHERE s.work_id = w.id) AS duration_ms
               FROM works w LEFT JOIN works al ON al.id = w.parent_id
              WHERE w.id = ? AND w.kind = 'song'",
        )
        .bind(song.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        row.map(|row| {
            Ok(SongToLookUp {
                library_id: parse_id(&row.try_get::<String, _>("library_id")?)?,
                title: row.try_get("title")?,
                artist: row.try_get("artist")?,
                album: row.try_get("album")?,
                duration_ms: row.try_get("duration_ms")?,
            })
        })
        .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music_browse::SongOrder;
    use crate::music_testing::collection;

    #[tokio::test]
    async fn a_song_is_looked_up_by_its_title_first_artist_and_album() {
        let (database, library, _) = collection().await;
        let songs = database
            .music_songs(library, SongOrder::Title, false, 0, 10)
            .await
            .expect("read")
            .items;
        let road = songs
            .iter()
            .find(|song| song.title == "The Long Road")
            .expect("in the collection");
        let asked = database
            .song_to_look_up(road.id)
            .await
            .expect("read")
            .expect("a song");
        assert_eq!(asked.library_id, library);
        assert_eq!(asked.title, "The Long Road");
        assert_eq!(asked.artist.as_deref(), Some("The Lanterns"));
        assert_eq!(asked.album.as_deref(), Some("Road"));

        let album = road.album.as_ref().expect("on an album").id;
        assert_eq!(database.song_to_look_up(album).await.expect("read"), None);
    }

    #[tokio::test]
    async fn what_was_found_online_is_kept_found_or_not() {
        let (database, library, _) = collection().await;
        let song = database
            .music_songs(library, SongOrder::Title, false, 0, 1)
            .await
            .expect("read")
            .items[0]
            .id;
        assert_eq!(database.looked_up_lyrics(song).await.expect("read"), None);

        let nothing = LookedUpLyrics::default();
        database
            .keep_looked_up_lyrics(song, &nothing, melyxar_core::time::now())
            .await
            .expect("kept");
        assert_eq!(
            database.looked_up_lyrics(song).await.expect("read"),
            Some(nothing)
        );

        let found = LookedUpLyrics {
            plain: Some("Words".to_string()),
            synced: Some("[00:01.00]Words".to_string()),
            instrumental: false,
        };
        database
            .keep_looked_up_lyrics(song, &found, melyxar_core::time::now())
            .await
            .expect("kept again");
        assert_eq!(
            database.looked_up_lyrics(song).await.expect("read"),
            Some(found)
        );
    }
}
