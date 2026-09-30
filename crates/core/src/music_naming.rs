//! The name a song's file is given from its tags, by a pattern a person
//! writes: `{track} - {title}` gives `03 - Quiet Harbour.flac`.
//!
//! Only the name changes, never the folder: the tag manager renames, it does
//! not file songs away into other folders.

/// What a pattern can name a file by.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameValues {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub year: Option<i32>,
}

/// Characters no file name may hold on the disks a server runs on, or on the
/// computers its files are copied to.
const FORBIDDEN: [char; 9] = ['/', '\\', ':', '*', '?', '"', '<', '>', '|'];

/// The file name a pattern gives, with its extension, or nothing when the
/// pattern names a word it does not know, or gives a name with nothing in
/// it. A value the song lacks is left out, and so are the separators it
/// leaves hanging at either end.
pub fn name_from_pattern(pattern: &str, values: &NameValues, extension: &str) -> Option<String> {
    let mut name = String::new();
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        name.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let close = after.find('}')?;
        name.push_str(&value_of(&after[..close], values)?);
        rest = &after[close + 1..];
    }
    name.push_str(rest);

    let cleaned: String = name
        .chars()
        .map(|character| {
            if FORBIDDEN.contains(&character) || character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect();
    let collapsed = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches(|character: char| {
        character == ' ' || character == '-' || character == '_' || character == '.'
    });
    if trimmed.is_empty() {
        return None;
    }
    Some(format!("{trimmed}.{extension}"))
}

fn value_of(placeholder: &str, values: &NameValues) -> Option<String> {
    let text = |value: &Option<String>| value.clone().unwrap_or_default();
    Some(match placeholder.trim().to_lowercase().as_str() {
        "track" => values
            .track
            .map(|track| format!("{track:02}"))
            .unwrap_or_default(),
        "disc" => values.disc.map(|disc| disc.to_string()).unwrap_or_default(),
        "title" => text(&values.title),
        "artist" => text(&values.artist),
        "album" => text(&values.album),
        "albumartist" => text(&values.album_artist),
        "year" => values.year.map(|year| year.to_string()).unwrap_or_default(),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tides() -> NameValues {
        NameValues {
            title: Some("Quiet Harbour".to_string()),
            artist: Some("Amber Field".to_string()),
            album: Some("Northern Lights".to_string()),
            album_artist: Some("Amber Field".to_string()),
            track: Some(3),
            disc: Some(1),
            year: Some(2019),
        }
    }

    #[test]
    fn a_pattern_gives_the_name_of_the_file_with_its_extension() {
        assert_eq!(
            name_from_pattern("{track} - {title}", &tides(), "flac").as_deref(),
            Some("03 - Quiet Harbour.flac")
        );
        assert_eq!(
            name_from_pattern(
                "{disc}-{track} {artist} - {title} ({year})",
                &tides(),
                "mp3"
            )
            .as_deref(),
            Some("1-03 Amber Field - Quiet Harbour (2019).mp3")
        );
        assert_eq!(
            name_from_pattern("{ALBUMARTIST} - {Album} - {title}", &tides(), "m4a").as_deref(),
            Some("Amber Field - Northern Lights - Quiet Harbour.m4a")
        );
    }

    #[test]
    fn a_value_the_song_lacks_leaves_no_separator_hanging() {
        let untracked = NameValues {
            track: None,
            ..tides()
        };
        assert_eq!(
            name_from_pattern("{track} - {title}", &untracked, "flac").as_deref(),
            Some("Quiet Harbour.flac")
        );
    }

    #[test]
    fn what_no_disk_allows_in_a_name_is_taken_out() {
        let odd = NameValues {
            title: Some("AC/DC: Live? \"Yes\"".to_string()),
            ..NameValues::default()
        };
        assert_eq!(
            name_from_pattern("{title}", &odd, "flac").as_deref(),
            Some("AC DC Live Yes.flac")
        );
    }

    #[test]
    fn an_unknown_word_or_an_empty_name_gives_nothing() {
        assert_eq!(
            name_from_pattern("{mood} - {title}", &tides(), "flac"),
            None
        );
        assert_eq!(name_from_pattern("{title", &tides(), "flac"), None);
        assert_eq!(
            name_from_pattern("{track}", &NameValues::default(), "flac"),
            None
        );
    }
}
