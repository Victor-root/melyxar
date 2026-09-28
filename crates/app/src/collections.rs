//! The server's collections, as each account sees them, and who may change
//! them.
//!
//! Everybody sees every collection, with only the works of the libraries
//! open to them. Making, renaming, deleting and filling one is kept to the
//! accounts given the right, an administrator always holding it; a saga the
//! provider made is never changed by hand.

use melyxar_core::id::{CollectionId, WorkId};
use melyxar_core::user::User;
pub use melyxar_database::collections::{CollectionHeld, CollectionSummary};

use crate::{AppError, AppState, Result};

/// The longest name a collection or a playlist may carry, in letters.
pub const LONGEST_NAME: usize = 80;

/// Every collection this account can see something of, by name.
pub async fn every_one(state: &AppState, who: &User) -> Result<Vec<CollectionSummary>> {
    let within = crate::reach::within(who);
    Ok(state
        .database()
        .collections(who.id, within.as_deref(), who.permissions.may_manage_collections)
        .await?)
}

/// One collection, with the works of it this account can reach.
pub async fn one(state: &AppState, who: &User, id: CollectionId) -> Result<Option<CollectionHeld>> {
    let within = crate::reach::within(who);
    Ok(state.database().collection(who.id, id, within.as_deref()).await?)
}

/// The collections made by hand a work is in, for the window that puts it in
/// or takes it out of them.
pub async fn holding(state: &AppState, who: &User, work_id: WorkId) -> Result<Vec<CollectionId>> {
    may_manage(who)?;
    Ok(state.database().hand_made_collections_of(work_id).await?)
}

/// Makes a collection, empty, with the works given in it.
pub async fn create(
    state: &AppState,
    who: &User,
    name: &str,
    works: &[WorkId],
) -> Result<CollectionId> {
    may_manage(who)?;
    let name = named(name)?;
    let database = state.database();
    let id = database
        .create_collection(name, &melyxar_library::naming::sort_title(name))
        .await?;
    database.add_to_collection(id, works).await?;
    tracing::info!(account = %who.name, collection = name, "a collection was made");
    Ok(id)
}

/// Calls a collection made by hand something else.
pub async fn rename(state: &AppState, who: &User, id: CollectionId, name: &str) -> Result<()> {
    may_manage(who)?;
    let name = named(name)?;
    let renamed = state
        .database()
        .rename_collection(id, name, &melyxar_library::naming::sort_title(name))
        .await?;
    found(renamed)
}

/// Deletes a collection made by hand, and nothing of the works in it.
pub async fn delete(state: &AppState, who: &User, id: CollectionId) -> Result<()> {
    may_manage(who)?;
    found(state.database().delete_collection(id).await?)?;
    tracing::info!(account = %who.name, "a collection was deleted");
    Ok(())
}

/// Puts works in a collection made by hand, or takes them out.
pub async fn put(
    state: &AppState,
    who: &User,
    id: CollectionId,
    works: &[WorkId],
    in_it: bool,
) -> Result<()> {
    may_manage(who)?;
    let database = state.database();
    let done = match in_it {
        true => database.add_to_collection(id, works).await?,
        false => database.remove_from_collection(id, works).await?,
    };
    found(done)
}

fn may_manage(who: &User) -> Result<()> {
    match who.permissions.may_manage_collections {
        true => Ok(()),
        false => Err(AppError::Domain(melyxar_core::Error::forbidden(
            "this account may not manage collections",
        ))),
    }
}

/// A collection that is not there, or that the provider made and nobody
/// changes by hand.
fn found(done: bool) -> Result<()> {
    match done {
        true => Ok(()),
        false => Err(AppError::Domain(melyxar_core::Error::not_found("collection"))),
    }
}

/// The name typed for a collection or a playlist, trimmed, or why it cannot
/// be one.
pub(crate) fn named(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > LONGEST_NAME {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "a list needs a name of a reasonable length",
        )));
    }
    Ok(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::user::Permissions;

    fn somebody(may_manage_collections: bool) -> User {
        let mut who = crate::an_ordinary_account(melyxar_core::id::UserId::new());
        who.permissions = Permissions {
            may_manage_collections,
            ..Permissions::viewer()
        };
        who
    }

    #[test]
    fn a_name_is_trimmed_and_kept_to_a_reasonable_length() {
        assert_eq!(named("  Noël  ").expect("kept"), "Noël");
        assert!(named("   ").is_err());
        assert!(named(&"a".repeat(LONGEST_NAME + 1)).is_err());
    }

    #[test]
    fn only_an_account_given_the_right_manages_collections() {
        assert!(may_manage(&somebody(true)).is_ok());
        assert!(may_manage(&somebody(false)).is_err());
    }
}
