//! The index of a Matroska file: MKV and WebM.
//!
//! A file of this family is a tree of elements, each one an identifier and a
//! length, both written on as many bytes as they need. What is wanted is the
//! list of search marks the file carries for itself, which is exactly the list
//! of places its picture stands on its own, and the marks belong to a track so
//! the picture's own have to be told from the sound's.
//!
//! The marks are counted in the file's own unit, which it declares once. It is
//! a thousandth of a second in every file anyone writes, and reading it rather
//! than assuming it costs four lines.

use melyxar_core::time::Millis;

use crate::reader::Reader;

/// What every file of this family begins with.
const OPENING: [u8; 4] = [0x1A, 0x45, 0xDF, 0xA3];

const SEGMENT: u32 = 0x1853_8067;
const INFORMATION: u32 = 0x1549_A966;
const UNIT_IN_NANOSECONDS: u32 = 0x2AD7B1;
const TRACKS: u32 = 0x1654_AE6B;
const TRACK: u32 = 0xAE;
const TRACK_NUMBER: u32 = 0xD7;
const TRACK_KIND: u32 = 0x83;
const TRACK_OWN_UNIT: u32 = 0x23314F;
const MARKS: u32 = 0x1C53_BB6B;
const MARK: u32 = 0xBB;
const MARK_TIME: u32 = 0xB3;
const MARK_PLACES: u32 = 0xB7;
const MARK_TRACK: u32 = 0xF7;

/// What a track carrying the picture calls itself.
const A_PICTURE: u64 = 1;

/// A nanosecond count meaning a thousandth of a second, the unit every file
/// of this family is written in.
const A_THOUSANDTH_OF_A_SECOND: u64 = 1_000_000;

pub(crate) fn is_one(head: &[u8]) -> bool {
    head.len() >= 4 && head[..4] == OPENING
}

/// Reads a number written on as many bytes as it needs.
///
/// The first byte says how many follow: the higher the first bit that is set,
/// the shorter the number. An identifier keeps that marker, since it is part
/// of what names the element; a length drops it.
fn variable(reader: &mut Reader, keep_the_marker: bool) -> Option<(u64, bool)> {
    let first = reader.u8()?;
    if first == 0 {
        return None;
    }
    let mut length = 1u32;
    let mut marker = 0x80u8;
    while first & marker == 0 {
        marker >>= 1;
        length += 1;
    }
    let mut value = u64::from(match keep_the_marker {
        true => first,
        false => first & (marker - 1),
    });
    // A length whose every meaningful bit is set means "until further notice",
    // which is what a file being written says about what it is still writing.
    let mut every_bit_set = first & (marker - 1) == marker - 1;
    for byte in reader.bytes(length as usize - 1)? {
        value = value.checked_mul(256)?.checked_add(u64::from(byte))?;
        every_bit_set = every_bit_set && byte == 0xFF;
    }
    Some((value, every_bit_set))
}

/// Where one element holds its contents.
#[derive(Clone, Copy)]
struct Span {
    body: u64,
    end: u64,
}

/// Reads one element's identifier and where its contents lie.
///
/// An element of unspecified length cannot be stepped over, so it stops the
/// reading: what follows it cannot be found without reading it through, which
/// is the very thing this exists to avoid.
fn element(reader: &mut Reader, within: Span) -> Option<(u32, Span)> {
    let (identifier, _) = variable(reader, true)?;
    let identifier = u32::try_from(identifier).ok()?;
    let (length, unspecified) = variable(reader, false)?;
    if unspecified {
        return None;
    }
    let body = reader.at();
    let end = body.checked_add(length)?;
    (end <= within.end).then_some((identifier, Span { body, end }))
}

/// Reads a whole number, which this family writes on as many bytes as it needs.
fn whole_number(reader: &mut Reader, at: Span) -> Option<u64> {
    reader.seek_to(at.body)?;
    let mut value = 0u64;
    for byte in reader.bytes(usize::try_from(at.end.checked_sub(at.body)?).ok()?)? {
        value = value.checked_mul(256)?.checked_add(u64::from(byte))?;
    }
    Some(value)
}

/// Walks the elements directly inside another one, handing each to a reader.
///
/// Stepping over an element is a seek, never a reading: this is what lets the
/// marks be found at the far end of a film without any of the film in between
/// being touched.
fn walk(
    reader: &mut Reader,
    within: Span,
    mut each: impl FnMut(&mut Reader, u32, Span) -> Option<()>,
) -> Option<()> {
    let mut at = within.body;
    while at < within.end {
        reader.seek_to(at)?;
        let (identifier, span) = element(reader, within)?;
        each(reader, identifier, span)?;
        at = span.end;
    }
    Some(())
}

/// Which track carries the picture, and whether it is one this can answer for.
///
/// A track may declare a clock of its own that stretches its marks. Nothing
/// writes one and the media tool does not agree with itself about what it
/// would mean, so a file carrying one is handed back to the full reading
/// rather than answered with marks that might be stretched.
fn the_picture_track(reader: &mut Reader, tracks: Span) -> Option<u64> {
    let mut picture = None;
    walk(reader, tracks, |reader, identifier, span| {
        if identifier != TRACK || picture.is_some() {
            return Some(());
        }
        let mut number = None;
        let mut kind = None;
        let mut has_its_own_clock = false;
        walk(reader, span, |reader, identifier, span| {
            match identifier {
                TRACK_NUMBER => number = whole_number(reader, span),
                TRACK_KIND => kind = whole_number(reader, span),
                TRACK_OWN_UNIT => has_its_own_clock = true,
                _ => {}
            }
            Some(())
        })?;
        if kind == Some(A_PICTURE) && !has_its_own_clock {
            picture = number;
        }
        Some(())
    })?;
    picture
}

