//! Reading a title and a year out of a file name.
//!
//! The maintainer's films sit flat in their folder, with no folder per film, so
//! the name is the only source of information. The rules below were drawn from
//! his real collection; the collection itself never appears here, and the test
//! set uses invented titles covering the same shapes.
//!
//! The whole approach rests on one idea: **the year is the boundary**.
//! Everything before it is the title, everything after is discarded. That is
//! far more robust than trying to recognise every technical tag, because the
//! list of tags is endless and grows, whereas a four digit year is a year.
//!
//! For a name carrying no year at all, which would otherwise hand a provider
//! the file name whole, the title stops at the first word that can only
//! describe a file. And whatever the name, two things are trimmed off the end
//! of the title: the word naming which cut of the film this is, and a marker
//! shouting in capitals at the end of a title that does not.
//!
//! Three things are taken off the front before any of that, because they stand
//! where the title is expected and a provider asked about them finds nothing:
//! the address of the site a file was handed out from, the name of a group in
//! brackets of its own, and a year written before the title rather than after
//! it. That last one is the only shape where the boundary sits the other way
//! round, and it says so by being in brackets: no film is called `(2019)`.
//!
//! Two more things are dropped, and neither is written down here: they are
//! read off the library itself. The word its owner signs names with, because a
//! signature repeats and a title does not. And whatever was stuck to the front
//! of a name that is otherwise, in full, another name of the same library,
//! because the copy without it is the proof that it was added.

use std::collections::BTreeSet;

/// Earliest year treated as a release year. Films exist from the 1890s, but a
/// number that small in a file name is almost always something else.
const EARLIEST_YEAR: i32 = 1900;

/// How far ahead a year is still plausible, to allow for announced titles.
const YEARS_AHEAD: i32 = 2;

/// What a file name turned out to say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedName {
    /// Title as read, with separators turned back into spaces.
    pub title: String,
    pub year: Option<i32>,
    /// Technical tags found after the year, lowercased.
    ///
    /// Kept as hints only. They say what the file claims to be; the analysis
    /// of the file itself says what it is, and the two disagree often enough
    /// that the claim must never win.
    pub tags: BTreeSet<String>,
}

/// Reads a file name.
///
/// `current_year` is passed in rather than read from the clock, so the same
/// name always parses the same way in a test.
pub fn parse(file_name: &str, current_year: i32) -> ParsedName {
    parse_signed(file_name, current_year, &LibrarySigns::default())
}

/// Reads a file name, knowing what the names of this library carry.
///
/// `signs` comes from `signs_in`, which reads them off the library rather than
/// from any list: what is dropped here is what those names carry and no title
/// ever does.
pub fn parse_signed(file_name: &str, current_year: i32, signs: &LibrarySigns) -> ParsedName {
    let file_name = without_the_site_in_front(signs.without_the_glued_prefix(file_name));
    let markers = &signs.marks;
    let stem = strip_extension(file_name);
    // The underscore separates words just like the dot does. Personal markers
    // attach themselves to the previous tag with one, and without this rule
    // the tag becomes unrecognisable and can land in the title.
    let normalised = stem.replace(['.', '_'], " ");
    let separated: Vec<&str> = normalised.split_whitespace().collect();
    let whole = separate_a_year_from_what_is_glued_to_it(&separated, current_year);
    let words = drop_what_is_bracketed_in_front(&whole);

    // A year written in front of the title says so plainly, so the rest of the
    // name is read as one that carries no year at all. Reading it for a year
    // as well would let a title carrying a number of its own outrank the date
    // somebody took the trouble to write down.
    if let Some((year, rest)) = a_year_written_in_front(words, current_year) {
        let boundary = first_technical_tag(rest).unwrap_or(rest.len());
        return ParsedName {
            title: title_of(&rest[..boundary], markers),
            year: Some(year),
            tags: tags_from(&rest[boundary..]),
        };
    }

    if let Some(position) = find_year(words, current_year) {
        return ParsedName {
            title: title_of(&words[..position], markers),
            year: bare(words[position]).parse().ok(),
            tags: tags_from(&words[position + 1..]),
        };
    }

    // No year to cut at. The name still has to give up a title, and a name
    // with nothing technical in it at all would have been found by now, so
    // what is left is the two shapes a year would have handled.
    let boundary = first_technical_tag(words).unwrap_or(words.len());
    ParsedName {
        title: title_of(&words[..boundary], markers),
        year: None,
        tags: tags_from(&words[boundary..]),
    }
}

/// A year somebody wrote in front of the title, and the rest of the name.
///
/// The whole reading rests on the year being a boundary with the title before
/// it, and this one shape puts it the other way round. It is told apart by its
/// brackets: no film is called `(2019)`, so a bracketed year standing in front
/// of everything else is a date and not a name. Written bare it is left where
/// it is, because a film really can be called `1917`.
fn a_year_written_in_front<'a, 'b>(
    words: &'b [&'a str],
    current_year: i32,
) -> Option<(i32, &'b [&'a str])> {
    let latest = current_year + YEARS_AHEAD;
    let (first, rest) = words.split_first()?;
    let stripped = bare(first);
    // Never the whole name, and never written bare.
    if rest.is_empty() || stripped.len() != 4 || stripped.len() == first.len() {
        return None;
    }
    stripped
        .parse::<i32>()
        .ok()
        .filter(|year| (EARLIEST_YEAR..=latest).contains(year))
        .map(|year| (year, rest))
}

/// Builds the title out of the words that came before the boundary.
fn title_of(words: &[&str], markers: &BTreeSet<String>) -> String {
    let spoken = drop_bracketed_asides(words);
    join_title(trim_the_end(&spoken, markers))
}

/// Separates a year that lost the dot between it and what follows.
///
/// A name written by hand loses a separator now and then, and a year welded to
/// the word after it is no longer a year: the boundary is gone and the whole
/// description lands in the title. Only ever split when what follows is a word
/// already known to describe a file, so a title carrying a number keeps it.
fn separate_a_year_from_what_is_glued_to_it<'a>(
    words: &[&'a str],
    current_year: i32,
) -> Vec<&'a str> {
    let latest = current_year + YEARS_AHEAD;
    let mut separated = Vec::with_capacity(words.len());

    for word in words {
        // Read on the bytes rather than cut first. Cutting a word at its
        // fourth byte splits a letter in half whenever an accent sits there,
        // which is not an error but a panic: `après` is three plain letters
        // and then one written in two bytes, and the whole scan died on the
        // first film with that word in its name. Four digits are four bytes,
        // so the cut below can only ever land between two letters.
        let opens_with_a_year = word.len() > 4
            && word.as_bytes()[..4].iter().all(u8::is_ascii_digit)
            && word[..4]
                .parse::<i32>()
                .is_ok_and(|year| (EARLIEST_YEAR..=latest).contains(&year));

        if opens_with_a_year && TECHNICAL_TAGS.contains(&word[4..].to_lowercase().as_str()) {
            separated.push(&word[..4]);
            separated.push(&word[4..]);
        } else {
            separated.push(word);
        }
    }
    separated
}

/// Drops whatever a release put in brackets in front of everything else.
///
/// A single bracketed word is left alone anywhere else in a name, because that
/// is how a year is written and how a tag is written. In front of the title it
/// is neither: a release that signs its work signs it there. What sits in it is
/// not looked at, since the group that put it there is exactly what no list can
/// hold.
///
/// Square brackets and braces only, and it has to hold a letter. Round brackets
/// belong to titles as well as to releases, and `(500) Days of Summer` is a
/// film that would otherwise lose the front of its name.
fn drop_what_is_bracketed_in_front<'a>(words: &'a [&'a str]) -> &'a [&'a str] {
    let wholly_bracketed = |word: &str| {
        word.chars().count() > 2
            && word.starts_with(['[', '{'])
            && word.ends_with([']', '}'])
            && word.chars().any(|c| c.is_alphabetic())
    };
    match words.split_first() {
        // Never the whole name: a film with no title left is a film nobody
        // finds again.
        Some((first, rest)) if !rest.is_empty() && wholly_bracketed(first) => rest,
        _ => words,
    }
}

/// How long the last piece of a site address can be.
const LONGEST_TOP_LEVEL: usize = 6;

/// The name with the address of a site taken off its front.
///
/// Whoever hands a file out often writes where it came from first, and that
/// address is then the title as far as anything here is concerned: a provider
/// asked about it finds nothing at all.
///
/// Only an address that says so is taken off: one beginning with `www.`, or
/// one sitting in brackets of its own. A bare `something.tld` is not enough,
/// because a dot separates words in these names and `Dr.No` is then an address
/// with as good a last piece as any: a film would lose its title to a rule
/// that went that far.
fn without_the_site_in_front(file_name: &str) -> &str {
    let trimmed = file_name.trim_start();
    let opened = trimmed.starts_with(['[', '(']);
    let rest = if opened {
        trimmed[1..].trim_start()
    } else {
        trimmed
    };

    let Some(after_www) = rest
        .get(..4)
        .filter(|start| start.eq_ignore_ascii_case("www."))
        .map(|_| &rest[4..])
    else {
        return file_name;
    };

    let host_ends = after_www
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '.' && c != '-')
        .unwrap_or(after_www.len());
    let host = &after_www[..host_ends];
    let Some((_, top_level)) = host.rsplit_once('.') else {
        return file_name;
    };
    if top_level.is_empty()
        || top_level.len() > LONGEST_TOP_LEVEL
        || !top_level.chars().all(|c| c.is_ascii_alphabetic())
    {
        return file_name;
    }

    // Past the address: whatever closes the brackets it was put in, and
    // whatever separates it from the name proper.
    let after_host = after_www[host_ends..].trim_start();
    let after_bracket = match opened {
        true => after_host.strip_prefix([']', ')']).unwrap_or(after_host),
        false => after_host,
    };
    let name = after_bracket.trim_start_matches([' ', '-', '_', '.']);
    // Never the whole name: an address and nothing else leaves nothing to look
    // a film up by, and the name as it stands at least says something.
    match name.is_empty() {
        true => file_name,
        false => name,
    }
}

/// Drops an aside somebody put in square brackets or braces inside a title.
///
/// The commonest is the title in another language, written next to the one the
/// file is named after. It is never part of either title, and handing both to a
/// provider at once finds nothing.
///
/// A single bracketed word is left alone: that is how a year is written, and
/// how a tag is written, and both are already read elsewhere.
fn drop_bracketed_asides<'a>(words: &[&'a str]) -> Vec<&'a str> {
    let opens = |word: &str| word.starts_with('[') || word.starts_with('{');
    let closes = |word: &str| word.ends_with(']') || word.ends_with('}');

    let mut kept = Vec::with_capacity(words.len());
    let mut index = 0;
    while index < words.len() {
        let word = words[index];
        let ends_at = (opens(word) && !closes(word))
            .then(|| words[index + 1..].iter().position(|word| closes(word)))
            .flatten();

        match ends_at {
            Some(length) => index += length + 2,
            None => {
                kept.push(word);
                index += 1;
            }
        }
    }
    kept
}

