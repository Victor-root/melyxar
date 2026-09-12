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
    let stem = strip_extension(file_name);
    // The underscore separates words just like the dot does. Personal markers
    // attach themselves to the previous tag with one, and without this rule
    // the tag becomes unrecognisable and can land in the title.
    let normalised = stem.replace(['.', '_'], " ");
    let words: Vec<&str> = normalised.split_whitespace().collect();

    match find_year(&words, current_year) {
        Some(position) => {
            let title = join_title(&words[..position]);
            let year = words[position].parse().ok();
            let tags = words[position + 1..]
                .iter()
                .flat_map(|word| split_release_group(word))
                .map(|word| word.to_lowercase())
                .filter(|word| !word.is_empty())
                .collect();
            ParsedName { title, year, tags }
        }
        None => ParsedName {
            title: join_title(&words),
            year: None,
            tags: BTreeSet::new(),
        },
    }
}

/// Finds the word holding the release year.
///
/// The *last* plausible year wins, which is what handles a title that carries
/// a year of its own: in such a name the release year comes after the title,
/// so taking the last one keeps the title intact.
///
/// A year is only accepted when something follows it, because a trailing
/// number is far more likely to be part of the title than a release year.
fn find_year(words: &[&str], current_year: i32) -> Option<usize> {
    let latest = current_year + YEARS_AHEAD;
    words
        .iter()
        .enumerate()
        .filter(|(index, word)| {
            // Nothing after it means it is not a boundary.
            *index + 1 < words.len()
                // A title cannot be only a year.
                && *index > 0
                && word.len() == 4
                && word.chars().all(|c| c.is_ascii_digit())
                && word
                    .parse::<i32>()
                    .is_ok_and(|year| (EARLIEST_YEAR..=latest).contains(&year))
        })
        .map(|(index, _)| index)
        .next_back()
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
pub fn fold_accents(value: &str) -> String {
    value
        .chars()
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
    fn a_name_with_no_year_still_yields_a_usable_title() {
        let result = parsed("Some Documentary Without A Year.mkv");
        assert_eq!(result.title, "Some Documentary Without A Year");
        assert_eq!(result.year, None);
        assert!(result.tags.is_empty());
    }

    #[test]
    fn a_trailing_number_is_not_mistaken_for_a_release_year() {
        // Nothing follows it, so it belongs to the title.
        let result = parsed("Harbour 2049.mkv");
        assert_eq!(result.title, "Harbour 2049");
        assert_eq!(result.year, None);

        // The same rule when the number would have been a plausible year:
        // a film called after a year keeps it in its title.
        let plainly_a_title = parse("Harbour 2019.mkv", NOW);
        assert_eq!(plainly_a_title.title, "Harbour 2019");
        assert_eq!(plainly_a_title.year, None);
    }

    #[test]
    fn a_name_starting_with_a_year_keeps_it_rather_than_ending_up_untitled() {
        let result = parse("2019.Remastered.mkv", NOW);
        assert_eq!(
            result.title, "2019 Remastered",
            "a film with no title left is a film nobody finds again"
        );
        assert_eq!(result.year, None);
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
        let cases: [(&str, &str, Option<i32>); 6] = [
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
}
