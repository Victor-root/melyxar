//! Tags handed over by another tool, as names and values.
//!
//! The few forms the reader here does not know, WMA and DSD among them, are
//! still read by the analyser of the films, which reports what it found as
//! plain pairs: `artist` = `Amber Field`. Those pairs are read by the same
//! rules as a tag read here, so a song files the same whichever of the two
//! read it.

use crate::Tags;
use crate::values::{cleaned, names, says_yes, year_in};

/// Reads the pairs another tool reported into tags.
///
/// Names are compared without their capitals, since every tag format spells
/// them its own way: `ARTIST` in one, `artist` in another.
pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Tags {
    let pairs: Vec<(String, &str)> = pairs
        .into_iter()
        .map(|(name, value)| (name.to_lowercase(), value))
        .collect();
    let values = |wanted: &[&str]| {
        pairs
            .iter()
            .filter(|(name, _)| wanted.contains(&name.as_str()))
            .map(|(_, value)| *value)
            .collect::<Vec<&str>>()
    };
    let first = |wanted: &[&str]| values(wanted).into_iter().find_map(cleaned);
    let (track, track_total) = number_and_total(first(&["track", "tracknumber"]).as_deref());
    let (disc, disc_total) = number_and_total(first(&["disc", "discnumber"]).as_deref());

    Tags {
        title: first(&["title"]),
        title_sort: first(&["titlesort", "sort_name"]),
        artists: names(values(&["artist"]).into_iter()),
        artist_sort: first(&["artistsort", "sort_artist"]),
        album: first(&["album"]),
        album_sort: first(&["albumsort", "sort_album"]),
        album_artists: names(values(&["album_artist", "albumartist", "album artist"]).into_iter()),
        album_artist_sort: first(&["albumartistsort", "sort_album_artist"]),
        track,
        track_total: track_total
            .or_else(|| first(&["tracktotal", "totaltracks"]).and_then(|total| total.parse().ok())),
        disc,
        disc_total: disc_total
            .or_else(|| first(&["disctotal", "totaldiscs"]).and_then(|total| total.parse().ok())),
        year: first(&["date", "year", "wm/year", "originaldate"]).and_then(|date| year_in(&date)),
        genres: names(values(&["genre"]).into_iter()),
        compilation: first(&["compilation"]).is_some_and(|flag| says_yes(&flag)),
    }
}

/// A number written alone or with its total: `3` or `3/12`.
fn number_and_total(text: Option<&str>) -> (Option<u32>, Option<u32>) {
    let Some(text) = text else {
        return (None, None);
    };
    let mut parts = text.splitn(2, '/');
    let mut read = || {
        parts
            .next()
            .and_then(|part| part.trim().parse::<u32>().ok())
            .filter(|number| *number > 0)
    };
    let number = read();
    (number, read())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_the_analyser_reported_is_read_whatever_the_capitals() {
        let tags = from_pairs([
            ("TITLE", "Quiet Harbour"),
            ("artist", "Amber Field; The Lanterns"),
            ("album", "Northern Lights"),
            ("album_artist", "Amber Field"),
            ("track", "3/12"),
            ("disc", "2"),
            ("date", "2019-05-03"),
            ("genre", "Folk"),
            ("compilation", "1"),
            ("encoder", "Lavf60.16.100"),
        ]);
        assert_eq!(tags.title.as_deref(), Some("Quiet Harbour"));
        assert_eq!(tags.artists, vec!["Amber Field", "The Lanterns"]);
        assert_eq!(tags.album.as_deref(), Some("Northern Lights"));
        assert_eq!(tags.album_artists, vec!["Amber Field"]);
        assert_eq!((tags.track, tags.track_total), (Some(3), Some(12)));
        assert_eq!((tags.disc, tags.disc_total), (Some(2), None));
        assert_eq!(tags.year, Some(2019));
        assert_eq!(tags.genres, vec!["Folk"]);
        assert!(tags.compilation);
    }

    #[test]
    fn nothing_reported_is_no_tag_at_all() {
        assert_eq!(from_pairs([]), Tags::default());
        assert_eq!(from_pairs([("encoder", "Lavf60.16.100")]), Tags::default());
    }

    #[test]
    fn a_year_written_the_way_windows_writes_it_is_read() {
        assert_eq!(from_pairs([("WM/Year", "1987")]).year, Some(1987));
    }

    #[test]
    fn a_number_is_read_alone_or_with_its_total_and_nonsense_is_left_out() {
        assert_eq!(number_and_total(Some("3")), (Some(3), None));
        assert_eq!(number_and_total(Some(" 3 / 12 ")), (Some(3), Some(12)));
        assert_eq!(number_and_total(Some("A/B")), (None, None));
        assert_eq!(number_and_total(Some("0")), (None, None));
        assert_eq!(number_and_total(None), (None, None));
    }
}