/// What the names of one library carry that no title ever does.
///
/// Learnt from the names themselves rather than written down here, for two
/// reasons. Such a thing names whoever produced it, and the repository is not
/// the place for that. And no list could hold them all: they are somebody's
/// initials, a site, a group, whatever a tool stuck on the way past.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LibrarySigns {
    /// Words that end name after name.
    marks: BTreeSet<String>,
    /// Strings stuck to the front of a name that is otherwise, in full,
    /// another name of the same library. Lowercased.
    prefixes: BTreeSet<String>,
}

impl LibrarySigns {
    /// The name with what was stuck to its front taken off.
    ///
    /// The longest one that fits, so a short sign is never preferred to the
    /// longer one it happens to begin.
    fn without_the_glued_prefix<'a>(&'a self, file_name: &'a str) -> &'a str {
        if self.prefixes.is_empty() {
            return file_name;
        }
        file_name
            .char_indices()
            .map(|(index, _)| index)
            .filter(|index| *index > 0 && *index <= LONGEST_GLUED_PREFIX)
            .rfind(|index| self.prefixes.contains(&file_name[..*index].to_lowercase()))
            .map_or(file_name, |index| &file_name[index..])
    }
}

/// Reads off a library what its names carry and no title does.
pub fn signs_in<S: AsRef<str>>(file_names: &[S]) -> LibrarySigns {
    LibrarySigns {
        marks: marks_in(file_names),
        prefixes: glued_prefixes(file_names),
    }
}

/// How long a thing stuck to the front of a name may be.
///
/// Short by nature: it is something a tool or a transfer left behind, never a
/// sentence. The limit is what stops a whole title from ever being read as one.
const LONGEST_GLUED_PREFIX: usize = 16;

/// The shortest thing worth calling a prefix.
///
/// A single character in front of a name that exists on its own is as likely
/// to be a coincidence as a sign, and the cost of being wrong is a lost word.
const SHORTEST_GLUED_PREFIX: usize = 2;

/// The things stuck to the front of names in this library.
///
/// Nothing is guessed from the shape of a word here. A prefix is recognised
/// only when the rest of the name is, in full and to the letter, another name
/// of the same library: the proof that it was added is that the same file is
/// there without it.
///
/// It must also be one piece, with no separator in it. A prefix that ends at a
/// word boundary cannot be told from the first word of a title, and two real
/// films can perfectly well be called one thing and that thing with a word in
/// front of it.
fn glued_prefixes<S: AsRef<str>>(file_names: &[S]) -> BTreeSet<String> {
    let known: std::collections::HashSet<&str> =
        file_names.iter().map(|name| name.as_ref()).collect();

    let mut found = BTreeSet::new();
    for name in file_names.iter().map(|name| name.as_ref()) {
        let glued = name
            .char_indices()
            .map(|(index, _)| index)
            .take_while(|index| *index <= LONGEST_GLUED_PREFIX)
            .find(|index| {
                *index >= SHORTEST_GLUED_PREFIX
                    && could_be_glued_on(&name[..*index])
                    && known.contains(&name[*index..])
            });
        if let Some(index) = glued {
            found.insert(name[..index].to_lowercase());
        }
    }
    found
}

/// Whether a piece of a name could be something stuck to its front.
fn could_be_glued_on(head: &str) -> bool {
    !head.contains([' ', '.', '_', '-'])
}

/// How many names a word must end before it counts as this library's mark.
///
/// Three is low enough to catch a mark on a handful of files and high enough
/// that a word two titles happen to share is not mistaken for one.
const REPEATED_ENOUGH: usize = 3;

/// The words this library signs its files with.
///
/// What can be said generally is that a signature repeats and a title does
/// not, so the last word of every name is counted and the ones that keep
/// coming back are the marks.
fn marks_in<S: AsRef<str>>(file_names: &[S]) -> BTreeSet<String> {
    let mut counted: std::collections::BTreeMap<String, usize> = std::collections::BTreeMap::new();

    for name in file_names {
        let stem = strip_extension(name.as_ref());
        let normalised = stem.replace(['.', '_'], " ");
        let Some(last) = normalised
            .split_whitespace()
            .next_back()
            .map(bare)
            .and_then(|word| word.rsplit('-').next())
        else {
            continue;
        };
        if !could_be_a_mark(last) {
            continue;
        }
        *counted.entry(last.to_lowercase()).or_default() += 1;
    }

    counted
        .into_iter()
        .filter(|(_, seen)| *seen >= REPEATED_ENOUGH)
        .map(|(word, _)| word)
        .collect()
}

/// Whether a word is the sort of thing a signature is made of.
///
/// A number is a sequel, a roman numeral is a sequel, and a word already known
/// to describe the file is handled elsewhere. What is left is a word, and a
/// word that ends many names is a signature.
fn could_be_a_mark(word: &str) -> bool {
    let lowered = word.to_lowercase();
    word.chars().count() >= 2
        && word.chars().any(|c| c.is_alphabetic())
        && !word.chars().all(|c| "IVXLCDMivxlcdm".contains(c))
        && !TECHNICAL_TAGS.contains(&lowered.as_str())
        && !EDITION_WORDS.contains(&lowered.as_str())
}