/// Reads the marks of one track, in the file's own unit.
fn marks_of(reader: &mut Reader, marks: Span, track: u64) -> Option<Vec<u64>> {
    let mut found = Vec::new();
    walk(reader, marks, |reader, identifier, span| {
        if identifier != MARK {
            return Some(());
        }
        let mut when = None;
        let mut belongs_to = None;
        walk(reader, span, |reader, identifier, span| {
            match identifier {
                MARK_TIME => when = whole_number(reader, span),
                MARK_PLACES if belongs_to.is_none() => {
                    walk(reader, span, |reader, identifier, span| {
                        if identifier == MARK_TRACK {
                            belongs_to = whole_number(reader, span);
                        }
                        Some(())
                    })?;
                }
                _ => {}
            }
            Some(())
        })?;
        if let (Some(when), Some(belongs_to)) = (when, belongs_to) {
            if belongs_to == track {
                found.push(when);
            }
        }
        Some(())
    })?;
    Some(found)
}

/// Reads where the picture of a Matroska file can be started.
pub(crate) fn key_frames(reader: &mut Reader) -> Option<Vec<Millis>> {
    let whole = Span {
        body: 0,
        end: reader.length(),
    };
    reader.seek_to(0)?;
    let (opening, header) = element(reader, whole)?;
    if opening != u32::from_be_bytes(OPENING) {
        return None;
    }
    reader.seek_to(header.end)?;
    // A file still being written declares no length for its body, and there is
    // nothing to find in it anyway: the marks are written last of all.
    let (identifier, segment) = element(reader, whole)?;
    if identifier != SEGMENT {
        return None;
    }

    let mut unit = A_THOUSANDTH_OF_A_SECOND;
    let mut track = None;
    let mut marks = None;
    walk(reader, segment, |reader, identifier, span| {
        match identifier {
            INFORMATION => {
                walk(reader, span, |reader, identifier, span| {
                    if identifier == UNIT_IN_NANOSECONDS {
                        unit = whole_number(reader, span)?;
                    }
                    Some(())
                })?;
            }
            TRACKS if track.is_none() => track = the_picture_track(reader, span),
            MARKS if marks.is_none() => marks = Some(span),
            _ => {}
        }
        Some(())
    })?;

    let found = marks_of(reader, marks?, track?)?;
    if unit == 0 {
        return None;
    }
    Some(
        found
            .into_iter()
            .map(|when| Millis::new((when as f64 * unit as f64 / 1_000_000.0).round() as i64))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn reader_of(bytes: &[u8]) -> (tempfile::TempDir, Reader) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("file");
        std::fs::File::create(&path)
            .expect("the file is created")
            .write_all(bytes)
            .expect("the file is written");
        let reader = Reader::open(&path).expect("the file opens");
        (directory, reader)
    }

    #[test]
    fn a_file_is_recognised_by_what_it_begins_with() {
        assert!(is_one(&[0x1A, 0x45, 0xDF, 0xA3, 0x01]));
        assert!(!is_one(b"\0\0\0\x18ftypisom"));
        assert!(!is_one(&[0x1A, 0x45]));
    }

    #[test]
    fn a_number_is_read_whatever_the_number_of_bytes_it_was_written_on() {
        // One byte, marker at the top: the value is what is left of it.
        let (_directory, mut reader) = reader_of(&[0x85]);
        assert_eq!(variable(&mut reader, false), Some((5, false)));

        // Two bytes: the marker is the second bit, and the rest carries on.
        let (_directory, mut reader) = reader_of(&[0x40, 0x7B]);
        assert_eq!(variable(&mut reader, false), Some((0x7B, false)));

        // An identifier keeps its marker, since that is part of its name.
        let (_directory, mut reader) = reader_of(&[0x1A, 0x45, 0xDF, 0xA3]);
        assert_eq!(variable(&mut reader, true), Some((0x1A45DFA3, false)));
    }

    #[test]
    fn a_length_still_being_written_says_so_and_is_not_read_as_a_number() {
        // Every meaningful bit set is what a file being written puts where a
        // length goes. Read as a number it would be eighteen million million,
        // which is a span past the end of every file there is.
        let (_directory, mut reader) = reader_of(&[0xFF]);
        assert_eq!(variable(&mut reader, false), Some((0x7F, true)));

        let (_directory, mut reader) = reader_of(&[0x01, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let (_, unspecified) = variable(&mut reader, false).expect("read");
        assert!(unspecified);
    }

    #[test]
    fn a_zero_first_byte_is_no_number_at_all() {
        let (_directory, mut reader) = reader_of(&[0x00, 0x01]);
        assert_eq!(variable(&mut reader, false), None);
    }

    #[test]
    fn a_whole_number_is_read_from_however_many_bytes_it_was_given() {
        let (_directory, mut reader) = reader_of(&[0x00, 0x0F, 0x42, 0x40]);
        let span = Span { body: 0, end: 4 };
        assert_eq!(whole_number(&mut reader, span), Some(1_000_000));
    }
}
