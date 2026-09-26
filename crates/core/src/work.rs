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
    /// In no catalogue and waiting for none: what somebody filmed or
    /// photographed themselves. Named by its file, and that is all it needs.
    Own,
}

impl IdentificationState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Identified => "identified",
            Self::Unidentified => "unidentified",
            Self::Manual => "manual",
            Self::Own => "own",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "identified" => Some(Self::Identified),
            "unidentified" => Some(Self::Unidentified),
            "manual" => Some(Self::Manual),
            "own" => Some(Self::Own),
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
    /// A folder of a library of home photos and videos, holding what was put
    /// in it on the disk.
    Folder,
    /// A video somebody filmed themselves.
    Video,
    /// A photo, looked at rather than played.
    Photo,
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
            Self::Folder => "folder",
            Self::Video => "video",
            Self::Photo => "photo",
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
            "folder" => Some(Self::Folder),
            "video" => Some(Self::Video),
            "photo" => Some(Self::Photo),
            _ => None,
        }
    }

    /// Whether a viewer plays this work itself, as opposed to opening it to
    /// find what is inside. Only these and a photo carry files of their own,
    /// and a photo is looked at rather than played.
    pub fn is_playable(self) -> bool {
        matches!(self, Self::Movie | Self::Episode | Self::Song | Self::Video)
    }

    /// Whether this is something somebody filmed or photographed themselves,
    /// which is never put before everybody the way a film is.
    pub fn is_home_media(self) -> bool {
        matches!(self, Self::Folder | Self::Video | Self::Photo)
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
    /// Where this sits among its parent's children: the season number, the
    /// episode number, the track number. Absent for anything met on its own.
    pub ordinal: Option<i32>,
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

/// When a work left partway counts as started, as watched, or as too short
/// to come back to. Chosen by each viewer, in Jellyfin's terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResumeRules {
    /// Below this share of the work, in percent, it was only glanced at: it
    /// counts as not started, and starts again from the beginning.
    pub min_percent: i64,
    /// From this share on, in percent, it counts as watched.
    pub max_percent: i64,
    /// A work shorter than this, in seconds, is never offered to carry on:
    /// past the smallest share it counts as watched.
    pub min_seconds: i64,
}

impl Default for ResumeRules {
    fn default() -> Self {
        Self {
            min_percent: 5,
            max_percent: 90,
            min_seconds: 120,
        }
    }
}

/// The widest each rule may be set to. The smallest share stops at half and
/// the largest starts there, so the two can never cross; an hour is longer
/// than any clip anybody means by short.
pub const MOST_MIN_PERCENT: i64 = 50;
pub const LEAST_MAX_PERCENT: i64 = 50;
pub const MOST_MIN_SECONDS: i64 = 3_600;

impl ResumeRules {
    /// Brought inside the range each rule is kept in.
    pub fn normalised(self) -> Self {
        Self {
            min_percent: self.min_percent.clamp(0, MOST_MIN_PERCENT),
            max_percent: self.max_percent.clamp(LEAST_MAX_PERCENT, 100),
            min_seconds: self.min_seconds.clamp(0, MOST_MIN_SECONDS),
        }
    }
}

