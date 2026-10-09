//! How the stamps of the lyrics of a song were moved to fall where its words
//! are heard, kept so the lyrics are shown moved every time and the song is
//! not listened to again for it. What came of the attempt is kept too when
//! nothing was moved.

use melyxar_core::id::WorkId;
use melyxar_core::time::Timestamp;
use sqlx::Row;

use crate::convert::timestamp_to_text;
use crate::{Database, DatabaseError, Result};

/// What was found of where the lines of a song stand.
#[derive(Debug, Clone, PartialEq)]
pub struct LyricsTiming {
    /// What came of the attempt, as the word the application gives it.
    pub conclusion: String,
    /// The move of the whole song, in milliseconds.
    pub shift_ms: i64,
    /// The move of each line, the shift included, in the order of the lines.
    /// Empty unless the lines were moved.
    pub moves_ms: Vec<i64>,
    /// How many lines were moved by a noticeable amount.
    pub lines_moved: u32,
    /// How sure the shift was, from nought to one.
    pub confidence: f32,
}

impl Database {
    /// How the lines of a song were moved, if they were.
    pub async fn lyrics_timing(&self, song: WorkId) -> Result<Option<LyricsTiming>> {
        let row = sqlx::query(
            "SELECT conclusion, shift_ms, moves_ms, lines_moved, confidence FROM music_lyrics_timing WHERE song_id = ?",
        )
        .bind(song.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        row.map(|row| {
            let moves: String = row.try_get("moves_ms")?;
            Ok(LyricsTiming {
                conclusion: row.try_get("conclusion")?,
                shift_ms: row.try_get("shift_ms")?,
                moves_ms: if moves.is_empty() {
                    Vec::new()
                } else {
                    moves
                        .split(',')
                        .map(|move_ms| {
                            move_ms.parse().map_err(|_| {
                                DatabaseError::Corrupt(format!("the move '{move_ms}' of a line is not a number"))
                            })
                        })
                        .collect::<Result<Vec<i64>>>()?
                },
                lines_moved: u32::try_from(row.try_get::<i64, _>("lines_moved")?).unwrap_or(0),
                confidence: row.try_get::<f64, _>("confidence")? as f32,
            })
        })
        .transpose()
    }

    /// Keeps how the lines of a song were moved, in place of what was.
    pub async fn keep_lyrics_timing(&self, song: WorkId, timing: &LyricsTiming, at: Timestamp) -> Result<()> {
        let moves = timing.moves_ms.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        sqlx::query(
            "INSERT INTO music_lyrics_timing (song_id, conclusion, shift_ms, moves_ms, lines_moved, confidence, measured_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (song_id) DO UPDATE SET
                conclusion = excluded.conclusion, shift_ms = excluded.shift_ms, moves_ms = excluded.moves_ms,
                lines_moved = excluded.lines_moved, confidence = excluded.confidence,
                measured_at = excluded.measured_at",
        )
        .bind(song.to_db_string())
        .bind(&timing.conclusion)
        .bind(timing.shift_ms)
        .bind(moves)
        .bind(i64::from(timing.lines_moved))
        .bind(f64::from(timing.confidence))
        .bind(timestamp_to_text(at))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Forgets how the lines of a song were moved, so they are shown as
    /// stamped.
    pub async fn forget_lyrics_timing(&self, song: WorkId) -> Result<()> {
        sqlx::query("DELETE FROM music_lyrics_timing WHERE song_id = ?")
            .bind(song.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music_browse::SongOrder;
    use crate::music_lyrics::LookedUpLyrics;
    use crate::music_testing::collection;

    async fn a_song(database: &Database, library: melyxar_core::id::LibraryId) -> WorkId {
        database
            .music_songs(library, SongOrder::Title, false, 0, 1)
            .await
            .expect("read")
            .items[0]
            .id
    }

    #[tokio::test]
    async fn how_the_lines_were_moved_is_kept_read_back_and_forgotten() {
        let (database, library, _) = collection().await;
        let song = a_song(&database, library).await;
        assert_eq!(database.lyrics_timing(song).await.expect("read"), None);

        let timing = LyricsTiming { conclusion: "aligned".into(), shift_ms: -350, moves_ms: vec![-350, -310, -400, 0], lines_moved: 3, confidence: 0.75 };
        database.keep_lyrics_timing(song, &timing, melyxar_core::time::now()).await.expect("kept");
        assert_eq!(database.lyrics_timing(song).await.expect("read"), Some(timing));

        database.forget_lyrics_timing(song).await.expect("forgotten");
        assert_eq!(database.lyrics_timing(song).await.expect("read"), None);
    }

    #[tokio::test]
    async fn an_attempt_that_moved_nothing_is_kept_with_what_came_of_it() {
        let (database, library, _) = collection().await;
        let song = a_song(&database, library).await;
        let timing = LyricsTiming { conclusion: "not_sure".into(), shift_ms: 0, moves_ms: Vec::new(), lines_moved: 0, confidence: 0.1 };
        database.keep_lyrics_timing(song, &timing, melyxar_core::time::now()).await.expect("kept");
        assert_eq!(database.lyrics_timing(song).await.expect("read"), Some(timing));
    }

    #[tokio::test]
    async fn other_lyrics_for_a_song_forget_how_the_old_ones_were_moved() {
        let (database, library, _) = collection().await;
        let song = a_song(&database, library).await;
        let timing = LyricsTiming { conclusion: "aligned".into(), shift_ms: 120, moves_ms: vec![120, 100], lines_moved: 2, confidence: 0.5 };
        let words = LookedUpLyrics { plain: Some("words".into()), ..LookedUpLyrics::default() };

        database.keep_lyrics_timing(song, &timing, melyxar_core::time::now()).await.expect("kept");
        database.keep_looked_up_lyrics(song, &words, melyxar_core::time::now(), 2).await.expect("kept");
        assert_eq!(database.lyrics_timing(song).await.expect("read"), None, "new words, new lines");

        database.keep_lyrics_timing(song, &timing, melyxar_core::time::now()).await.expect("kept");
        database.forget_looked_up_lyrics(song).await.expect("forgotten");
        assert_eq!(database.lyrics_timing(song).await.expect("read"), None);
    }
}
