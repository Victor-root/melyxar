//! The index of an ISO base media file: MP4, M4V, MOV, and their relatives.
//!
//! A file of this family is a tree of boxes, each one four letters and a
//! length. What is wanted sits in three tables of the picture track: which
//! pictures stand on their own, how long each picture lasts, and by how much
//! each one is shown later than it is read. Together they say when every
//! picture that can be started on is shown, which is the whole question.
//!
//! The delicate part is the montage list. A file can carry, beside its
//! pictures, an instruction to skip the first fraction of a second of them,
//! and the media tool obeys it: ignored here, every position in the film comes
//! out a few hundredths early, which is a segment boundary landing where there
//! is no picture. Only the one plain shape of that list is understood; every
//! other shape makes this give up rather than guess.

use melyxar_core::time::Millis;

use crate::reader::Reader;

/// Whether a file begins the way one of this family does.
///
/// Read from the bytes rather than from the name: a film named `.mkv` that is
/// really an MP4 is a film somebody remuxed and did not rename, and the file
/// itself is never wrong about what it is.
pub(crate) fn is_one(head: &[u8]) -> bool {
    // The first box of one of these is a brand, and a QuickTime file of a
    // certain age begins straight on one of the boxes below instead.
    const OPENING: [&[u8; 4]; 7] = [
        b"ftyp", b"moov", b"mdat", b"free", b"skip", b"wide", b"pnot",
    ];
    head.len() >= 8 && OPENING.iter().any(|kind| &head[4..8] == kind.as_slice())
}

/// Where one box holds its contents.
#[derive(Clone, Copy)]
struct Span {
    body: u64,
    end: u64,
}

/// The rate a montage list runs its film at, as a fixed point number.
const AT_THE_FILM_S_OWN_PACE: u32 = 0x0001_0000;

/// Lists the boxes directly inside another one.
///
/// A box whose length runs past the box holding it is a malformed file, and it
/// stops the reading here rather than being trimmed to fit: what is being read
/// is an index, and an index that does not add up is not one.
fn children(reader: &mut Reader, within: Span) -> Option<Vec<([u8; 4], Span)>> {
    let mut found = Vec::new();
    let mut at = within.body;
    while at.checked_add(8)? <= within.end {
        reader.seek_to(at)?;
        let announced = u64::from(reader.u32()?);
        let mut kind = [0u8; 4];
        reader.exactly(&mut kind)?;
        let (length, body) = match announced {
            // One means the real length comes next, on eight bytes.
            1 => (reader.u64()?, at.checked_add(16)?),
            // Nothing means this box runs to the end of the one holding it.
            0 => (within.end.checked_sub(at)?, at.checked_add(8)?),
            _ => (announced, at.checked_add(8)?),
        };
        let end = at.checked_add(length)?;
        if body > end || end > within.end {
            return None;
        }
        found.push((kind, Span { body, end }));
        at = end;
    }
    Some(found)
}

fn first(children: &[([u8; 4], Span)], kind: &[u8; 4]) -> Option<Span> {
    children
        .iter()
        .find(|(found, _)| found == kind)
        .map(|(_, span)| *span)
}

/// Reads the header every table of this family begins with, and its length.
fn table_header(reader: &mut Reader, table: Span) -> Option<(u8, u32)> {
    reader.seek_to(table.body)?;
    let version = reader.u8()?;
    // Three bytes of flags, which none of these tables uses.
    reader.bytes(3)?;
    Some((version, reader.u32()?))
}

/// Reads a table written as runs of identical entries.
///
/// Both the lengths of the pictures and their delays are written this way: a
/// count and a value, which is why a film shot at a fixed rate carries one
/// entry for all of it. Only the delays are ever negative, and only in the
/// newer of the two shapes that table takes.
fn runs(reader: &mut Reader, table: Span, may_be_negative: bool) -> Option<Vec<(u32, i64)>> {
    let (version, how_many) = table_header(reader, table)?;
    let signed = may_be_negative && version >= 1;
    let bytes = reader.bytes((how_many as usize).checked_mul(8)?)?;
    let mut entries = Vec::with_capacity(how_many as usize);
    for entry in bytes.chunks_exact(8) {
        let count = u32::from_be_bytes(entry[..4].try_into().ok()?);
        let value = u32::from_be_bytes(entry[4..].try_into().ok()?);
        let value = match signed {
            true => i64::from(value as i32),
            false => i64::from(value),
        };
        entries.push((count, value));
    }
    Some(entries)
}

