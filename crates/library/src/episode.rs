//! Reading a season and an episode out of a file name and the folders above it.
//!
//! The film side of the house rests on one idea: the year is the boundary of a
//! name. An episode almost never carries a year, so the same idea is applied to
//! the thing an episode does carry: **the episode marker is the boundary**.
//! What comes before it names the series, the marker itself gives the season
//! and the number, and what comes after is the episode's own title followed by
//! the usual technical words.
//!
//! The words are prepared by the naming module and by nothing else, so a name
//! means the same thing whether it is read as a film or as an episode.
//!
//! Four arrangements exist on real disks and all four have to work: a folder
//! per series with a folder per season inside it, which is what most people do;
//! a folder per series with the episodes laid flat in it; everything in one
//! folder with nothing but the file names to tell them apart; and season
//! folders named in whatever language and shape their owner felt like, or
//! missing entirely for a series that only ever had one season.
//!
//! That is why the name is read first and the folder only ever confirms or
//! fills in. Reading the folder first works beautifully on a tidy collection
//! and collapses on the other three. The one exception is what a season folder
//! says: it is the single mark that tells a tidy collection apart from the
//! others, so where there is one, the folder holding it names the series and
//! groups everything under it, whatever each file calls itself.

use std::collections::BTreeSet;

use crate::naming::{self, LibrarySigns};

/// Highest number still read as a season.
///
/// A resolution is written exactly like a crossed pair, and `1920x1080` is not
/// the eighty eighth episode of the one thousand nine hundred and twentieth
/// season.
const HIGHEST_SEASON: i32 = 99;

/// How many digits a number in a marker may carry.
///
/// Four, because a season or an episode never needs more and a year does: it
/// keeps a bare year from being read as a number of something.
const LONGEST_NUMBER: usize = 4;

/// What a file name turned out to say about an episode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedEpisode {
    /// The series, as this name gave it.
    ///
    /// Empty when the name opened with the marker, which is what a file named
    /// inside a season folder does. The folder above is what names it then.
    pub series: String,
    /// The season, when the name said which one.
    ///
    /// Absent when the name carried only an episode number, and the folder is
    /// then the one to answer.
    pub season: Option<i32>,
    /// The first episode this file holds.
    pub first: i32,
    /// The last one, which differs only for a file holding several episodes.
    pub last: i32,
    /// The year the series carries in its own name, when it carries one.
    pub year: Option<i32>,
    /// The episode's own title, when the name carried one after the marker.
    pub title: Option<String>,
    /// Technical tags found after the title, lowercased. Hints only, exactly
    /// as they are for a film.
    pub tags: BTreeSet<String>,
}

impl ParsedEpisode {
    /// Whether this file stands for more than one episode.
    pub fn holds_several(&self) -> bool {
        self.last > self.first
    }
}

/// Reads a file name as an episode, or says it is not one.
///
/// `current_year` is passed in rather than read from the clock, so the same
/// name always reads the same way in a test.
pub fn parse_episode(
    file_name: &str,
    current_year: i32,
    signs: &LibrarySigns,
) -> Option<ParsedEpisode> {
    naming::with_the_words_of(file_name, current_year, signs, |words| {
        let marker = find_the_marker(words)?;

        let after = &words[marker.through..];
        let boundary = naming::first_technical_tag(after, 0).unwrap_or(after.len());
        let title = naming::title_of(
            naming::trim_leading_separators(&after[..boundary]),
            signs.marks(),
        );

        // The series stops at the first word that can only describe a file,
        // exactly as a film's title does. A name carrying one before the
        // marker would otherwise hand it to the series, and a series named
        // with a technical word in it groups apart from the same series
        // without one.
        let named = &words[..marker.at];
        let named = &named[..naming::first_technical_tag(named, 1).unwrap_or(named.len())];
        let (named, year) = the_series(named, current_year);

        Some(ParsedEpisode {
            series: naming::title_of(named, signs.marks()),
            year,
            season: marker.season,
            first: marker.first,
            last: marker.last,
            title: (!title.is_empty()).then_some(title),
            tags: naming::tags_from(&after[boundary..]),
        })
    })
}

/// The series part of a name, and the year it carried if it carried one.
///
/// A series written `Distant Signal 2019 S01E02` and the same one written
/// `Distant Signal S01E03` are one series, and grouping them by the name as
/// written would make two. The year is taken off so both land together, and
/// kept because it is worth knowing when the time comes to look the series up.
///
/// Never the whole of the name: a series really can be called `1883`, and one
/// called that has a name of one word which happens to read like a year.
fn the_series<'a>(words: &'a [&'a str], current_year: i32) -> (&'a [&'a str], Option<i32>) {
    let Some((last, rest)) = words.split_last() else {
        return (words, None);
    };
    if rest.is_empty() {
        return (words, None);
    }
    match naming::a_plausible_year(last, current_year) {
        Some(year) => (rest, Some(year)),
        None => (words, None),
    }
}

/// A series as something naming it gave it: a folder, or a file name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedSeries {
    pub title: String,
    /// The year written next to the name, when one was.
    pub year: Option<i32>,
}

/// How many digits a number standing on its own may carry to still be an
/// episode.
///
/// Three, one fewer than a marker allows, because a number of four digits at
/// the head of a name is a year and never an episode.
const LONGEST_NUMBER_ALONE: usize = 3;

