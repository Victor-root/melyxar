//! The lyrics of a song as an administrator wrote them in the editor, kept
//! for every account and read before any other words the song has.
//!
//! They were written over the lines as they were shown, lined up and moved by
//! hand: keeping them forgets both, which are in them already.

use melyxar_core::id::WorkId;
use melyxar_core::time::Timestamp;

use crate::convert::timestamp_to_text;
use crate::{Database, Result};

impl Database {
    /// The words written by hand for a song, as they were written.
    pub async fn lyrics_by_hand(&self, song: WorkId) -> Result<Option<String>> {
        let words = sqlx::query_scalar("SELECT words FROM music_lyrics_by_hand WHERE song_id = ?")
            .bind(song.to_db_string())
            .fetch_optional(self.reader())
            .await?;
        Ok(words)
    }

    /// Keeps the words written by hand for a song, in place of any written
    /// before.
    pub async fn keep_lyrics_by_hand(&self, song: WorkId, words: &str, at: Timestamp) -> Result<()> {
        sqlx::query(
            "INSERT INTO music_lyrics_by_hand (song_id, words, written_at) VALUES (?, ?, ?)
             ON CONFLICT (song_id) DO UPDATE SET words = excluded.words, written_at = excluded.written_at",
        )
        .bind(song.to_db_string())
        .bind(words)
        .bind(timestamp_to_text(at))
        .execute(self.writer())
        .await?;
        self.forget_lyrics_timing(song).await
    }

    /// Forgets the words written by hand for a song, so the ones it had
    /// before are shown again.
    pub async fn forget_lyrics_by_hand(&self, song: WorkId) -> Result<()> {
        sqlx::query("DELETE FROM music_lyrics_by_hand WHERE song_id = ?")
            .bind(song.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::music_browse::SongOrder;
    use crate::music_lyrics_timing::LyricsTiming;
    use crate::music_testing::collection;

    #[tokio::test]
    async fn words_written_by_hand_are_kept_replaced_and_forgotten() {
        let (database, library, _) = collection().await;
        let song = database.music_songs(library, SongOrder::Title, false, 0, 1).await.expect("read").items[0].id;
        let now = melyxar_core::time::now();
        assert_eq!(database.lyrics_by_hand(song).await.expect("read"), None);

        database.keep_lyrics_by_hand(song, "[00:01.00]First", now).await.expect("kept");
        database.keep_lyrics_by_hand(song, "[00:02.50]Second", now).await.expect("kept");
        assert_eq!(database.lyrics_by_hand(song).await.expect("read").as_deref(), Some("[00:02.50]Second"));

        database.forget_lyrics_by_hand(song).await.expect("forgotten");
        assert_eq!(database.lyrics_by_hand(song).await.expect("read"), None);
        database.forget_lyrics_by_hand(song).await.expect("nothing left to forget");
    }

    #[tokio::test]
    async fn the_lyrics_of_a_song_follow_it_into_the_song_it_is_merged_into() {
        let (database, library, _) = collection().await;
        let songs = database.music_songs(library, SongOrder::Title, false, 0, 2).await.expect("read").items;
        let (gone, here) = (songs[0].id, songs[1].id);
        let now = melyxar_core::time::now();
        let timing = LyricsTiming { conclusion: "aligned".into(), shift_ms: 120, moves_ms: vec![120], lines_moved: 1, confidence: 0.5 };
        let found = crate::music_lyrics::LookedUpLyrics { plain: Some("Chosen".into()), ..Default::default() };
        // In this order, since each forgets what was set after the one before.
        database.keep_lyrics_by_hand(here, "[00:09.00]Its own", now).await.expect("kept");
        database.keep_lyrics_by_hand(gone, "[00:01.00]Written", now).await.expect("kept");
        database.keep_looked_up_lyrics(gone, &found, now, 4).await.expect("kept");
        database.keep_lyrics_timing(gone, &timing, now).await.expect("kept");
        database.set_lyrics_offset(gone, 800, now).await.expect("set");

        database.merge_work_into(gone, here).await.expect("merged");
        assert_eq!(database.looked_up_lyrics(here).await.expect("read"), Some(found));
        assert_eq!(database.lyrics_timing(here).await.expect("read"), Some(timing));
        assert_eq!(database.lyrics_offset(here).await.expect("read"), 800);
        assert_eq!(
            database.lyrics_by_hand(here).await.expect("read").as_deref(),
            Some("[00:09.00]Its own"),
            "what the song kept already stays"
        );
    }

    #[tokio::test]
    async fn words_written_by_hand_survive_a_merge_into_a_song_without_any() {
        let (database, library, _) = collection().await;
        let songs = database.music_songs(library, SongOrder::Title, false, 0, 2).await.expect("read").items;
        let (gone, here) = (songs[0].id, songs[1].id);
        database.keep_lyrics_by_hand(gone, "[00:01.00]Written", melyxar_core::time::now()).await.expect("kept");
        database.merge_work_into(gone, here).await.expect("merged");
        assert_eq!(database.lyrics_by_hand(here).await.expect("read").as_deref(), Some("[00:01.00]Written"));
    }

    #[tokio::test]
    async fn words_written_by_hand_forget_how_the_lines_were_lined_up_and_moved() {
        let (database, library, _) = collection().await;
        let song = database.music_songs(library, SongOrder::Title, false, 0, 1).await.expect("read").items[0].id;
        let now = melyxar_core::time::now();
        let timing = LyricsTiming { conclusion: "aligned".into(), shift_ms: 120, moves_ms: vec![120], lines_moved: 1, confidence: 0.5 };
        database.keep_lyrics_timing(song, &timing, now).await.expect("kept");
        database.set_lyrics_offset(song, 800, now).await.expect("set");

        database.keep_lyrics_by_hand(song, "Plain words", now).await.expect("kept");
        assert_eq!(database.lyrics_timing(song).await.expect("read"), None);
        assert_eq!(database.lyrics_offset(song).await.expect("read"), 0);
    }
}
