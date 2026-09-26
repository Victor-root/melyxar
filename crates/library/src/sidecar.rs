//! Subtitle files sitting next to a film.
//!
//! A subtitle in its own file is a track like any other, so it belongs to the
//! model rather than to a special case in the player. What it is, however, has
//! to be read off its name, since the file itself says nothing: the language,
//! whether it is forced, and whether it is meant for viewers who are hard of
//! hearing all live in the words the name carries after the film's own.

use std::path::Path;

use melyxar_core::media::{normalise_language, says_forced, SubtitleLayout};

/// What a subtitle file turned out to be.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SidecarSubtitle {
    pub codec: &'static str,
    pub layout: SubtitleLayout,
    /// Three letter code when the name declared one.
    pub language: Option<String>,
    /// Meant to be shown even when the viewer reads the spoken language, for
    /// the foreign lines and the signs.
    pub is_forced: bool,
    pub is_hearing_impaired: bool,
}

/// Subtitle formats worth picking up, and what they hold.
///
/// Picture subtitles are kept apart because they can only be shown by burning
/// them into the video, which forces a full transcode. That difference decides
/// what playback does, so it is read here and stored, never guessed later.
const SUBTITLE_FORMATS: &[(&str, &str, SubtitleLayout)] = &[
    ("srt", "subrip", SubtitleLayout::Text),
    ("ass", "ass", SubtitleLayout::Text),
    ("ssa", "ssa", SubtitleLayout::Text),
    ("vtt", "webvtt", SubtitleLayout::Text),
    ("sub", "dvd_subtitle", SubtitleLayout::Bitmap),
    ("idx", "dvd_subtitle", SubtitleLayout::Bitmap),
    ("sup", "hdmv_pgs_subtitle", SubtitleLayout::Bitmap),
];

/// Whether a name looks like a subtitle file.
pub fn is_subtitle_file(file_name: &str) -> bool {
    subtitle_format(file_name).is_some()
}

fn subtitle_format(file_name: &str) -> Option<(&'static str, SubtitleLayout)> {
    let extension = Path::new(file_name).extension()?.to_str()?.to_lowercase();
    SUBTITLE_FORMATS
        .iter()
        .find(|(suffix, _, _)| *suffix == extension)
        .map(|(_, codec, layout)| (*codec, *layout))
}

/// Words that say a subtitle is for viewers hard of hearing rather than which
/// language it is in.
const HEARING_MARKS: [&str; 5] = ["sdh", "cc", "hi", "sme", "malentendants"];

/// Reads a subtitle file sitting next to a film.
///
/// `remainder` is what the name carries once the film's own name has been
/// taken off, for example `.fr.forced`. An empty remainder is perfectly
/// ordinary: it simply says nothing, and nothing is what gets stored.
pub fn read(file_name: &str, remainder: &str) -> Option<SidecarSubtitle> {
    let (codec, layout) = subtitle_format(file_name)?;
    let mut subtitle = SidecarSubtitle {
        codec,
        layout,
        language: None,
        is_forced: false,
        is_hearing_impaired: false,
    };

    for word in remainder
        .split(['.', '-', '_', ' '])
        .filter(|word| !word.is_empty())
    {
        let lowered = word.to_lowercase();
        if says_forced(&lowered) {
            subtitle.is_forced = true;
        } else if HEARING_MARKS.contains(&lowered.as_str()) {
            subtitle.is_hearing_impaired = true;
        } else if subtitle.language.is_none() && looks_like_a_language(&lowered) {
            subtitle.language = Some(normalise_language(&lowered));
        }
    }

    Some(subtitle)
}

/// Whether a word is short enough and plain enough to be a language code.
///
/// Deliberately narrow: taking a release group for a language would put a
/// language nobody speaks in the picker, which is worse than showing none.
fn looks_like_a_language(word: &str) -> bool {
    (word.len() == 2 || word.len() == 3) && word.chars().all(|c| c.is_ascii_alphabetic())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subtitle_files_are_recognised_and_the_rest_is_left_alone() {
        assert!(is_subtitle_file("Quiet.Harbour.2019.fr.srt"));
        assert!(is_subtitle_file("Quiet.Harbour.2019.SUP"));
        assert!(!is_subtitle_file("Quiet.Harbour.2019.mkv"));
        assert!(!is_subtitle_file("cover.jpg"));
    }

    #[test]
    fn a_plain_subtitle_gives_up_its_language() {
        let subtitle = read("film.fr.srt", ".fr").expect("a subtitle");
        assert_eq!(subtitle.codec, "subrip");
        assert_eq!(subtitle.layout, SubtitleLayout::Text);
        assert_eq!(subtitle.language.as_deref(), Some("fre"));
        assert!(!subtitle.is_forced);
        assert!(!subtitle.is_hearing_impaired);
    }

    #[test]
    fn the_words_after_the_language_say_what_the_subtitle_is_for() {
        let forced = read("film.fr.forced.srt", ".fr.forced").expect("a subtitle");
        assert!(forced.is_forced);
        assert_eq!(forced.language.as_deref(), Some("fre"));

        let deaf = read("film.en.sdh.srt", ".en.sdh").expect("a subtitle");
        assert!(deaf.is_hearing_impaired);
        assert_eq!(deaf.language.as_deref(), Some("eng"));

        let both = read("film.fr-forced-sdh.srt", "-fr-forced-sdh").expect("a subtitle");
        assert!(both.is_forced && both.is_hearing_impaired);
    }

    #[test]
    fn picture_subtitles_are_told_apart_because_showing_them_forces_a_transcode() {
        for (name, remainder) in [("film.fr.sup", ".fr"), ("film.fr.sub", ".fr")] {
            let subtitle = read(name, remainder).expect("a subtitle");
            assert_eq!(
                subtitle.layout,
                SubtitleLayout::Bitmap,
                "{name} holds pictures, and pictures can only be burnt in"
            );
        }
    }

    #[test]
    fn a_subtitle_that_says_nothing_stores_nothing_rather_than_a_guess() {
        let subtitle = read("film.srt", "").expect("a subtitle");
        assert_eq!(subtitle.language, None);
        assert!(!subtitle.is_forced);
    }

    #[test]
    fn a_release_group_is_not_taken_for_a_language() {
        let subtitle = read("film.SomeGroup.srt", ".SomeGroup").expect("a subtitle");
        assert_eq!(
            subtitle.language, None,
            "a language nobody speaks in the picker is worse than no language at all"
        );
    }

    #[test]
    fn the_language_is_stored_the_same_way_whatever_form_the_name_used() {
        for tag in [".fr", ".fra", ".FRE", ".fr-FR"] {
            assert_eq!(
                read("film.srt", tag)
                    .expect("a subtitle")
                    .language
                    .as_deref(),
                Some("fre"),
                "the picker must not show French twice"
            );
        }
    }
}