/// Reads a file name that says nothing but a number, as the episode that
/// number stands for.
///
/// Only ever asked once a season folder above the file has said which season
/// this is, and so which series: a name opening on a number has one meaning
/// left by then, and a whole run of episodes taken off a disc is usually named
/// no other way. Asked anywhere else, the same name means nothing in
/// particular, which is why this is a question of its own rather than another
/// shape inside the marker.
pub fn episode_of_a_leading_number(
    file_name: &str,
    current_year: i32,
    signs: &LibrarySigns,
) -> Option<ParsedEpisode> {
    naming::with_the_words_of(file_name, current_year, signs, |words| {
        let (number, after) = naming::trim_leading_separators(words).split_first()?;
        let number = a_number_alone(plain(number).trim_end_matches(['-', '.']))?;

        let after = naming::trim_leading_separators(after);
        let boundary = naming::first_technical_tag(after, 0).unwrap_or(after.len());
        let title = naming::title_of(&after[..boundary], signs.marks());

        Some(ParsedEpisode {
            // Both left to the folders above, which is the only reason this
            // name could be read at all.
            series: String::new(),
            season: None,
            first: number,
            last: number,
            year: None,
            title: (!title.is_empty()).then_some(title),
            tags: naming::tags_from(&after[boundary..]),
        })
    })
}

/// A word that is a number short enough to stand for an episode on its own.
fn a_number_alone(word: &str) -> Option<i32> {
    (word.len() <= LONGEST_NUMBER_ALONE)
        .then(|| number_of(word))
        .flatten()
}

/// Reads a file name the way anime releases are named, or says it is not one.
///
/// Two shapes that carry no marker at all and that nearly every such release
/// uses: the number after a dash, `[Group] Series - 12 [1080p][ABCD1234]`, and
/// the number in brackets of its own, `[Group][Series][12][1080p]`. A season
/// written just before the dash is read with it: `Series S2 - 05`.
///
/// Only ever asked in a library of anime, once the ordinary reading found no
/// marker. Anywhere else a dash and a number are just as likely to be the
/// second film of something, which is why a bare number is refused there.
pub fn parse_anime_episode(
    file_name: &str,
    current_year: i32,
    signs: &LibrarySigns,
) -> Option<ParsedEpisode> {
    // Written with no space anywhere, a run of brackets is one long word with
    // nothing to find in it.
    let spaced = file_name.replace("][", "] [");
    naming::with_the_words_of(&spaced, current_year, signs, |words| {
        let (marker, bracketed) = match a_dashed_number(words, current_year) {
            Some(marker) => (marker, false),
            None => (a_bracketed_number(words, current_year)?, true),
        };

        // Such a name puts nothing after the number but its own title and
        // what describes the file, and the description always opens with a
        // bracket when no technical word opened it first.
        let after = naming::trim_leading_separators(&words[marker.through..]);
        let boundary = after
            .iter()
            .position(|word| word.starts_with(['[', '(', '{']))
            .into_iter()
            .chain(naming::first_technical_tag(after, 0))
            .min()
            .unwrap_or(after.len());
        let title = naming::title_of(&after[..boundary], signs.marks());

        // In the bracketed shape the series sits in brackets of its own, which
        // would otherwise read as an aside and be dropped from its own name.
        let named: Vec<&str> = words[..marker.at]
            .iter()
            .map(|word| if bracketed { naming::bare(word) } else { word })
            .collect();
        let named = &named[..naming::first_technical_tag(&named, 1).unwrap_or(named.len())];
        let (named, year) = the_series(named, current_year);

        Some(ParsedEpisode {
            series: naming::title_of(named, signs.marks()),
            year,
            season: marker.season,
            first: marker.first,
            last: marker.last,
            title: (!title.is_empty()).then_some(title),
            tags: naming::tags_from(&after[boundary..]),
        })
    })
}

/// `Series - 12`, with the season written just before the dash if it was.
fn a_dashed_number(words: &[&str], current_year: i32) -> Option<Marker> {
    (1..words.len()).find_map(|dash| {
        if !is_a_dash(words[dash]) {
            return None;
        }
        let (number, through) = word_after(words, dash)?;
        let first = an_anime_number(&plain(number), current_year)?;
        let (at, season) = a_season_just_before(words, dash).unwrap_or((dash, None));
        Some(Marker {
            at,
            through,
            season,
            first,
            last: first,
        })
    })
}

/// `[Series][12]`: a number standing in brackets of its own.
fn a_bracketed_number(words: &[&str], current_year: i32) -> Option<Marker> {
    (0..words.len()).find_map(|at| {
        let inside = words[at].strip_prefix('[')?.strip_suffix(']')?;
        let first = an_anime_number(&inside.to_lowercase(), current_year)?;
        Some(Marker {
            at,
            through: at + 1,
            season: None,
            first,
            last: first,
        })
    })
}

/// A word that is nothing but a dash.
fn is_a_dash(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| matches!(c, '-' | '\u{2013}'))
}

/// The number of an anime episode.
///
/// Four digits, because a long series runs past a thousand episodes, but never
/// a year: `Series - 2019` is the year of something. A release put out again
/// after a fix says so with a `v` and a digit, `12v2`, and is the same episode.
fn an_anime_number(word: &str, current_year: i32) -> Option<i32> {
    if naming::a_plausible_year(word, current_year).is_some() {
        return None;
    }
    let (number, rest) = digits_at(word)?;
    let a_version = |rest: &str| {
        rest.strip_prefix('v').is_some_and(|version| {
            !version.is_empty() && version.chars().all(|c| c.is_ascii_digit())
        })
    };
    (rest.is_empty() || a_version(rest)).then_some(number)
}

