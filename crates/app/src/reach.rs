//! What a viewer may read, decided in one place.
//!
//! An account is either shown every library there is, which is the ordinary
//! one, or the ones it was granted and nothing besides. Every screen that
//! reads the library asks here rather than deciding for itself, because a
//! narrowing applied in nine places out of ten is a narrowing that does not
//! exist: the tenth is the one somebody finds.
//!
//! A library somebody was not granted is answered as a library that is not
//! there, never as one they may not have. Told apart, a restricted account
//! could ask about one identifier after another and learn what this server
//! holds, which is the thing being kept from them.

use std::collections::HashMap;
use std::sync::Arc;

use melyxar_core::error::ErrorCode;
use melyxar_core::id::LibraryId;
use melyxar_core::user::User;

use crate::catalogue::Filters;
use crate::counted::Counted;
use crate::{AppError, AppState, Result};

/// The libraries a request may be answered from.
///
/// Nothing at all for an account that sees every library, which is what lets
/// every query below skip the narrowing entirely for the account this server
/// mostly answers.
pub fn within(who: &User) -> Option<Vec<LibraryId>> {
    match who.permissions.sees_the_whole_server() {
        true => None,
        false => Some(who.permissions.allowed_libraries.clone()),
    }
}

/// Refuses a library this account was not granted.
pub fn may_read(who: &User, library: LibraryId) -> Result<()> {
    match who.permissions.may_access_library(library) {
        true => Ok(()),
        false => Err(AppError::Domain(melyxar_core::Error::new(
            ErrorCode::NotFound,
            "no library with that identifier",
        ))),
    }
}

/// Refuses a work whose library this account was not granted.
///
/// Costs nothing for an account that sees every library, which is the one this
/// server mostly answers: the question is settled before anything is read.
///
/// A work that is not there at all is not this function's to report. The
/// caller is about to find that out for itself and has its own answer for it.
pub async fn may_read_the_work(
    state: &AppState,
    who: &User,
    work_id: melyxar_core::id::WorkId,
) -> Result<()> {
    if who.permissions.sees_the_whole_server() {
        return Ok(());
    }
    let Some(work) = state.database().work(work_id).await? else {
        return Ok(());
    };
    may_read(who, work.library_id)
}

/// What one viewer has been counted for.
///
/// One library asked for is that library, once this account is allowed it.
/// Nothing asked for is the whole server for an ordinary account, and the
/// libraries they were granted for anybody else, added up.
///
/// Adding up rather than counting again: each library is counted once per
/// change and kept, so what happens here is the reading back of a handful of
/// small lists. Counting the granted ones together would mean a count of its
/// own per account, kept per account, and walked again whenever any library
/// anywhere changed.
pub async fn counted_for(
    state: &AppState,
    who: &User,
    library_id: Option<LibraryId>,
) -> Result<Arc<Counted>> {
    if let Some(id) = library_id {
        may_read(who, id)?;
        return crate::counted::counted(state, Some(id)).await;
    }

    let Some(granted) = within(who) else {
        return crate::counted::counted(state, None).await;
    };

    let mut parts = Vec::with_capacity(granted.len());
    for id in granted {
        parts.push(crate::counted::counted(state, Some(id)).await?);
    }
    Ok(Arc::new(added_up(&parts)))
}

