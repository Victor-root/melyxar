//! Other ways to read a title that found nothing as it was read.
//!
//! The reading of a file name in `naming` is made for the names most
//! collections carry, and a provider finds most films from it. What it leaves
//! is the names written by hand, each in its own way: a number to sort a
//! collection by in front of the title, a word saying which edition it is,
//! the language of the copy welded into one word, two words welded into one,
//! the real title written after the year of a collection's name, the volume
//! of a series in French where the provider numbers it its own way.
//!
//! **Asked for only once the title as read has found nothing**, so a film
//! named today is named the same way tomorrow: none of this ever runs for it.
//! And each reading says whether the year has to agree for a film to be taken
//! on it. The ones that only take noise away do not change which film is
//! meant. The ones that take a number away do, `Tome 2` is not `Tome 1`, and
//! only the year tells which one was.

use std::collections::BTreeSet;

use crate::naming;

/// One more way to read a title.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rereading {
    pub title: String,
    pub year: Option<i32>,
    /// Whether a film is only taken on this reading when its year agrees
    /// with [`Rereading::year`], which is then always known.
    pub year_must_agree: bool,
}

/// What a release writes about its copy, alone or welded into one word with
/// others: never a word of a title, and dropped wherever it stands.
const RELEASE_PIECES: &[&str] = &[
    "multi",
    "truefrench",
    "vff",
    "vf2",
    "vfq",
    "vfi",
    "vof",
    "vostfr",
    "subfrench",
    "hdrip",
    "webrip",
    "webdl",
    "hdlight",
    "mhd",
    "bluray",
    "bdrip",
    "brrip",
    "dvdrip",
    "x264",
    "x265",
    "h264",
    "h265",
    "hevc",
    "xvid",
    "1080p",
    "720p",
    "2160p",
    "480p",
    "ac3",
    "eac3",
    "aac",
    "dts",
    "custom",
    "readnfo",
];

/// What a release writes about its copy that is also a word in its own
/// right, `french` or `fr`: only dropped welded to another piece, where no
/// title would ever have it.
const PIECES_ONLY_WELDED: &[&str] = &["french", "true", "fr", "en", "eng", "vf", "vo", "web", "dl"];

/// Which edition a copy is. Never what a provider names the film.
const EDITIONS: &[&str] = &["extended", "uncut", "unrated", "remastered", "imax"];

/// The same, in two words.
const EDITIONS_IN_TWO_WORDS: &[(&str, &str)] = &[
    ("version", "longue"),
    ("directors", "cut"),
    ("director's", "cut"),
    ("extended", "cut"),
    ("special", "edition"),
    ("edition", "speciale"),
];

/// The extension of a video, left in the name when it was written by hand.
const EXTENSIONS: &[&str] = &["avi", "mkv", "mp4", "m4v"];

/// The words a collection numbers its volumes with.
const VOLUMES: &[&str] = &["tome", "vol", "volume"];

/// Every other reading of a title, the readings that change nothing about
/// which film is meant first, each once and none the same as the title.
///
/// `file_name` is the name of the file the title was read from, for the one
/// reading that needs what the title left behind: a real title written after
/// the year of the collection's own name.
pub fn rereadings(
    title: &str,
    year: Option<i32>,
    file_name: Option<&str>,
    current_year: i32,
) -> Vec<Rereading> {
    let mut found: Vec<Rereading> = Vec::new();
    let mut seen: BTreeSet<Vec<String>> = BTreeSet::new();
    seen.insert(naming::matchable_title(title));
    let mut keep = |reading: Rereading| {
        let words = naming::matchable_title(&reading.title);
        if !words.is_empty() && seen.insert(words) {
            found.push(reading);
        }
    };

    let cleaned = without_noise(title);
    keep(Rereading {
        title: cleaned.clone(),
        year,
        year_must_agree: false,
    });
    let unwelded = with_welded_words_apart(&cleaned);
    keep(Rereading {
        title: unwelded.clone(),
        year,
        year_must_agree: false,
    });

    if let Some((after, its_year)) =
        file_name.and_then(|name| title_after_the_year(name, current_year))
    {
        keep(Rereading {
            title: with_welded_words_apart(&without_noise(&after)),
            year: Some(its_year),
            year_must_agree: true,
        });
    }

    if year.is_some() {
        for title in [
            without_volume_numbers(&unwelded),
            with_the_volume_numbered(&unwelded),
            with_numbers_in_figures(&unwelded),
        ] {
            keep(Rereading {
                title,
                year,
                year_must_agree: true,
            });
        }
    }
    found
}

