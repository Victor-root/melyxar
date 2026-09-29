//! Lyrics as they are written, in a file of their own, inside a song, or by
//! an online source: plain words, or words each stamped with when they are
//! sung, the form every player and LRCLIB call LRC.

use crate::time::Millis;

/// One line sung at a known moment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncedLine {
    pub at: Millis,
    pub text: String,
}

/// The words of a song: always as plain text, and line by line with their
/// moments when whoever wrote them gave those.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Lyrics {
    pub plain: String,
    pub synced: Vec<SyncedLine>,
}

impl Lyrics {
    pub fn is_empty(&self) -> bool {
        self.plain.trim().is_empty() && self.synced.is_empty()
    }
}

/// Reads lyrics whatever their form. Stamped lines give the synced words and
/// the plain words both; unstamped text is plain words alone. The headings of
/// the form (`[ar:...]`, `[ti:...]`) are left out, and its `[offset:...]`,
/// which says how much sooner every line comes, is applied.
pub fn read_lyrics(text: &str) -> Lyrics {
    let mut offset = 0i64;
    let mut synced = Vec::new();
    for line in text.lines() {
        let (stamps, words) = stamps_of(line.trim());
        if let Some(value) = heading(line.trim(), "offset") {
            offset = value.trim().parse().unwrap_or(0);
            continue;
        }
        for at in stamps {
            synced.push(SyncedLine {
                at: Millis::new((at - offset).max(0)),
                text: words.trim().to_string(),
            });
        }
    }
    if synced.is_empty() {
        return Lyrics {
            plain: text.trim().to_string(),
            synced,
        };
    }
    synced.sort_by_key(|line| line.at);
    let plain = synced
        .iter()
        .map(|line| line.text.as_str())
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    Lyrics { plain, synced }
}

/// The moments a line opens with, in milliseconds, and the words after them.
fn stamps_of(line: &str) -> (Vec<i64>, &str) {
    let mut stamps = Vec::new();
    let mut rest = line;
    while let Some(inside) = rest.strip_prefix('[') {
        let Some(end) = inside.find(']') else {
            break;
        };
        let Some(at) = moment(&inside[..end]) else {
            break;
        };
        stamps.push(at);
        rest = &inside[end + 1..];
    }
    (stamps, rest)
}

/// `mm:ss`, `mm:ss.xx` or `mm:ss.xxx`, in milliseconds.
fn moment(stamp: &str) -> Option<i64> {
    let (minutes, seconds) = stamp.split_once(':')?;
    let minutes: i64 = minutes.trim().parse().ok()?;
    let (whole, fraction) = seconds.split_once(['.', ':']).unwrap_or((seconds, ""));
    let whole: i64 = whole.trim().parse().ok()?;
    if !(0..60).contains(&whole) || fraction.len() > 3 || !fraction.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let fraction = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().ok()? * 100,
        2 => fraction.parse::<i64>().ok()? * 10,
        _ => fraction.parse::<i64>().ok()?,
    };
    Some(minutes * 60_000 + whole * 1000 + fraction)
}

/// The value of a heading of the form, such as `[offset:+250]`.
fn heading<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    let inside = line.strip_prefix('[')?.strip_suffix(']')?;
    let (key, value) = inside.split_once(':')?;
    key.trim().eq_ignore_ascii_case(name).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(at: i64, text: &str) -> SyncedLine {
        SyncedLine {
            at: Millis::new(at),
            text: text.to_string(),
        }
    }

    #[test]
    fn stamped_lines_are_read_with_their_moments_and_as_plain_words() {
        let read = read_lyrics("[ar:Amber Field]\n[ti:Tides]\n[00:07.78]First line\n[00:12.5]Second line\n[01:02.345]Third\n");
        assert_eq!(
            read.synced,
            vec![line(7780, "First line"), line(12500, "Second line"), line(62345, "Third")]
        );
        assert_eq!(read.plain, "First line\nSecond line\nThird");
    }

    #[test]
    fn a_line_sung_twice_comes_at_both_moments_in_order() {
        let read = read_lyrics("[00:30.00][00:10.00]Chorus\n[00:20.00]Verse");
        assert_eq!(
            read.synced,
            vec![line(10_000, "Chorus"), line(20_000, "Verse"), line(30_000, "Chorus")]
        );
    }

    #[test]
    fn the_offset_brings_every_line_sooner() {
        let read = read_lyrics("[offset:+500]\n[00:01.00]Early\n[00:00.20]Never before nought");
        assert_eq!(read.synced, vec![line(0, "Never before nought"), line(500, "Early")]);
    }

    #[test]
    fn words_without_moments_are_plain_words_alone() {
        let read = read_lyrics("  Just the words\nand more words  \n");
        assert!(read.synced.is_empty());
        assert_eq!(read.plain, "Just the words\nand more words");
        assert!(read_lyrics("   \n ").is_empty());
    }

    #[test]
    fn brackets_that_are_not_moments_stay_in_the_words() {
        let read = read_lyrics("[Chorus]\n[00:05.00][softly] here");
        assert_eq!(read.synced, vec![line(5000, "[softly] here")]);
    }
}
