//! How a file the server did not make itself is opened: how far into it the
//! tool may read.
//!
//! A file brought in from anywhere says of itself how long it lasts and when
//! each of its pictures and sounds falls, and the tool believes it. Whatever
//! reads a whole file through takes these options before the file, so that
//! what it says can only cost so much.

use std::ffi::OsString;

use melyxar_core::media::LONGEST_BELIEVABLE;

/// Reads no further into a file than any file is believed to last, whatever
/// the times written inside it: a file whose times leap ahead would otherwise
/// keep a reading going, and filling a disk, without end. Said before the
/// file, whose reading it limits.
pub(crate) fn no_further_than_believable() -> [OsString; 2] {
    [
        OsString::from("-t"),
        OsString::from((LONGEST_BELIEVABLE.get() / 1000).to_string()),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reading_stops_a_week_in() {
        assert_eq!(no_further_than_believable(), [OsString::from("-t"), OsString::from("604800")]);
    }
}