/// The state a reported position leaves a work in, and the position kept.
///
/// A manual mark always wins. Without a known length any progress means the
/// work was started. Otherwise, in this order: from the largest share on, or
/// at the end, it is watched; below the smallest share it was only glanced
/// at, and starts again from the beginning; a work too short to come back to
/// is watched once past that smallest share; anything else is in progress.
///
/// A pure function so it can be tested exhaustively and reused by every client.
pub fn progress_after(
    position: Millis,
    duration: Option<Millis>,
    rules: ResumeRules,
    manually_marked: bool,
) -> (PlaybackState, Millis) {
    if manually_marked {
        return (PlaybackState::Watched, position);
    }
    let Some(duration) = duration.filter(|value| value.get() > 0) else {
        let state = match position.get() > 0 {
            true => PlaybackState::InProgress,
            false => PlaybackState::NotStarted,
        };
        return (state, position);
    };
    let percent = position.ratio_of(duration) * 100.0;
    if position >= duration || percent >= rules.max_percent as f64 {
        (PlaybackState::Watched, position)
    } else if position.get() <= 0 || percent < rules.min_percent as f64 {
        (PlaybackState::NotStarted, Millis::ZERO)
    } else if duration.get() < rules.min_seconds * 1_000 {
        (PlaybackState::Watched, position)
    } else {
        (PlaybackState::InProgress, position)
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

/// How many episodes one season of a series holds, as its provider counts them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SeasonLength {
    pub season: i32,
    pub episodes: i32,
}

/// Where an episode numbered across its whole series sits: which season, and
/// which episode of that season.
///
/// The seasons are counted off in order, the specials left out, since nobody
/// numbering across a series counts them. A number past everything counted
/// continues the last season: a series still airing is described a few
/// episodes behind what is already on the disk. Nothing counted at all is no
/// answer, and the number is then left as it was written.
pub fn place_across(number: i32, lengths: &[SeasonLength]) -> Option<(i32, i32)> {
    let mut seasons: Vec<SeasonLength> = lengths
        .iter()
        .copied()
        .filter(|length| length.season > 0 && length.episodes > 0)
        .collect();
    seasons.sort_by_key(|length| length.season);
    let (last, before) = seasons.split_last()?;

    let mut left = number;
    for length in before {
        if left <= length.episodes {
            return Some((length.season, left));
        }
        left -= length.episodes;
    }
    Some((last.season, left))
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    fn lengths(counted: &[(i32, i32)]) -> Vec<SeasonLength> {
        counted
            .iter()
            .map(|&(season, episodes)| SeasonLength { season, episodes })
            .collect()
    }

    #[test]
    fn a_number_across_the_series_lands_in_its_season() {
        let counted = lengths(&[(1, 28), (2, 12), (3, 10)]);
        assert_eq!(place_across(1, &counted), Some((1, 1)));
        assert_eq!(place_across(28, &counted), Some((1, 28)));
        assert_eq!(place_across(29, &counted), Some((2, 1)));
        assert_eq!(place_across(40, &counted), Some((2, 12)));
        assert_eq!(place_across(41, &counted), Some((3, 1)));
    }

    #[test]
    fn a_number_past_everything_counted_continues_the_last_season() {
        let counted = lengths(&[(1, 12), (2, 12)]);
        assert_eq!(place_across(26, &counted), Some((2, 14)));
    }

    #[test]
    fn the_specials_and_the_order_they_came_in_change_nothing() {
        let counted = lengths(&[(2, 12), (0, 5), (1, 12)]);
        assert_eq!(place_across(13, &counted), Some((2, 1)));
    }

    #[test]
    fn nothing_counted_is_no_answer() {
        assert_eq!(place_across(29, &[]), None);
        assert_eq!(place_across(29, &lengths(&[(0, 4)])), None);
    }

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
            (WorkKind::Folder, "folder"),
            (WorkKind::Video, "video"),
            (WorkKind::Photo, "photo"),
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
        assert!(WorkKind::Video.is_playable());
        assert!(!WorkKind::Folder.is_playable());
        assert!(
            !WorkKind::Photo.is_playable(),
            "a photo carries a file of its own, and it is looked at, not played"
        );
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

    const RULES: ResumeRules = ResumeRules {
        min_percent: 5,
        max_percent: 90,
        min_seconds: 120,
    };

    fn after(position_ms: i64, duration: Option<Millis>) -> (PlaybackState, Millis) {
        progress_after(Millis::new(position_ms), duration, RULES, false)
    }

    #[test]
    fn a_position_past_the_largest_share_counts_as_watched() {
        assert_eq!(after(3_300_000, Some(HOUR)).0, PlaybackState::Watched);
        assert_eq!(after(3_600_000, Some(HOUR)).0, PlaybackState::Watched);
    }

    #[test]
    fn a_position_between_the_two_shares_is_in_progress_and_kept() {
        assert_eq!(
            after(1_800_000, Some(HOUR)),
            (PlaybackState::InProgress, Millis::new(1_800_000))
        );
    }

    #[test]
    fn a_position_below_the_smallest_share_starts_again_from_the_beginning() {
        // Five percent of an hour is three minutes.
        assert_eq!(after(170_000, Some(HOUR)), (PlaybackState::NotStarted, Millis::ZERO));
        assert_eq!(after(190_000, Some(HOUR)).0, PlaybackState::InProgress);
        assert_eq!(after(0, Some(HOUR)), (PlaybackState::NotStarted, Millis::ZERO));
    }

    #[test]
    fn a_work_too_short_to_come_back_to_is_watched_once_past_the_smallest_share() {
        let clip = Some(Millis::new(100_000));
        assert_eq!(after(30_000, clip).0, PlaybackState::Watched);
        assert_eq!(after(2_000, clip), (PlaybackState::NotStarted, Millis::ZERO));
        let longer = Some(Millis::new(121_000));
        assert_eq!(after(30_000, longer).0, PlaybackState::InProgress);
    }

    #[test]
    fn a_manual_mark_wins_over_every_rule() {
        let marked = progress_after(Millis::ZERO, Some(HOUR), RULES, true);
        assert_eq!(marked.0, PlaybackState::Watched);
    }

    #[test]
    fn an_unknown_duration_still_distinguishes_started_from_untouched() {
        assert_eq!(after(1000, None).0, PlaybackState::InProgress);
        assert_eq!(after(0, None).0, PlaybackState::NotStarted);
    }

    #[test]
    fn a_zero_duration_is_treated_as_unknown_rather_than_dividing_by_zero() {
        assert_eq!(after(1000, Some(Millis::ZERO)).0, PlaybackState::InProgress);
    }

    #[test]
    fn rules_are_kept_inside_their_ranges_and_never_cross() {
        let wild = ResumeRules {
            min_percent: 80,
            max_percent: 10,
            min_seconds: -5,
        }
        .normalised();
        assert_eq!(
            wild,
            ResumeRules {
                min_percent: MOST_MIN_PERCENT,
                max_percent: LEAST_MAX_PERCENT,
                min_seconds: 0,
            }
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
            IdentificationState::Own,
        ] {
            assert_eq!(IdentificationState::parse(state.as_str()), Some(state));
        }
        assert!(!IdentificationState::Own.may_be_looked_up_again());
        assert!(IdentificationState::Pending.may_be_looked_up_again());
        assert!(IdentificationState::Unidentified.may_be_looked_up_again());
        assert!(!IdentificationState::Manual.may_be_looked_up_again());
        assert!(!IdentificationState::Identified.may_be_looked_up_again());
    }
}
