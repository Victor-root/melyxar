//! What one viewer has decided, and the choices they can decide between.
//!
//! These are not decorations. The fold to stereo and the preferred languages
//! are read by the playback decision every time someone presses play: the fold
//! turns a copy into a rebuild, and a preferred language decides which
//! soundtrack starts. A setting that reaches that far has to be reachable from
//! a screen, or it is a rule nobody can see and nobody can change.

use melyxar_core::id::UserId;
use melyxar_core::user::Preferences;
use melyxar_database::users::AvailableLanguages;

use crate::{AppError, AppState, Result};

/// What one viewer has chosen.
pub async fn of(state: &AppState, user_id: UserId) -> Result<Preferences> {
    state
        .database()
        .user(user_id)
        .await?
        .map(|user| user.preferences)
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("account")))
}

/// Writes them back, answering what was actually kept.
///
/// A value out of range is brought back into range rather than refused: a
/// slider that goes a hair too far should not throw away everything else on
/// the page. What comes back is what a screen shows afterwards.
pub async fn save(
    state: &AppState,
    user_id: UserId,
    preferences: Preferences,
) -> Result<Preferences> {
    let database = state.database();
    if database.user(user_id).await?.is_none() {
        return Err(AppError::Domain(melyxar_core::Error::not_found("account")));
    }
    database.save_preferences(user_id, &preferences).await?;
    of(state, user_id).await
}

/// The languages a picker can actually offer.
pub async fn languages_available(state: &AppState) -> Result<AvailableLanguages> {
    Ok(state.database().languages_in_use().await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::user::{DownmixMethod, Permissions, MAX_DOWNMIX_GAIN};
    use melyxar_database::Database;

    async fn state_with_an_account() -> (tempfile::TempDir, AppState, UserId) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = melyxar_config::Config {
            directories: melyxar_config::Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
                ..Default::default()
            },
            ..melyxar_config::Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");

        let database = Database::open_in_memory().await.expect("database opens");
        let user = database
            .create_user("victor", None, &Permissions::administrator())
            .await
            .expect("account created");
        let state = AppState::new(config, database, None, None);
        (directory, state, user.id)
    }

    #[tokio::test]
    async fn a_choice_made_is_a_choice_read_back() {
        let (_directory, state, user_id) = state_with_an_account().await;

        let mut chosen = of(&state, user_id).await.expect("the defaults");
        chosen.downmix_method = DownmixMethod::NightDialogue;
        chosen.preferred_audio_language = Some("fre".into());
        chosen.preferred_subtitle_language = Some("eng".into());

        let kept = save(&state, user_id, chosen).await.expect("kept");
        assert_eq!(kept.downmix_method, DownmixMethod::NightDialogue);
        assert_eq!(kept.preferred_audio_language.as_deref(), Some("fre"));

        let read_again = of(&state, user_id).await.expect("read again");
        assert_eq!(read_again, kept, "what was kept is what is read back");
    }

    #[tokio::test]
    async fn a_value_out_of_range_is_brought_back_rather_than_refused() {
        // A slider that goes a hair too far should not throw away everything
        // else on the page.
        let (_directory, state, user_id) = state_with_an_account().await;

        let mut wild = of(&state, user_id).await.expect("the defaults");
        wild.downmix_gain = 99.0;
        wild.volume = 4.0;

        let kept = save(&state, user_id, wild)
            .await
            .expect("kept all the same");
        assert_eq!(kept.downmix_gain, MAX_DOWNMIX_GAIN);
        assert_eq!(kept.volume, 1.0);
    }

    #[tokio::test]
    async fn an_account_that_is_not_there_is_said_so_rather_than_written_to() {
        let (_directory, state, _user_id) = state_with_an_account().await;
        assert!(of(&state, UserId::new()).await.is_err());
        assert!(save(&state, UserId::new(), Preferences::default())
            .await
            .is_err());
    }

    #[tokio::test]
    async fn a_library_with_nothing_in_it_offers_no_language_at_all() {
        // Better than a list of five hundred: someone would pick one no film
        // in the house carries.
        let (_directory, state, _user_id) = state_with_an_account().await;
        let available = languages_available(&state).await.expect("read");
        assert!(available.audio.is_empty());
        assert!(available.subtitle.is_empty());
    }
}
