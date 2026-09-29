//! What an account chose for its music, read and changed by that account
//! alone.

use melyxar_core::music_preferences::MusicPreferences;
use melyxar_core::user::User;

use crate::{AppState, Result};

pub async fn preferences(state: &AppState, who: &User) -> Result<MusicPreferences> {
    Ok(state.database().music_preferences(who.id).await?)
}

pub async fn set_preferences(
    state: &AppState,
    who: &User,
    chosen: &MusicPreferences,
) -> Result<MusicPreferences> {
    state
        .database()
        .save_music_preferences(who.id, chosen)
        .await?;
    Ok(*chosen)
}
