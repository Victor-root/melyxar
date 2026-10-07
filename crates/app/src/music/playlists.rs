//! Each account's playlists of songs: made, filled, put in order and emptied
//! by their owner alone, with songs of the libraries it may read.

use melyxar_core::id::{PlaylistId, WorkId};
use melyxar_core::user::User;
use melyxar_core::work::WorkKind;
pub use melyxar_database::music_playlists::MusicPlaylistSummary;

use super::browse::SongRow;
use crate::collections::{named, not_too_long, room_for_another};
use crate::{AppError, AppState, Result};

/// Every playlist of songs of this account, by name.
pub async fn every_one(state: &AppState, who: &User) -> Result<Vec<MusicPlaylistSummary>> {
    let within = crate::reach::within(who);
    Ok(state
        .database()
        .music_playlists(who.id, within.as_deref())
        .await?)
}

/// One playlist of this account, with the songs of it it can reach.
pub async fn one(
    state: &AppState,
    who: &User,
    id: PlaylistId,
) -> Result<Option<(String, Vec<SongRow>)>> {
    let within = crate::reach::within(who);
    Ok(state
        .database()
        .music_playlist(who.id, id, within.as_deref())
        .await?)
}

/// Makes a playlist, with the songs given in it.
pub async fn create(
    state: &AppState,
    who: &User,
    name: &str,
    songs: &[WorkId],
) -> Result<PlaylistId> {
    let name = named(name)?;
    not_too_long(songs.len())?;
    let database = state.database();
    room_for_another(database.music_playlists(who.id, None).await?.len())?;
    readable_songs(state, who, songs).await?;
    let id = database.create_music_playlist(who.id, name).await?;
    database.add_to_music_playlist(who.id, id, songs).await?;
    Ok(id)
}

pub async fn rename(state: &AppState, who: &User, id: PlaylistId, name: &str) -> Result<()> {
    let name = named(name)?;
    found(
        state
            .database()
            .rename_music_playlist(who.id, id, name)
            .await?,
    )
}

/// Deletes a playlist, and nothing of its songs.
pub async fn delete(state: &AppState, who: &User, id: PlaylistId) -> Result<()> {
    found(state.database().delete_music_playlist(who.id, id).await?)
}

/// Puts songs at the end of a playlist.
pub async fn add(state: &AppState, who: &User, id: PlaylistId, songs: &[WorkId]) -> Result<()> {
    let held = state
        .database()
        .music_playlists(who.id, None)
        .await?
        .into_iter()
        .find(|playlist| playlist.id == id)
        .map_or(0, |playlist| playlist.songs);
    not_too_long(usize::try_from(held).unwrap_or(usize::MAX).saturating_add(songs.len()))?;
    readable_songs(state, who, songs).await?;
    found(
        state
            .database()
            .add_to_music_playlist(who.id, id, songs)
            .await?,
    )
}

/// The songs of a playlist replaced by these, in this order.
pub async fn set(state: &AppState, who: &User, id: PlaylistId, songs: &[WorkId]) -> Result<()> {
    not_too_long(songs.len())?;
    readable_songs(state, who, songs).await?;
    found(
        state
            .database()
            .set_music_playlist_songs(who.id, id, songs)
            .await?,
    )
}

/// Refuses anything but a song this account can reach.
async fn readable_songs(state: &AppState, who: &User, songs: &[WorkId]) -> Result<()> {
    for song in songs {
        let work = state
            .database()
            .work(*song)
            .await?
            .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("song")))?;
        crate::reach::may_read(who, work.library_id)?;
        if work.kind != WorkKind::Song {
            return Err(AppError::Domain(melyxar_core::Error::invalid_input(
                "only a song goes in a playlist of songs",
            )));
        }
    }
    Ok(())
}

/// A playlist that is not there, or not this account's.
fn found(done: bool) -> Result<()> {
    match done {
        true => Ok(()),
        false => Err(AppError::Domain(melyxar_core::Error::not_found("playlist"))),
    }
}
