//! What the hot path is never allowed to count.
//!
//! Three things a library page asks for are answers about the whole library
//! rather than about the page: how many works a grid holds, which genres,
//! decades and letters it offers with how many under each, and how many films
//! are still waiting for a name. Every one of them is a walk through the
//! collection, and every one of them was being walked afresh on each visit.
//! Measured on a hundred thousand works: the menus alone took two thirds of a
//! second, against a whole page that is supposed to arrive in fifty
//! milliseconds.
//!
//! None of it changes between two visits unless the library itself changed,
//! and the library already says so: a counter on it is raised by every write
//! that alters what a listing would return, which is exactly what it was put
//! there for. So the counting happens once per change and the answer is kept.
//!
//! Counted once and waited for once. The visit that first finds the answer out
//! of date does the counting; anybody arriving while that count is running is
//! served what was there a moment ago rather than queueing behind a walk
//! through a hundred thousand works. So a change costs one slow page and not
//! one slow page per visitor, and nothing is ever served from before a change
//! once the count that followed it has landed.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use melyxar_core::id::LibraryId;

use crate::catalogue::Filters;
use crate::{AppState, Result};

/// What one library, or the whole catalogue, had to be counted for.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Counted {
    pub filters: Filters,
    /// How many works a grid of it holds. Seasons and episodes are not met on
    /// their own, so they are not in it.
    pub browsable: i64,
    /// How many works are still waiting for a name.
    pub awaiting_identification: i64,
}

/// The counted answers, each with the state of the library it was counted on.
#[derive(Default)]
pub struct Counts {
    kept: Mutex<HashMap<Option<LibraryId>, Kept>>,
}

struct Kept {
    /// The library's own counter, as it stood when this was counted.
    at_version: i64,
    counted: Arc<Counted>,
    /// Whether a fresh count is already on its way, so a busy page does not
    /// start one per visitor.
    counting: bool,
}

/// What a library, or the whole catalogue, has been counted for.
///
/// Counts on the way through whenever the answer kept is not the answer for
/// the library as it stands, unless somebody else is already counting it, in
/// which case what was there a moment ago is served instead of a queue.
pub async fn counted(state: &AppState, library_id: Option<LibraryId>) -> Result<Arc<Counted>> {
    let version = version_of(state, library_id).await?;

    match state.counts().look_up(library_id, version) {
        Found::Current(counted) => Ok(counted),
        // Somebody is already walking the library for this very answer. Two
        // walks would cost twice and end with the same number.
        Found::OutOfDate(counted) if !state.counts().take_it_on(library_id) => Ok(counted),
        Found::OutOfDate(_) | Found::Nothing => {
            let counted = count(state, library_id).await;
            state.counts().done_counting(library_id);
            let counted = Arc::new(counted?);
            state.counts().remember(library_id, version, counted.clone());
            Ok(counted)
        }
    }
}

enum Found {
    Current(Arc<Counted>),
    OutOfDate(Arc<Counted>),
    Nothing,
}

impl Counts {
    /// Not the same number rather than an older one: a library taken away
    /// lowers the sum the whole catalogue is read by, and a count held to be
    /// current because its number is the larger of the two would be held to be
    /// current for ever.
    fn look_up(&self, library_id: Option<LibraryId>, version: i64) -> Found {
        let kept = self.kept();
        match kept.get(&library_id) {
            None => Found::Nothing,
            Some(held) if held.at_version == version => Found::Current(held.counted.clone()),
            Some(held) => Found::OutOfDate(held.counted.clone()),
        }
    }

    /// Writes down a count, keeping whether one is running.
    fn remember(&self, library_id: Option<LibraryId>, version: i64, counted: Arc<Counted>) {
        let mut kept = self.kept();
        let counting = kept.get(&library_id).is_some_and(|held| held.counting);
        kept.insert(
            library_id,
            Kept {
                at_version: version,
                counted,
                counting,
            },
        );
    }

    /// Takes on the counting, unless somebody else already has.
    fn take_it_on(&self, library_id: Option<LibraryId>) -> bool {
        let mut kept = self.kept();
        match kept.get_mut(&library_id) {
            Some(held) if held.counting => false,
            Some(held) => {
                held.counting = true;
                true
            }
            // Nothing is kept at all, so there is nothing to serve anybody
            // arriving meanwhile: everyone waits for this one.
            None => false,
        }
    }