/// Numbers written in words, in English and French, and what they are in
/// figures: `Seven` where the provider has `7`.
const NUMBERS_IN_WORDS: &[(&str, &str)] = &[
    ("two", "2"),
    ("three", "3"),
    ("four", "4"),
    ("five", "5"),
    ("six", "6"),
    ("seven", "7"),
    ("eight", "8"),
    ("nine", "9"),
    ("ten", "10"),
    ("deux", "2"),
    ("trois", "3"),
    ("quatre", "4"),
    ("cinq", "5"),
    ("sept", "7"),
    ("huit", "8"),
    ("neuf", "9"),
    ("dix", "10"),
];

/// The title with its numbers written in words put in figures, after its
/// first word: `Seven` of `Northwind Seven`, never the `Seven` a title opens on.
fn with_numbers_in_figures(title: &str) -> String {
    title
        .split_whitespace()
        .enumerate()
        .map(|(index, word)| {
            let lowered = lowered(word);
            NUMBERS_IN_WORDS
                .iter()
                .find(|(written, _)| index > 0 && *written == lowered)
                .map_or_else(|| word.to_string(), |(_, figure)| (*figure).to_string())
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The title with `Tome 2` written as a provider numbers a sequel, `2`: a
/// title matched reads its roman numerals as figures, so `II` is found too.
fn with_the_volume_numbered(title: &str) -> String {
    let words: Vec<&str> = title.split_whitespace().collect();
    let mut kept: Vec<String> = Vec::new();
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        let number = words.get(index + 1).filter(|next| {
            (1..=2).contains(&next.len()) && next.chars().all(|c| c.is_ascii_digit())
        });
        if let (true, Some(number)) = (VOLUMES.contains(&lowered(word).as_str()), number) {
            kept.push((*number).to_string());
            index += 2;
            continue;
        }
        kept.push(word.to_string());
        index += 1;
    }
    kept.join(" ")
}

/// The title with what a release or a collection put in it taken out: the
/// number it is sorted by in front, the edition, the language and the format
/// of the copy, the extension of the file.
fn without_noise(title: &str) -> String {
    let words: Vec<&str> = title.split_whitespace().collect();
    let mut kept: Vec<String> = Vec::new();
    let mut index = 0;

    // A number to sort by, `01` or `1-`, in front of a title that goes on.
    if let Some(first) = words.first() {
        let number = first.trim_end_matches(['-', '.']);
        if words.len() > 1
            && (1..=2).contains(&number.len())
            && number.chars().all(|c| c.is_ascii_digit())
            && words[1].chars().next().is_some_and(char::is_alphabetic)
        {
            index = 1;
        }
    }
    // The dash of `1 - Title`, left on its own once the number went.
    if index == 1 && words.get(1) == Some(&"-") {
        index = 2;
    }

    while index < words.len() {
        let word = words[index];
        if let Some(next) = words.get(index + 1) {
            let pair = (lowered(word), lowered(next));
            if EDITIONS_IN_TWO_WORDS
                .iter()
                .any(|(first, second)| pair.0 == *first && pair.1 == *second)
            {
                index += 2;
                continue;
            }
        }
        // A word welded to its noise with a dash keeps its own part.
        let parts: Vec<&str> = word.split('-').collect();
        let meant: Vec<&str> = parts
            .iter()
            .copied()
            .filter(|part| !is_noise(part))
            .collect();
        if meant.len() == parts.len() {
            kept.push(word.to_string());
        } else if !meant.is_empty() {
            kept.push(meant.join("-"));
        }
        index += 1;
    }

    // Never the whole title: a name made only of noise is still the name.
    if kept.is_empty() {
        return title.trim().to_string();
    }
    kept.join(" ")
}

fn lowered(word: &str) -> String {
    word.trim_matches(|c: char| !c.is_alphanumeric() && c != '\'')
        .to_lowercase()
}

/// Whether a word says something about the copy rather than the film.
fn is_noise(word: &str) -> bool {
    let word = lowered(word);
    if word.is_empty() {
        return false;
    }
    EDITIONS.contains(&word.as_str())
        || EXTENSIONS.contains(&word.as_str())
        || RELEASE_PIECES.contains(&word.as_str())
        || made_of_pieces(&word)
}

/// Whether a word is two pieces or more of what a release writes about its
/// copy, welded: `multitruefrench`, `frenchwebrip`.
fn made_of_pieces(word: &str) -> bool {
    fn pieces_from(rest: &str, count: usize) -> bool {
        if rest.is_empty() {
            return count >= 2;
        }
        RELEASE_PIECES
            .iter()
            .chain(PIECES_ONLY_WELDED)
            .any(|piece| {
                rest.strip_prefix(piece)
                    .is_some_and(|after| pieces_from(after, count + 1))
            })
    }
    pieces_from(word, 0)
}

/// The title with two words that lost the space between them put apart: a
/// name and its number, `Northwind1`, or two words in capitals, `IronGate`.
fn with_welded_words_apart(title: &str) -> String {
    title
        .split_whitespace()
        .map(apart)
        .collect::<Vec<_>>()
        .join(" ")
}

fn apart(word: &str) -> String {
    let letters: Vec<char> = word.chars().collect();
    let digits_at_the_end = letters
        .iter()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .count();
    let before_them = letters.len() - digits_at_the_end;
    // A name and its number: three letters at least, then one or two digits.
    if (1..=2).contains(&digits_at_the_end)
        && before_them >= 3
        && letters[..before_them].iter().all(|c| c.is_alphabetic())
    {
        let name: String = letters[..before_them].iter().collect();
        let number: String = letters[before_them..].iter().collect();
        return format!("{} {number}", apart(&name));
    }
    // Two words in capitals: a small letter then a capital, each side three
    // letters long at least, and nothing but letters.
    if letters.iter().all(|c| c.is_alphabetic())
        && let Some(cut) = (3..letters.len().saturating_sub(2))
            .find(|&at| letters[at - 1].is_lowercase() && letters[at].is_uppercase())
    {
        let first: String = letters[..cut].iter().collect();
        let rest: String = letters[cut..].iter().collect();
        return format!("{first} {}", apart(&rest));
    }
    word.to_string()
}

/// The title with the volume of a series taken out, `Tome 2`, and every small
/// number standing alone after its first word.
fn without_volume_numbers(title: &str) -> String {
    let words: Vec<&str> = title.split_whitespace().collect();
    let small_number =
        |word: &str| (1..=2).contains(&word.len()) && word.chars().all(|c| c.is_ascii_digit());
    let mut kept: Vec<&str> = Vec::new();
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        if VOLUMES.contains(&lowered(word).as_str())
            && words.get(index + 1).is_some_and(|next| small_number(next))
        {
            index += 2;
            continue;
        }
        if index > 0 && small_number(word) {
            index += 1;
            continue;
        }
        kept.push(word);
        index += 1;
    }
    kept.join(" ")
}

/// The real title of a name written `Collection - 2006 - Title - ...`, read
/// as a file name of its own, and the year it was written after.
fn title_after_the_year(file_name: &str, current_year: i32) -> Option<(String, i32)> {
    let pieces: Vec<&str> = file_name.split('-').map(str::trim).collect();
    let at = pieces
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, piece)| {
            naming::a_plausible_year(piece, current_year).map(|year| (index, year))
        })?;
    let (index, year) = at;
    let after = pieces.get(index + 1..)?.join(" - ");
    let title = naming::parse(&after, current_year).title;
    (!title.trim().is_empty() && title.chars().any(char::is_alphabetic)).then_some((title, year))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i32 = 2026;

    fn titles(readings: &[Rereading]) -> Vec<&str> {
        readings
            .iter()
            .map(|reading| reading.title.as_str())
            .collect()
    }

    #[test]
    fn a_title_with_nothing_to_take_away_has_no_other_reading() {
        assert!(rereadings("Quiet Harbour", Some(2019), None, NOW).is_empty());
        assert!(rereadings("Quiet Harbour", None, None, NOW).is_empty());
    }

    #[test]
    fn what_a_collection_or_a_release_put_in_the_title_is_taken_out() {
        assert_eq!(
            titles(&rereadings(
                "01 Quiet Harbour Version Longue",
                None,
                None,
                NOW
            )),
            ["Quiet Harbour"]
        );
        assert_eq!(
            titles(&rereadings("1- Amber Coast", Some(2014), None, NOW))[0],
            "Amber Coast"
        );
        assert_eq!(
            titles(&rereadings("Amber Coast-EXTENDED", None, None, NOW)),
            ["Amber Coast"]
        );
        assert_eq!(
            titles(&rereadings("Silver Lake MultiTrueFrench", None, None, NOW)),
            ["Silver Lake"]
        );
        assert_eq!(
            titles(&rereadings("Silver Lake 1080p", Some(1998), None, NOW))[0],
            "Silver Lake"
        );
        assert_eq!(
            titles(&rereadings(
                "SILVER LAKE version longue part 1 avi",
                None,
                None,
                NOW
            )),
            ["SILVER LAKE part 1"]
        );
    }

    #[test]
    fn a_word_of_a_title_that_a_release_also_writes_is_kept_alone() {
        // `French` is a word of titles; only welded to another piece is it a
        // release's.
        assert!(rereadings("The French Harbour", Some(1971), None, NOW).is_empty());
        assert!(rereadings("Coast en Ville", None, None, NOW).is_empty());
        // A hyphen inside a title keeps it.
        assert!(rereadings("Spider-Coast", None, None, NOW).is_empty());
    }

    #[test]
    fn two_welded_words_are_put_apart() {
        assert_eq!(
            titles(&rereadings("Northwind1", Some(2019), None, NOW))[0],
            "Northwind 1"
        );
        assert_eq!(
            titles(&rereadings("IronGate 2", Some(2023), None, NOW))[0],
            "Iron Gate 2"
        );
        // Too short on either side to be two words.
        assert!(rereadings("McCoy", None, None, NOW).is_empty());
    }

    #[test]
    fn readings_that_change_the_film_meant_ask_for_the_year() {
        let readings = rereadings("Quiet Harbour Tome 2", Some(2011), None, NOW);
        assert_eq!(titles(&readings), ["Quiet Harbour", "Quiet Harbour 2"]);
        assert!(
            readings
                .iter()
                .all(|reading| reading.year_must_agree && reading.year == Some(2011))
        );

        // Without a year to agree with, a number is never taken away.
        assert!(rereadings("Quiet Harbour Tome 2", None, None, NOW).is_empty());

        let numbered = rereadings(
            "Amber Saga 7 2 Et La Fin 2eme Partie",
            Some(2011),
            None,
            NOW,
        );
        assert_eq!(titles(&numbered), ["Amber Saga Et La Fin 2eme Partie"]);
        assert_eq!(
            titles(&rereadings("Northwind Seven", Some(2015), None, NOW)),
            ["Northwind 7"]
        );
        assert!(
            rereadings("Seven Harbours", Some(1995), None, NOW).is_empty(),
            "a title may open on a number"
        );
        // What only took noise away never asks for the year.
        assert!(!rereadings("01 Quiet Harbour", Some(2019), None, NOW)[0].year_must_agree);
    }

    #[test]
    fn the_title_written_after_the_year_of_a_collection_is_read() {
        let readings = rereadings(
            "Saga 007",
            Some(2006),
            Some("Saga 007 - 2006 - Quiet Harbour Uncut - 1080p X264 Ac3 mHDgz.mkv"),
            NOW,
        );
        let after = readings
            .iter()
            .find(|reading| reading.title == "Quiet Harbour")
            .expect("read after the year");
        assert_eq!(after.year, Some(2006));
        assert!(after.year_must_agree);

        let squeezed = rereadings(
            "Saga 007-",
            Some(2021),
            Some("Saga 007- 2021 -Amber Coast FRENCH.720p.WEB.H264-GROUP.mkv"),
            NOW,
        );
        assert!(titles(&squeezed).contains(&"Amber Coast"), "{squeezed:?}");
    }
}