/// Reads the numbers of the pictures that stand on their own.
fn sync_samples(reader: &mut Reader, table: Span) -> Option<Vec<u32>> {
    let (_, how_many) = table_header(reader, table)?;
    let bytes = reader.bytes((how_many as usize).checked_mul(4)?)?;
    bytes
        .chunks_exact(4)
        .map(|number| Some(u32::from_be_bytes(number.try_into().ok()?)))
        .collect()
}

/// By how much the montage list of a track shifts the whole film.
///
/// Nothing at all when there is no list. Otherwise only the one plain shape is
/// read: a single stretch, played at the film's own pace, starting somewhere
/// inside the pictures. Anything else, several stretches or a stretch of
/// nothing put in front, shifts the film in a way that is not this one, and
/// this gives up instead of producing positions that are quietly wrong.
fn how_far_the_montage_shifts(reader: &mut Reader, track: &[([u8; 4], Span)]) -> Option<i64> {
    let Some(edits) = first(track, b"edts") else {
        return Some(0);
    };
    let list = first(&children(reader, edits)?, b"elst")?;
    let (version, how_many) = table_header(reader, list)?;
    if how_many != 1 {
        return None;
    }
    let (starts_at, pace) = match version {
        0 => {
            reader.bytes(4)?;
            (i64::from(reader.u32()? as i32), reader.u32()?)
        }
        1 => {
            reader.bytes(8)?;
            (reader.u64()? as i64, reader.u32()?)
        }
        _ => return None,
    };
    (starts_at >= 0 && pace == AT_THE_FILM_S_OWN_PACE).then_some(starts_at)
}

/// The picture track's own clock, in ticks a second.
fn ticks_a_second(reader: &mut Reader, media: Span) -> Option<u32> {
    let header = first(&children(reader, media)?, b"mdhd")?;
    reader.seek_to(header.body)?;
    let version = reader.u8()?;
    reader.bytes(3)?;
    match version {
        // Made and changed, then the clock.
        0 => {
            reader.bytes(8)?;
            reader.u32()
        }
        1 => {
            reader.bytes(16)?;
            reader.u32()
        }
        _ => None,
    }
}

/// Whether a track carries the picture.
fn is_the_picture(reader: &mut Reader, media: Span) -> Option<bool> {
    let handler = first(&children(reader, media)?, b"hdlr")?;
    reader.seek_to(handler.body)?;
    // Version and flags, then a field nothing uses, then the four letters.
    reader.bytes(8)?;
    let mut kind = [0u8; 4];
    reader.exactly(&mut kind)?;
    Some(&kind == b"vide")
}

/// When each picture that stands on its own is shown.
///
/// Walked once through the runs rather than laid out picture by picture: a
/// three hour film holds a quarter of a million of them, and only a few
/// thousand are ever asked about.
fn shown_at(
    lengths: &[(u32, i64)],
    delays: &[(u32, i64)],
    standing_alone: &[u32],
) -> Option<Vec<(u32, i64)>> {
    let mut shown = Vec::with_capacity(standing_alone.len());
    let mut wanted = standing_alone.iter().copied().peekable();
    let mut picture = 1u32;
    let mut read_at = 0i64;
    for &(count, length) in lengths {
        let after = picture.checked_add(count)?;
        while let Some(&number) = wanted.peek() {
            if number >= after {
                break;
            }
            if number >= picture {
                let since = i64::from(number - picture).checked_mul(length)?;
                shown.push((number, read_at.checked_add(since)?));
            }
            wanted.next();
        }
        read_at = read_at.checked_add(i64::from(count).checked_mul(length)?)?;
        picture = after;
    }

    // A picture is shown later than it is read whenever the film holds
    // pictures built from the ones around them, which is nearly every film.
    let mut next = 0usize;
    let mut picture = 1u32;
    for &(count, delay) in delays {
        let after = picture.checked_add(count)?;
        while let Some(entry) = shown.get_mut(next) {
            if entry.0 >= after {
                break;
            }
            entry.1 = entry.1.checked_add(delay)?;
            next += 1;
        }
        picture = after;
    }
    Some(shown)
}

