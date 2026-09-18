//! What a season and an episode are called before anything better is known.
//!
//! A row has to carry a title, and a scan that has only read a file name often
//! has nothing to put there but the number. What it writes then is a
//! placeholder, and this is the one place that knows what one looks like: the
//! scan writes them here, and a page asks here whether the name it was handed
//! actually says anything.
//!
//! English, like every other name written into the database. What a page shows
//! is worked out from the number in whichever language it is being read in, so
//! these are what is left if such a work is ever met outside a page.

use melyxar_core::work::WorkKind;

/// The season everything that belongs to no season is filed under.
pub const SEASON_OF_SPECIALS: i32 = 0;

/// What a season is called before anything better is known about it.
pub fn name_of_season(season: i32) -> String {
    if season == SEASON_OF_SPECIALS {
        return "Specials".to_string();
    }
    format!("Season {season}")
}

/// What an episode is called when its own name never said.
pub fn name_of_episode(first: i32, last: i32) -> String {
    if last > first {
        return format!("Episodes {first}-{last}");
    }
    format!("Episode {first}")
}

/// Whether a title says nothing the number does not already say.
///
/// A page draws the number in its own language and adds the name beside it,
/// and a name that is only the number written out in English would be the
/// number twice, once in the wrong language.
pub fn is_only_a_number(kind: WorkKind, ordinal: Option<i32>, title: &str) -> bool {
    let Some(ordinal) = ordinal else {
        return false;
    };
    match kind {
        WorkKind::Season => title == name_of_season(ordinal),
        WorkKind::Episode => title == name_of_episode(ordinal, ordinal),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_that_is_only_the_number_is_recognised_as_one() {
        assert!(is_only_a_number(WorkKind::Season, Some(2), "Season 2"));
        assert!(is_only_a_number(WorkKind::Season, Some(0), "Specials"));
        assert!(is_only_a_number(WorkKind::Episode, Some(7), "Episode 7"));
    }

    #[test]
    fn a_real_name_is_left_alone() {
        assert!(!is_only_a_number(
            WorkKind::Season,
            Some(1),
            "The Beginning"
        ));
        assert!(!is_only_a_number(
            WorkKind::Episode,
            Some(1),
            "The Long Night"
        ));
        // A file holding two episodes is named after both, which says more
        // than the one number a page would draw.
        assert!(!is_only_a_number(
            WorkKind::Episode,
            Some(1),
            "Episodes 1-2"
        ));
        // Nothing else is ever named after a number.
        assert!(!is_only_a_number(WorkKind::Movie, Some(1), "Episode 1"));
        assert!(!is_only_a_number(WorkKind::Episode, None, "Episode 1"));
    }
}
