//! Stretches of a film nobody wants to sit through.
//!
//! What somebody reaches for the remote during: the reminder of what happened
//! last week, the opening titles, the closing ones. Marking them is what puts a
//! button on the screen that skips exactly that and nothing else.
//!
//! Three things can say where one is. A file that carries chapters often names
//! them, and reading those costs nothing because the chapters are already read
//! when the file is analysed. Comparing what one episode of a season sounds
//! like against its neighbours finds the same music in all of them, which is
//! the only thing that works on a file carrying no chapters at all. And a
//! person can always say so themselves, which wins over both.
//!
//! Only the first is done here, and it is pure: chapters in, stretches out,
//! nothing read and nothing written.

use crate::media::Chapter;
use crate::time::Millis;

/// What a stretch of a film is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SegmentKind {
    /// What happened last week, before this week starts.
    Recap,
    /// The opening titles.
    Intro,
    /// The closing ones.
    Outro,
    /// What a recording off the air carries and nothing else does.
    Advertisement,
}

impl SegmentKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Recap => "recap",
            Self::Intro => "intro",
            Self::Outro => "outro",
            Self::Advertisement => "advertisement",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "recap" => Self::Recap,
            "intro" => Self::Intro,
            "outro" => Self::Outro,
            "advertisement" => Self::Advertisement,
            _ => return None,
        })
    }
}

/// How a stretch came to be known.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SegmentOrigin {
    /// Read off a chapter the file names.
    Chapter,
    /// Found by comparing one episode against the others of its season.
    Detected,
    /// Said by a person, which wins over the other two.
    Manual,
}

impl SegmentOrigin {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chapter => "chapter",
            Self::Detected => "detected",
            Self::Manual => "manual",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "chapter" => Self::Chapter,
            "detected" => Self::Detected,
            "manual" => Self::Manual,
            _ => return None,
        })
    }
}

/// One stretch of one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MediaSegment {
    pub kind: SegmentKind,
    pub start: Millis,
    pub end: Millis,
    pub origin: SegmentOrigin,
}

/// Words a chapter of each kind goes by, in the languages these files come in.
///
/// Recognised whole, or as the first word of a longer name: a chapter called
/// `Opening Credits` is the opening, and one called `Openings and Endings`
/// would be too, which is a price worth paying for `Opening Titles`. Never as a
/// part of a word, or `Cooperation` becomes an opening.
const WHAT_A_RECAP_IS_CALLED: &[&str] = &["recap", "previously", "resume", "precedemment"];
const WHAT_AN_INTRO_IS_CALLED: &[&str] = &["intro", "opening", "op", "generique", "titles"];
const WHAT_AN_OUTRO_IS_CALLED: &[&str] = &["outro", "ending", "ed", "credits", "endcard"];

/// What a chapter of this name stands for, if it stands for anything.
///
/// Anything else is a chapter of the film itself, which is most of them: a file
/// whose chapters are called `Chapter 1` to `Chapter 12` says nothing about
/// where its opening is, and guessing from a number would skip the film.
fn what_it_is(title: &str) -> Option<SegmentKind> {
    let folded = crate::text::fold_accents(title).to_lowercase();
    let first = folded.split_whitespace().next()?;
    // The outro list is asked first: `ending credits` carries a word of the
    // intro list nowhere near its front, and a name is read from its front.
    for (kind, names) in [
        (SegmentKind::Recap, WHAT_A_RECAP_IS_CALLED),
        (SegmentKind::Outro, WHAT_AN_OUTRO_IS_CALLED),
        (SegmentKind::Intro, WHAT_AN_INTRO_IS_CALLED),
    ] {
        if names.contains(&first) {
            return Some(kind);
        }
    }
    None
}

