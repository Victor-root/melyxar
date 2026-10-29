//! The lyrics of a song as an administrator wrote them in the editor, kept
//! for every account and read before any other words the song has.
//!
//! Every save is a version, kept until somebody deletes it by hand: going
//! back to the lyrics the song had, or taking others, only sets the one in
//! service aside. The versions are not tied to the song by the database, so
//! that taking the song away leaves them; they keep the folder and the path
//! of its file, and a file back at the same place takes them up again.
//!
//! They were written over the lines as they were shown, lined up and moved by
//! hand: putting one in service forgets both, which are in it already.

use melyxar_core::id::WorkId;
use melyxar_core::time::Timestamp;
use sqlx::Row;

use crate::convert::{parse_timestamp, timestamp_to_text};
use crate::{Database, Result};

/// One version of the lyrics of a song, as it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LyricsVersion {
    pub id: i64,
    pub words: String,
    pub written_at: Timestamp,
    /// The one shown, before any other words the song has.
    pub in_use: bool,
}

impl Database {
    /// The words written by hand in service for a song.
    pub async fn lyrics_by_hand(&self, song: WorkId) -> Result<Option<String>> {
        let words = sqlx::query_scalar("SELECT words FROM music_lyrics_versions WHERE song_id = ? AND in_use = 1")
            .bind(song.to_db_string())
            .fetch_optional(self.reader())
            .await?;
        Ok(words)
    }

