//! How far each account moved the sound and the subtitles of a file against
//! its picture.

use melyxar_core::id::{MediaSourceId, UserId};
use melyxar_core::time::Millis;
use sqlx::Row;

use crate::{Database, Result};

/// How far one account moved a file's sound and subtitles, later when
/// positive. Both zero when it never moved them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncOffsets {
    pub sound: Millis,
    pub words: Millis,
}

impl Database {
    /// How far this account moved the sound and subtitles of this file.
    pub async fn sync_offsets(
        &self,
        user_id: UserId,
        source_id: MediaSourceId,
    ) -> Result<SyncOffsets> {
        let row = sqlx::query(
            "SELECT sound_offset_ms, words_offset_ms FROM sync_offsets
             WHERE user_id = ? AND source_id = ?",
        )
        .bind(user_id.to_db_string())
        .bind(source_id.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        row.map(|row| {
            Ok(SyncOffsets {
                sound: Millis::new(row.try_get("sound_offset_ms")?),
                words: Millis::new(row.try_get("words_offset_ms")?),
            })
        })
        .transpose()
        .map(Option::unwrap_or_default)
    }

    /// Keeps how far this account moved the sound and subtitles of this file.
    /// Both back at zero leaves nothing behind.
    pub async fn record_sync_offsets(
        &self,
        user_id: UserId,
        source_id: MediaSourceId,
        offsets: SyncOffsets,
    ) -> Result<()> {
        if offsets == SyncOffsets::default() {
            sqlx::query("DELETE FROM sync_offsets WHERE user_id = ? AND source_id = ?")
                .bind(user_id.to_db_string())
                .bind(source_id.to_db_string())
                .execute(self.writer())
                .await?;
            return Ok(());
        }
        sqlx::query(
            "INSERT INTO sync_offsets (user_id, source_id, sound_offset_ms, words_offset_ms)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (user_id, source_id) DO UPDATE SET
                sound_offset_ms = excluded.sound_offset_ms,
                words_offset_ms = excluded.words_offset_ms",
        )
        .bind(user_id.to_db_string())
        .bind(source_id.to_db_string())
        .bind(offsets.sound.get())
        .bind(offsets.words.get())
        .execute(self.writer())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::time::now;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::WorkKind;
    use std::path::{Path, PathBuf};

    /// One film, and two accounts to watch it.
    async fn one_film_two_viewers() -> (Database, MediaSourceId, UserId, UserId) {
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
        let work = database
            .create_work(
                library.id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        let source = database
            .insert_source(
                work.id,
                library.roots[0].id,
                Path::new("Quiet.Harbour.2019.mkv"),
                1_000,
                now(),
            )
            .await
            .expect("source recorded");
        let first = database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");
        let second = database
            .create_user("karim", None, &Permissions::administrator())
            .await
            .expect("account created");
        (database, source, first.id, second.id)
    }

    #[tokio::test]
    async fn a_file_nobody_moved_is_in_step() {
        let (database, source, viewer, _) = one_film_two_viewers().await;
        assert_eq!(
            database.sync_offsets(viewer, source).await.expect("read"),
            SyncOffsets::default()
        );
    }

    #[tokio::test]
    async fn what_one_account_moved_is_kept_for_it_alone() {
        let (database, source, viewer, other) = one_film_two_viewers().await;
        let moved = SyncOffsets {
            sound: Millis::new(-300),
            words: Millis::new(1_200),
        };
        database.record_sync_offsets(viewer, source, moved).await.expect("kept");
        assert_eq!(database.sync_offsets(viewer, source).await.expect("read"), moved);
        assert_eq!(
            database.sync_offsets(other, source).await.expect("read"),
            SyncOffsets::default(),
            "another account watches the file as it is"
        );

        let moved_again = SyncOffsets {
            sound: Millis::new(400),
            ..moved
        };
        database
            .record_sync_offsets(viewer, source, moved_again)
            .await
            .expect("kept");
        assert_eq!(
            database.sync_offsets(viewer, source).await.expect("read"),
            moved_again
        );
    }

    #[tokio::test]
    async fn both_put_back_at_zero_leave_nothing_behind() {
        let (database, source, viewer, _) = one_film_two_viewers().await;
        database
            .record_sync_offsets(
                viewer,
                source,
                SyncOffsets {
                    sound: Millis::new(500),
                    words: Millis::ZERO,
                },
            )
            .await
            .expect("kept");
        database
            .record_sync_offsets(viewer, source, SyncOffsets::default())
            .await
            .expect("forgotten");

        let rows: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sync_offsets")
            .fetch_one(database.reader())
            .await
            .expect("counted");
        assert_eq!(rows, 0);
        assert_eq!(
            database.sync_offsets(viewer, source).await.expect("read"),
            SyncOffsets::default()
        );
    }
}