/// A season written just before the dash, and the first word it takes.
///
/// `S2`, `Season 2` in either language, and the `2nd Season` a release often
/// carries. A number on its own there is never read as one: `Series 100` is
/// far more often the name of the series.
fn a_season_just_before(words: &[&str], dash: usize) -> Option<(usize, Option<i32>)> {
    let last = plain(words.get(dash.checked_sub(1)?)?);
    if let Some(Mark::Season(season)) = a_season_alone(&last) {
        return Some((dash - 1, Some(season)));
    }
    let before = plain(words.get(dash.checked_sub(2)?)?);
    if is_a_season_word(&before) {
        return Some((dash - 2, Some(number_of(&last)?)));
    }
    if is_a_season_word(&last) {
        return Some((dash - 2, Some(an_ordinal(&before)?)));
    }
    None
}

/// `2nd`, and every other way English writes a rank.
fn an_ordinal(word: &str) -> Option<i32> {
    let (number, rest) = digits_at(word)?;
    matches!(rest, "st" | "nd" | "rd" | "th").then_some(number)
}

/// Reads a folder name as the name of a series, or says it names none.
///
/// Read exactly as a file name is read, because a folder is named the way a
/// file is: separators of every shape, a year at the end, and the technical
/// words a release sticks on. A folder carrying an episode marker names one
/// episode rather than a series, so its name stops at the marker, the same way
/// a file name does.
///
/// Answers nothing for a season folder, which never names anything, and
/// nothing for a name that has no words left once all that is taken off.
pub fn series_of_folder(
    folder: &str,
    current_year: i32,
    signs: &LibrarySigns,
) -> Option<NamedSeries> {
    if season_of_folder(folder).is_some() {
        return None;
    }
    naming::with_the_words_of_a_folder(folder, current_year, signs, |words| {
        let named = match find_the_marker(words) {
            Some(marker) => &words[..marker.at],
            None => words,
        };
        let named = &named[..naming::first_technical_tag(named, 1).unwrap_or(named.len())];
        let (named, year) = the_series(named, current_year);
        let title = naming::title_of(named, signs.marks());
        (!title.is_empty()).then_some(NamedSeries { title, year })
    })
}

/// The season a folder says it holds.
///
/// Answers for the shapes people really use, in both languages, plus the one
/// that stands for everything that belongs to no season: a special, which is
/// season zero by long convention and is where every other server puts them.
///
/// A folder that says nothing recognisable answers nothing, and the caller is
/// then left with whatever the file name gave it. Guessing a season out of a
/// folder named something else is how a series ends up with a season nobody
/// ever wrote.
pub fn season_of_folder(folder: &str) -> Option<i32> {
    let folded = naming::fold_accents(folder).to_lowercase();
    let normalised = folded.replace(['.', '_'], " ");
    let words: Vec<&str> = normalised.split_whitespace().collect();
    let (first, rest) = words.split_first()?;

    if rest.is_empty() {
        if is_a_special(first) {
            return Some(0);
        }
        // `S01` on its own, the shape somebody types when they are in a hurry.
        return match read_a_mark(first) {
            Some(Mark::Season(season)) => Some(season),
            _ => None,
        };
    }

    // `Saison 1`, `Season 01`, and the `Series 1` the British write. That last
    // word is only ever read here, never in a file name: a file called
    // `Series 1 ...` is far more likely to be a series actually called that.
    (is_a_season_word(first) || *first == "series").then(|| number_of(rest[0]))?
}

/// Whether a folder holds what belongs to no season.
fn is_a_special(word: &str) -> bool {
    matches!(word, "specials" | "special" | "speciaux" | "hors")
}

/// Where the marker sits in a name, and what it says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Marker {
    /// First word of the marker: everything before it names the series.
    at: usize,
    /// First word after the marker: everything from there is the episode's.
    through: usize,
    season: Option<i32>,
    first: i32,
    last: i32,
}

/// What one word of a name turned out to be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// A season and an episode in one word: `S01E02`, `1x02`.
    Both { season: i32, first: i32, last: i32 },
    /// A season with nothing after it, waiting for the next word.
    Season(i32),
    /// An episode with no season said anywhere: `E02`.
    Episode { first: i32, last: i32 },
}

/// Finds the marker, strongest shape first.
///
/// The order is the whole point. A name carrying both an aspect ratio and a
/// proper marker, `Series 16x9 S01E02`, has to be read on the marker; asked
/// for the first thing shaped like a crossed pair it would come back as the
/// ninth episode of the sixteenth season. So every shape that names its parts
/// is looked for across the whole name before any shape that does not.
fn find_the_marker(words: &[&str]) -> Option<Marker> {
    a_named_pair(words)
        .or_else(|| a_crossed_pair_somewhere(words))
        .or_else(|| an_episode_on_its_own(words))
}

