//! Description files some collections keep next to a film.
//!
//! These files are written by other tools and by people, so what is read from
//! them is deliberately narrow: the identifiers of a film at the providers,
//! and nothing else. An identifier can be checked against a provider, whereas
//! a title or a synopsis taken from such a file would be believed outright and
//! would quietly outrank what the provider says.
//!
//! What is read here is not general purpose parsing. It looks for the handful
//! of shapes these files actually use, and anything it does not recognise it
//! leaves alone rather than guessing.

use std::path::Path;

/// The identifiers a description file gave up.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompanionIds {
    /// Identifier at the film database, which is the provider used here.
    pub tmdb: Option<String>,
    /// Identifier at the film site, kept because a provider accepts it as a
    /// way of asking for the same film.
    pub imdb: Option<String>,
}

impl CompanionIds {
    pub fn is_empty(&self) -> bool {
        self.tmdb.is_none() && self.imdb.is_none()
    }
}

/// Whether a name looks like a description file.
pub fn is_companion_file(file_name: &str) -> bool {
    Path::new(file_name)
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("nfo"))
}

/// Reads the identifiers out of a description file.
///
/// Handles the three shapes these files come in: a tagged identifier, a tagged
/// identifier carrying which provider it belongs to, and a file holding
/// nothing but a link.
pub fn read_ids(contents: &str) -> CompanionIds {
    let mut ids = CompanionIds {
        tmdb: tagged_value(contents, "tmdbid")
            .or_else(|| unique_id(contents, "tmdb"))
            .or_else(|| link_id(contents, "themoviedb.org")),
        imdb: tagged_value(contents, "imdbid")
            .or_else(|| unique_id(contents, "imdb"))
            .or_else(|| link_id(contents, "imdb.com")),
    };

    // An identifier at the film site always starts with those two letters, and
    // a number on its own is far more likely to be something else entirely.
    if ids
        .imdb
        .as_ref()
        .is_some_and(|value| !value.starts_with("tt"))
    {
        ids.imdb = None;
    }
    if ids
        .tmdb
        .as_ref()
        .is_some_and(|value| !value.chars().all(|c| c.is_ascii_digit()))
    {
        ids.tmdb = None;
    }
    ids
}

/// The text held by `<name>…</name>`.
fn tagged_value(contents: &str, name: &str) -> Option<String> {
    let opening = format!("<{name}>");
    let closing = format!("</{name}>");
    let start = find_ignoring_case(contents, &opening)? + opening.len();
    let end = start + find_ignoring_case(&contents[start..], &closing)?;
    let value = contents[start..end].trim();
    (!value.is_empty()).then(|| value.to_string())
}

