//! Works: the thing a viewer picks, which is never a file on disk.
//!
//! Keeping the work apart from the file is the decision the whole schema rests
//! on. Replacing a file with a better copy must not erase the watch history,
//! and a work must exist even when no provider recognised it.

use crate::id::{LibraryId, WorkId};
use crate::time::{Millis, Timestamp};

/// How confident we are about what this work is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentificationState {
    /// Found on disk, not looked up yet.
    Pending,
    /// A provider matched it.
    Identified,
    /// Looked up and no match found. Stays visible in the library with a
    /// marker, because a set-aside file is a forgotten file.
    Unidentified,
    /// A person picked the match by hand. Never overwritten by a refresh.
    Manual,
}

impl IdentificationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Identified => "identified",
            Self::Unidentified => "unidentified",
            Self::Manual => "manual",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "identified" => Some(Self::Identified),
            "unidentified" => Some(Self::Unidentified),
            "manual" => Some(Self::Manual),
            _ => None,
        }
    }

    /// Whether a background refresh may still try to identify this work.
    pub fn may_be_looked_up_again(self) -> bool {
        matches!(self, Self::Pending | Self::Unidentified)
    }
}

/// What stopped the last look up from naming a work.
///
/// Written down next to the work rather than left in a log, because the person
/// who wants to know why a film is still nameless is looking at that film on a
/// screen, not at a terminal. A work that has never been looked up carries no
/// note at all, which is itself the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentificationNote {
    /// The provider answered, and knew nothing under this title. The name read
    /// off the file is the thing to look at.
    NoMatch,
    /// The provider could not be reached. The work is still waiting.
    ProviderUnreachable,
    /// The provider asked to be left alone for a while. Still waiting.
    ProviderBusy,
    /// The provider answered something that could not be read.
    ProviderUnreadable,
}

impl IdentificationNote {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NoMatch => "no_match",
            Self::ProviderUnreachable => "provider_unreachable",
            Self::ProviderBusy => "provider_busy",
            Self::ProviderUnreadable => "provider_unreadable",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "no_match" => Some(Self::NoMatch),
            "provider_unreachable" => Some(Self::ProviderUnreachable),
            "provider_busy" => Some(Self::ProviderBusy),
            "provider_unreadable" => Some(Self::ProviderUnreadable),
            _ => None,
        }
    }
}

/// What a work is, which decides how it is shown and what may parent it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkKind {
    Movie,
    Series,
    Season,
    Episode,
    Artist,
    Album,
    Song,
}

impl WorkKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Movie => "movie",
            Self::Series => "series",
            Self::Season => "season",
            Self::Episode => "episode",
            Self::Artist => "artist",
            Self::Album => "album",
            Self::Song => "song",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "movie" => Some(Self::Movie),
            "series" => Some(Self::Series),
            "season" => Some(Self::Season),
            "episode" => Some(Self::Episode),
            "artist" => Some(Self::Artist),
            "album" => Some(Self::Album),
            "song" => Some(Self::Song),
            _ => None,
        }
    }

    /// Whether a viewer plays this work itself, as opposed to opening it to
    /// find what is inside. Only these carry files of their own.
    pub fn is_playable(self) -> bool {
        matches!(self, Self::Movie | Self::Episode | Self::Song)
    }
}

/// A work as stored: the common trunk only. Domain specific metadata, such as
/// the fields proper to a film or to an album, live in their own tables so
/// that adding music later is an addition rather than a rewrite.
#[derive(Debug, Clone, PartialEq)]
pub struct Work {
    pub id: WorkId,
    pub library_id: LibraryId,
    /// Parent work, used by seasons and episodes. Absent for a film.
    pub parent_id: Option<WorkId>,
    pub kind: WorkKind,
    /// Title as displayed, in the preferred language when one is available.
    pub title: String,
    /// Title used for sorting: leading articles removed, accents folded.
    /// Precomputed on write so that ordering costs an index lookup.
    pub sort_title: String,
    pub release_year: Option<i32>,
    /// How long the work runs, as the provider gives it. Not the length of any
    /// one file: a page shows the film's runtime, and a file that is a few
    /// seconds short of it is still that film.
    pub runtime: Option<Millis>,
    /// What viewers elsewhere thought of it, on the provider's scale.
    pub community_rating: Option<f64>,
    /// Age rating as the country that issued it writes it.
    pub age_rating_label: Option<String>,
    pub identification: IdentificationState,
    /// What stopped the last look up, when one has run and failed.
    pub identification_note: Option<IdentificationNote>,
    /// Dominant colour of the poster, sent with every card so a grid shows
    /// colour before a single image has arrived.
    pub dominant_color: Option<String>,
    pub added_at: Timestamp,
    pub updated_at: Timestamp,
}

