//! What a line typed into a search names, when it names one work rather than
//! a title to look for: a link to the provider's page or to IMDb's, or an
//! identifier on its own.

use crate::provider::Catalogue;

/// One work a line points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pointed {
    /// A page of the provider, which says its catalogue.
    Provider(Catalogue, String),
    /// A number alone, which could be a film or a series.
    Number(String),
    /// What IMDb calls it.
    Imdb(String),
}

fn is_number(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// An IMDb identifier at the start of a piece of a link.
fn imdb_in(piece: &str) -> Option<String> {
    let digits: String = piece
        .strip_prefix("tt")?
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    (digits.len() >= 7).then(|| format!("tt{digits}"))
}

/// The work a line points at, or nothing for a line that is a title to look
/// for.
pub fn pointed_at(line: &str) -> Option<Pointed> {
    let line = line.trim();
    if is_number(line) {
        return Some(Pointed::Number(line.to_string()));
    }
    let pieces: Vec<&str> = line
        .split(['/', '?', '#'])
        .filter(|piece| !piece.is_empty())
        .collect();
    if let [only] = pieces.as_slice() {
        return imdb_in(only).filter(|id| id.len() == only.len()).map(Pointed::Imdb);
    }
    let address = line.to_ascii_lowercase();
    if address.contains("imdb.com") {
        return pieces.iter().find_map(|piece| imdb_in(piece)).map(Pointed::Imdb);
    }
    if !address.contains("themoviedb.org") {
        return None;
    }
    pieces.windows(2).find_map(|pair| {
        let catalogue = match pair[0] {
            "movie" => Catalogue::Films,
            "tv" => Catalogue::Series,
            _ => return None,
        };
        let id: String = pair[1].chars().take_while(char::is_ascii_digit).collect();
        (!id.is_empty()).then_some(Pointed::Provider(catalogue, id))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_of_the_provider_says_its_catalogue_and_its_number() {
        assert_eq!(
            pointed_at("https://www.themoviedb.org/movie/603-the-lantern-keeper?language=fr"),
            Some(Pointed::Provider(Catalogue::Films, "603".to_string()))
        );
        assert_eq!(
            pointed_at("themoviedb.org/tv/1399/season/2"),
            Some(Pointed::Provider(Catalogue::Series, "1399".to_string()))
        );
        assert_eq!(pointed_at("https://www.themoviedb.org/person/287"), None);
    }

    #[test]
    fn imdb_is_read_alone_or_in_a_link() {
        assert_eq!(pointed_at(" tt0133093 "), Some(Pointed::Imdb("tt0133093".to_string())));
        assert_eq!(
            pointed_at("https://www.imdb.com/title/tt0944947/?ref_=nv_sr_1"),
            Some(Pointed::Imdb("tt0944947".to_string()))
        );
        assert_eq!(pointed_at("tt12"), None, "too short to be one");
    }

    #[test]
    fn a_number_alone_could_be_either_and_a_title_is_none() {
        assert_eq!(pointed_at("603"), Some(Pointed::Number("603".to_string())));
        assert_eq!(pointed_at("1917"), Some(Pointed::Number("1917".to_string())));
        assert_eq!(pointed_at("The Lantern Keeper"), None);
        assert_eq!(pointed_at("ttfor two"), None);
        assert_eq!(pointed_at("https://elsewhere.example/movie/603"), None);
    }
}