    fn done_counting(&self, library_id: Option<LibraryId>) {
        if let Some(held) = self.kept().get_mut(&library_id) {
            held.counting = false;
        }
    }

    /// Takes the counts, whatever a poisoned lock says.
    ///
    /// A poisoned lock means a thread panicked while holding it. These are
    /// counts, never a result: carrying on with them is better than taking the
    /// server down over a menu.
    fn kept(&self) -> std::sync::MutexGuard<'_, HashMap<Option<LibraryId>, Kept>> {
        self.kept
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// How far along the library is, in one small read.
///
/// The sum over every library when the question is about all of them: any
/// change to any of them moves the sum, whether a library grew or went away
/// altogether.
async fn version_of(state: &AppState, library_id: Option<LibraryId>) -> Result<i64> {
    Ok(match library_id {
        Some(id) => state.database().library_version(id).await?,
        None => state.database().every_library_version().await?,
    })
}

/// Walks the library and counts everything a page asks about it.
async fn count(state: &AppState, library_id: Option<LibraryId>) -> Result<Counted> {
    let database = state.database();
    Ok(Counted {
        filters: Filters {
            genres: database.genres_in_use(library_id).await?,
            decades: database.decades_in_use(library_id).await?,
            initials: database.initials_in_use(library_id).await?,
        },
        browsable: database.count_browsable(library_id).await?,
        awaiting_identification: database.count_awaiting_identification(library_id).await?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a_count(browsable: i64) -> Arc<Counted> {
        Arc::new(Counted {
            browsable,
            ..Counted::default()
        })
    }

    #[test]
    fn a_library_that_has_not_moved_is_never_counted_again() {
        let counts = Counts::default();
        assert!(matches!(counts.look_up(None, 4), Found::Nothing));

        counts.remember(None, 4, a_count(10));
        let Found::Current(held) = counts.look_up(None, 4) else {
            panic!("a library at the version it was counted at is current");
        };
        assert_eq!(held.browsable, 10);
    }

    #[test]
    fn a_library_that_has_moved_is_answered_from_before_and_counted_again() {
        let counts = Counts::default();
        counts.remember(None, 4, a_count(10));

        let Found::OutOfDate(held) = counts.look_up(None, 5) else {
            panic!("a library past the version it was counted at is out of date");
        };
        assert_eq!(held.browsable, 10, "the page is served what was there");
    }

    #[test]
    fn a_library_taken_away_is_counted_again_rather_than_held_to_be_current() {
        // The whole catalogue is read by the sum of every library's counter,
        // and removing one lowers it. Held to be current because its number is
        // the larger of the two, the old count would stand for ever.
        let counts = Counts::default();
        counts.remember(None, 9, a_count(90));
        assert!(matches!(counts.look_up(None, 4), Found::OutOfDate(_)));
    }

    #[test]
    fn only_one_recount_of_a_library_runs_at_a_time() {
        let counts = Counts::default();
        assert!(
            !counts.take_it_on(None),
            "nothing is kept, so nobody is counting behind a page"
        );

        counts.remember(None, 1, a_count(5));
        assert!(counts.take_it_on(None));
        assert!(
            !counts.take_it_on(None),
            "a second visitor never starts a second count"
        );

        counts.done_counting(None);
        assert!(counts.take_it_on(None));
    }

    #[test]
    fn a_count_that_lands_while_another_runs_leaves_the_counting_in_hand() {
        let counts = Counts::default();
        counts.remember(None, 1, a_count(5));
        assert!(counts.take_it_on(None));
        counts.remember(None, 2, a_count(6));
        assert!(
            !counts.take_it_on(None),
            "writing a count down must not lose the fact that one is running"
        );
    }

    #[test]
    fn two_libraries_are_counted_apart() {
        let counts = Counts::default();
        let one = Some(LibraryId::new());
        let other = Some(LibraryId::new());
        counts.remember(one, 1, a_count(11));
        counts.remember(other, 1, a_count(22));

        let Found::Current(held) = counts.look_up(one, 1) else {
            panic!("the first library is current");
        };
        assert_eq!(held.browsable, 11);
        assert!(matches!(counts.look_up(None, 1), Found::Nothing));
    }

}
