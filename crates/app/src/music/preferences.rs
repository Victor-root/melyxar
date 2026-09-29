//! What was chosen for music: by an account, read and changed by that
//! account alone, and for a library, changed by an administrator (which the
//! route asks for).

use melyxar_core::id::LibraryId;
use melyxar_core::music_preferences::{MusicLibraryOptions, MusicPreferences};
use melyxar_core::user::User;

use crate::reach::may_read;
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

pub async fn library_options(
    state: &AppState,
    who: &User,
    library: LibraryId,
) -> Result<MusicLibraryOptions> {
    may_read(who, library)?;
    Ok(state.database().music_library_options(library).await?)
}

pub async fn set_library_options(
    state: &AppState,
    who: &User,
    library: LibraryId,
    options: &MusicLibraryOptions,
) -> Result<MusicLibraryOptions> {
    may_read(who, library)?;
    state
        .database()
        .set_music_library_options(library, options)
        .await?;
    Ok(*options)
}