/// Where a viewer is in a work, per user.
///
/// The state is explicit rather than derived from a percentage, because a
/// manual mark has to win over the automatic threshold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackState {
    NotStarted,
    InProgress,
    Watched,
}

impl PlaybackState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::InProgress => "in_progress",
            Self::Watched => "watched",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "not_started" => Some(Self::NotStarted),
            "in_progress" => Some(Self::InProgress),
            "watched" => Some(Self::Watched),
            _ => None,
        }
    }
}

/// Share of a work that has to be played before it counts as watched.
pub const DEFAULT_WATCHED_THRESHOLD: f64 = 0.9;

/// Decides the state a reported position implies, without overriding a manual
/// mark.
///
/// A pure function so it can be tested exhaustively and reused by every client.
pub fn state_for_position(
    position: Millis,
    duration: Option<Millis>,
    threshold: f64,
    manually_marked: bool,
) -> PlaybackState {
    if manually_marked {
        return PlaybackState::Watched;
    }
    let Some(duration) = duration.filter(|value| value.get() > 0) else {
        // Without a known duration, any progress means the work was started.
        return if position.get() > 0 {
            PlaybackState::InProgress
        } else {
            PlaybackState::NotStarted
        };
    };
    if position.ratio_of(duration) >= threshold {
        PlaybackState::Watched
    } else if position.get() > 0 {
        PlaybackState::InProgress
    } else {
        PlaybackState::NotStarted
    }
}