/// Reads where the picture of an ISO base media file can be started.
pub(crate) fn key_frames(reader: &mut Reader) -> Option<Vec<Millis>> {
    let whole = Span {
        body: 0,
        end: reader.length(),
    };
    let movie = first(&children(reader, whole)?, b"moov")?;
    let tracks: Vec<Span> = children(reader, movie)?
        .into_iter()
        .filter(|(kind, _)| kind == b"trak")
        .map(|(_, span)| span)
        .collect();

    for track in tracks {
        let inside = children(reader, track)?;
        let Some(media) = first(&inside, b"mdia") else {
            continue;
        };
        if !is_the_picture(reader, media)? {
            continue;
        }

        let ticks = ticks_a_second(reader, media)?;
        if ticks == 0 {
            return None;
        }
        let shift = how_far_the_montage_shifts(reader, &inside)?;

        let information = first(&children(reader, media)?, b"minf")?;
        let samples = first(&children(reader, information)?, b"stbl")?;
        let tables = children(reader, samples)?;
        // No table of pictures standing on their own means every picture does.
        // True, and of no use: such a film is read through, which says the
        // same thing and says it about a file this has not had to understand.
        let standing_alone = sync_samples(reader, first(&tables, b"stss")?)?;
        let lengths = runs(reader, first(&tables, b"stts")?, false)?;
        let delays = match first(&tables, b"ctts") {
            Some(table) => runs(reader, table, true)?,
            None => Vec::new(),
        };

        let shown = shown_at(&lengths, &delays, &standing_alone)?;
        return Some(
            shown
                .into_iter()
                // A picture shown before the montage list starts the film is
                // one the media tool never hands out, so it is no place to
                // begin.
                .filter_map(|(_, when)| (when >= shift).then_some(when - shift))
                .map(|when| Millis::new((when as f64 * 1000.0 / f64::from(ticks)).round() as i64))
                .collect(),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_recognised_by_what_it_begins_with_and_not_by_its_name() {
        assert!(is_one(b"\0\0\0\x18ftypisom"));
        assert!(is_one(b"\0\0\0\x08wide____"), "an older QuickTime file");
        assert!(
            !is_one(b"\x1a\x45\xdf\xa3\x01\x00\x00\x00"),
            "a Matroska file"
        );
        assert!(!is_one(b"RIFF"), "too short to say anything");
    }

    #[test]
    fn a_film_shot_at_a_fixed_rate_carries_one_run_for_all_of_it() {
        // One entry saying "a hundred pictures, each a thousand ticks", and
        // three of them stand on their own.
        let shown = shown_at(&[(100, 1_000)], &[], &[1, 25, 73]).expect("read");
        assert_eq!(shown, vec![(1, 0), (25, 24_000), (73, 72_000)]);
    }

    #[test]
    fn a_picture_shown_later_than_it_is_read_is_placed_where_it_is_shown() {
        // Every film holding pictures built from the ones around them reads
        // them out of order, and the delay is what puts them back.
        let shown = shown_at(&[(10, 1_000)], &[(10, 2_000)], &[1, 5]).expect("read");
        assert_eq!(shown, vec![(1, 2_000), (5, 6_000)]);
    }

    #[test]
    fn a_delay_written_run_by_run_follows_the_pictures_it_belongs_to() {
        let shown = shown_at(&[(10, 1_000)], &[(3, 0), (7, 500)], &[1, 4, 9]).expect("read");
        assert_eq!(shown, vec![(1, 0), (4, 3_500), (9, 8_500)]);
    }

    #[test]
    fn a_film_whose_lengths_change_partway_is_still_walked_run_by_run() {
        let shown = shown_at(&[(4, 1_000), (4, 2_000)], &[], &[1, 5, 8]).expect("read");
        assert_eq!(shown, vec![(1, 0), (5, 4_000), (8, 10_000)]);
    }
}
