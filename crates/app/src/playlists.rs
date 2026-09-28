//! Each account's playlists: made, filled, put in order and emptied by their
//! owner alone.
//!
//! Only what plays on its own goes in one, a film, an episode or a video: a
//! series in a list would be either every episode or one nobody could
//! foresee.

use melyxar_core::id::{PlaylistId, WorkId};
use melyxar_core::user::User;
use melyxar_core::work::WorkKind;
pub use melyxar_database::playlists::{PlaylistHeld, PlaylistSummary};

use crate::collections::named;
use crate::{AppError, AppState, Result};

/// Every playlist of this account, by name.
pub async fn every_one(state: &AppState, who: &User) -> Result<Vec<PlaylistSummary>> {
    let within = crate::reach::within(who);
    Ok(state.database().playlists(who.id, within.as_deref()).await?)
}

/// One playlist of this account, with the works of it it can reach.
pub async fn one(state: &AppState, who: &User, id: PlaylistId) -> Result<Option<PlaylistHeld>> {
    let within = crate::reach::within(who);
    Ok(state.database().playlist(who.id, id, within.as_deref()).await?)
}

/// The playlists of this account a work is in.
pub async fn holding(state: &AppState, who: &User, work_id: WorkId) -> Result<Vec<PlaylistId>> {
    Ok(state.database().playlists_holding(who.id, work_id).await?)
}

/// Makes a playlist, with the works given in it.
pub async fn create(state: &AppState, who: &User, name: &str, works: &[WorkId]) -> Result<PlaylistId> {
    let name = named(name)?;
    playable(state, who, works).await?;
    let database = state.database();
    let id = database.create_playlist(who.id, name).await?;
    database.add_to_playlist(who.id, id, works).await?;
    Ok(id)
}

/// Calls a playlist something else.
pub async fn rename(state: &AppState, who: &User, id: PlaylistId, name: &str) -> Result<()> {
    let name = named(name)?;
    found(state.database().rename_playlist(who.id, id, name).await?)
}

/// Deletes a playlist, and nothing of the works in it.
pub async fn delete(state: &AppState, who: &User, id: PlaylistId) -> Result<()> {
    found(state.database().delete_playlist(who.id, id).await?)
}

/// Puts works at the end of a playlist, or takes them out.
pub async fn put(state: &AppState, who: &User, id: PlaylistId, works: &[WorkId], in_it: bool) -> Result<()> {
    let database = state.database();
    let done = match in_it {
        true => {
            playable(state, who, works).await?;
            database.add_to_playlist(who.id, id, works).await?
        }
        false => database.remove_from_playlist(who.id, id, works).await?,
    };
    found(done)
}

/// Puts the works of a playlist in the order given.
pub async fn reorder(state: &AppState, who: &User, id: PlaylistId, order: &[WorkId]) -> Result<()> {
    found(state.database().reorder_playlist(who.id, id, order).await?)
}

/// Whether a work of this kind goes in a playlist.
fn plays_on_its_own(kind: WorkKind) -> bool {
    matches!(kind, WorkKind::Movie | WorkKind::Episode | WorkKind::Video)
}

/// Refuses a work that does not play on its own, or that this account cannot
/// reach.
async fn playable(state: &AppState, who: &User, works: &[WorkId]) -> Result<()> {
    for work_id in works {
        let work = state
            .database()
            .work(*work_id)
            .await?
            .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("work")))?;
        crate::reach::may_read(who, work.library_id)?;
        if !plays_on_its_own(work.kind) {
            return Err(AppError::Domain(melyxar_core::Error::invalid_input(
                "only a film, an episode or a video goes in a playlist",
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_what_plays_on_its_own_goes_in_a_playlist() {
        assert!(plays_on_its_own(WorkKind::Movie));
        assert!(plays_on_its_own(WorkKind::Episode));
        assert!(plays_on_its_own(WorkKind::Video));
        assert!(!plays_on_its_own(WorkKind::Series));
        assert!(!plays_on_its_own(WorkKind::Season));
        assert!(!plays_on_its_own(WorkKind::Photo));
    }
}