/// Several libraries counted as one.
fn added_up(parts: &[Arc<Counted>]) -> Counted {
    let mut genres: HashMap<&str, i64> = HashMap::new();
    let mut decades: HashMap<i32, i64> = HashMap::new();
    let mut initials: HashMap<&str, i64> = HashMap::new();
    let mut browsable = 0;
    let mut awaiting_identification = 0;

    for part in parts {
        browsable += part.browsable;
        awaiting_identification += part.awaiting_identification;
        for (name, works) in &part.filters.genres {
            *genres.entry(name.as_str()).or_default() += works;
        }
        for (decade, works) in &part.filters.decades {
            *decades.entry(*decade).or_default() += works;
        }
        for (name, works) in &part.filters.initials {
            *initials.entry(name.as_str()).or_default() += works;
        }
    }

    // Each one back in the order the query it comes from returns: the most
    // held genre first, the latest decade first, and the bucket for titles
    // beginning with no letter before the letters.
    let mut genres: Vec<(String, i64)> = genres
        .into_iter()
        .map(|(name, works)| (name.to_string(), works))
        .collect();
    genres.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));

    let mut decades: Vec<(i32, i64)> = decades.into_iter().collect();
    decades.sort_by(|left, right| right.0.cmp(&left.0));

    let mut initials: Vec<(String, i64)> = initials
        .into_iter()
        .map(|(name, works)| (name.to_string(), works))
        .collect();
    initials.sort_by(|left, right| left.0.cmp(&right.0));

    Counted {
        filters: Filters {
            genres,
            decades,
            initials,
        },
        browsable,
        awaiting_identification,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::user::{Permissions, Preferences};

    fn somebody(permissions: Permissions) -> User {
        User {
            id: melyxar_core::id::UserId::new(),
            name: "victor".to_string(),
            avatar_path: None,
            permissions,
            preferences: Preferences::default(),
            created_at: melyxar_core::time::now(),
        }
    }

    #[test]
    fn an_ordinary_account_narrows_nothing() {
        assert_eq!(within(&somebody(Permissions::viewer())), None);
        assert!(may_read(&somebody(Permissions::viewer()), LibraryId::new()).is_ok());
    }

    #[test]
    fn a_granted_account_is_narrowed_to_what_it_was_granted() {
        let allowed = LibraryId::new();
        let who = somebody(Permissions {
            sees_every_library: false,
            allowed_libraries: vec![allowed],
            ..Permissions::viewer()
        });

        assert_eq!(within(&who), Some(vec![allowed]));
        assert!(may_read(&who, allowed).is_ok());

        // Answered as a library that is not there, so that asking about one
        // identifier after another says nothing about what this server holds.
        let refused = may_read(&who, LibraryId::new()).expect_err("not granted");
        assert!(
            matches!(refused, AppError::Domain(error) if error.code == ErrorCode::NotFound),
            "a library somebody may not see is a library that is not there"
        );
    }

    #[test]
    fn an_account_granted_nothing_reaches_nothing() {
        let who = somebody(Permissions {
            sees_every_library: false,
            allowed_libraries: Vec::new(),
            ..Permissions::viewer()
        });
        assert_eq!(within(&who), Some(Vec::new()));
        assert!(may_read(&who, LibraryId::new()).is_err());
    }

    #[test]
    fn several_libraries_counted_as_one_keep_every_ordering() {
        let first = Arc::new(Counted {
            filters: Filters {
                genres: vec![("Drama".into(), 10), ("Comedy".into(), 4)],
                decades: vec![(2020, 6), (1990, 8)],
                initials: vec![("#".into(), 1), ("a".into(), 5)],
            },
            browsable: 14,
            awaiting_identification: 2,
        });
        let second = Arc::new(Counted {
            filters: Filters {
                genres: vec![("Comedy".into(), 9), ("Horror".into(), 9)],
                decades: vec![(2020, 3)],
                initials: vec![("b".into(), 12), ("a".into(), 1)],
            },
            browsable: 12,
            awaiting_identification: 1,
        });

        let both = added_up(&[first, second]);
        assert_eq!(both.browsable, 26);
        assert_eq!(both.awaiting_identification, 3);
        // The most held first, and a tie broken by the name so the row does
        // not shuffle itself between two visits.
        assert_eq!(
            both.filters.genres,
            vec![
                ("Comedy".to_string(), 13),
                ("Drama".to_string(), 10),
                ("Horror".to_string(), 9),
            ]
        );
        assert_eq!(both.filters.decades, vec![(2020, 9), (1990, 8)]);
        assert_eq!(
            both.filters.initials,
            vec![
                ("#".to_string(), 1),
                ("a".to_string(), 6),
                ("b".to_string(), 12),
            ]
        );
    }
}