    /// Every version written for a song, the latest first.
    pub async fn lyrics_versions(&self, song: WorkId) -> Result<Vec<LyricsVersion>> {
        let rows = sqlx::query(
            "SELECT id, words, written_at, in_use FROM music_lyrics_versions WHERE song_id = ? ORDER BY id DESC",
        )
        .bind(song.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter()
            .map(|row| {
                Ok(LyricsVersion {
                    id: row.try_get("id")?,
                    words: row.try_get("words")?,
                    written_at: parse_timestamp(&row.try_get::<String, _>("written_at")?)?,
                    in_use: row.try_get::<i64, _>("in_use")? != 0,
                })
            })
            .collect()
    }

    /// Keeps the words written by hand for a song as a new version, in
    /// service from now on; the same words as the latest version put that one
    /// back in service rather than keeping them twice.
    pub async fn keep_lyrics_by_hand(&self, song: WorkId, words: &str, at: Timestamp) -> Result<()> {
        let mut transaction = self.begin().await?;
        sqlx::query("UPDATE music_lyrics_versions SET in_use = 0 WHERE song_id = ?")
            .bind(song.to_db_string())
            .execute(&mut *transaction)
            .await?;
        let latest: Option<(i64, String)> = sqlx::query_as(
            "SELECT id, words FROM music_lyrics_versions WHERE song_id = ? ORDER BY id DESC LIMIT 1",
        )
        .bind(song.to_db_string())
        .fetch_optional(&mut *transaction)
        .await?;
        match latest {
            Some((id, kept)) if kept == words => {
                sqlx::query("UPDATE music_lyrics_versions SET in_use = 1 WHERE id = ?")
                    .bind(id)
                    .execute(&mut *transaction)
                    .await?;
            }
            _ => {
                sqlx::query(
                    "INSERT INTO music_lyrics_versions (song_id, root_path, relative_path, words, written_at, in_use)
                     SELECT ?1,
                            (SELECT r.path FROM media_sources s JOIN library_roots r ON r.id = s.root_id
                              WHERE s.work_id = ?1 ORDER BY s.relative_path LIMIT 1),
                            (SELECT s.relative_path FROM media_sources s
                              WHERE s.work_id = ?1 ORDER BY s.relative_path LIMIT 1),
                            ?2, ?3, 1",
                )
                .bind(song.to_db_string())
                .bind(words)
                .bind(timestamp_to_text(at))
                .execute(&mut *transaction)
                .await?;
            }
        }
        transaction.commit().await?;
        self.forget_lyrics_timing(song).await
    }

    /// Puts one version of a song back in service. False when the song has
    /// no such version.
    pub async fn use_lyrics_version(&self, song: WorkId, version: i64) -> Result<bool> {
        let mut transaction = self.begin().await?;
        let found = sqlx::query("SELECT 1 FROM music_lyrics_versions WHERE id = ? AND song_id = ?")
            .bind(version)
            .bind(song.to_db_string())
            .fetch_optional(&mut *transaction)
            .await?
            .is_some();
        if !found {
            return Ok(false);
        }
        sqlx::query("UPDATE music_lyrics_versions SET in_use = (id = ?) WHERE song_id = ?")
            .bind(version)
            .bind(song.to_db_string())
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        self.forget_lyrics_timing(song).await?;
        Ok(true)
    }

    /// Sets the version in service aside, so the words the song had before
    /// are shown again. Every version stays.
    pub async fn set_lyrics_by_hand_aside(&self, song: WorkId) -> Result<()> {
        sqlx::query("UPDATE music_lyrics_versions SET in_use = 0 WHERE song_id = ?")
            .bind(song.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Deletes one version of a song for good, the only way one goes. False
    /// when the song has no such version.
    pub async fn delete_lyrics_version(&self, song: WorkId, version: i64) -> Result<bool> {
        let deleted = sqlx::query("DELETE FROM music_lyrics_versions WHERE id = ? AND song_id = ?")
            .bind(version)
            .bind(song.to_db_string())
            .execute(self.writer())
            .await?
            .rows_affected();
        Ok(deleted > 0)
    }

    /// Gives the versions of songs no longer here to the song whose file is
    /// now where theirs was, in the same folder: a song taken away and back.
    /// Returns how many were taken up.
    pub async fn take_up_lyrics_versions(&self) -> Result<u64> {
        let taken = sqlx::query(
            "UPDATE music_lyrics_versions
                SET song_id = (SELECT s.work_id FROM media_sources s JOIN library_roots r ON r.id = s.root_id
                                WHERE r.path = music_lyrics_versions.root_path
                                  AND s.relative_path = music_lyrics_versions.relative_path
                                ORDER BY s.work_id LIMIT 1)
              WHERE NOT EXISTS (SELECT 1 FROM works w WHERE w.id = music_lyrics_versions.song_id)
                AND EXISTS (SELECT 1 FROM media_sources s JOIN library_roots r ON r.id = s.root_id
                             WHERE r.path = music_lyrics_versions.root_path
                               AND s.relative_path = music_lyrics_versions.relative_path)",
        )
        .execute(self.writer())
        .await?
        .rows_affected();
        Ok(taken)
    }
}

#[cfg(test)]
mod tests {
    use melyxar_core::id::WorkId;

    use crate::Database;
    use crate::music_browse::SongOrder;
    use crate::music_lyrics_timing::LyricsTiming;
    use crate::music_testing::collection;

    async fn songs(database: &Database, library: melyxar_core::id::LibraryId) -> Vec<WorkId> {
        database.music_songs(library, SongOrder::Title, false, 0, 2).await.expect("read").items.iter().map(|song| song.id).collect()
    }

    fn timing() -> LyricsTiming {
        LyricsTiming { conclusion: "aligned".into(), shift_ms: 120, moves_ms: vec![120], lines_moved: 1, confidence: 0.5 }
    }

    #[tokio::test]
    async fn every_save_is_a_version_kept_and_the_last_is_in_service() {
        let (database, library, _) = collection().await;
        let song = songs(&database, library).await[0];
        let now = melyxar_core::time::now();
        assert_eq!(database.lyrics_by_hand(song).await.expect("read"), None);

        database.keep_lyrics_by_hand(song, "[00:01.00]First", now).await.expect("kept");
        database.keep_lyrics_by_hand(song, "[00:02.50]Second", now).await.expect("kept");
        database.keep_lyrics_by_hand(song, "[00:02.50]Second", now).await.expect("kept, the same");
        assert_eq!(database.lyrics_by_hand(song).await.expect("read").as_deref(), Some("[00:02.50]Second"));
        let versions = database.lyrics_versions(song).await.expect("read");
        let said: Vec<(&str, bool)> = versions.iter().map(|one| (one.words.as_str(), one.in_use)).collect();
        assert_eq!(said, vec![("[00:02.50]Second", true), ("[00:01.00]First", false)], "the latest first, none twice");
    }

    #[tokio::test]
    async fn a_version_set_aside_stays_and_is_put_back_in_service() {
        let (database, library, _) = collection().await;
        let song = songs(&database, library).await[0];
        let now = melyxar_core::time::now();
        database.keep_lyrics_by_hand(song, "[00:01.00]First", now).await.expect("kept");
        database.keep_lyrics_by_hand(song, "[00:02.00]Second", now).await.expect("kept");

        database.set_lyrics_by_hand_aside(song).await.expect("set aside");
        assert_eq!(database.lyrics_by_hand(song).await.expect("read"), None);
        let versions = database.lyrics_versions(song).await.expect("read");
        assert_eq!(versions.len(), 2, "nothing went");

        let first = versions[1].id;
        assert!(database.use_lyrics_version(song, first).await.expect("used"));
        assert_eq!(database.lyrics_by_hand(song).await.expect("read").as_deref(), Some("[00:01.00]First"));
        assert!(!database.use_lyrics_version(songs(&database, library).await[1], first).await.expect("refused"), "another song's");
    }

    #[tokio::test]
    async fn a_version_goes_only_when_it_is_deleted() {
        let (database, library, _) = collection().await;
        let song = songs(&database, library).await[0];
        database.keep_lyrics_by_hand(song, "[00:01.00]Only", melyxar_core::time::now()).await.expect("kept");
        let version = database.lyrics_versions(song).await.expect("read")[0].id;

        assert!(database.delete_lyrics_version(song, version).await.expect("deleted"));
        assert_eq!(database.lyrics_versions(song).await.expect("read"), Vec::new());
        assert_eq!(database.lyrics_by_hand(song).await.expect("read"), None);
        assert!(!database.delete_lyrics_version(song, version).await.expect("nothing left"));
    }

    #[tokio::test]
    async fn the_versions_outlive_their_song_and_come_back_with_its_file() {
        let (database, library, _) = collection().await;
        let song = songs(&database, library).await[0];
        database.keep_lyrics_by_hand(song, "[00:01.00]Kept", melyxar_core::time::now()).await.expect("kept");
        let on_disk = database.song_on_disk(song).await.expect("read").expect("a file");

        // The song goes; the same file comes back as another song.
        database.delete_works(&[song], false).await.expect("taken away");
        assert_eq!(database.take_up_lyrics_versions().await.expect("looked"), 0, "nothing there to take them up");
        let other = songs(&database, library).await[0];
        sqlx::query("UPDATE media_sources SET relative_path = ? WHERE work_id = ?")
            .bind(on_disk.relative_path.to_string_lossy().into_owned())
            .bind(other.to_db_string())
            .execute(database.writer())
            .await
            .expect("moved there");

        assert_eq!(database.take_up_lyrics_versions().await.expect("taken up"), 1);
        assert_eq!(database.lyrics_by_hand(other).await.expect("read").as_deref(), Some("[00:01.00]Kept"), "in service as it was");
    }

    #[tokio::test]
    async fn the_versions_of_a_song_follow_it_into_the_song_it_is_merged_into() {
        let (database, library, _) = collection().await;
        let [gone, here] = songs(&database, library).await[..] else { panic!("two songs") };
        let now = melyxar_core::time::now();
        let found = crate::music_lyrics::LookedUpLyrics { plain: Some("Chosen".into()), ..Default::default() };
        // In this order, since each forgets what was set after the one before.
        database.keep_lyrics_by_hand(here, "[00:09.00]Its own", now).await.expect("kept");
        database.keep_lyrics_by_hand(gone, "[00:01.00]Written", now).await.expect("kept");
        database.keep_looked_up_lyrics(gone, &found, now, 4).await.expect("kept");
        database.keep_lyrics_timing(gone, &timing(), now).await.expect("kept");
        database.set_lyrics_offset(gone, 800, now).await.expect("set");

        database.merge_work_into(gone, here).await.expect("merged");
        assert_eq!(database.looked_up_lyrics(here).await.expect("read"), Some(found));
        assert_eq!(database.lyrics_timing(here).await.expect("read"), Some(timing()));
        assert_eq!(database.lyrics_offset(here).await.expect("read"), 800);
        assert_eq!(database.lyrics_by_hand(here).await.expect("read").as_deref(), Some("[00:09.00]Its own"), "its own stays in service");
        let words: Vec<String> = database.lyrics_versions(here).await.expect("read").into_iter().map(|one| one.words).collect();
        assert_eq!(words, vec!["[00:01.00]Written".to_string(), "[00:09.00]Its own".to_string()], "both kept");
    }

    #[tokio::test]
    async fn a_version_in_service_comes_along_into_a_song_without_any() {
        let (database, library, _) = collection().await;
        let [gone, here] = songs(&database, library).await[..] else { panic!("two songs") };
        database.keep_lyrics_by_hand(gone, "[00:01.00]Written", melyxar_core::time::now()).await.expect("kept");
        database.merge_work_into(gone, here).await.expect("merged");
        assert_eq!(database.lyrics_by_hand(here).await.expect("read").as_deref(), Some("[00:01.00]Written"));
    }

    #[tokio::test]
    async fn a_version_put_in_service_forgets_how_the_lines_were_lined_up_and_moved() {
        let (database, library, _) = collection().await;
        let song = songs(&database, library).await[0];
        let now = melyxar_core::time::now();
        database.keep_lyrics_by_hand(song, "[00:01.00]First", now).await.expect("kept");
        let first = database.lyrics_versions(song).await.expect("read")[0].id;
        database.keep_lyrics_timing(song, &timing(), now).await.expect("kept");
        database.set_lyrics_offset(song, 800, now).await.expect("set");
        database.keep_lyrics_by_hand(song, "Plain words", now).await.expect("kept");
        assert_eq!(database.lyrics_timing(song).await.expect("read"), None);
        assert_eq!(database.lyrics_offset(song).await.expect("read"), 0);

        database.keep_lyrics_timing(song, &timing(), now).await.expect("kept");
        database.use_lyrics_version(song, first).await.expect("used");
        assert_eq!(database.lyrics_timing(song).await.expect("read"), None);
    }
}
