//! Time handling.
//!
//! Two rules hold everywhere in Melyxar: instants are UTC, and durations and
//! positions are expressed in whole milliseconds. Picking a single unit once
//! avoids the class of bug where a client and a server disagree on the scale.

use std::fmt;

use time::OffsetDateTime;

/// A duration or a playback position, in whole milliseconds.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct Millis(i64);

impl Millis {
    pub const ZERO: Self = Self(0);

    pub const fn new(value: i64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> i64 {
        self.0
    }

    pub fn from_seconds_f64(seconds: f64) -> Self {
        Self((seconds * 1000.0).round() as i64)
    }

    pub fn as_seconds_f64(self) -> f64 {
        self.0 as f64 / 1000.0
    }

    pub const fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    /// Share of `self` within `total`, clamped to the zero to one range.
    ///
    /// Returns zero when `total` is not positive, so callers never have to
    /// guard against a missing or absurd duration.
    pub fn ratio_of(self, total: Self) -> f64 {
        if total.0 <= 0 {
            return 0.0;
        }
        (self.0 as f64 / total.0 as f64).clamp(0.0, 1.0)
    }
}

impl fmt::Display for Millis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}ms", self.0)
    }
}

/// An instant, always in UTC.
pub type Timestamp = OffsetDateTime;

/// The current instant in UTC.
pub fn now() -> Timestamp {
    OffsetDateTime::now_utc()
}

/// An instant written the one way everything reads it: sortable, and in UTC.
///
/// The same form is what goes in the database and what goes out to a client,
/// so nobody has to know which of the two they are looking at.
pub fn to_text(value: Timestamp) -> String {
    value
        .to_offset(time::UtcOffset::UTC)
        .format(&time::format_description::well_known::Rfc3339)
        .expect("an instant always formats")
}

/// The year it is now.
///
/// Reading a file name needs to know how far ahead a year is still plausible.
/// The rules themselves take the year as an argument so a test never depends
/// on the clock; this is what the server passes them when it is not a test.
pub fn current_year() -> i32 {
    now().year()
}

/// The hour of the day it is now, in UTC, from nought to twenty three.
///
/// UTC because it is the only clock a server can read with certainty: the hour
/// a machine calls its own comes from a setting that cannot be asked for
/// safely from several threads at once, so a value that looked local would be
/// wrong on any server not set to this one's offset, and silently wrong half
/// the year on every other. Whatever shows an hour to somebody turns it into
/// theirs, which is the only place that conversion can be made honestly.
pub fn hour_of_day_utc() -> u32 {
    u32::from(now().hour())
}

/// When an hour of the day in UTC next comes round after `now`.
///
/// For saying when work that happens once a day will happen next. An hour
/// already gone today is tomorrow's, and the hour it is right now is
/// tomorrow's too: the run of this hour has either happened or is happening,
/// and announcing it as still to come would be a promise about the past.
///
/// An hour that is not an hour of the day is read as midnight. The
/// configuration refuses such a value long before this is reached; reading it
/// as something rather than refusing here keeps a screen from having no answer
/// at all to show.
pub fn next_occurrence_of_utc_hour_after(now: Timestamp, hour: u32) -> Timestamp {
    let at = time::Time::from_hms(hour.min(23) as u8, 0, 0).expect("an hour of the day is a time");
    let today = now.replace_time(at);
    if today > now {
        today
    } else {
        today.saturating_add(time::Duration::days(1))
    }
}

/// The same, from right now.
pub fn next_occurrence_of_utc_hour(hour: u32) -> Timestamp {
    next_occurrence_of_utc_hour_after(now(), hour)
}

/// The day it is now, in UTC.
///
/// For work that happens once a day: the day it last ran is compared with
/// this, rather than a delay being counted, so a run missed while the machine
/// was asleep is not a run silently skipped for ever.
pub fn today_utc() -> time::Date {
    now().date()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seconds_convert_to_whole_milliseconds() {
        assert_eq!(Millis::from_seconds_f64(1.5), Millis::new(1500));
        assert_eq!(Millis::from_seconds_f64(0.0005), Millis::new(1));
    }

    #[test]
    fn the_year_it_is_now_is_a_year_a_film_could_carry() {
        // What decides how far ahead a year read off a file name is still
        // plausible: a wrong answer here makes every recent film unreadable.
        let year = current_year();
        assert!(
            (2026..=2100).contains(&year),
            "the clock says {year}, which is not a year anyone releases films in"
        );
    }

    #[test]
    fn a_duration_says_its_unit_when_written_into_a_log() {
        assert_eq!(Millis::new(1500).to_string(), "1500ms");
        assert_eq!(Millis::ZERO.to_string(), "0ms");
    }

    #[test]
    fn milliseconds_convert_back_to_the_seconds_a_seek_is_asked_for_in() {
        // This is the number handed to the media tool to start somewhere in
        // the middle: a factor out of place sends a viewer to another scene.
        assert_eq!(Millis::new(1500).as_seconds_f64(), 1.5);
        assert_eq!(Millis::new(3_600_000).as_seconds_f64(), 3600.0);
        assert_eq!(Millis::ZERO.as_seconds_f64(), 0.0);
        assert_eq!(
            Millis::from_seconds_f64(42.125).as_seconds_f64(),
            42.125,
            "a position survives the round trip"
        );
    }

    #[test]
    fn a_ratio_never_escapes_the_zero_to_one_range() {
        assert_eq!(Millis::new(500).ratio_of(Millis::new(1000)), 0.5);
        assert_eq!(Millis::new(5000).ratio_of(Millis::new(1000)), 1.0);
        assert_eq!(Millis::new(-5).ratio_of(Millis::new(1000)), 0.0);
    }

    #[test]
    fn a_missing_duration_yields_a_zero_ratio_instead_of_a_division_by_zero() {
        assert_eq!(Millis::new(500).ratio_of(Millis::ZERO), 0.0);
    }

    #[test]
    fn an_hour_still_to_come_today_is_today_and_one_already_gone_is_tomorrow() {
        let at = |day, hour| {
            time::Date::from_calendar_date(2026, time::Month::September, day)
                .expect("a date")
                .with_hms(hour, 0, 0)
                .expect("a time")
                .assume_utc()
        };
        let midday = at(17, 12);

        assert_eq!(next_occurrence_of_utc_hour_after(midday, 20), at(17, 20));
        assert_eq!(
            next_occurrence_of_utc_hour_after(midday, 3),
            at(18, 3),
            "three in the morning is tomorrow once the afternoon has come"
        );
        assert_eq!(
            next_occurrence_of_utc_hour_after(midday, 12),
            at(18, 12),
            "the hour it is now has either happened or is happening, so the next \
             one is tomorrow's rather than a promise about the past"
        );
    }

    #[test]
    fn an_hour_that_is_not_an_hour_of_the_day_reads_as_the_last_one() {
        // The configuration refuses such a value long before this is reached.
        // Answering something rather than refusing here keeps a screen from
        // having nothing at all to show.
        let now = time::Date::from_calendar_date(2026, time::Month::September, 17)
            .expect("a date")
            .with_hms(1, 0, 0)
            .expect("a time")
            .assume_utc();
        assert_eq!(next_occurrence_of_utc_hour_after(now, 99).hour(), 23);
    }
}