/// Reads the stretches a file's own chapters name.
///
/// A chapter says where it starts and never where it ends, so each one ends
/// where the next begins, and the last one at the end of the film. A film of
/// unknown length gives up its last chapter rather than guessing a length for
/// it: a stretch with no end is a skip button that skips to nowhere.
///
/// The chapters are read in the order they start, whatever order they arrive
/// in: a stretch built from two chapters the wrong way round would run
/// backwards, and nothing below would notice.
pub fn segments_from_chapters(chapters: &[Chapter], duration: Option<Millis>) -> Vec<MediaSegment> {
    let mut in_order: Vec<&Chapter> = chapters.iter().collect();
    in_order.sort_by_key(|chapter| chapter.start);

    let mut found = Vec::new();
    for (index, chapter) in in_order.iter().enumerate() {
        let Some(kind) = chapter.title.as_deref().and_then(what_it_is) else {
            continue;
        };
        let end = match in_order.get(index + 1) {
            Some(next) => Some(next.start),
            None => duration,
        };
        let Some(end) = end.filter(|end| *end > chapter.start) else {
            continue;
        };
        found.push(MediaSegment {
            kind,
            start: chapter.start,
            end,
            origin: SegmentOrigin::Chapter,
        });
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chapter(ordinal: i32, start_ms: i64, title: &str) -> Chapter {
        Chapter {
            ordinal,
            start: Millis::new(start_ms),
            title: Some(title.to_string()),
            thumbnail_path: None,
        }
    }

    #[test]
    fn a_chapter_that_names_itself_becomes_the_stretch_it_names() {
        let read = segments_from_chapters(
            &[
                chapter(1, 0, "Recap"),
                chapter(2, 30_000, "Opening Credits"),
                chapter(3, 120_000, "Part One"),
                chapter(4, 1_200_000, "Ending"),
            ],
            Some(Millis::new(1_300_000)),
        );

        assert_eq!(
            read.iter()
                .map(|found| (found.kind, found.start.get(), found.end.get()))
                .collect::<Vec<_>>(),
            vec![
                (SegmentKind::Recap, 0, 30_000),
                (SegmentKind::Intro, 30_000, 120_000),
                (SegmentKind::Outro, 1_200_000, 1_300_000),
            ],
            "each one ends where the next begins, and the last at the end"
        );
        assert!(read
            .iter()
            .all(|found| found.origin == SegmentOrigin::Chapter));
    }

    #[test]
    fn chapters_that_name_nothing_name_nothing() {
        // Most files are like this, and guessing from a number would skip the
        // film rather than its opening.
        assert!(segments_from_chapters(
            &[
                chapter(1, 0, "Chapter 1"),
                chapter(2, 600_000, "Chapter 2"),
                chapter(3, 1_200_000, "Cooperation"),
            ],
            Some(Millis::new(1_800_000)),
        )
        .is_empty());
    }

    #[test]
    fn the_short_names_an_animation_uses_are_read() {
        let read = segments_from_chapters(
            &[
                chapter(1, 0, "OP"),
                chapter(2, 90_000, "Episode"),
                chapter(3, 1_300_000, "ED"),
            ],
            Some(Millis::new(1_400_000)),
        );
        assert_eq!(
            read.iter().map(|found| found.kind).collect::<Vec<_>>(),
            vec![SegmentKind::Intro, SegmentKind::Outro]
        );
    }

    #[test]
    fn a_name_is_read_from_its_front() {
        // `Ending credits` carries a word of the closing list and a word of
        // nothing else; read from anywhere it would be two things at once.
        let read = segments_from_chapters(
            &[chapter(1, 0, "Ending credits"), chapter(2, 60_000, "Part")],
            Some(Millis::new(600_000)),
        );
        assert_eq!(read[0].kind, SegmentKind::Outro);
    }

    #[test]
    fn an_accent_written_either_way_is_read_the_same() {
        for title in ["Générique", "Generique", "GÉNÉRIQUE"] {
            let read = segments_from_chapters(
                &[chapter(1, 0, title), chapter(2, 90_000, "Film")],
                Some(Millis::new(600_000)),
            );
            assert_eq!(read.len(), 1, "{title}");
            assert_eq!(read[0].kind, SegmentKind::Intro, "{title}");
        }
    }

    #[test]
    fn a_stretch_with_no_end_is_never_offered() {
        // A skip button with nowhere to skip to is worse than no button.
        assert!(segments_from_chapters(&[chapter(1, 0, "Intro")], None).is_empty());
        // And one whose next chapter starts where it does has no length.
        assert!(
            segments_from_chapters(&[chapter(1, 0, "Intro"), chapter(2, 0, "Film")], None)
                .is_empty()
        );
    }

    #[test]
    fn chapters_out_of_order_are_read_in_order() {
        // A stretch built from two chapters the wrong way round would run
        // backwards, and nothing downstream would notice.
        let read = segments_from_chapters(
            &[chapter(2, 90_000, "Part"), chapter(1, 0, "Intro")],
            Some(Millis::new(600_000)),
        );
        assert_eq!(read.len(), 1);
        assert_eq!((read[0].start.get(), read[0].end.get()), (0, 90_000));
    }
}
