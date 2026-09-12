//! Conversions between stored text and domain values.
//!
//! Instants are stored as text in a sortable form and always in UTC, so that
//! ordering by date is a plain string comparison and no reader has to guess a
//! zone. Booleans are stored as zero or one, which is what the engine offers.

use melyxar_core::time::Timestamp;
use time::format_description::well_known::Rfc3339;

use crate::{DatabaseError, Result};

/// Renders an instant in the stored form.
pub fn timestamp_to_text(value: Timestamp) -> String {
    value
        .to_offset(time::UtcOffset::UTC)
        .format(&Rfc3339)
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
