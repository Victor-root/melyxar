//! Conversions between stored text and domain values.
//!
//! Instants are stored as text in a sortable form and always in UTC, so that
//! ordering by date is a plain string comparison and no reader has to guess a
//! zone. Sortable means a fixed width: every digit after the second is written,
//! zeros included, since `01.51Z` trimmed sorts after `01.515Z`. Booleans are stored as zero or one, which is what the engine offers.

use melyxar_core::time::Timestamp;
use time::format_description::well_known::Rfc3339;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;
use time::UtcOffset;

use crate::{DatabaseError, Result};

/// Reads an identifier back from the stored form.
///
/// A value that no longer parses means the column holds something no version
/// of the schema ever wrote, so it is reported as corruption rather than
/// quietly replaced.
pub fn parse_id<T: std::str::FromStr>(value: &str) -> Result<T> {
    value
        .parse()
        .map_err(|_| DatabaseError::Corrupt(format!("identifier '{value}' is malformed")))
}

/// The stored form of an instant: UTC, with all nine digits after the second.
const STORED: &[BorrowedFormatItem<'static>] =
    format_description!("[year]-[month]-[day]T[hour]:[minute]:[second].[subsecond digits:9]Z");

/// Renders an instant in the stored form.
pub fn timestamp_to_text(value: Timestamp) -> String {
    value
        .to_offset(UtcOffset::UTC)
        .format(STORED)
        .expect("an instant always formats")
}

/// Reads an instant back from the stored form.
pub fn parse_timestamp(value: &str) -> Result<Timestamp> {
    Timestamp::parse(value, &Rfc3339)
        .map_err(|error| DatabaseError::Corrupt(format!("instant '{value}' is malformed: {error}")))
}

/// Reads an optional instant back, keeping absence distinct from an error.
pub fn parse_optional_timestamp(value: Option<&str>) -> Result<Option<Timestamp>> {
    value.map(parse_timestamp).transpose()
}

/// Stored form of a boolean.
pub fn bool_to_int(value: bool) -> i64 {
    i64::from(value)
}

/// Reads a boolean back. Any non-zero value counts as true, which is what the
/// engine itself does.
pub fn int_to_bool(value: i64) -> bool {
    value != 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    #[test]
    fn an_instant_survives_a_round_trip() {
        let original = datetime!(2026-09-12 21:24:05 UTC);
        let text = timestamp_to_text(original);
        assert_eq!(parse_timestamp(&text).expect("parses"), original);
    }

    #[test]
    fn instants_are_stored_in_utc_whatever_the_offset_they_arrive_with() {
        let paris = datetime!(2026-09-12 23:24:05 +02:00);
        let text = timestamp_to_text(paris);
        assert!(
            text.ends_with('Z'),
            "stored instants must be in UTC: {text}"
        );
        assert_eq!(parse_timestamp(&text).expect("parses"), paris);
    }

    #[test]
    fn stored_instants_keep_every_digit_after_the_second() {
        assert_eq!(
            timestamp_to_text(datetime!(2026-09-24 16:32:01.51 UTC)),
            "2026-09-24T16:32:01.510000000Z"
        );
        assert_eq!(
            timestamp_to_text(datetime!(2026-09-24 16:32:01 UTC)),
            "2026-09-24T16:32:01.000000000Z"
        );
    }

    #[test]
    fn instants_a_trimmed_form_put_in_the_wrong_order_sort_right() {
        let round = timestamp_to_text(datetime!(2026-09-24 16:32:01.51 UTC));
        let later = timestamp_to_text(datetime!(2026-09-24 16:32:01.515 UTC));
        assert!(round < later, "{round} must sort before {later}");
    }

    #[test]
    fn stored_instants_sort_the_same_way_as_the_instants_themselves() {
        let earlier = timestamp_to_text(datetime!(2026-01-02 03:04:05 UTC));
        let later = timestamp_to_text(datetime!(2026-01-02 03:04:06 UTC));
        assert!(
            earlier < later,
            "ordering by date must be a text comparison"
        );
    }

    #[test]
    fn a_malformed_instant_is_reported_rather_than_silently_defaulted() {
        let error = parse_timestamp("not a date").expect_err("must fail");
        assert!(matches!(error, DatabaseError::Corrupt(_)));
    }

    #[test]
    fn an_absent_instant_stays_absent() {
        assert!(parse_optional_timestamp(None).expect("no error").is_none());
    }

    #[test]
    fn booleans_round_trip_through_their_stored_form() {
        assert!(int_to_bool(bool_to_int(true)));
        assert!(!int_to_bool(bool_to_int(false)));
        assert!(int_to_bool(2), "any non-zero value reads as true");
    }
}
