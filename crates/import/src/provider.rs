//! The identifiers an item has at providers, as Emby and Jellyfin write them:
//! a provider's name in any case, and next to the identifiers the links some
//! sources add under a name of several words.

/// One identifier, the provider in lower case, or none for a link or an
/// empty value.
pub(crate) fn id(provider: &str, id: &str) -> Option<(String, String)> {
    let (provider, id) = (provider.trim(), id.trim());
    (!id.is_empty() && !provider.contains(' ')).then(|| (provider.to_ascii_lowercase(), id.to_string()))
}

/// `Tmdb=603|Imdb=tt0133093|Official Website=...`, for the identifiers alone.
pub(crate) fn ids(written: Option<&str>) -> Vec<(String, String)> {
    written
        .unwrap_or_default()
        .split('|')
        .filter_map(|pair| pair.split_once('='))
        .filter_map(|(provider, value)| id(provider, value))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_identifiers_are_kept_and_the_links_left() {
        assert_eq!(
            ids(Some("Tmdb=603|IMDB=tt0133093|Official Website=https://example.org/x|Tvdb=")),
            vec![("tmdb".to_string(), "603".to_string()), ("imdb".to_string(), "tt0133093".to_string())]
        );
        assert!(ids(None).is_empty());
        assert_eq!(id("MusicBrainzAlbum", " abc "), Some(("musicbrainzalbum".to_string(), "abc".to_string())));
    }
}