/// Drops what a title picked up at its end and never had.
///
/// Both kinds are trailing by nature: the word naming which cut of the film
/// this is, and the marker a release or a person signs with. Neither belongs
/// to a title, and a provider asked about either finds nothing.
fn trim_the_end<'a>(words: &'a [&'a str], markers: &BTreeSet<String>) -> &'a [&'a str] {
    trim_dangling_separators(trim_trailing_marker(trim_edition_words(trim_known_marks(
        words, markers,
    ))))
}

/// Drops a separator left standing on its own at the end of a title.
///
/// A name written `Title - Year - 1080p` puts the dash between the two, and
/// cutting at the year leaves it hanging. Last of the trims, so that it also
/// catches the one left behind by a word the others took off.
fn trim_dangling_separators<'a>(words: &'a [&'a str]) -> &'a [&'a str] {
    let mut kept = words;
    while let Some((last, rest)) = kept.split_last() {
        if rest.is_empty() || last.chars().any(|c| c.is_alphanumeric()) {
            break;
        }
        kept = rest;
    }
    kept
}

/// Drops the marks this library signs with, however many are stacked up.
///
/// Knowing the mark is what lets it go from a title written wholly in
/// capitals, where nothing about its shape tells it apart from a word.
fn trim_known_marks<'a>(words: &'a [&'a str], markers: &BTreeSet<String>) -> &'a [&'a str] {
    let mut kept = words;
    while let Some((last, rest)) = kept.split_last() {
        // Never the whole title: a film with no title left is a film nobody
        // finds again.
        if rest.is_empty() || !markers.contains(&bare(last).to_lowercase()) {
            break;
        }
        kept = rest;
    }
    kept
}

/// Words that say which cut of a film this copy is, never what it is called.
///
/// Short and unambiguous on purpose: a word here is dropped from the end of a
/// title, and a word dropped wrongly is a film nobody finds again. Anything
/// that could begin or sit inside a real title stays out of this list.
const EDITION_WORDS: &[&str] = &[
    "extended",
    "unrated",
    "uncut",
    "uncensored",
    "uncensured",
    "remastered",
    "remaster",
    "theatrical",
    "redux",
    "integrale",
];

/// Words that name the cut only together with the word before them.
///
/// `Edition` takes whatever qualifies it: no film is called anything Edition,
/// and the word in front of it belongs to the edition rather than to the film.
/// `Cut` is the opposite, since a real title can end in it, so only the two
/// spellings that can mean nothing else are listed.
const EDITION_ENDINGS: &[(&str, Option<&str>)] = &[
    ("edition", None),
    ("cut", Some("directors")),
    ("cut", Some("director's")),
];

/// Drops the words naming the cut, however many of them are stacked up.
fn trim_edition_words<'a>(words: &'a [&'a str]) -> &'a [&'a str] {
    let mut kept = words;
    loop {
        let Some((last, rest)) = kept.split_last() else {
            break;
        };
        // Never the whole title: a name made only of these says nothing, and
        // an empty title is a film nobody finds again.
        if rest.is_empty() {
            break;
        }
        let lowered = bare(last).to_lowercase();

        if EDITION_WORDS.contains(&lowered.as_str()) {
            kept = rest;
            continue;
        }

        // Two words, and the one in front has to be there: on its own the last
        // of them is an ordinary word that titles really do end in.
        let paired = EDITION_ENDINGS.iter().any(|(ending, qualifier)| {
            *ending == lowered
                && rest.len() > 1
                && qualifier.is_none_or(|expected| {
                    rest.last()
                        .is_some_and(|word| bare(word).eq_ignore_ascii_case(expected))
                })
        });
        if !paired {
            break;
        }
        kept = &rest[..rest.len() - 1];
    }
    kept
}

/// Lowercased technical markers, with any release group split off.
fn tags_from(words: &[&str]) -> BTreeSet<String> {
    words
        .iter()
        .flat_map(|word| split_release_group(word))
        .map(|word| bare(word).to_lowercase())
        .filter(|word| !word.is_empty())
        .collect()
}

/// A word without the brackets a release may have wrapped it in.
fn bare(word: &str) -> &str {
    word.trim_matches(|c| matches!(c, '(' | ')' | '[' | ']' | '{' | '}'))
}

/// Finds the word holding the release year.
///
/// The *last* plausible year wins, which is what handles a title that carries
/// a year of its own: in such a name the release year comes after the title,
/// so taking the last one keeps the title intact.
///
/// A year at the very end of a name, with nothing after it, is still the year.
/// Measured at the provider rather than argued: of a broad sample of real
/// films, one in a hundred and forty carries a year in its own title, while a
/// name written `Title Year` and nothing else is one of the commonest shapes
/// there is. Asked for such a title with the year still stuck on it, the
/// provider answered nothing at all twenty five times out of twenty eight, so
/// leaving it there is not the cautious reading: it is the one that never
/// finds the film.
fn find_year(words: &[&str], current_year: i32) -> Option<usize> {
    let latest = current_year + YEARS_AHEAD;
    words
        .iter()
        .enumerate()
        .filter(|(index, word)| {
            let stripped = bare(word);
            // A title cannot be only a year.
            *index > 0
                && stripped.len() == 4
                && stripped.chars().all(|c| c.is_ascii_digit())
                && stripped
                    .parse::<i32>()
                    .is_ok_and(|year| (EARLIEST_YEAR..=latest).contains(&year))
        })
        .map(|(index, _)| index)
        .next_back()
}

/// Words that can only ever describe the file, never name a film.
///
/// Kept deliberately short. The list of technical markers is endless, and this
/// one is not trying to be complete: it only has to hold the words that no
/// title contains, so that a name carrying no year still has a boundary.
const TECHNICAL_TAGS: &[&str] = &[
    "2160p",
    "1440p",
    "1080p",
    "1080i",
    "720p",
    "576p",
    "480p",
    "4k",
    "uhd",
    "4klight",
    "hdlight",
    "bluray",
    "brrip",
    "bdrip",
    "webrip",
    "web-dl",
    "webdl",
    "hdtv",
    "dvdrip",
    "dvdscr",
    "remux",
    "x264",
    "x265",
    "h264",
    "h265",
    "hevc",
    "xvid",
    "divx",
    "av1",
    "10bit",
    "8bit",
    "hdr",
    "hdr10",
    "truehd",
    "atmos",
    "dts",
    "ac3",
    "eac3",
    "aac",
    "multi",
    "vostfr",
    "vff",
    "vfq",
    "vfi",
    "vf2",
    "subfrench",
    "truefrench",
    "proper",
    "repack",
];

/// Where the description of the file starts, in a name that has no year.
///
/// The first word that can only be technical ends the title. Everything after
/// it is description, whatever it happens to be, which is what makes this work
/// on markers the list has never heard of.
///
/// A tag is also recognised when whoever named the file wrote it in two words.
/// The same marker turns up both ways, and a name where it went unrecognised
/// hands a provider a title with a description stuck on the end, which finds
/// nothing at all.
fn first_technical_tag(words: &[&str]) -> Option<usize> {
    words.iter().enumerate().skip(1).find_map(|(index, word)| {
        let lowered = bare(word).to_lowercase();
        let is_a_tag = TECHNICAL_TAGS.contains(&lowered.as_str())
            || words
                .get(index + 1)
                .map(|next| format!("{lowered}{}", bare(next).to_lowercase()))
                .is_some_and(|joined| TECHNICAL_TAGS.contains(&joined.as_str()));
        is_a_tag.then_some(index)
    })
}

/// Drops the marker a release or a person put at the end of a name.
///
/// Recognised by its shape rather than by a list, because the list would be
/// one name long and would name its owner: a marker shouts in capitals at the
/// end of a title that does not. A title written wholly in capitals keeps
/// every word, since there is then nothing to tell apart.
///
/// Whether the title shouts is decided only on the words that could. A number
/// cannot shout, nor can a word of one letter, and counting those as words that
/// do not cost `12 ANGRY MEN` and `A CLOCKWORK ORANGE` their last word: the
/// title was read as one that does not shout because of what stands in front of
/// it, and the word that shouts loudest in it went.
fn trim_trailing_marker<'a>(words: &'a [&'a str]) -> &'a [&'a str] {
    let letters_of = |word: &str| word.chars().filter(|c| c.is_alphabetic()).count();
    let shouting = |word: &str| {
        letters_of(word) >= 2
            && word
                .chars()
                .filter(|c| c.is_alphabetic())
                .all(char::is_uppercase)
    };
    // A sequel number is written in capitals and belongs to the title. Asked
    // of the word being dropped and of that one only: a word of the title that
    // happens to be spelt out of those same letters, `DIX` or `MI`, is still a
    // word of a title written in capitals, and reading it as a number said the
    // title did not shout and took its last word away.
    let a_sequel_number = |word: &str| word.chars().all(|c| "IVXLCDM".contains(c));
    let could_shout = |word: &&&str| letters_of(word) >= 2;

    match words.split_last() {
        Some((last, rest))
            if shouting(last)
                && !a_sequel_number(last)
                && rest.iter().filter(could_shout).any(|word| !shouting(word)) =>
        {
            rest
        }
        _ => words,
    }
}

/// Turns the words before the year back into a readable title.
///
/// Nothing is filtered out here. The boundary rule already removed everything
/// technical, and filtering inside the title is what turns a single letter
/// word, which several real titles contain, into a lost word.
fn join_title(words: &[&str]) -> String {
    words.join(" ").trim().to_string()
}

/// Splits a trailing release group off a tag.
///
/// A group is attached to the last tag with a hyphen, so both halves are kept
/// rather than one swallowing the other.
fn split_release_group(word: &str) -> impl Iterator<Item = &str> {
    word.split('-').filter(|part| !part.is_empty())
}

fn strip_extension(file_name: &str) -> &str {
    match file_name.rfind('.') {
        // Only treat it as an extension when it looks like one: a short run of
        // letters and digits. A title ending in a dot and a word would
        // otherwise lose that word.
        Some(position)
            if file_name.len() - position <= 6
                && file_name[position + 1..]
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric()) =>
        {
            &file_name[..position]
        }
        _ => file_name,
    }
}

/// Builds the title used for ordering.
///
/// Leading articles are moved out of the way and accents are folded, so that a
/// list reads the way a person expects and so that searching for a title
/// without its accents still finds it. Computed once on write, because sorting
/// a large collection has to be an index walk.
pub fn sort_title(title: &str) -> String {
    let without_article = strip_leading_article(title);
    fold_accents(&without_article).to_lowercase()
}

/// Drops a leading article in the languages this server is used in.
fn strip_leading_article(title: &str) -> String {
    const ARTICLES: [&str; 10] = [
        "le ", "la ", "les ", "l'", "un ", "une ", "des ", "the ", "a ", "an ",
    ];
    let lowered = title.to_lowercase();
    for article in ARTICLES {
        if lowered.starts_with(article) {
            return title[article.len()..].trim().to_string();
        }
    }
    title.trim().to_string()
}

/// Replaces accented letters with their plain form.
///
/// Handles the letters that actually occur in the languages at hand rather
/// than pulling in a full normalisation library for a handful of characters.
///
/// An accent reaches us written one of two ways. Usually it is one character,
/// and the table below covers those. But a file can carry the plain letter
/// followed by the accent as a mark of its own, which looks identical on any
/// screen and is not the same text at all. Those marks are dropped, so that
/// the folded form is the same either way: without that, two spellings of one
/// title sort apart, compare as different, and one of them finds nothing at a
/// provider.
pub fn fold_accents(value: &str) -> String {
    value
        .chars()
        .filter(|c| !is_a_combining_mark(*c))
        .map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' => 'A',
            'è' | 'é' | 'ê' | 'ë' => 'e',
            'È' | 'É' | 'Ê' | 'Ë' => 'E',
            'ì' | 'í' | 'î' | 'ï' => 'i',
            'Ì' | 'Í' | 'Î' | 'Ï' => 'I',
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
            'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' => 'O',
            'ù' | 'ú' | 'û' | 'ü' => 'u',
            'Ù' | 'Ú' | 'Û' | 'Ü' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            'ñ' => 'n',
            'Ñ' => 'N',
            'ÿ' => 'y',
            other => other,
        })
        .collect()
}

/// Whether a character is an accent written on its own, after the letter it
/// belongs to. The block below is the one Latin scripts use.
fn is_a_combining_mark(c: char) -> bool {
    ('\u{0300}'..='\u{036f}').contains(&c)
}

/// The words two titles are compared by when deciding whether they are the
/// same title.
///
/// Only the words survive: accents are folded, capitals go, and everything
/// that is neither a letter nor a number becomes a space. Whoever named a file
/// dropped a colon or spelled a hyphenated name as two words, and none of that
/// makes it another film. What it never does is join two words into one,
/// because two words and one word are two different names.
///
/// An apostrophe goes without leaving a space, which is the one punctuation
/// mark that sits inside a word rather than between two.
///
/// A sequel written in roman numerals becomes the number it is. One side of a
/// comparison writes it one way and the other the other way, name after name,
/// and they are the same number.
///
/// Kept apart from the ordering title, which has to leave a title readable and
/// therefore cannot go this far.
pub fn matchable_title(title: &str) -> Vec<String> {
    let folded = fold_accents(title)
        .to_lowercase()
        .replace(['\'', '\u{2019}'], "");
    folded
        .split(|c: char| !c.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(|word| {
            number_of_a_roman_numeral(word).map_or_else(|| word.to_string(), |n| n.to_string())
        })
        .collect()
}

/// The number a sequel written in roman numerals stands for.
///
/// Only the ones a sequel is ever numbered with. Read as a whole word from a
/// short list rather than worked out letter by letter, because plenty of real
/// words are made only of those letters and none of them is a number.
fn number_of_a_roman_numeral(word: &str) -> Option<u8> {
    const NUMERALS: [&str; 13] = [
        "i", "ii", "iii", "iv", "v", "vi", "vii", "viii", "ix", "x", "xi", "xii", "xiii",
    ];
    NUMERALS
        .iter()
        .position(|numeral| *numeral == word)
        .map(|index| index as u8 + 1)
}

/// How much two titles have in common, between nothing and one.
///
/// The share of the words either of them uses that both of them use. Counted
/// both ways on purpose: a title that holds every word of another and six more
/// besides is a different title, and counting one way only would call them the
/// same.
pub fn how_alike(left: &[String], right: &[String]) -> f64 {
    if left.is_empty() || right.is_empty() {
        return 0.0;
    }
    let left: BTreeSet<&String> = left.iter().collect();
    let right: BTreeSet<&String> = right.iter().collect();

    let shared = left.intersection(&right).count() as f64;
    let between_them = left.union(&right).count() as f64;
    shared / between_them
}

/// Whether one title is the other with more said after it.
///
/// A film released under a subtitle is very often written on a disk without
/// it: `Détour Mortel 5` for a film the provider calls `Détour mortel 5 : Les
/// Liens du sang`. A share of words is bad at seeing that, and bad in a way
/// that depends on nothing that matters: the same collection named the same
/// way passes on the film whose subtitle is two words and fails on the one
/// whose subtitle is four. What actually says these are one film is that the
/// shorter name is exactly how the longer one begins.
///
/// **What the longer one adds must not be a number**, which is what keeps this
/// from swallowing a sequel: `Saw` begins `Saw II` too, and they are not one
/// film. A subtitle is words.
///
/// Word for word from the start, so `Mortel 5` is not how `Détour mortel 5`
/// begins. The caller decides what to do with it, and this is only ever worth
/// anything alongside a year that agrees.
pub fn one_is_how_the_other_begins(left: &[String], right: &[String]) -> bool {
    what_the_longer_adds(left, right).is_some_and(|added| !is_an_instalment(&added[0]))
}

/// Whether a candidate is a later instalment of the very film being searched
/// for, rather than that film itself.
///
/// `Quiet Harbour` and `Quiet Harbour 2` are not one film, and nothing about
/// how alike they read says so: they share every word one of them has. Two
/// instalments of a series come out a year apart as readily as not, so a year
/// does not say it either. This is the one thing that does.
///
/// **The direction matters, and only one of them is dangerous.** A candidate
/// that carries a number the searched title lacks is a sequel offered instead
/// of the film asked for, and has to be refused. A searched title that
/// carries a number the candidate lacks is the ordinary way a collection
/// numbers even its first film on disk, `Bad Boys 1` for a film simply called
/// `Bad Boys`, and refusing that would turn a real match into a fault nobody
/// could place with the year sitting right there in agreement.
pub fn candidate_is_a_later_instalment(candidate: &[String], searched: &[String]) -> bool {
    candidate.len() > searched.len()
        && !searched.is_empty()
        && candidate.starts_with(searched)
        && is_an_instalment(&candidate[searched.len()])
}

/// What the longer of two titles says after the shorter one it begins with.
///
/// Nothing at all unless one really is how the other begins, word for word
/// from the start and with something left to say after it.
fn what_the_longer_adds<'a>(left: &'a [String], right: &'a [String]) -> Option<&'a [String]> {
    let (shorter, longer) = if left.len() <= right.len() {
        (left, right)
    } else {
        (right, left)
    };
    // Equal lengths are the same title or two different ones, and either way
    // nothing here is what answers that.
    if shorter.is_empty() || shorter.len() == longer.len() || !longer.starts_with(shorter) {
        return None;
    }
    Some(&longer[shorter.len()..])
}

/// Whether a word is the number a sequel is counted by rather than a word of a
/// subtitle. Roman numerals arrive here already written as digits.
fn is_an_instalment(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|letter| letter.is_ascii_digit())
}

/// Whether a title is written in plain letters, which decides whether asking a
/// provider for its folded form is a second question or the same one twice.
pub fn carries_accents(title: &str) -> bool {
    !title.is_ascii()
}

/// The longest a shorthand somebody wrote in front of a title can be.
///
/// Long enough for the ones people really write, which are the initials of a
/// series and a number, and short enough that no word of a title reaches it.
const SHORTHAND_AT_MOST: usize = 5;

/// The title with a shorthand somebody wrote in front of it taken off.
///
/// Whoever keeps a series together often numbers it themselves, in front of
/// the name and welded to it: `HP1-`, `SW3_`, `LOTR2 `. It sorts their folder
/// and it is the first thing a provider is given, which is why the provider
/// answers nothing at all: no film was ever released under a name beginning
/// with it.
///
/// **A last resort and never a rule.** It is asked for only once the title as
/// written has found nothing, because the shape it looks for is also the shape
/// of real titles: `WALL-E` is a film, and so is `2 Fast 2 Furious`. Those find
/// themselves on the first question and never reach this one.
///
/// Answers nothing when there is no such shorthand, which is the ordinary case.
pub fn without_a_shorthand_prefix(title: &str) -> Option<String> {
    let title = title.trim();
    // Its end is whichever separator comes first: welded with a dash or an
    // underscore, or simply left as the first word.
    let cut = title.find(['-', '_', ' '])?;
    let (shorthand, rest) = title.split_at(cut);
    let rest = rest[1..].trim();

    // A letter and a digit is what a shorthand is made of. Letters alone are a
    // word of the title, and digits alone are a year or a number of it.
    if shorthand.is_empty()
        || shorthand.chars().count() > SHORTHAND_AT_MOST
        || !shorthand
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric())
        || !shorthand.chars().any(|letter| letter.is_ascii_digit())
        || !shorthand.chars().any(|letter| letter.is_ascii_alphabetic())
    {
        return None;
    }

    // And there has to be a title left to ask about.
    if rest.chars().count() < 3 || !rest.chars().any(char::is_alphabetic) {
        return None;
    }
    Some(rest.to_string())
}

/// File extensions treated as video.
const VIDEO_EXTENSIONS: [&str; 14] = [
    "mkv", "mp4", "m4v", "avi", "mov", "wmv", "flv", "webm", "mpg", "mpeg", "ts", "m2ts", "mts",
    "ogv",
];

/// Whether a name looks like a video file worth scanning.
pub fn is_video_file(file_name: &str) -> bool {
    let Some(extension) = file_name.rsplit('.').next() else {
        return false;
    };
    // Files a downloader leaves behind mid-transfer carry a second extension.
    // Picking them up means analysing a truncated file and storing nonsense.
    if file_name.to_lowercase().ends_with(".part")
        || file_name.to_lowercase().ends_with(".!qb")
        || file_name.starts_with('.')
    {
        return false;
    }
    VIDEO_EXTENSIONS.contains(&extension.to_lowercase().as_str())
}

/// Markers a release puts at the end of a clip that is not the film itself.
const COMPANION_MARKERS: &[(&str, &str)] = &[
    ("-trailer", "trailer"),
    (".trailer", "trailer"),
    (" trailer", "trailer"),
    ("-sample", "sample"),
    (".sample", "sample"),
];

/// The marker a companion clip carries, and where the name proper ends.
fn companion_marker(file_name: &str) -> Option<(&'static str, usize)> {
    let stem = strip_extension(file_name);
    if stem.eq_ignore_ascii_case("sample") {
        return Some(("sample", 0));
    }
    COMPANION_MARKERS.iter().find_map(|(marker, kind)| {
        let start = stem.len().checked_sub(marker.len())?;
        // The markers are plain characters, so comparing without regard to
        // case keeps every position valid in the name as it was given.
        (stem.is_char_boundary(start) && stem[start..].eq_ignore_ascii_case(marker))
            .then_some((*kind, start))
    })
}

/// Whether a name looks like a companion clip rather than the film itself.
///
/// Two families to leave out: sample clips some releases ship, and trailers,
/// which are a kind of their own rather than another film.
pub fn is_companion_clip(file_name: &str) -> Option<&'static str> {
    companion_marker(file_name).map(|(kind, _)| kind)
}

