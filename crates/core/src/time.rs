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

/// The year it is now.
///
/// Reading a file name needs to know how far ahead a year is still plausible.
/// The rules themselves take the year as an argument so a test never depends
/// on the clock; this is what the server passes them when it is not a test.
pub fn current_year() -> i32 {
    now().year()
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
    fn a_ratio_never_escapes_the_zero_to_one_range() {
        assert_eq!(Millis::new(500).ratio_of(Millis::new(1000)), 0.5);
        assert_eq!(Millis::new(5000).ratio_of(Millis::new(1000)), 1.0);
        assert_eq!(Millis::new(-5).ratio_of(Millis::new(1000)), 0.0);
    }

    #[test]
    fn a_missing_duration_yields_a_zero_ratio_instead_of_a_division_by_zero() {
        assert_eq!(Millis::new(500).ratio_of(Millis::ZERO), 0.0);
    }
}
