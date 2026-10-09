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

    /// Forgets what was found for a song, so that it is looked up again.
    pub async fn forget_looked_up_lyrics(&self, song: WorkId) -> Result<()> {
        sqlx::query("DELETE FROM music_lyrics WHERE song_id = ?")
            .bind(song.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Forgets what was kept for a song when it knew no words, if that was
    /// found by a way older than `method` or longer ago than `older_than_seconds`:
    /// a song unknown to a poorer way of looking, or unknown for a long
    /// time, is worth asking again. What had words is never forgotten here.
    pub async fn forget_unknown_lyrics_out_of_date(
        &self,
        song: WorkId,
        method: i64,
        older_than_seconds: i64,
    ) -> Result<()> {
        sqlx::query(
            "DELETE FROM music_lyrics
              WHERE song_id = ? AND plain IS NULL AND synced IS NULL
                AND (method < ?
                     OR CAST(strftime('%s', 'now') AS INTEGER)
                        - CAST(strftime('%s', looked_up_at) AS INTEGER) > ?)",
        )
        .bind(song.to_db_string())
        .bind(method)
        .bind(older_than_seconds)
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Keeps what was found for a song, with the way of looking it was found by.
    pub async fn keep_looked_up_lyrics(
        &self,
        song: WorkId,
        found: &LookedUpLyrics,
        at: Timestamp,
        method: i64,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO music_lyrics (song_id, plain, synced, instrumental, looked_up_at, method)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (song_id) DO UPDATE SET
                plain = excluded.plain, synced = excluded.synced,
                instrumental = excluded.instrumental, looked_up_at = excluded.looked_up_at,
                method = excluded.method",
        )
        .bind(song.to_db_string())
        .bind(&found.plain)
        .bind(&found.synced)
        .bind(found.instrumental)
        .bind(timestamp_to_text(at))
        .bind(method)
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
    async fn the_empty_answers_kept_so_far_are_forgotten_and_the_found_ones_stay() {
        let (database, library, _) = collection().await;
        let songs = database
            .music_songs(library, SongOrder::Title, false, 0, 3)
            .await
            .expect("read")
            .items;
        let (unknown, found, instrumental) = (songs[0].id, songs[1].id, songs[2].id);
        let words = LookedUpLyrics { plain: Some("words".into()), ..LookedUpLyrics::default() };
        let without = LookedUpLyrics { instrumental: true, ..LookedUpLyrics::default() };
        for (song, answer) in [(unknown, &LookedUpLyrics::default()), (found, &words), (instrumental, &without)] {
            database
                .keep_looked_up_lyrics(song, answer, melyxar_core::time::now(), 0)
                .await
                .expect("kept");
        }

        sqlx::query(include_str!("../migrations/0111_forget_empty_lyrics_answers.sql"))
            .execute(database.writer())
            .await
            .expect("run");

        assert_eq!(database.looked_up_lyrics(unknown).await.expect("read"), None);
        assert_eq!(database.looked_up_lyrics(found).await.expect("read"), Some(words));
        assert_eq!(database.looked_up_lyrics(instrumental).await.expect("read"), Some(without));
    }

    #[tokio::test]
    async fn the_answers_kept_as_a_song_without_words_are_forgotten_and_the_ones_with_words_stay() {
        let (database, library, _) = collection().await;
        let songs = database
            .music_songs(library, SongOrder::Title, false, 0, 2)
            .await
            .expect("read")
            .items;
        let (wordless, found) = (songs[0].id, songs[1].id);
        let words = LookedUpLyrics { plain: Some("words".into()), instrumental: true, ..LookedUpLyrics::default() };
        let without = LookedUpLyrics { instrumental: true, ..LookedUpLyrics::default() };
        for (song, answer) in [(wordless, &without), (found, &words)] {
            database
                .keep_looked_up_lyrics(song, answer, melyxar_core::time::now(), 0)
                .await
                .expect("kept");
        }

        sqlx::query(include_str!("../migrations/0112_forget_wordless_lyrics_answers.sql"))
            .execute(database.writer())
            .await
            .expect("run");

        assert_eq!(database.looked_up_lyrics(wordless).await.expect("read"), None);
        assert_eq!(database.looked_up_lyrics(found).await.expect("read"), Some(words));
    }

    #[tokio::test]
    async fn an_unknown_song_is_asked_again_when_a_better_way_of_looking_exists_or_a_long_time_has_passed() {
        let (database, library, _) = collection().await;
        let songs = database
            .music_songs(library, SongOrder::Title, false, 0, 3)
            .await
            .expect("read")
            .items;
        let (old_way, recent, with_words) = (songs[0].id, songs[1].id, songs[2].id);
        let words = LookedUpLyrics { plain: Some("words".into()), ..LookedUpLyrics::default() };
        let nothing = LookedUpLyrics::default();
        let now = melyxar_core::time::now();
        database.keep_looked_up_lyrics(old_way, &nothing, now, 1).await.expect("kept");
        database.keep_looked_up_lyrics(recent, &nothing, now, 2).await.expect("kept");
        database.keep_looked_up_lyrics(with_words, &words, now, 1).await.expect("kept");

        for song in [old_way, recent, with_words] {
            database.forget_unknown_lyrics_out_of_date(song, 2, 3600).await.expect("forgotten");
        }
        assert_eq!(database.looked_up_lyrics(old_way).await.expect("read"), None);
        assert_eq!(database.looked_up_lyrics(recent).await.expect("read"), Some(nothing.clone()));
        assert_eq!(database.looked_up_lyrics(with_words).await.expect("read"), Some(words), "words are not forgotten");

        // The same answer, kept long ago, is asked again whatever its way.
        let long_ago = now - time::Duration::days(40);
        database.keep_looked_up_lyrics(recent, &nothing, long_ago, 2).await.expect("kept");
        database.forget_unknown_lyrics_out_of_date(recent, 2, 30 * 86_400).await.expect("forgotten");
        assert_eq!(database.looked_up_lyrics(recent).await.expect("read"), None);
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
            .keep_looked_up_lyrics(song, &nothing, melyxar_core::time::now(), 0)
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
            .keep_looked_up_lyrics(song, &found, melyxar_core::time::now(), 0)
            .await
            .expect("kept again");
        assert_eq!(
            database.looked_up_lyrics(song).await.expect("read"),
            Some(found)
        );

        database.forget_looked_up_lyrics(song).await.expect("forgotten");
        assert_eq!(database.looked_up_lyrics(song).await.expect("read"), None);
    }
}
