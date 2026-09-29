//! Making plain values out of what a tag holds.
//!
//! Tags are typed by hand, by many programs, over many years, and they carry
//! whatever those left in them: blank values, stray spaces, several names in
//! one field, a full date where a year was meant. What comes out of here is
//! what the rest of the server can take at its word.

/// A value worth keeping: trimmed, and absent when nothing is left.
pub(crate) fn cleaned(text: &str) -> Option<String> {
    let trimmed =
        text.trim_matches(|character: char| character.is_whitespace() || character == '\0');
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Every name held by the values of one field, in the order they came.
///
/// A field holding several names is written in two ways: as several values,
/// which the newer formats allow, or as one value with the names separated by
/// a semicolon, which is how the older ones get around not allowing it. Both
/// are read. Nothing else splits a name: a slash is part of `AC/DC`, and an
/// ampersand or `feat.` is part of how a duo or a guest is billed.
///
/// A name met twice, whatever its capitals, is kept once.
pub(crate) fn names<'a>(values: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for value in values {
        for part in value.split([';', '\0']) {
            let Some(name) = cleaned(part) else {
                continue;
            };
            if !found
                .iter()
                .any(|kept| kept.to_lowercase() == name.to_lowercase())
            {
                found.push(name);
            }
        }
    }
    found
}

/// The year a date starts with.
///
/// A year field holds `2019`, `2019-05-03`, `2019-05-03T00:00:00` or worse,
/// and every one of them starts with the year.
pub(crate) fn year_in(text: &str) -> Option<i32> {
    let digits: String = text.trim().chars().take(4).collect();
    if digits.len() != 4 || !digits.chars().all(|character| character.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok().filter(|year| *year > 0)
}

/// Whether a flag field says yes.
///
/// Written `1` by nearly everything, and `true` or `yes` by the rest.
pub(crate) fn says_yes(text: &str) -> bool {
    matches!(text.trim().to_lowercase().as_str(), "1" | "true" | "yes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_blank_value_is_no_value() {
        assert_eq!(
            cleaned("  Quiet Harbour \0"),
            Some("Quiet Harbour".to_string())
        );
        assert_eq!(cleaned("   "), None);
        assert_eq!(cleaned("\0"), None);
    }

    #[test]
    fn several_names_are_read_from_several_values_or_from_one_split_by_semicolons() {
        assert_eq!(
            names(["Amber Field", "The Lanterns"].into_iter()),
            vec!["Amber Field", "The Lanterns"]
        );
        assert_eq!(
            names(["Amber Field; The Lanterns ;"].into_iter()),
            vec!["Amber Field", "The Lanterns"]
        );
        assert_eq!(
            names(["Amber Field\0The Lanterns"].into_iter()),
            vec!["Amber Field", "The Lanterns"],
            "the separator of the newest tag format"
        );
    }

    #[test]
    fn a_name_is_never_cut_at_a_slash_an_ampersand_or_a_guest() {
        assert_eq!(names(["AC/DC"].into_iter()), vec!["AC/DC"]);
        assert_eq!(names(["Sean & Bobo"].into_iter()), vec!["Sean & Bobo"]);
        assert_eq!(
            names(["Amber Field feat. The Lanterns"].into_iter()),
            vec!["Amber Field feat. The Lanterns"]
        );
    }

    #[test]
    fn a_name_met_twice_is_kept_once() {
        assert_eq!(
            names(["Amber Field", "amber field", "The Lanterns"].into_iter()),
            vec!["Amber Field", "The Lanterns"]
        );
    }

    #[test]
    fn a_year_is_read_off_the_front_of_a_date() {
        assert_eq!(year_in("2019"), Some(2019));
        assert_eq!(year_in("2019-05-03"), Some(2019));
        assert_eq!(year_in(" 1987-01-01T00:00:00 "), Some(1987));
        assert_eq!(year_in("19"), None);
        assert_eq!(year_in("unknown"), None);
        assert_eq!(year_in("0000"), None);
    }

    #[test]
    fn a_flag_says_yes_in_every_way_it_is_written() {
        for yes in ["1", "true", "TRUE", " yes "] {
            assert!(says_yes(yes), "{yes}");
        }
        for no in ["0", "", "false", "no"] {
            assert!(!says_yes(no), "{no}");
        }
    }
}