/// A season and an episode that both say which they are.
///
/// Three shapes, all naming their parts: welded into one word, split across
/// two, or spelled out in full with the numbers between the words.
fn a_named_pair(words: &[&str]) -> Option<Marker> {
    (0..words.len()).find_map(|at| {
        if let Some(Mark::Both {
            season,
            first,
            last,
        }) = a_season_and_an_episode(&plain(words[at]))
        {
            return Some(Marker {
                at,
                through: at + 1,
                season: Some(season),
                first,
                last,
            });
        }

        let Some(Mark::Season(season)) = a_season_alone(&plain(words[at])) else {
            return spelled_out(words, at);
        };
        let (next, after) = word_after(words, at)?;
        let Some(Mark::Episode { first, last }) = an_episode_alone(&plain(next)) else {
            return None;
        };
        Some(Marker {
            at,
            through: after,
            season: Some(season),
            first,
            last,
        })
    })
}

/// `Season 1 Episode 2`, in either language, however it is punctuated.
fn spelled_out(words: &[&str], at: usize) -> Option<Marker> {
    if !is_a_season_word(&plain(words[at])) {
        return None;
    }
    let (season, after) = word_after(words, at)?;
    let season = number_of(&plain(season))?;
    let (word, after) = word_after(words, after - 1)?;
    if !is_an_episode_word(&plain(word)) {
        return None;
    }
    let (episode, after) = word_after(words, after - 1)?;
    let first = number_of(&plain(episode))?;
    Some(Marker {
        at,
        through: after,
        season: Some(season),
        first,
        last: first,
    })
}

/// `1x02`, once nothing better has been found anywhere in the name.
fn a_crossed_pair_somewhere(words: &[&str]) -> Option<Marker> {
    (0..words.len()).find_map(|at| {
        let Some(Mark::Both {
            season,
            first,
            last,
        }) = a_crossed_pair(&plain(words[at]))
        else {
            return None;
        };
        Some(Marker {
            at,
            through: at + 1,
            season: Some(season),
            first,
            last,
        })
    })
}

/// An episode number with no season beside it, which the folder will answer for.
fn an_episode_on_its_own(words: &[&str]) -> Option<Marker> {
    (0..words.len()).find_map(|at| {
        if let Some(Mark::Episode { first, last }) = an_episode_alone(&plain(words[at])) {
            return Some(Marker {
                at,
                through: at + 1,
                season: None,
                first,
                last,
            });
        }

        // `Episode 2`, spelled out with the number beside it.
        if !is_an_episode_word(&plain(words[at])) {
            return None;
        }
        let (episode, after) = word_after(words, at)?;
        let first = number_of(&plain(episode))?;
        Some(Marker {
            at,
            through: after,
            season: None,
            first,
            last: first,
        })
    })
}

/// The next word that says something, and where the one after it starts.
///
/// Separators are stepped over rather than dropped, because a dash inside an
/// episode's own title belongs to it and only the ones around the marker do
/// not.
fn word_after<'a>(words: &[&'a str], at: usize) -> Option<(&'a str, usize)> {
    words
        .iter()
        .enumerate()
        .skip(at + 1)
        .find(|(_, word)| !is_only_a_separator(word))
        .map(|(index, word)| (*word, index + 1))
}

/// Whether a word is nothing but the punctuation somebody put between parts.
fn is_only_a_separator(word: &str) -> bool {
    !word.is_empty() && !word.chars().any(|c| c.is_alphanumeric())
}

/// A word stripped of its brackets and lowercased, ready to be recognised.
fn plain(word: &str) -> String {
    naming::bare(word).to_lowercase()
}

fn is_a_season_word(word: &str) -> bool {
    matches!(naming::fold_accents(word).as_str(), "season" | "saison")
}

fn is_an_episode_word(word: &str) -> bool {
    matches!(
        naming::fold_accents(word).as_str(),
        "episode" | "episodes" | "ep"
    )
}

/// A word that is only a number, small enough to be one of ours.
fn number_of(word: &str) -> Option<i32> {
    let (value, rest) = digits_at(word)?;
    rest.is_empty().then_some(value)
}

/// Reads a run of digits off the front, and gives back what follows.
fn digits_at(text: &str) -> Option<(i32, &str)> {
    let end = text
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(text.len());
    if end == 0 || end > LONGEST_NUMBER {
        return None;
    }
    text[..end].parse().ok().map(|value| (value, &text[end..]))
}

/// What one word says, whatever shape it says it in.
fn read_a_mark(word: &str) -> Option<Mark> {
    a_season_and_an_episode(word)
        .or_else(|| a_crossed_pair(word))
        .or_else(|| an_episode_alone(word))
        .or_else(|| a_season_alone(word))
}

/// `S01E02`, and every way of writing an episode that spans several.
fn a_season_and_an_episode(word: &str) -> Option<Mark> {
    let rest = word.strip_prefix('s')?;
    let (season, rest) = digits_at(rest)?;
    let rest = rest.strip_prefix('-').unwrap_or(rest);
    let rest = rest.strip_prefix('e')?;
    let (first, rest) = digits_at(rest)?;
    let last = a_range_ending(rest, first)?;
    Some(Mark::Both {
        season,
        first,
        last,
    })
}

