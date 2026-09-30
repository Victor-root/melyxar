//! What was chosen for music: by each account, and for each library of it.

use melyxar_core::id::{LibraryId, UserId};
use melyxar_core::music_preferences::{
    FilmOnScreen, MusicLibraryOptions, MusicPreferences, VolumeMode,
};
use sqlx::Row;

use crate::{Database, Result};

impl Database {
    /// What this account chose, or the defaults while it has chosen nothing.
    pub async fn music_preferences(&self, user: UserId) -> Result<MusicPreferences> {
        let row = sqlx::query(
            "SELECT film_on_screen, resume_queue, max_bitrate_kbps, volume_mode, crossfade_seconds,
                    tag_preview
               FROM music_preferences WHERE user_id = ?",
        )
        .bind(user.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        let Some(row) = row else {
            return Ok(MusicPreferences::default());
        };
        Ok(MusicPreferences {
            film_on_screen: FilmOnScreen::parse(&row.try_get::<String, _>("film_on_screen")?)
                .unwrap_or_default(),
            resume_queue: row.try_get("resume_queue")?,
            max_bitrate_kbps: row
                .try_get::<Option<i64>, _>("max_bitrate_kbps")?
                .and_then(|kbps| u32::try_from(kbps).ok()),
            volume_mode: VolumeMode::parse(&row.try_get::<String, _>("volume_mode")?)
                .unwrap_or_default(),
            crossfade_seconds: u32::try_from(row.try_get::<i64, _>("crossfade_seconds")?)
                .unwrap_or(0),
            tag_preview: row.try_get("tag_preview")?,
        })
    }

    pub async fn save_music_preferences(
        &self,
        user: UserId,
        chosen: &MusicPreferences,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO music_preferences
                (user_id, film_on_screen, resume_queue, max_bitrate_kbps, volume_mode,
                 crossfade_seconds, tag_preview)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (user_id) DO UPDATE SET
                film_on_screen = excluded.film_on_screen,
                resume_queue = excluded.resume_queue,
                max_bitrate_kbps = excluded.max_bitrate_kbps,
                volume_mode = excluded.volume_mode,
                crossfade_seconds = excluded.crossfade_seconds,
                tag_preview = excluded.tag_preview",
        )
        .bind(user.to_db_string())
        .bind(chosen.film_on_screen.as_str())
        .bind(chosen.resume_queue)
        .bind(chosen.max_bitrate_kbps.map(i64::from))
        .bind(chosen.volume_mode.as_str())
        .bind(i64::from(chosen.crossfade_seconds))
        .bind(chosen.tag_preview)
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// What a library of music does beyond the rest, the defaults while
    /// nothing was chosen.
    pub async fn music_library_options(&self, library: LibraryId) -> Result<MusicLibraryOptions> {
        let row = sqlx::query(
            "SELECT lyrics_online, tag_writing, covers_online
               FROM music_library_options WHERE library_id = ?",
        )
        .bind(library.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        Ok(match row {
            Some(row) => MusicLibraryOptions {
                lyrics_online: row.try_get::<i64, _>("lyrics_online")? != 0,
                tag_writing: row.try_get::<i64, _>("tag_writing")? != 0,
                covers_online: row.try_get::<i64, _>("covers_online")? != 0,
            },
            None => MusicLibraryOptions::default(),
        })
    }

    pub async fn set_music_library_options(
        &self,
        library: LibraryId,
        options: &MusicLibraryOptions,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO music_library_options (library_id, lyrics_online, tag_writing, covers_online)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (library_id) DO UPDATE SET
                lyrics_online = excluded.lyrics_online, tag_writing = excluded.tag_writing,
                covers_online = excluded.covers_online",
        )
        .bind(library.to_db_string())
        .bind(options.lyrics_online)
        .bind(options.tag_writing)
        .bind(options.covers_online)
        .execute(self.writer())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use melyxar_core::user::Permissions;

    use super::*;
    use crate::music_testing::collection;

    #[tokio::test]
    async fn an_account_that_chose_nothing_has_the_defaults_and_keeps_what_it_chooses() {
        let database = Database::open_in_memory().await.expect("opens");
        let user = database
            .create_user("listener", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account")
            .id;
        assert_eq!(
            database.music_preferences(user).await.expect("read"),
            MusicPreferences::default()
        );

        let chosen = MusicPreferences {
            film_on_screen: FilmOnScreen::Pause,
            resume_queue: false,
            max_bitrate_kbps: Some(128),
            volume_mode: VolumeMode::Album,
            crossfade_seconds: 6,
            tag_preview: false,
        };
        database
            .save_music_preferences(user, &chosen)
            .await
            .expect("saved");
        assert_eq!(
            database.music_preferences(user).await.expect("read"),
            chosen
        );

        let again = MusicPreferences {
            max_bitrate_kbps: None,
            ..chosen
        };
        database
            .save_music_preferences(user, &again)
            .await
            .expect("saved again");
        assert_eq!(database.music_preferences(user).await.expect("read"), again);
    }

    #[tokio::test]
    async fn a_library_asks_nothing_online_until_told_to() {
        let (database, library, _) = collection().await;
        assert!(
            !database
                .music_library_options(library)
                .await
                .expect("read")
                .lyrics_online
        );
        let on = MusicLibraryOptions {
            lyrics_online: true,
            tag_writing: true,
            covers_online: true,
        };
        database
            .set_music_library_options(library, &on)
            .await
            .expect("set");
        assert_eq!(
            database.music_library_options(library).await.expect("read"),
            on
        );
    }
}
