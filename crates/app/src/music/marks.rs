//! What an account makes of its music: what it likes, and what it listened
//! to. Each account reads and writes its own, in the libraries it may read.

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::user::User;
pub use melyxar_database::music_marks::Listened;

use super::browse::{MusicFound, SongRow};
use crate::reach::{may_read, may_read_the_work};
use crate::{AppState, Result};

/// The most of what was listened to a row shows.
pub const LISTENED_IN_A_ROW: i64 = 24;

/// The most of what is liked a list of favourites shows of each kind.
pub const FAVOURITES_OF_EACH: i64 = 500;

/// One more listen of a song, once it has been heard for long enough to
/// count, which is the interface's to say.
pub async fn record_listen(state: &AppState, who: &User, song: WorkId) -> Result<()> {
    may_read_the_work(state, who, song).await?;
    Ok(state
        .database()
        .record_listen(who.id, song, melyxar_core::time::now())
        .await?)
}

/// Every song, album and artist this account likes, to light the hearts of
/// whatever screen shows one.
pub async fn favourite_ids(state: &AppState, who: &User) -> Result<Vec<WorkId>> {
    Ok(state.database().music_favourite_ids(who.id).await?)
}

pub async fn favourites(state: &AppState, who: &User, library: LibraryId) -> Result<MusicFound> {
    may_read(who, library)?;
    Ok(state
        .database()
        .music_favourites(who.id, library, FAVOURITES_OF_EACH)
        .await?)
}

pub async fn listened(
    state: &AppState,
    who: &User,
    library: LibraryId,
    order: Listened,
) -> Result<Vec<SongRow>> {
    may_read(who, library)?;
    Ok(state
        .database()
        .listened_songs(who.id, library, order, LISTENED_IN_A_ROW)
        .await?)
}