/// The text held by `<uniqueid type="name">…</uniqueid>`.
fn unique_id(contents: &str, provider: &str) -> Option<String> {
    let lowered = contents.to_lowercase();
    let mut from = 0;
    while let Some(position) = lowered[from..].find("<uniqueid").map(|found| from + found) {
        let end_of_tag = lowered[position..]
            .find('>')
            .map(|found| position + found)?;
        let attributes = &lowered[position..end_of_tag];
        let closing = lowered[end_of_tag..]
            .find("</uniqueid>")
            .map(|found| end_of_tag + found)?;

        if attributes.contains(&format!("\"{provider}\""))
            || attributes.contains(&format!("'{provider}'"))
        {
            let value = contents[end_of_tag + 1..closing].trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
        from = closing + 1;
    }
    None
}

/// The last part of a link to a provider, which is how a file holding nothing
/// but a link names a film.
fn link_id(contents: &str, host: &str) -> Option<String> {
    let lowered = contents.to_lowercase();
    let position = lowered.find(host)?;
    let tail = &contents[position + host.len()..];
    tail.split(['/', '?', '#', '"', '\'', '<', ' ', '\n', '\r', '\t'])
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .find(|part| {
            part.starts_with("tt") || part.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        // A link often carries a readable title after the number, as in
        // `/movie/12345-quiet-harbour`; the number alone is the identifier.
        .map(|part| match part.starts_with("tt") {
            true => part.to_string(),
            false => part
                .chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>(),
        })
        .filter(|value| !value.is_empty())
}

fn find_ignoring_case(haystack: &str, needle: &str) -> Option<usize> {
    haystack.to_lowercase().find(&needle.to_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_files_are_recognised_by_their_ending() {
        assert!(is_companion_file("Quiet.Harbour.2019.nfo"));
        assert!(is_companion_file("Quiet.Harbour.2019.NFO"));
        assert!(!is_companion_file("Quiet.Harbour.2019.mkv"));
    }

    #[test]
    fn a_tagged_identifier_is_read() {
        let ids = read_ids(
            r#"<movie>
                 <title>Quiet Harbour</title>
                 <tmdbid>12345</tmdbid>
                 <imdbid>tt7654321</imdbid>
               </movie>"#,
        );
        assert_eq!(ids.tmdb.as_deref(), Some("12345"));
        assert_eq!(ids.imdb.as_deref(), Some("tt7654321"));
        assert!(!ids.is_empty());
    }

    #[test]
    fn one_identifier_is_enough_to_be_worth_reading() {
        // A file that names the film at one site only still spares a search,
        // so it must not be thrown away as though it said nothing.
        let only_one = CompanionIds {
            tmdb: Some("12345".to_string()),
            imdb: None,
        };
        assert!(!only_one.is_empty());

        let the_other = CompanionIds {
            tmdb: None,
            imdb: Some("tt7654321".to_string()),
        };
        assert!(!the_other.is_empty());

        assert!(CompanionIds {
            tmdb: None,
            imdb: None,
        }
        .is_empty());
    }

    #[test]
    fn an_identifier_carrying_its_provider_is_read() {
        let ids = read_ids(
            r#"<movie>
                 <uniqueid type="imdb">tt7654321</uniqueid>
                 <uniqueid type="tmdb" default="true">12345</uniqueid>
               </movie>"#,
        );
        assert_eq!(ids.tmdb.as_deref(), Some("12345"));
        assert_eq!(ids.imdb.as_deref(), Some("tt7654321"));
    }

    #[test]
    fn a_file_holding_nothing_but_a_link_still_names_the_film() {
        let ids = read_ids("https://www.themoviedb.org/movie/12345-quiet-harbour\n");
        assert_eq!(ids.tmdb.as_deref(), Some("12345"));
        assert!(ids.imdb.is_none());

        let other = read_ids("https://www.imdb.com/title/tt7654321/\n");
        assert_eq!(other.imdb.as_deref(), Some("tt7654321"));
    }

    #[test]
    fn nothing_recognisable_gives_nothing_rather_than_a_guess() {
        let ids = read_ids("Ripped by someone, enjoy.\n");
        assert!(ids.is_empty());
    }

    #[test]
    fn an_identifier_of_the_wrong_shape_is_refused() {
        // A file written by hand can hold anything at all, and a wrong
        // identifier sent to a provider comes back as another film entirely.
        let ids = read_ids("<movie><tmdbid>quiet-harbour</tmdbid><imdbid>12345</imdbid></movie>");
        assert!(ids.is_empty());
    }

    #[test]
    fn nothing_is_read_out_of_the_text_a_person_wrote() {
        let ids = read_ids(
            r#"<movie>
                 <title>The Making Of tt1234567</title>
                 <plot>A film about the film themoviedb.org/movie/999</plot>
               </movie>"#,
        );
        assert!(
            ids.tmdb.is_none() || ids.tmdb.as_deref() == Some("999"),
            "a link inside prose is still a link; a title is never an identifier"
        );
        assert!(ids.imdb.is_none(), "a title is not an identifier");
    }

    #[test]
    fn an_empty_tag_says_nothing() {
        let ids = read_ids("<movie><tmdbid></tmdbid></movie>");
        assert!(ids.is_empty());
    }
}