/// `1x02`.
///
/// The episode must be written with at least two digits, which is what tells
/// this shape apart from an aspect ratio. Everybody who numbers this way pads
/// the episode, `1x02` and `2x11`; nobody writes a screen as `16x09`. A real
/// `1x2` is refused along with them, and a file nobody could number stays
/// visible and correctable rather than being filed under the wrong season.
fn a_crossed_pair(word: &str) -> Option<Mark> {
    let (season, rest) = digits_at(word)?;
    if season > HIGHEST_SEASON {
        return None;
    }
    let rest = rest.strip_prefix('x')?;
    if rest.len() < 2 || !rest.starts_with(|c: char| c.is_ascii_digit()) {
        return None;
    }
    let (first, rest) = digits_at(rest)?;
    let last = a_range_ending(rest, first)?;
    Some(Mark::Both {
        season,
        first,
        last,
    })
}

/// `E02`, or `Ep02`, with no season anywhere near it.
fn an_episode_alone(word: &str) -> Option<Mark> {
    let rest = word.strip_prefix("ep").or_else(|| word.strip_prefix('e'))?;
    let (first, rest) = digits_at(rest)?;
    let last = a_range_ending(rest, first)?;
    Some(Mark::Episode { first, last })
}

/// `S01`, with the episode expected in the word after it.
fn a_season_alone(word: &str) -> Option<Mark> {
    let rest = word.strip_prefix('s')?;
    let (season, rest) = digits_at(rest)?;
    rest.is_empty().then_some(Mark::Season(season))
}

/// The second half of a file holding several episodes, if it holds several.
///
/// `S01E01E02`, `S01E01-E02` and `S01E01-02` all say the same thing. Nothing
/// at all says the file holds the one episode. Anything carrying another digit
/// means the word was never a marker, so it is refused whole rather than read
/// up to the part that stopped making sense.
fn a_range_ending(rest: &str, first: i32) -> Option<i32> {
    if rest.is_empty() {
        return Some(first);
    }
    if let Some(last) = a_second_number(rest, first) {
        return Some(last);
    }
    // What is left stuck on the end of the marker without a digit is a slip of
    // somebody's keyboard and not a number: `S10E04n` is still the fourth
    // episode of the tenth season, and `05x05x-` the fifth of the fifth.
    // Refusing the whole marker over it left the file belonging to no season
    // at all. Anything carrying another digit is still refused: a number
    // nobody can explain is exactly what must never be guessed at.
    (!rest.chars().any(|c| c.is_ascii_digit())).then_some(first)
}