/// Whether a newly reported position should replace the stored one.
///
/// Positions carry the instant they were measured. A late report arriving
/// after a fresher one must not make the resume point go backwards, which is
/// what happens when a client reconnects and flushes a stale local copy.
pub fn should_accept_position(stored_at: Option<Timestamp>, reported_at: Timestamp) -> bool {
    match stored_at {
        None => true,
        Some(stored) => reported_at > stored,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    const HOUR: Millis = Millis::new(3_600_000);

    /// These words are written into the database and sent to the interface.
    /// One of them changing silently would turn every stored row of that kind
    /// into something nothing recognises, so they are pinned here.
    #[test]
    fn every_kind_of_work_survives_a_round_trip_through_its_stored_form() {
        for (kind, written) in [
            (WorkKind::Movie, "movie"),
            (WorkKind::Series, "series"),
            (WorkKind::Season, "season"),
            (WorkKind::Episode, "episode"),
            (WorkKind::Artist, "artist"),
            (WorkKind::Album, "album"),
            (WorkKind::Song, "song"),
        ] {
            assert_eq!(kind.as_str(), written);
            assert_eq!(WorkKind::parse(written), Some(kind));
        }
        assert_eq!(WorkKind::parse("photograph"), None);
    }

    #[test]
    fn only_what_carries_a_file_of_its_own_is_played() {
        assert!(WorkKind::Movie.is_playable());
        assert!(WorkKind::Episode.is_playable());
        assert!(WorkKind::Song.is_playable());
        assert!(
            !WorkKind::Series.is_playable(),
            "a series is opened to find what is inside it"
        );
        assert!(!WorkKind::Season.is_playable());
        assert!(!WorkKind::Artist.is_playable());
        assert!(!WorkKind::Album.is_playable());
    }

    #[test]
    fn every_reason_a_look_up_failed_survives_a_round_trip_through_its_stored_form() {
        for (note, written) in [
            (IdentificationNote::NoMatch, "no_match"),
            (
                IdentificationNote::ProviderUnreachable,
                "provider_unreachable",
            ),
            (IdentificationNote::ProviderBusy, "provider_busy"),
            (
                IdentificationNote::ProviderUnreadable,
                "provider_unreadable",
            ),
        ] {
            assert_eq!(note.as_str(), written);
            assert_eq!(IdentificationNote::parse(written), Some(note));
        }
        assert_eq!(IdentificationNote::parse("no idea"), None);
    }

    #[test]
    fn every_playback_state_survives_a_round_trip_through_its_stored_form() {
        for (state, written) in [
            (PlaybackState::NotStarted, "not_started"),
            (PlaybackState::InProgress, "in_progress"),
            (PlaybackState::Watched, "watched"),
        ] {
            assert_eq!(state.as_str(), written);
            assert_eq!(PlaybackState::parse(written), Some(state));
        }
        assert_eq!(PlaybackState::parse("halfway"), None);
    }

    #[test]
    fn a_position_past_the_threshold_counts_as_watched() {
        let state = state_for_position(Millis::new(3_300_000), Some(HOUR), 0.9, false);
        assert_eq!(state, PlaybackState::Watched);
    }

    #[test]
    fn a_position_below_the_threshold_stays_in_progress() {
        let state = state_for_position(Millis::new(1_800_000), Some(HOUR), 0.9, false);
        assert_eq!(state, PlaybackState::InProgress);
    }

    #[test]
    fn a_zero_position_means_not_started() {
        let state = state_for_position(Millis::ZERO, Some(HOUR), 0.9, false);
        assert_eq!(state, PlaybackState::NotStarted);
    }

    #[test]
    fn a_manual_mark_wins_over_the_automatic_threshold() {
        let state = state_for_position(Millis::ZERO, Some(HOUR), 0.9, true);
        assert_eq!(state, PlaybackState::Watched);
    }

    #[test]
    fn an_unknown_duration_still_distinguishes_started_from_untouched() {
        assert_eq!(
            state_for_position(Millis::new(1000), None, 0.9, false),
            PlaybackState::InProgress
        );
        assert_eq!(
            state_for_position(Millis::ZERO, None, 0.9, false),
            PlaybackState::NotStarted
        );
    }

    #[test]
    fn a_zero_duration_is_treated_as_unknown_rather_than_dividing_by_zero() {
        assert_eq!(
            state_for_position(Millis::new(1000), Some(Millis::ZERO), 0.9, false),
            PlaybackState::InProgress
        );
    }

    #[test]
    fn a_first_position_is_always_accepted() {
        assert!(should_accept_position(
            None,
            datetime!(2026-01-01 12:00 UTC)
        ));
    }

    #[test]
    fn a_fresher_position_replaces_the_stored_one() {
        assert!(should_accept_position(
            Some(datetime!(2026-01-01 12:00 UTC)),
            datetime!(2026-01-01 12:00:10 UTC)
        ));
    }

    #[test]
    fn a_stale_position_never_makes_the_resume_point_go_backwards() {
        assert!(!should_accept_position(
            Some(datetime!(2026-01-01 12:00:10 UTC)),
            datetime!(2026-01-01 12:00 UTC)
        ));
        assert!(
            !should_accept_position(
                Some(datetime!(2026-01-01 12:00 UTC)),
                datetime!(2026-01-01 12:00 UTC)
            ),
            "a report from the same instant is not fresher, so the stored one stays"
        );
    }

    #[test]
    fn identification_states_round_trip_and_say_what_may_be_retried() {
        for state in [
            IdentificationState::Pending,
            IdentificationState::Identified,
            IdentificationState::Unidentified,
            IdentificationState::Manual,
        ] {
            assert_eq!(IdentificationState::parse(state.as_str()), Some(state));
        }
        assert!(IdentificationState::Pending.may_be_looked_up_again());
        assert!(IdentificationState::Unidentified.may_be_looked_up_again());
        assert!(!IdentificationState::Manual.may_be_looked_up_again());
        assert!(!IdentificationState::Identified.may_be_looked_up_again());
    }
}
