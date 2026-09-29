//! What each account chose for its music.

use melyxar_core::id::UserId;
use melyxar_core::music_preferences::{FilmOnScreen, MusicPreferences, VolumeMode};
use sqlx::Row;

use crate::{Database, Result};

impl Database {
    /// What this account chose, or the defaults while it has chosen nothing.
    pub async fn music_preferences(&self, user: UserId) -> Result<MusicPreferences> {
        let row = sqlx::query(
            "SELECT film_on_screen, resume_queue, max_bitrate_kbps, volume_mode
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
        })
    }

    pub async fn save_music_preferences(
        &self,
        user: UserId,
        chosen: &MusicPreferences,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO music_preferences
                (user_id, film_on_screen, resume_queue, max_bitrate_kbps, volume_mode)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (user_id) DO UPDATE SET
                film_on_screen = excluded.film_on_screen,
                resume_queue = excluded.resume_queue,
                max_bitrate_kbps = excluded.max_bitrate_kbps,
                volume_mode = excluded.volume_mode",
        )
        .bind(user.to_db_string())
        .bind(chosen.film_on_screen.as_str())
        .bind(chosen.resume_queue)
        .bind(chosen.max_bitrate_kbps.map(i64::from))
        .bind(chosen.volume_mode.as_str())
        .execute(self.writer())
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use melyxar_core::user::Permissions;

    use super::*;

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
}