/// The name the film itself would carry, taken from one of its companions.
///
/// A trailer sits next to its film under the film's own name plus a marker.
/// Taking the marker off is what lets the clip be read by the very rules the
/// film was read with, rather than by a second set that would drift.
pub fn without_companion_marker(file_name: &str) -> Option<String> {
    let (_, end_of_name) = companion_marker(file_name)?;
    if end_of_name == 0 {
        // A clip called nothing but "sample" says nothing about which film it
        // belongs to, and guessing from the folder would be a guess.
        return None;
    }
    let extension = &file_name[strip_extension(file_name).len()..];
    Some(format!("{}{}", &file_name[..end_of_name], extension))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixed so the test set never changes meaning as time passes.
    const NOW: i32 = 2026;

    fn parsed(name: &str) -> ParsedName {
        parse(name, NOW)
    }

    #[test]
    fn a_plain_name_gives_up_its_title_and_year() {
        let result = parsed("Quiet.Harbour.2019.MULTi.1080p.BluRay.x264.mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert_eq!(result.year, Some(2019));
        assert!(result.tags.contains("1080p"));
        assert!(result.tags.contains("bluray"));
        assert!(result.tags.contains("x264"));
    }

    #[test]
    fn a_title_of_several_words_survives_intact() {
        let result = parsed("The.Long.Road.North.2018.1080p.x264.mkv");
        assert_eq!(result.title, "The Long Road North");
        assert_eq!(result.year, Some(2018));
    }

    #[test]
    fn a_personal_marker_glued_with_an_underscore_never_lands_in_the_title() {
        // The marker attaches to the last tag with an underscore rather than a
        // dot. Without treating the underscore as a separator the tag becomes
        // unrecognisable.
        let result = parsed("Quiet.Harbour.2019.MULTi.1080p.x264_MYTAG.mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert!(result.tags.contains("x264"));
        assert!(result.tags.contains("mytag"));
        assert!(
            !result.title.contains("MYTAG"),
            "a marker must never reach the title"
        );
    }

    #[test]
    fn a_marker_glued_to_the_definition_is_split_too() {
        let result = parsed("Quiet.Harbour.2020.MULTi.1080p_new.mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert_eq!(result.year, Some(2020));
        assert!(result.tags.contains("1080p"));
        assert!(result.tags.contains("new"));
    }

    #[test]
    fn two_titles_sharing_their_first_words_are_told_apart_by_the_year() {
        // The reason the year is not optional when asking a provider: without
        // it these two come back as the same film.
        let first = parsed("Quiet.Harbour.2016.MULTi.1080p.x264.mkv");
        let second = parsed("Quiet.Harbour.Rising.Tide.2020.MULTi.1080p.mkv");

        assert_eq!(first.title, "Quiet Harbour");
        assert_eq!(first.year, Some(2016));
        assert_eq!(second.title, "Quiet Harbour Rising Tide");
        assert_eq!(second.year, Some(2020));
        assert_ne!(first.year, second.year);
    }

    #[test]
    fn a_single_letter_word_inside_a_title_is_kept() {
        // Several real titles contain one, and it looks exactly like a tag.
        // The boundary rule protects it, as long as nothing filters words
        // inside the title.
        let result = parsed("Harbour.v.Lighthouse.Dawn.Of.Tides.2016.VL.MULTi.VFF.1080p.AV1.mkv");
        assert_eq!(result.title, "Harbour v Lighthouse Dawn Of Tides");
        assert_eq!(result.year, Some(2016));
        assert!(result.tags.contains("vff"));
        assert!(result.tags.contains("av1"));
    }

    #[test]
    fn a_release_group_at_the_end_is_kept_apart_from_the_tag_before_it() {
        let result = parsed("Quiet.Harbour.2016.1080p.TrueHD.Atmos.7.1-SOMEGROUP.mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert!(result.tags.contains("truehd"));
        assert!(result.tags.contains("atmos"));
        assert!(result.tags.contains("somegroup"));
    }

    #[test]
    fn a_title_carrying_a_year_of_its_own_keeps_it() {
        // The last plausible year is the boundary, which is exactly what makes
        // this work.
        let result = parsed("Harbour.2049.2017.MULTi.1080p.x264.mkv");
        assert_eq!(result.title, "Harbour 2049");
        assert_eq!(result.year, Some(2017));
    }

    #[test]
    fn capitalisation_varies_between_files_and_does_not_change_the_outcome() {
        let first = parsed("quiet.harbour.2016.multi.1080p.x264.mkv");
        let second = parsed("Quiet.Harbour.2016.MULTi.1080p.x264.mkv");
        assert_eq!(sort_title(&first.title), sort_title(&second.title));
        assert_eq!(first.year, second.year);
    }

    #[test]
    fn a_year_somebody_put_in_brackets_is_still_the_boundary() {
        let result = parsed("Quiet Royale (2006) MULTI 4KLight HDR 265 SOMEGROUP.mkv");
        assert_eq!(
            result.title, "Quiet Royale",
            "brackets round the year are as common as none, and cost the whole title"
        );
        assert_eq!(result.year, Some(2006));
        assert!(result.tags.contains("multi") && result.tags.contains("hdr"));
    }

    #[test]
    fn a_bracketed_year_at_the_very_end_is_a_year_and_not_part_of_the_title() {
        // Nothing follows it, which for a bare number would mean the title
        // carries it. Nobody calls a film "(2019)", so the doubt does not
        // apply once it is in brackets.
        let result = parsed("Quiet Harbour (2019).mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert_eq!(result.year, Some(2019));
    }

    #[test]
    fn a_name_with_no_year_is_cut_at_the_first_word_that_can_only_be_technical() {
        let result = parsed("Quiet Harbour 2160p SOMEGROUP.mkv");
        assert_eq!(
            result.title, "Quiet Harbour",
            "a definition is never part of a title, and asking a provider about one finds nothing"
        );
        assert_eq!(result.year, None);
        assert!(result.tags.contains("2160p"));
    }

    #[test]
    fn a_technical_tag_written_in_two_words_is_cut_at_like_any_other() {
        // The same marker is written both ways, and one of them used to leave
        // the title with a description stuck on the end. A provider asked
        // about that answers nothing at all, which is the whole cost.
        for name in [
            "Quiet Harbour BD Rip.avi",
            "Quiet Harbour Blu Ray.mkv",
            "Quiet Harbour Web Rip.mp4",
            "Quiet Harbour DVD Rip.avi",
        ] {
            let result = parsed(name);
            assert_eq!(result.title, "Quiet Harbour", "title of {name}");
            assert_eq!(result.year, None, "year of {name}");
        }
    }

    #[test]
    fn two_words_that_only_look_like_a_tag_side_by_side_leave_the_title_alone() {
        // The second word is what makes it a tag; on its own the first is an
        // ordinary word, and dropping it would cost the rest of the title.
        assert_eq!(
            parsed("Quiet Harbour Web Of Lies.mkv").title,
            "Quiet Harbour Web Of Lies"
        );
    }

    #[test]
    fn a_marker_shouting_at_the_end_of_a_title_is_not_part_of_it() {
        let result = parsed("Quiet Harbour By The Sea SOMEGROUP.mkv");
        assert_eq!(result.title, "Quiet Harbour By The Sea");
        assert_eq!(result.year, None);
    }

    #[test]
    fn the_word_naming_the_cut_is_not_part_of_the_title() {
        // Written before the year, where the boundary rule cannot reach it.
        let shouted = parsed("Quiet Harbour EXTENDED (2019) MULTi 1080p.mkv");
        assert_eq!(shouted.title, "Quiet Harbour");
        assert_eq!(shouted.year, Some(2019));

        let spoken = parsed("Quiet Harbour Extended.mkv");
        assert_eq!(spoken.title, "Quiet Harbour");

        let stacked = parsed("Quiet Harbour Remastered Uncut (2019) 1080p.mkv");
        assert_eq!(stacked.title, "Quiet Harbour", "however many are piled up");
    }

    #[test]
    fn the_two_words_naming_a_cut_go_together() {
        // The second word on its own is an ordinary word, and the first is
        // what makes the pair mean an edition rather than a title.
        assert_eq!(
            parsed("Quiet Harbour Directors Cut (2019) 1080p.mkv").title,
            "Quiet Harbour"
        );
        assert_eq!(
            parsed("Quiet Harbour 2 Director's Cut (2020) 1080p.mkv").title,
            "Quiet Harbour 2"
        );
        assert_eq!(
            parsed("Quiet Harbour 3 Extreme Edition (2021) 1080p.mkv").title,
            "Quiet Harbour 3"
        );
        assert_eq!(
            parsed("Quiet Harbour Collectors Edition.mkv").title,
            "Quiet Harbour",
            "whatever qualifies an edition belongs to the edition"
        );
    }

    #[test]
    fn a_title_that_really_ends_in_such_a_word_keeps_it() {
        // The reason only two spellings are listed for the last word: films
        // are called this, and a word dropped wrongly is a film nobody finds.
        assert_eq!(
            parsed("The Final Cut (2004) 1080p.mkv").title,
            "The Final Cut"
        );
        assert_eq!(parsed("The Last Cut.mkv").title, "The Last Cut");
    }

    #[test]
    fn a_year_welded_to_the_word_after_it_is_still_a_year() {
        // A name written by hand loses a separator now and then. Without this
        // the boundary is gone and the whole description lands in the title.
        let result = parsed("Quiet.Harbour.2019MULTi.TRUEFRENCH.1080p.x264.mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert_eq!(result.year, Some(2019));
        assert!(result.tags.contains("multi"));
    }

    #[test]
    fn a_number_welded_to_an_ordinary_word_is_left_alone() {
        // Only ever split when what follows is already known to describe a
        // file: a title carrying a number keeps it, whatever it is glued to.
        let result = parsed("Quiet Harbour 2019Nights.mkv");
        assert_eq!(result.title, "Quiet Harbour 2019Nights");
        assert_eq!(result.year, None);
    }

    #[test]
    fn a_second_title_in_brackets_is_an_aside_and_not_part_of_the_title() {
        // The commonest is the title in another language written next to the
        // one the file is named after. Handing both to a provider at once
        // finds nothing at all.
        let result = parsed("Quiet Harbour [Le Port Tranquille] (2019) 1080p x264.mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert_eq!(result.year, Some(2019));

        assert_eq!(
            parsed("Quiet Harbour {Le Port Tranquille} (2019).mkv").title,
            "Quiet Harbour"
        );
    }

    #[test]
    fn a_single_bracketed_word_is_left_to_the_rules_that_already_read_it() {
        // That is how a year is written, and how a tag is written.
        let result = parsed("Quiet Harbour [2019] 1080p.mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert_eq!(result.year, Some(2019));
    }

    #[test]
    fn a_title_made_only_of_such_words_keeps_them_rather_than_vanishing() {
        // Whatever it is, a film with no title left is a film nobody finds.
        assert_eq!(parsed("Extended.mkv").title, "Extended");
    }

    #[test]
    fn the_mark_a_library_signs_with_is_learnt_from_the_library() {
        // Nothing about the shape of a word tells a signature from a title, so
        // it is read off the names themselves: whatever keeps ending them is
        // not part of any title.
        let names = [
            "Quiet Harbour (2019) MULTi 1080p SOMEGROUP.mkv",
            "Amber Field (2020) 1080p SOMEGROUP.mkv",
            "Winter Signal 2160p SOMEGROUP.mkv",
            "The Long Road North (2018) x264.mkv",
        ];
        let marks = signs_in(&names).marks;
        assert!(marks.contains("somegroup"), "{marks:?}");
        assert!(!marks.contains("x264"), "a definition is not a signature");
    }

    #[test]
    fn a_thing_stuck_to_the_front_of_a_name_is_recognised_by_the_copy_without_it() {
        // Nothing about the shape of these says anything. What says it is that
        // the very same name is there on its own: the proof that the rest was
        // added is the file that does not carry it.
        let names = [
            "Quiet Harbour BD.Rip 1080 x264.mkv",
            "zz12Quiet Harbour BD.Rip 1080 x264.mkv",
            "wxyzQuiet Harbour BD.Rip 1080 x264.mkv",
            "Amber Field BD.Rip 1080 x264.mkv",
            "zz12Amber Field BD.Rip 1080 x264.mkv",
        ];
        let signs = signs_in(&names);
        assert_eq!(
            signs.prefixes,
            ["wxyz".to_string(), "zz12".to_string()]
                .into_iter()
                .collect()
        );

        for name in names {
            let read = parse_signed(name, NOW, &signs);
            assert!(
                read.title == "Quiet Harbour" || read.title == "Amber Field",
                "{name} read as {}",
                read.title
            );
        }
    }

    #[test]
    fn a_film_whose_name_is_another_with_a_word_in_front_keeps_that_word() {
        // Two real films, one called what the other is called with a word
        // before it. Everything else about the names is identical, which is
        // exactly the shape a prefix has. What tells them apart is the space:
        // a piece that ends at a word boundary is a word.
        let names = [
            "Harbour (2019) 1080p x264.mkv",
            "Quiet Harbour (2019) 1080p x264.mkv",
        ];
        let signs = signs_in(&names);
        assert!(signs.prefixes.is_empty(), "{:?}", signs.prefixes);
        assert_eq!(
            parse_signed(names[1], NOW, &signs).title,
            "Quiet Harbour",
            "a word of a real title must never be taken for a sign"
        );
    }

    #[test]
    fn nothing_is_stripped_from_a_library_where_no_copy_proves_it() {
        // The same odd names, with no plain copy anywhere to prove that the
        // front of them was added. Guessing here would cost a real title.
        let names = [
            "zz12Quiet Harbour BD.Rip 1080 x264.mkv",
            "zz12Amber Field BD.Rip 1080 x264.mkv",
        ];
        let signs = signs_in(&names);
        assert!(signs.prefixes.is_empty(), "{:?}", signs.prefixes);
        assert_eq!(
            parse_signed(names[0], NOW, &signs).title,
            "zz12Quiet Harbour"
        );
    }

    #[test]
    fn the_longest_sign_that_fits_is_the_one_taken_off() {
        let signs = LibrarySigns {
            prefixes: ["zz".to_string(), "zz12".to_string()].into_iter().collect(),
            ..LibrarySigns::default()
        };
        assert_eq!(
            parse_signed("zz12Quiet Harbour 1080p.mkv", NOW, &signs).title,
            "Quiet Harbour"
        );
    }

    #[test]
    fn a_sequel_number_is_never_taken_for_a_signature() {
        // Several titles end in the same number, which is exactly what a
        // signature looks like from a distance and is the opposite of one.
        let names = [
            "Quiet Harbour 2.mkv",
            "Amber Field 2.mkv",
            "Winter Signal 2.mkv",
            "Harbour Rising II.mkv",
            "Amber Rising II.mkv",
            "Signal Rising II.mkv",
        ];
        let marks = signs_in(&names).marks;
        assert!(marks.is_empty(), "{marks:?}");
    }

    #[test]
    fn a_known_mark_leaves_a_title_that_is_shouting_as_loudly_as_it_is() {
        // The case nothing else can reach: every word is in capitals, so no
        // rule of shape can tell the signature from the title. Knowing the
        // mark is what makes it possible.
        let signs = signs_in(&[
            "QUIET HARBOUR CONTRE ATTAQUE SOMEGROUP.mkv",
            "Amber Field (2020) 1080p SOMEGROUP.mkv",
            "Winter Signal 2160p SOMEGROUP.mkv",
        ]);

        let read = parse_signed("QUIET HARBOUR CONTRE ATTAQUE SOMEGROUP.mkv", NOW, &signs);
        assert_eq!(read.title, "QUIET HARBOUR CONTRE ATTAQUE");
        assert_eq!(
            parsed("QUIET HARBOUR CONTRE ATTAQUE SOMEGROUP.mkv").title,
            "QUIET HARBOUR CONTRE ATTAQUE SOMEGROUP",
            "and without knowing the mark there is nothing to go on"
        );
    }

    #[test]
    fn a_title_made_only_of_the_mark_keeps_it_rather_than_vanishing() {
        let signs = LibrarySigns {
            marks: ["somegroup".to_string()].into_iter().collect(),
            ..LibrarySigns::default()
        };
        assert_eq!(
            parse_signed("SOMEGROUP.mkv", NOW, &signs).title,
            "SOMEGROUP"
        );
    }

    #[test]
    fn a_title_written_wholly_in_capitals_keeps_every_word() {
        // Nothing stands out, so nothing is dropped: guessing here would cost
        // a word of the title itself.
        let result = parsed("QUIET HARBOUR RISING.mkv");
        assert_eq!(result.title, "QUIET HARBOUR RISING");
    }

    #[test]
    fn a_sequel_number_at_the_end_belongs_to_the_title() {
        let result = parsed("Quiet Harbour II.mkv");
        assert_eq!(
            result.title, "Quiet Harbour II",
            "a roman numeral shouts like a marker and is the opposite of one"
        );
    }

    #[test]
    fn a_name_with_no_year_still_yields_a_usable_title() {
        let result = parsed("Some Documentary Without A Year.mkv");
        assert_eq!(result.title, "Some Documentary Without A Year");
        assert_eq!(result.year, None);
        assert!(result.tags.is_empty());
    }

    #[test]
    fn a_year_at_the_very_end_of_a_name_is_still_the_year() {
        // `Title Year` and nothing else is one of the commonest names there
        // is, and it used to be read as a title carrying a number. Asked for
        // such a title with the year still stuck to it, the provider answers
        // nothing at all: measured on real films, twenty five times out of
        // twenty eight. Reading it as part of the title was not the cautious
        // answer, it was the one that never found the film.
        let result = parsed("Quiet Harbour 2019.mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert_eq!(result.year, Some(2019));

        // A number that could not be a year is still part of the title.
        let carried = parsed("Harbour 2049.mkv");
        assert_eq!(carried.title, "Harbour 2049");
        assert_eq!(carried.year, None);

        // What it costs, said out loud: a film whose own title ends in a year
        // and whose name says nothing else loses it. Nothing in such a name
        // tells the two apart, and the reading that finds the other films is
        // the one taken.
        let cost = parsed("Harbour 1984.mkv");
        assert_eq!(cost.title, "Harbour");
        assert_eq!(cost.year, Some(1984));
    }

    #[test]
    fn a_name_starting_with_a_year_keeps_it_rather_than_ending_up_untitled() {
        // The year is the whole title here, and the word after it names the
        // cut. Both rules agree on what is left, and what is left is not
        // nothing: a film with no title is a film nobody finds again.
        let result = parse("2019.Remastered.mkv", NOW);
        assert_eq!(result.title, "2019");
        assert_eq!(result.year, None);

        let plain = parse("2019.Harbour.mkv", NOW);
        assert_eq!(plain.title, "2019 Harbour");
        assert_eq!(plain.year, None);
    }

    #[test]
    fn a_film_from_this_year_or_the_next_is_still_a_film() {
        for year in [NOW - 1, NOW, NOW + 1] {
            let result = parse(&format!("Quiet.Harbour.{year}.1080p.mkv"), NOW);
            assert_eq!(result.year, Some(year), "a film from {year}");
            assert_eq!(result.title, "Quiet Harbour");
        }
    }

    #[test]
    fn the_year_itself_never_lands_among_the_markers() {
        let result = parse("Quiet.Harbour.2019.MULTi.1080p.mkv", NOW);
        assert_eq!(result.year, Some(2019));
        assert!(
            !result.tags.contains("2019"),
            "the year is a field of its own, not a marker: {:?}",
            result.tags
        );
        assert!(result.tags.contains("multi") && result.tags.contains("1080p"));
    }

    #[test]
    fn an_implausible_number_is_not_taken_for_a_year() {
        let result = parsed("Room.1408.Revisited.mkv");
        assert_eq!(result.year, None);
        assert_eq!(result.title, "Room 1408 Revisited");
    }

    #[test]
    fn a_year_further_ahead_than_announced_titles_go_is_refused() {
        let result = parse("Harbour.2999.Extended.mkv", NOW);
        assert_eq!(result.year, None);
    }

    #[test]
    fn a_name_using_spaces_and_brackets_parses_as_well_as_a_dotted_one() {
        let result = parsed("Quiet Harbour 2019 1080p BluRay.mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert_eq!(result.year, Some(2019));
    }

    #[test]
    fn an_accent_written_as_a_mark_of_its_own_folds_like_any_other() {
        // The same title twice, spelled the two ways a file can carry it: one
        // character for the accented letter, or the plain letter followed by
        // the accent. They look identical and are different text.
        let one_character = "La ru\u{e9}e vers l'or";
        let letter_then_accent = "La rue\u{301}e vers l'or";
        assert_ne!(one_character, letter_then_accent, "different text");

        assert_eq!(
            fold_accents(one_character),
            fold_accents(letter_then_accent)
        );
        assert_eq!(
            sort_title(one_character),
            sort_title(letter_then_accent),
            "or the same film sorts in two places and never matches itself"
        );
        assert!(sort_title(letter_then_accent).is_ascii());
    }

    #[test]
    fn two_spellings_of_one_title_compare_as_the_same_title() {
        // What whoever named the file did to the punctuation, and what no
        // provider ever does: a colon dropped, a hyphenated name written as
        // two words, an apostrophe of another shape.
        let same = |left: &str, right: &str| matchable_title(left) == matchable_title(right);
        assert!(same(
            "Quiet Harbour: Rising Tide",
            "Quiet Harbour Rising Tide"
        ));
        assert!(same("Amber-Field", "Amber Field"));
        assert!(same("L'Auberge du Nord", "L\u{2019}Auberge du Nord"));
        assert!(same("Le Dernier Été", "le dernier ete"));
        assert!(same("Harbour  Rising ", "Harbour Rising"));
    }

    #[test]
    fn two_words_never_become_one_when_titles_are_compared() {
        // The line this must not cross: dropping a hyphen without leaving a
        // space would make two different names look like one.
        assert!(matchable_title("Amber-Field") != matchable_title("Amberfield"));
        assert_eq!(matchable_title("Amber-Field"), ["amber", "field"]);
    }

    #[test]
    fn an_apostrophe_goes_without_leaving_a_space_behind_it() {
        // The one mark that sits inside a word rather than between two, and
        // the one a file name drops most often.
        assert_eq!(
            matchable_title("Don't Breathe 2"),
            matchable_title("Dont Breathe 2")
        );
        assert_eq!(matchable_title("L'Auberge"), ["lauberge"]);
    }

    #[test]
    fn a_sequel_numbered_in_roman_numerals_is_the_number_it_is() {
        // One hand writes it one way and the other hand the other way, name
        // after name, and it is the same number.
        assert_eq!(
            matchable_title("Quiet Harbour II"),
            matchable_title("Quiet Harbour 2")
        );
        assert_eq!(
            matchable_title("Quiet Harbour VIII"),
            matchable_title("Quiet Harbour 8")
        );
    }

    #[test]
    fn a_word_that_merely_looks_like_roman_numerals_is_left_alone() {
        // Plenty of real words are made only of those letters, and none of
        // them is a number.
        assert_eq!(matchable_title("Mix"), ["mix"]);
        assert_eq!(matchable_title("Civil"), ["civil"]);
    }

    #[test]
    fn how_alike_counts_the_words_both_titles_use_and_neither_one_alone() {
        let alike =
            |left: &str, right: &str| how_alike(&matchable_title(left), &matchable_title(right));

        assert_eq!(alike("Quiet Harbour", "Quiet Harbour"), 1.0);
        // One word of five differs, which is one hand writing a conjunction
        // the other hand wrote as a sign.
        assert!(alike("Amber And Field Rising Tide", "Amber Field Rising Tide") > 0.7);
        // A title holding every word of another and several more besides is a
        // different title, which counting one way only would miss.
        assert!(alike("Quiet Harbour", "Quiet Harbour The Making Of It All") < 0.5);
        assert_eq!(alike("Quiet Harbour", "Amber Field"), 0.0);
        assert_eq!(alike("", "Quiet Harbour"), 0.0);
    }

    #[test]
    fn a_title_with_no_accent_at_all_is_told_apart_from_one_that_has_them() {
        assert!(!carries_accents("Quiet Harbour"));
        assert!(carries_accents("La ru\u{e9}e vers l'or"));
        assert!(carries_accents("La rue\u{301}e vers l'or"));
    }

    #[test]
    fn a_shorthand_somebody_numbered_a_series_with_comes_off_the_front() {
        // Written every way people write it, welded or spaced.
        for written in [
            "QH1-Quiet Harbour",
            "QH1_Quiet Harbour",
            "QH1 Quiet Harbour",
            "AF12-Quiet Harbour",
        ] {
            assert_eq!(
                without_a_shorthand_prefix(written).as_deref(),
                Some("Quiet Harbour"),
                "{written}"
            );
        }
    }

    #[test]
    fn a_name_that_is_how_a_longer_one_begins_is_told_from_one_that_is_not() {
        let words = |title: &str| matchable_title(title);
        let begins =
            |short: &str, long: &str| one_is_how_the_other_begins(&words(short), &words(long));

        // The shape this exists for: a subtitle the file name left out, of any
        // length at all, which is the whole point.
        assert!(begins(
            "Quiet Harbour 5",
            "Quiet Harbour 5 : The Ties of Blood"
        ));
        assert!(begins(
            "Quiet Harbour 4",
            "Quiet Harbour 4 : Bloody Origins"
        ));
        assert!(begins("Quiet Harbour", "Quiet Harbour The Long Way Round"));
        // Either way round.
        assert!(begins(
            "Quiet Harbour 5 : The Ties of Blood",
            "Quiet Harbour 5"
        ));

        // A sequel is not a subtitle, whichever way it is numbered.
        assert!(!begins("Quiet Harbour", "Quiet Harbour 2"));
        assert!(!begins("Quiet Harbour", "Quiet Harbour II"));
        // Word for word from the start, so the middle of a title is not a
        // beginning of it.
        assert!(!begins("Harbour 5", "Quiet Harbour 5"));
        // The same title is not this question.
        assert!(!begins("Quiet Harbour", "Quiet Harbour"));
        assert!(!begins("", "Quiet Harbour"));
        // And two titles that merely share their first word are not one film.
        assert!(!begins("Quiet Harbour", "Amber Field"));
    }

    #[test]
    fn a_candidate_offering_a_sequel_is_told_from_a_search_naming_its_first_film() {
        let words = |title: &str| matchable_title(title);
        let is_a_sequel_offered = |candidate: &str, searched: &str| {
            candidate_is_a_later_instalment(&words(candidate), &words(searched))
        };

        // The bug this exists for: a sequel offered up as the film that was
        // searched for. `Quiet Harbour 2` shares every word `Quiet Harbour`
        // has, so it reads as alike as anything ever does, and two instalments
        // come out a year apart as readily as not, so the year does not save
        // it either.
        assert!(is_a_sequel_offered("Quiet Harbour 2", "Quiet Harbour"));
        assert!(is_a_sequel_offered("Quiet Harbour II", "Quiet Harbour"));
        assert!(is_a_sequel_offered(
            "Quiet Harbour 2 The Return",
            "Quiet Harbour"
        ));

        // **The direction matters, and this is the other one.** A collection
        // often numbers even its first film on disk, `Bad Boys 1` for a film
        // simply called `Bad Boys`: measured on a real collection where this
        // very shape refused the film outright, with its year sitting right
        // there in agreement. The search carries the number here, not the
        // candidate, so nothing may be refused.
        assert!(!is_a_sequel_offered("Bad Boys", "Bad Boys 1"));
        assert!(!is_a_sequel_offered("Quiet Harbour", "Quiet Harbour 2"));

        // A subtitle is never one of these either, which is what keeps the two
        // rules from ever answering yes together.
        assert!(!is_a_sequel_offered(
            "Quiet Harbour 5 : The Ties of Blood",
            "Quiet Harbour 5"
        ));
        assert!(!is_a_sequel_offered("Amber Field", "Quiet Harbour"));
    }

    #[test]
    fn a_title_that_merely_looks_like_one_keeps_every_word_it_has() {
        // Each of these is the shape a shorthand has, and each is a title. They
        // find themselves on the question asked before this one, and this one
        // must not offer a shortened name for them even so.
        for real in [
            // Letters alone in front: a word of the title, not a shorthand.
            "WALL-E",
            "Spider-Man",
            // Digits alone: a number of the title.
            "2 Fast 2 Furious",
            "9-1-1",
            // Nothing after it to ask about.
            "QH1",
            "QH1-",
            // Too long to be one.
            "Harbour1 Quiet",
            // Not a name at all on the other side.
            "QH1-77",
        ] {
            assert_eq!(without_a_shorthand_prefix(real), None, "{real}");
        }
    }

    #[test]
    fn the_ordering_title_moves_articles_out_of_the_way_and_folds_accents() {
        assert_eq!(sort_title("The Long Road"), "long road");
        assert_eq!(sort_title("Le Dernier Été"), "dernier ete");
        assert_eq!(sort_title("Une Histoire Simple"), "histoire simple");
        assert_eq!(sort_title("L'Auberge"), "auberge");
        assert_eq!(sort_title("Élodie"), "elodie");
    }

    #[test]
    fn a_title_starting_with_a_word_that_merely_looks_like_an_article_is_left_alone() {
        assert_eq!(sort_title("Lest We Forget"), "lest we forget");
        assert_eq!(sort_title("Anna"), "anna");
    }

    #[test]
    fn video_files_are_recognised_and_the_rest_is_left_alone() {
        assert!(is_video_file("film.mkv"));
        assert!(is_video_file("film.MP4"));
        assert!(is_video_file("film.m2ts"));
        assert!(!is_video_file("cover.jpg"));
        assert!(!is_video_file("notes.txt"));
        assert!(!is_video_file("film.nfo"));
    }

    #[test]
    fn a_partly_downloaded_file_is_skipped_rather_than_analysed_as_truncated() {
        assert!(!is_video_file("film.mkv.part"));
        assert!(!is_video_file("film.mkv.!qb"));
        assert!(!is_video_file(".hidden.mkv"));
    }

    #[test]
    fn trailers_and_sample_clips_are_recognised_as_companions() {
        assert_eq!(
            is_companion_clip("Quiet.Harbour-trailer.mkv"),
            Some("trailer")
        );
        assert_eq!(
            is_companion_clip("Quiet Harbour trailer.mp4"),
            Some("trailer")
        );
        assert_eq!(is_companion_clip("sample.mkv"), Some("sample"));
        assert_eq!(
            is_companion_clip("Quiet.Harbour-sample.mkv"),
            Some("sample")
        );
        assert_eq!(is_companion_clip("Quiet.Harbour.2019.mkv"), None);
        assert_eq!(
            is_companion_clip("Quiet.Harbour.2019-TRAILER.mkv"),
            Some("trailer"),
            "a marker shouted in capitals is the same marker"
        );
    }

    #[test]
    fn a_companion_gives_up_the_name_of_the_film_it_belongs_to() {
        assert_eq!(
            without_companion_marker("Quiet.Harbour.2019-trailer.mkv").as_deref(),
            Some("Quiet.Harbour.2019.mkv")
        );
        assert_eq!(
            without_companion_marker("Amber Field 2020 trailer.mp4").as_deref(),
            Some("Amber Field 2020.mp4")
        );
        assert_eq!(
            without_companion_marker("Quiet.Harbour.2019.mkv"),
            None,
            "a film is not a companion of anything"
        );
        assert_eq!(
            without_companion_marker("sample.mkv"),
            None,
            "a clip named only sample says nothing about which film it belongs to"
        );
    }

    #[test]
    fn the_name_taken_from_a_companion_reads_like_the_film_itself() {
        // A trailer carries the whole name of its film, tags included, which is
        // what lets one set of rules read both.
        let base = without_companion_marker("Quiet.Harbour.2019.MULTi.1080p-trailer.mkv")
            .expect("a companion");
        assert_eq!(base, "Quiet.Harbour.2019.MULTi.1080p.mkv");

        let film = parsed(&base);
        assert_eq!(film.title, "Quiet Harbour");
        assert_eq!(film.year, Some(2019));
        assert_eq!(film, parsed("Quiet.Harbour.2019.MULTi.1080p.mkv"));
    }

    #[test]
    fn an_extension_like_ending_is_only_stripped_when_it_really_is_one() {
        // A title ending in a short word must not lose it.
        assert_eq!(strip_extension("Quiet.Harbour.mkv"), "Quiet.Harbour");
        assert_eq!(strip_extension("Quiet.Harbour"), "Quiet.Harbour");
        assert_eq!(
            strip_extension("Quiet.Harbour.Rising"),
            "Quiet.Harbour.Rising"
        );
    }

    #[test]
    fn every_shape_the_collection_actually_uses_parses_correctly() {
        // One case per shape observed in the real collection, written with
        // invented titles. Extended whenever a new shape turns up.
        let cases: [(&str, &str, Option<i32>); 7] = [
            (
                "Amber Field BD Rip.avi",
                "Amber Field",
                None,
            ),
            (
                "Quiet.Harbour.Rising.2019.MULTi.TRUEFRENCH.1080p.BluRay.x264_MYTAG.mkv",
                "Quiet Harbour Rising",
                Some(2019),
            ),
            (
                "Quiet.Harbour.Second.Tide.2018.1080p.x264.MULTI_MYTAG.mkv",
                "Quiet Harbour Second Tide",
                Some(2018),
            ),
            (
                "Amber.field.2016.MULTi.1080p.x264_MYTAG.mkv",
                "Amber field",
                Some(2016),
            ),
            (
                "Amber.Field.Shadow.Of.The.Pines.2020.MULTi.1080p_new.mkv",
                "Amber Field Shadow Of The Pines",
                Some(2020),
            ),
            (
                "Harbour.v.Lighthouse.Dawn.Of.Tides.2016.VL.MULTi.VFF.1080p.BluRay.AV1.TrueHD.Atmos.7.1-SOMEGROUP.mkv",
                "Harbour v Lighthouse Dawn Of Tides",
                Some(2016),
            ),
            (
                "Quiet Harbour.mkv",
                "Quiet Harbour",
                None,
            ),
        ];

        for (name, expected_title, expected_year) in cases {
            let result = parse(name, NOW);
            assert_eq!(result.title, expected_title, "title of {name}");
            assert_eq!(result.year, expected_year, "year of {name}");
        }
    }

    #[test]
    fn a_group_that_signs_in_front_of_the_title_is_not_the_title() {
        // Square brackets and braces only. A title really can begin with a
        // number in round brackets, and there is a well known film that does.
        let signed = parsed("[SOMEGROUP] Quiet Harbour (2019) 1080p.mkv");
        assert_eq!(signed.title, "Quiet Harbour");
        assert_eq!(signed.year, Some(2019));

        let round = parsed("(500) Days Of Summer (2009) 1080p.mkv");
        assert_eq!(
            round.title, "(500) Days Of Summer",
            "round brackets belong to titles as well as to releases"
        );
        assert_eq!(round.year, Some(2009));

        let nothing_else = parsed("[SOMEGROUP].mkv");
        assert_eq!(
            nothing_else.title, "[SOMEGROUP]",
            "a film with no title left is a film nobody finds again"
        );
    }

    #[test]
    fn the_address_of_a_site_written_in_front_of_a_name_is_taken_off() {
        for name in [
            "www.Somewhere.com - Quiet.Harbour.2019.1080p.WEB-DL.mkv",
            "[ www.somewhere.cd ] Quiet.Harbour.2019.1080p.WEB-DL.mkv",
            "www.somewhere.co.uk_Quiet.Harbour.2019.1080p.WEB-DL.mkv",
        ] {
            let result = parse(name, NOW);
            assert_eq!(result.title, "Quiet Harbour", "title of {name}");
            assert_eq!(result.year, Some(2019), "year of {name}");
        }

        // Nothing that does not say it is an address. A dot separates words
        // in these names, so `Dr.No` is an address as good as any and a film
        // would lose its title to a rule that went that far.
        let doctor = parsed("Dr.No.1962.1080p.BluRay.mkv");
        assert_eq!(doctor.title, "Dr No");
        assert_eq!(doctor.year, Some(1962));
    }

    #[test]
    fn a_year_written_in_front_of_the_title_is_read_where_it_stands() {
        let result = parsed("(2009) Quiet Harbour 1080p BluRay.mkv");
        assert_eq!(result.title, "Quiet Harbour");
        assert_eq!(result.year, Some(2009));

        // It settles the question by itself: a number in the title that reads
        // like a year no longer outranks the date somebody wrote down.
        let carried = parsed("(2020) Quiet Harbour 1984 1080p BluRay.mkv");
        assert_eq!(carried.title, "Quiet Harbour 1984");
        assert_eq!(carried.year, Some(2020));

        // Written bare it stays where it is: a film really can be called 1917.
        let bare_one = parsed("2019 Quiet Harbour 1080p.mkv");
        assert_eq!(bare_one.title, "2019 Quiet Harbour");
        assert_eq!(bare_one.year, None);
    }

    #[test]
    fn a_title_written_wholly_in_capitals_keeps_every_word_of_itself() {
        // What this catches is the word in front rather than the word taken
        // off: a number, a word of one letter, and a word spelt out of the
        // letters a sequel is numbered with all used to say the title did not
        // shout, and the last word of the title went with it.
        for (name, expected) in [
            ("12.ANGRY.MEN.1957.1080P.BLURAY.MKV", "12 ANGRY MEN"),
            (
                "A.CLOCKWORK.ORANGE.1971.1080P.BLURAY.MKV",
                "A CLOCKWORK ORANGE",
            ),
            (
                "DIX.POUR.CENT.LE.FILM.2021.1080P.MKV",
                "DIX POUR CENT LE FILM",
            ),
        ] {
            assert_eq!(parse(name, NOW).title, expected, "title of {name}");
        }

        // And the marker is still taken off a title that does not shout.
        let signed = parsed("Quiet Harbour SOMEGROUP.mkv");
        assert_eq!(signed.title, "Quiet Harbour");
    }

    #[test]
    fn a_separator_left_standing_between_the_title_and_the_year_is_dropped() {
        let result = parsed("Quiet Harbour - 2019 - 720p.mp4");
        assert_eq!(result.title, "Quiet Harbour");
        assert_eq!(result.year, Some(2019));
    }

    /// Real films, named every way a real film is named.
    ///
    /// The rest of this test set is invented on purpose, because the
    /// maintainer's own files must never reach the repository. These titles
    /// are neither his nor invented: they are read off a public catalogue of
    /// films, and they are here because an invented title is always the shape
    /// its inventor had in mind. A real catalogue holds titles that are only a
    /// year, titles carrying a year of their own, titles made of one letter
    /// words, titles written in brackets and titles that end in a word this
    /// reading takes for a technical one, and every one of those broke
    /// something below.
    ///
    /// The names are built rather than listed: one film crossed with every
    /// convention gives a thousand names out of forty lines, and a convention
    /// that has to be spelled out once is a convention somebody can read.
    mod names_of_films_that_exist {
        use super::*;

        /// Films that exist, chosen for the shapes their names take.
        ///
        /// Each of these is a case that cost something: `12 Angry Men` and
        /// `A Clockwork Orange` lost their last word on a name written wholly
        /// in capitals, `(500) Days of Summer` lost the front of its name to
        /// the rule that drops a group's brackets, `Une Bataille après
        /// l'autre` killed the scan outright, and `Blade Runner 2049` and
        /// `1917` are the two ways a title carries a number that reads like a
        /// year.
        const FILMS: &[(&str, i32)] = &[
            ("Inception", 2010),
            ("Parasite", 2019),
            ("12 Angry Men", 1957),
            ("(500) Days of Summer", 2009),
            ("8 Mile", 2002),
            ("10 Cloverfield Lane", 2016),
            ("1917", 2019),
            ("2012", 2009),
            ("Blade Runner 2049", 2017),
            ("The Godfather Part II", 1974),
            ("Rocky IV", 1985),
            ("Mad Max: Fury Road", 2015),
            ("Amélie", 2001),
            ("Léon: The Professional", 1994),
            ("La Haine", 1995),
            ("Ocean's Eleven", 2001),
            ("Uncut Gems", 2019),
            ("The Final Cut", 2004),
            ("A Clockwork Orange", 1971),
            ("M*A*S*H", 1970),
            ("Se7en", 1995),
            ("Fahrenheit 9/11", 2004),
            ("Face/Off", 1997),
            ("Guardians of the Galaxy Vol. 2", 2017),
            (
                "Dr. Strangelove or: How I Learned to Stop Worrying and Love the Bomb",
                1964,
            ),
            ("The Good, the Bad and the Ugly", 1966),
            ("2001: A Space Odyssey", 1968),
            ("Spirited Away", 2001),
            ("Kill Bill: Vol. 1", 2003),
            ("The Lord of the Rings: The Return of the King", 2003),
            ("Three Billboards Outside Ebbing, Missouri", 2017),
            ("Dune: Part Two", 2024),
            ("Everything Everywhere All at Once", 2022),
            ("The Hateful Eight", 2015),
            ("Zack Snyder's Justice League", 2021),
            ("The Intouchables", 2011),
            ("Avengers: Endgame", 2019),
            ("John Wick: Chapter 4", 2023),
            ("Terminator 2: Judgment Day", 1991),
            ("X-Men: Days of Future Past", 2014),
            ("Une Bataille après l'autre", 2025),
        ];

        /// A title as somebody typing a name writes it: what stands between
        /// two words becomes a space, and the rest of the punctuation stays
        /// where it is.
        fn in_words(title: &str) -> String {
            let spaced: String = title
                .chars()
                .map(|c| match c {
                    ':' | '/' => ' ',
                    other => other,
                })
                .collect();
            spaced.split_whitespace().collect::<Vec<_>>().join(" ")
        }

        /// The same title as a release writes it, words joined by dots.
        fn in_dots(title: &str) -> String {
            in_words(title).replace(' ', ".")
        }

        /// Every way one film is named, with the year written in.
        fn every_way(title: &str, year: i32) -> Vec<String> {
            let dots = in_dots(title);
            let words = in_words(title);
            let unders = words.replace(' ', "_");
            vec![
                format!("{dots}.{year}.1080p.BluRay.x264-FGT.mkv"),
                format!("{dots}.{year}.2160p.UHD.BluRay.REMUX.HDR.DTS-HD.MA.5.1-SPARKS.mkv"),
                format!("{dots}.{year}.mkv"),
                format!("{dots}.{year}.MULTi.TRUEFRENCH.1080p.BluRay.x265-EVO.mkv"),
                format!("{dots}.{year}.VOSTFR.HDLight.1080p.x264-GECKOS.mkv"),
                format!("{dots}.{year}.EXTENDED.1080p.BluRay.x264-AMIABLE.mkv"),
                format!("{dots}.{year}.Directors.Cut.REMASTERED.1080p.BluRay.x264-FGT.mkv"),
                format!("{dots}.{year}.PROPER.REPACK.1080p.WEBRip.x264-EVO.mkv"),
                format!("{dots}.{year}.NF.WEB-DL.DDP5.1.Atmos.H.264-playSD.mkv"),
                format!("{dots}.{year}.[1080p].[x264].[AC3]-Tigole.mkv"),
                format!("{dots}.{year}.720p.BluRay.x264.[YTS.MX].mp4"),
                format!("{dots}.{year}1080p.BluRay.x264-QxR.mkv"),
                format!("{}.{year}.1080P.BLURAY.X264-FGT.MKV", dots.to_uppercase()),
                format!("{}.{year}.1080p.bluray.x264-fgt.mkv", dots.to_lowercase()),
                format!("{words} ({year}).mkv"),
                format!("{words} ({year}) [1080p] [BluRay] [x264].mkv"),
                format!("{words} ({year}) - [Bluray-1080p][DTS 5.1][x264]-AMIABLE.mkv"),
                format!("{words} {year} 1080p WEBRip x265 10bit.mkv"),
                format!("{words} {{{year}}} 1080p.mkv"),
                format!("{words} - {year} - 720p.mp4"),
                format!("{unders}_{year}_1080p_WEB-DL.mkv"),
                format!("[SPARKS] {words} ({year}) 1080p.mkv"),
                format!("[YTS.MX] {words} ({year}) [1080p] [BluRay].mp4"),
                format!("www.Torrenting.com - {dots}.{year}.1080p.WEB-DL.mkv"),
                format!("[ www.speed.cd ] {dots}.{year}.1080p.WEB-DL.mkv"),
                format!("({year}) {words} 1080p BluRay.mkv"),
            ]
        }

        /// Whether a name gave back the film it was built from.
        ///
        /// Compared on the words rather than to the letter, which is the
        /// comparison the identification itself makes: a name written without
        /// a colon is the same title, and nothing downstream cares.
        fn reads_as(read: &ParsedName, title: &str) -> bool {
            matchable_title(&read.title) == matchable_title(title)
        }

        #[test]
        fn every_way_a_real_film_is_named_gives_back_its_title_and_its_year() {
            let mut wrong = Vec::new();
            let mut counted = 0;

            for (title, year) in FILMS {
                for name in every_way(title, *year) {
                    counted += 1;
                    let read = parse(&name, NOW);
                    if !reads_as(&read, title) || read.year != Some(*year) {
                        wrong.push(format!(
                            "{name}\n      wanted {title:?} {year}, read {:?} {:?}",
                            read.title, read.year
                        ));
                    }
                }
            }

            assert!(
                wrong.is_empty(),
                "{} of {counted} names were read wrong:\n   {}",
                wrong.len(),
                wrong.join("\n   ")
            );
        }

        #[test]
        fn a_name_that_never_had_a_year_still_gives_back_its_title() {
            // The commonest name with nothing to cut at. The title has to
            // stop at the first word that can only describe the file, and
            // what is left has to be the whole of it.
            let mut wrong = Vec::new();

            for (title, _) in FILMS {
                // Left out: a film whose own title ends in something that
                // reads as a year has nothing in such a name to tell the two
                // apart, and no rule can invent it.
                if title.ends_with(|c: char| c.is_ascii_digit()) {
                    continue;
                }
                for name in [
                    format!("{}.1080p.BluRay.x264-FGT.mkv", in_dots(title)),
                    format!("{} 2160p UHD HDR.mkv", in_words(title)),
                ] {
                    let read = parse(&name, NOW);
                    if !reads_as(&read, title) || read.year.is_some() {
                        wrong.push(format!(
                            "{name}\n      wanted {title:?} and no year, read {:?} {:?}",
                            read.title, read.year
                        ));
                    }
                }
            }

            assert!(wrong.is_empty(), "read wrong:\n   {}", wrong.join("\n   "));
        }

        #[test]
        fn a_word_written_with_an_accent_does_not_bring_the_scan_down() {
            // Not a wrong answer but a panic, which took the whole scan with
            // it. The name was cut at its fourth byte to look for a year
            // welded to what follows, and in `après` the fourth byte is the
            // middle of a letter. Plain French and plain German, and every
            // film after it in the folder went unread.
            for name in [
                "Une.Bataille.après.l'autre.2025.1080p.mkv",
                "La.forêt.2014.MULTi.1080p.BluRay.x264.mkv",
                "Das.Mädchen.mit.der.heißen.Masche.1972.1080p.mkv",
            ] {
                let read = parse(name, NOW);
                assert!(!read.title.is_empty(), "{name} gave up no title at all");
            }
        }
    }
}