/// The second number of a file holding several episodes.
fn a_second_number(rest: &str, first: i32) -> Option<i32> {
    let rest = rest.strip_prefix('-').unwrap_or(rest);
    let rest = rest.strip_prefix('e').unwrap_or(rest);
    let (last, rest) = digits_at(rest)?;
    (rest.is_empty() && last >= first).then_some(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The year every test reads names against, so a name always reads the
    /// same way however long this code lives.
    const THIS_YEAR: i32 = 2026;

    /// Reads a name the way a library that knows nothing about itself would.
    fn read(file_name: &str) -> Option<ParsedEpisode> {
        parse_episode(file_name, THIS_YEAR, &LibrarySigns::default())
    }

    /// What a name said, in the short form the tests below compare against:
    /// series, season, first episode, last episode, episode title.
    fn said(file_name: &str) -> (String, Option<i32>, i32, i32, Option<String>) {
        let read = read(file_name).expect("this name says which episode it is");
        (read.series, read.season, read.first, read.last, read.title)
    }

    #[test]
    fn the_tidy_shape_is_read_whole() {
        // A folder per series, a folder per season, and the episode named
        // after both. What most people do and what every other server asks
        // for, so it is the shape that has to be right before any other.
        assert_eq!(
            said("Distant Signal - S01E02 - The Long Night.mkv"),
            (
                "Distant Signal".to_string(),
                Some(1),
                2,
                2,
                Some("The Long Night".to_string())
            )
        );
    }

    #[test]
    fn a_name_written_by_a_tool_is_read_the_same() {
        // Dots for spaces and a tail of technical words, which is what comes
        // off a download rather than off a keyboard.
        assert_eq!(
            said("Distant.Signal.S01E02.1080p.WEB-DL.x264-GROUP.mkv"),
            ("Distant Signal".to_string(), Some(1), 2, 2, None)
        );
    }

    #[test]
    fn the_marker_is_read_however_it_is_punctuated() {
        // The same episode, written the six ways it gets written.
        for name in [
            "Distant Signal S01E02.mkv",
            "Distant Signal s01e02.mkv",
            "Distant Signal S1E2.mkv",
            "Distant.Signal.S01.E02.mkv",
            "Distant Signal S01 E02.mkv",
            "Distant Signal S01-E02.mkv",
        ] {
            assert_eq!(
                said(name),
                ("Distant Signal".to_string(), Some(1), 2, 2, None),
                "{name}"
            );
        }
    }

    #[test]
    fn the_crossed_pair_is_read() {
        assert_eq!(
            said("Distant Signal 1x02.mkv"),
            ("Distant Signal".to_string(), Some(1), 2, 2, None)
        );
        assert_eq!(
            said("Distant Signal - 2x11 - The Long Night.mkv"),
            (
                "Distant Signal".to_string(),
                Some(2),
                11,
                11,
                Some("The Long Night".to_string())
            )
        );
    }

    #[test]
    fn the_marker_spelled_out_is_read_in_both_languages() {
        for name in [
            "Distant Signal Season 1 Episode 2.mkv",
            "Distant Signal Saison 1 Episode 2.mkv",
            "Distant Signal Saison 1 Épisode 2.mkv",
            "Distant Signal - Season 1 - Episode 2.mkv",
        ] {
            assert_eq!(
                said(name),
                ("Distant Signal".to_string(), Some(1), 2, 2, None),
                "{name}"
            );
        }
    }

    #[test]
    fn a_file_holding_several_episodes_says_so() {
        for name in [
            "Distant Signal S01E01-E02.mkv",
            "Distant Signal S01E01E02.mkv",
            "Distant Signal S01E01-02.mkv",
        ] {
            let read = read(name).expect("read");
            assert_eq!((read.first, read.last), (1, 2), "{name}");
            assert!(read.holds_several(), "{name}");
        }
        assert!(!read("Distant Signal S01E01.mkv")
            .expect("read")
            .holds_several());
    }

    #[test]
    fn a_letter_stuck_on_the_end_of_the_marker_is_not_a_number() {
        // Seen on a real collection: one file of a season of twenty four,
        // named with a letter left over. Refusing the marker over it filed
        // that episode under no season at all, which is the one thing worse
        // than filing it wrongly.
        assert_eq!(
            said("Distant Signal_S10E04n.mkv"),
            ("Distant Signal".to_string(), Some(10), 4, 4, None)
        );
        let slipped = read("5x05x- Waiting Room.avi").expect("read");
        assert_eq!(
            (slipped.season, slipped.first, slipped.last),
            (Some(5), 5, 5)
        );
        assert_eq!(slipped.title.as_deref(), Some("Waiting Room"));
        // What carries another number is still refused: a number nobody can
        // explain is what must never be guessed at.
        assert!(read("Distant Signal S01E02x264.mkv").is_none());
    }

    #[test]
    fn an_episode_number_with_no_season_leaves_the_season_to_the_folder() {
        // What a file named inside a season folder looks like: the season is
        // written on the folder and nowhere else.
        for name in [
            "Distant Signal - E02 - The Long Night.mkv",
            "Distant Signal Ep02.mkv",
            "Distant Signal Episode 2.mkv",
            "Distant Signal Épisode 2.mkv",
        ] {
            let read = read(name).expect("read");
            assert_eq!(read.season, None, "{name}");
            assert_eq!(read.first, 2, "{name}");
        }
    }

    #[test]
    fn a_name_that_is_only_a_marker_leaves_the_series_to_the_folder() {
        // Everything this name says is the number. The folders above it are
        // what name the series, and that is step three's business.
        assert_eq!(said("S01E02.mkv"), (String::new(), Some(1), 2, 2, None));
        assert_eq!(
            said("E02 - The Long Night.mkv"),
            (
                String::new(),
                None,
                2,
                2,
                Some("The Long Night".to_string())
            )
        );
    }

    #[test]
    fn a_film_is_not_an_episode() {
        // The whole point of answering nothing: these go on being read as
        // films, and a library of films never asks this question at all.
        for name in [
            "Quiet Harbour 2019 1080p BluRay x264.mkv",
            "Quiet Harbour (2019).mkv",
            "2 Fast 2 Furious.mkv",
            "Quiet Harbour.mkv",
        ] {
            assert_eq!(read(name), None, "{name}");
        }
    }

    #[test]
    fn a_measurement_is_never_read_as_a_season() {
        // A crossed pair is also how a screen and a picture shape are written,
        // and reading one as an episode files the film under a season nobody
        // ever wrote. The name has to say nothing at all instead.
        for name in [
            "Quiet Harbour 1920x1080.mkv",
            "Quiet Harbour 16x9.mkv",
            "Quiet Harbour 4x3.mkv",
            "Quiet Harbour x264.mkv",
            "Quiet Harbour 2xAAC.mkv",
        ] {
            assert_eq!(read(name), None, "{name}");
        }
    }

    #[test]
    fn a_marker_is_preferred_to_anything_shaped_like_one() {
        // Both are in the name and only one of them is the episode. Asked for
        // whatever comes first, this reads as the ninth of season sixteen.
        let read = read("Distant Signal 16x9 S02E05.mkv").expect("read");
        assert_eq!((read.season, read.first), (Some(2), 5));
    }

    #[test]
    fn the_series_stops_where_the_technical_words_start() {
        // The same series written with a technical word before the marker and
        // without has to come out under one name, or it becomes two series.
        assert_eq!(
            read("Distant Signal 1080p S02E05.mkv")
                .expect("read")
                .series,
            read("Distant Signal S02E06.mkv").expect("read").series
        );
    }

    #[test]
    fn a_bare_number_is_never_a_marker() {
        // `102` for season one episode two is a real habit and an unreadable
        // one: it is also a year, a resolution and a piece of a title. A file
        // nobody could number stays visible and gets corrected, which is what
        // saying nothing here brings about.
        for name in [
            "Distant Signal 102.mkv",
            "Distant Signal - 02.mkv",
            "[Group] Distant Signal - 02 [1080p].mkv",
        ] {
            assert_eq!(read(name), None, "{name}");
        }
    }

    #[test]
    fn a_series_named_with_numbers_keeps_its_name() {
        assert_eq!(
            said("24 S01E01.mkv"),
            ("24".to_string(), Some(1), 1, 1, None)
        );
        // A name of one word that reads like a year is the name, not a year.
        let read = read("1883 S01E01.mkv").expect("read");
        assert_eq!(read.series, "1883");
        assert_eq!(read.year, None);
    }

    #[test]
    fn the_year_of_a_series_is_taken_out_of_its_name() {
        // Half the files of one series carrying the year and half not is how
        // one series becomes two, so the name has to come out the same way
        // whichever half it came from.
        let with = read("Distant Signal 2019 S01E02.mkv").expect("read");
        let without = read("Distant Signal S01E03.mkv").expect("read");
        assert_eq!(with.series, without.series);
        assert_eq!(with.year, Some(2019));
        assert_eq!(without.year, None);

        // Written in brackets it was already being dropped, and still is.
        assert_eq!(
            read("Distant Signal (2019) S01E04.mkv")
                .expect("read")
                .series,
            without.series
        );
    }

    #[test]
    fn the_episode_title_stops_where_the_technical_words_start() {
        assert_eq!(
            said("Distant Signal S01E02 The Long Night 1080p x264.mkv"),
            (
                "Distant Signal".to_string(),
                Some(1),
                2,
                2,
                Some("The Long Night".to_string())
            )
        );
        // Nothing but technical words after the marker is no title at all,
        // rather than a title made of them.
        assert_eq!(
            read("Distant Signal S01E02 1080p.mkv").expect("read").title,
            None
        );
    }

    #[test]
    fn a_dash_inside_an_episode_title_belongs_to_it() {
        // The dashes around the marker are punctuation somebody typed. The one
        // in the middle of a title is part of the title.
        assert_eq!(
            read("Distant Signal - S02E10 - Part 1 - The End.mkv")
                .expect("read")
                .title,
            Some("Part 1 - The End".to_string())
        );
    }

    #[test]
    fn the_technical_words_are_kept_as_hints() {
        let read = read("Distant.Signal.S01E02.1080p.x264.mkv").expect("read");
        assert!(read.tags.contains("1080p"), "{:?}", read.tags);
        assert!(read.tags.contains("x264"), "{:?}", read.tags);
    }

    #[test]
    fn a_season_folder_says_which_season_it_holds() {
        for (folder, season) in [
            ("Saison 1", 1),
            ("Saison 01", 1),
            ("Season 1", 1),
            ("Season 01", 1),
            ("SEASON 2", 2),
            ("S01", 1),
            ("S1", 1),
            ("s02", 2),
            ("Series 1", 1),
            ("Saison 10", 10),
        ] {
            assert_eq!(season_of_folder(folder), Some(season), "{folder}");
        }
    }

    #[test]
    fn what_belongs_to_no_season_is_season_zero() {
        // Where every other server puts them, so a collection moved from one
        // lands where its owner expects.
        for folder in ["Specials", "specials", "Spéciaux", "Saison 0", "Season 00"] {
            assert_eq!(season_of_folder(folder), Some(0), "{folder}");
        }
    }

    #[test]
    fn a_folder_that_says_nothing_is_not_guessed_at() {
        // Guessing a season out of a folder named something else is how a
        // series ends up with a season nobody ever wrote.
        for folder in [
            "Distant Signal",
            "Season",
            "Saison",
            "Films",
            "Sous-titres",
            "Extras",
            "S",
            "",
        ] {
            assert_eq!(season_of_folder(folder), None, "{folder}");
        }
    }

    /// Reads a name that carries nothing but a number, as a season folder
    /// above it lets the scan do.
    fn numbered(name: &str) -> Option<(i32, i32, Option<String>)> {
        episode_of_a_leading_number(name, THIS_YEAR, &LibrarySigns::default())
            .map(|read| (read.first, read.last, read.title))
    }

    #[test]
    fn a_number_in_front_is_the_episode_and_the_rest_is_its_title() {
        for (name, title) in [
            ("01 - The Amber Field.mkv", "The Amber Field"),
            ("01. The Amber Field.mkv", "The Amber Field"),
            ("01 The Amber Field.mkv", "The Amber Field"),
            // The technical words are taken off the title here as everywhere.
            ("01 - The Amber Field 1080p.mkv", "The Amber Field"),
            // The separator glued to the number rather than standing apart.
            ("01- The Amber Field.mkv", "The Amber Field"),
        ] {
            assert_eq!(
                numbered(name),
                Some((1, 1, Some(title.to_string()))),
                "{name}"
            );
        }
    }

    #[test]
    fn a_number_alone_is_an_episode_with_no_title_of_its_own() {
        assert_eq!(numbered("07.mkv"), Some((7, 7, None)));
        assert_eq!(numbered("052.mkv"), Some((52, 52, None)));
    }

    #[test]
    fn a_number_too_long_to_be_an_episode_is_not_read_as_one() {
        // Four digits are a year, which is the one number that turns up in
        // front of a name and means something else entirely.
        assert_eq!(numbered("2019 Lost Footage.mkv"), None);
        assert_eq!(numbered("2019.mkv"), None);
    }

    #[test]
    fn a_name_that_does_not_open_on_a_number_says_nothing_here() {
        for name in [
            "The Amber Field.mkv",
            "Distant Signal S01E01.mkv",
            "Part 1 The Amber Field.mkv",
            // A number welded to the word after it is one word and not a
            // number, here as everywhere else in this module.
            "1-The.Amber.Field.mkv",
            "",
        ] {
            assert_eq!(numbered(name), None, "{name}");
        }
    }

    /// What an anime name said: series, season, episode, episode title.
    fn anime(name: &str) -> Option<(String, Option<i32>, i32, Option<String>)> {
        parse_anime_episode(name, THIS_YEAR, &LibrarySigns::default())
            .map(|read| (read.series, read.season, read.first, read.title))
    }

    #[test]
    fn the_number_after_a_dash_is_the_episode() {
        for name in [
            "Amber Field - 12.mkv",
            "[Group] Amber Field - 12 [1080p].mkv",
            "[Group] Amber Field - 12 (1080p) [A1B2C3D4].mkv",
            "[Group]_Amber_Field_-_12_[BD_1080p][A1B2C3D4].mkv",
            "Amber Field - 012.mkv",
            // A release put out again after a fix is the same episode.
            "[Group] Amber Field - 12v2 [1080p].mkv",
        ] {
            assert_eq!(
                anime(name),
                Some(("Amber Field".to_string(), None, 12, None)),
                "{name}"
            );
        }
    }

    #[test]
    fn a_long_series_runs_past_a_thousand_but_never_into_a_year() {
        assert_eq!(
            anime("Amber Field - 1071.mkv").map(|read| read.2),
            Some(1071)
        );
        assert_eq!(anime("Amber Field - 2019.mkv"), None);
    }

    #[test]
    fn the_number_in_brackets_of_its_own_is_the_episode() {
        assert_eq!(
            anime("[Group][Amber Field][07][1080p][A1B2C3D4].mkv"),
            Some(("Amber Field".to_string(), None, 7, None))
        );
    }

    #[test]
    fn a_season_written_before_the_dash_is_read_with_it() {
        for name in [
            "[Group] Amber Field S2 - 05 [1080p].mkv",
            "Amber Field Season 2 - 05.mkv",
            "Amber Field Saison 2 - 05.mkv",
            "Amber Field 2nd Season - 05.mkv",
        ] {
            assert_eq!(
                anime(name),
                Some(("Amber Field".to_string(), Some(2), 5, None)),
                "{name}"
            );
        }
    }

    #[test]
    fn what_follows_the_number_is_the_episode_title_up_to_the_description() {
        assert_eq!(
            anime("[Group] Amber Field - 03 - The Long Night [1080p].mkv"),
            Some((
                "Amber Field".to_string(),
                None,
                3,
                Some("The Long Night".to_string())
            ))
        );
    }

    #[test]
    fn a_number_glued_to_the_name_is_the_name() {
        // Far more often the name of the series than one of its episodes.
        assert_eq!(anime("Signal 100.mkv"), None);
        // With a dash after it, the dash is what says which is which.
        assert_eq!(
            anime("Signal 100 - 04.mkv"),
            Some(("Signal 100".to_string(), None, 4, None))
        );
        // And a number that is only a description is no episode at all.
        assert_eq!(anime("Amber Field - 1080p.mkv"), None);
    }

    /// Reads a folder the way a library that knows nothing about itself would.
    fn folder(name: &str) -> Option<(String, Option<i32>)> {
        series_of_folder(name, THIS_YEAR, &LibrarySigns::default())
            .map(|named| (named.title, named.year))
    }

    #[test]
    fn a_folder_of_a_series_is_read_as_a_name_is_read() {
        for (name, title) in [
            ("Distant Signal", "Distant Signal"),
            ("Distant.Signal", "Distant Signal"),
            ("Distant_Signal", "Distant Signal"),
            // What looks like an extension at the end of a file name is the
            // end of the title at the end of a folder's.
            ("Mr.Vane", "Mr Vane"),
            // The technical words a release sticks on the end of everything.
            ("Distant Signal S01-S03 MULTi 1080p", "Distant Signal"),
        ] {
            assert_eq!(folder(name), Some((title.to_string(), None)), "{name}");
        }
    }

    #[test]
    fn a_folder_gives_up_the_year_it_carries() {
        for name in ["Distant Signal 2019", "Distant Signal (2019)"] {
            assert_eq!(folder(name), Some(("Distant Signal".into(), Some(2019))));
        }
        // A series really can be called by a number that reads like a year,
        // and a name of one word is never the year of something else.
        assert_eq!(folder("1923"), Some(("1923".into(), None)));
    }

    #[test]
    fn a_folder_holding_one_episode_names_the_series_and_not_the_episode() {
        // What a release puts around a single file: the folder carries the
        // whole name of the file inside it, marker and all.
        assert_eq!(
            folder("Distant.Signal.S01E07.MULTi.2160p.WEB.H265-TEAM"),
            Some(("Distant Signal".into(), None))
        );
    }

    #[test]
    fn a_folder_saying_it_holds_the_whole_series_still_names_the_series() {
        // The shape a whole series arrives in, where the word saying so is
        // written with its accent. Left on the end it is handed to a provider
        // as part of the name, and the provider answers nothing at all.
        for name in [
            "Distant Signal Int\u{e9}grale 1080p x265 BluRay Multi Truefrench",
            "Distant Signal Integrale 1080p x265 BluRay Multi Truefrench",
            "Distant.Signal.iNT\u{c9}GRALE.MULTi.1080p",
        ] {
            assert_eq!(
                folder(name),
                Some(("Distant Signal".into(), None)),
                "{name}"
            );
        }
    }

    #[test]
    fn a_folder_that_names_no_series_names_nothing() {
        // A season folder says which season it holds and never which series,
        // and a name with nothing left in it names nothing at all.
        for name in ["Saison 1", "Season 01", "Specials", "S02", ""] {
            assert_eq!(folder(name), None, "{name}");
        }
    }
}
