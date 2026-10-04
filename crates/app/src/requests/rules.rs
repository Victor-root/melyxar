//! What may be asked for, and when what was asked for has arrived, from
//! what the provider knows of a title and what the libraries hold of it.

use melyxar_database::requests::HeldTitle;

use super::{Catalogue, Refused};

/// Whether these seasons of a title may be asked for: a film or a whole
/// series not held, or seasons the provider knows and no library holds.
pub(super) fn may_be_asked(
    catalogue: Catalogue,
    asked: &[i32],
    known: &[i32],
    held: Option<&HeldTitle>,
) -> Result<(), Refused> {
    match catalogue {
        Catalogue::Films if !asked.is_empty() => Err(Refused::NoSuchSeason),
        _ if asked.iter().any(|season| !known.contains(season)) => Err(Refused::NoSuchSeason),
        _ => match held {
            Some(_) if asked.is_empty() => Err(Refused::AlreadyHere),
            Some(held) if asked.iter().any(|season| held.seasons.contains(season)) => {
                Err(Refused::AlreadyHere)
            }
            _ => Ok(()),
        },
    }
}

/// Whether what was asked for of a title now held is all here: the title
/// itself for a film or a whole series, every season asked for otherwise.
pub(super) fn has_arrived(asked: &[i32], held: &HeldTitle) -> bool {
    asked.iter().all(|season| held.seasons.contains(season))
}

#[cfg(test)]
mod tests {
    use melyxar_core::id::{LibraryId, WorkId};

    use super::*;

    fn holding(seasons: &[i32]) -> HeldTitle {
        HeldTitle {
            work_id: WorkId::new(),
            library_id: LibraryId::new(),
            seasons: seasons.to_vec(),
        }
    }

    #[test]
    fn a_film_is_asked_for_whole_and_only_when_absent() {
        assert_eq!(may_be_asked(Catalogue::Films, &[], &[], None), Ok(()));
        assert_eq!(
            may_be_asked(Catalogue::Films, &[], &[], Some(&holding(&[]))),
            Err(Refused::AlreadyHere)
        );
        assert_eq!(
            may_be_asked(Catalogue::Films, &[1], &[1], None),
            Err(Refused::NoSuchSeason)
        );
    }

    #[test]
    fn a_series_held_may_still_be_asked_for_the_seasons_it_lacks() {
        let held = holding(&[1, 2]);
        let known = [0, 1, 2, 3];
        assert_eq!(may_be_asked(Catalogue::Series, &[], &known, None), Ok(()));
        assert_eq!(
            may_be_asked(Catalogue::Series, &[], &known, Some(&held)),
            Err(Refused::AlreadyHere),
            "whole, it is here"
        );
        assert_eq!(may_be_asked(Catalogue::Series, &[3], &known, Some(&held)), Ok(()));
        assert_eq!(
            may_be_asked(Catalogue::Series, &[2, 3], &known, Some(&held)),
            Err(Refused::AlreadyHere)
        );
        assert_eq!(
            may_be_asked(Catalogue::Series, &[4], &known, None),
            Err(Refused::NoSuchSeason)
        );
    }

    #[test]
    fn a_request_arrives_once_all_it_asked_for_is_held() {
        assert!(has_arrived(&[], &holding(&[])));
        assert!(has_arrived(&[], &holding(&[1])), "a whole series arrives with its first season");
        assert!(!has_arrived(&[2, 3], &holding(&[1, 2])));
        assert!(has_arrived(&[2, 3], &holding(&[1, 2, 3])));
    }
}
