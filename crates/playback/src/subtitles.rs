//! Which subtitle a film starts with when nobody picked one for it.
//!
//! The account's mode decides, with the language it reads and the language
//! the film is heard in. A subtitle picked for this very film, or turned off
//! in it, is more particular than any of this and is looked at first,
//! elsewhere.

use melyxar_core::media::{normalise_language, MediaSource, Track};
use melyxar_core::user::SubtitleMode;

use crate::decision::chosen_audio;

/// The subtitle the mode picks, or none.
///
/// `audio` is the soundtrack asked for, if any: the one that will really be
/// heard is worked out from it the same way the decision does, since the
/// language heard is what "forced" and "smart" are about.
pub fn subtitle_by_mode<'a>(
    source: &'a MediaSource,
    audio: Option<&'a Track>,
    mode: SubtitleMode,
    read: Option<&str>,
) -> Option<&'a Track> {
    let heard = chosen_audio(source, audio)
        .and_then(|(track, _)| track.language.as_deref())
        .map(normalise_language);
    let read = read.map(normalise_language);
    let in_language = |track: &Track, language: &str| {
        track
            .language
            .as_deref()
            .is_some_and(|written| normalise_language(written) == language)
    };

    // Whole subtitles in the language read, one for viewers hard of hearing
    // only when there is no ordinary one.
    let whole = || {
        let language = read.as_deref()?;
        let whole_ones = || {
            source
                .subtitle_tracks()
                .filter(|(track, _)| !track.is_forced && in_language(track, language))
        };
        whole_ones()
            .find(|(_, details)| !details.is_hearing_impaired)
            .or_else(|| whole_ones().next())
            .map(|(track, _)| track)
    };
    let forced = || {
        [heard.as_deref(), read.as_deref()]
            .into_iter()
            .flatten()
            .find_map(|language| {
                source
                    .subtitle_tracks()
                    .map(|(track, _)| track)
                    .find(|track| track.is_forced && in_language(track, language))
            })
    };
    // Heard in a language other than the one read. A soundtrack that does
    // not say its language is not taken for a foreign one: subtitles over a
    // film somebody understands are a film somebody turns them off in.
    let foreign = matches!((&heard, &read), (Some(heard), Some(read)) if heard != read);
    let smart = || {
        if foreign {
            whole().or_else(forced)
        } else {
            forced()
        }
    };
    let marked_by_the_file = || {
        let marked = || {
            source
                .subtitle_tracks()
                .filter(|(track, _)| track.is_default)
        };
        read.as_deref()
            .and_then(|language| marked().find(|(track, _)| in_language(track, language)))
            .or_else(|| marked().next())
            .map(|(track, _)| track)
    };

    match mode {
        SubtitleMode::Always => whole().or_else(forced),
        SubtitleMode::Smart => smart(),
        SubtitleMode::OnlyForced => forced(),
        SubtitleMode::FromTheFile => marked_by_the_file().or_else(smart),
        SubtitleMode::Never => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use melyxar_core::id::{LibraryRootId, MediaSourceId, TrackId, WorkId};
    use melyxar_core::media::{
        AudioDetails, FileIdentity, Loudness, SubtitleDetails, SubtitleLayout, TrackKind,
    };
    use melyxar_core::time::{now, Millis};

    use super::*;

    fn heard_in(language: &str) -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 1,
            language: Some(language.to_string()),
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "aac".into(),
                profile: None,
                channels: 2,
                channel_layout: None,
                sample_rate: Some(48_000),
                bit_depth: None,
                bitrate: None,
                loudness: Loudness::default(),
            }),
        }
    }

    fn written_in(language: &str) -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 2,
            language: Some(language.to_string()),
            title: None,
            is_default: false,
            is_forced: false,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "subrip".into(),
                layout: SubtitleLayout::Text,
                is_hearing_impaired: false,
                is_external: false,
                external_relative_path: None,
            }),
        }
    }

    fn forced_in(language: &str) -> Track {
        Track {
            is_forced: true,
            ..written_in(language)
        }
    }

    fn for_the_hard_of_hearing_in(language: &str) -> Track {
        let mut track = written_in(language);
        if let TrackKind::Subtitle(details) = &mut track.kind {
            details.is_hearing_impaired = true;
        }
        track
    }

    fn film(tracks: Vec<Track>) -> MediaSource {
        MediaSource {
            id: MediaSourceId::new(),
            work_id: WorkId::new(),
            root_id: LibraryRootId::new(),
            relative_path: PathBuf::from("film.mkv"),
            container: Some("matroska,webm".into()),
            duration: Some(Millis::new(6_000_000)),
            overall_bitrate: None,
            identity: FileIdentity {
                size_bytes: 1,
                modified_at: now(),
                content_fingerprint: None,
            },
            added_at: now(),
            tracks,
        }
    }

    /// The language of what the mode picked, and whether it is forced.
    fn picked(
        source: &MediaSource,
        mode: SubtitleMode,
        read: Option<&str>,
    ) -> Option<(String, bool)> {
        subtitle_by_mode(source, None, mode, read)
            .map(|track| (track.language.clone().unwrap_or_default(), track.is_forced))
    }

    fn whole(language: &str) -> Option<(String, bool)> {
        Some((language.to_string(), false))
    }

    fn forced(language: &str) -> Option<(String, bool)> {
        Some((language.to_string(), true))
    }

    #[test]
    fn always_shows_the_language_read_and_the_forced_lines_otherwise() {
        let dubbed = film(vec![heard_in("fre"), forced_in("fre"), written_in("fre")]);
        assert_eq!(
            picked(&dubbed, SubtitleMode::Always, Some("fr")),
            whole("fre")
        );
        assert_eq!(
            picked(&dubbed, SubtitleMode::Always, None),
            forced("fre"),
            "without a language read, the forced lines of the language heard still show"
        );
    }

    #[test]
    fn smart_shows_the_language_read_only_over_another_language() {
        let original = film(vec![heard_in("eng"), written_in("fre"), forced_in("eng")]);
        assert_eq!(
            picked(&original, SubtitleMode::Smart, Some("fre")),
            whole("fre")
        );

        let dubbed = film(vec![heard_in("fre"), written_in("fre"), forced_in("fre")]);
        assert_eq!(
            picked(&dubbed, SubtitleMode::Smart, Some("fre")),
            forced("fre"),
            "a film heard in the language read only needs the lines it does not say in it"
        );
    }

    #[test]
    fn a_soundtrack_that_does_not_say_its_language_is_not_taken_for_a_foreign_one() {
        let mut unsaid = heard_in("fre");
        unsaid.language = None;
        let film = film(vec![unsaid, written_in("fre")]);
        assert_eq!(picked(&film, SubtitleMode::Smart, Some("fre")), None);
    }

    #[test]
    fn only_forced_never_shows_whole_subtitles() {
        let original = film(vec![heard_in("eng"), written_in("fre")]);
        assert_eq!(
            picked(&original, SubtitleMode::OnlyForced, Some("fre")),
            None
        );

        let with_forced = film(vec![heard_in("eng"), written_in("fre"), forced_in("fre")]);
        assert_eq!(
            picked(&with_forced, SubtitleMode::OnlyForced, Some("fre")),
            forced("fre"),
            "forced lines in the language read, for a film heard in another"
        );
    }

    #[test]
    fn forced_lines_are_looked_for_in_the_language_heard_first() {
        let film = film(vec![heard_in("eng"), forced_in("fre"), forced_in("eng")]);
        assert_eq!(
            picked(&film, SubtitleMode::OnlyForced, Some("fre")),
            forced("eng")
        );
    }

    #[test]
    fn from_the_file_takes_the_track_it_marks_and_is_smart_otherwise() {
        let mut marked = written_in("ger");
        marked.is_default = true;
        let film_with_a_mark = film(vec![heard_in("eng"), written_in("fre"), marked]);
        assert_eq!(
            picked(&film_with_a_mark, SubtitleMode::FromTheFile, Some("fre")),
            whole("ger")
        );

        let unmarked = film(vec![heard_in("eng"), written_in("fre")]);
        assert_eq!(
            picked(&unmarked, SubtitleMode::FromTheFile, Some("fre")),
            whole("fre")
        );
    }

    #[test]
    fn never_shows_anything_not_even_forced_lines() {
        let film = film(vec![heard_in("fre"), forced_in("fre"), written_in("fre")]);
        assert_eq!(picked(&film, SubtitleMode::Never, Some("fre")), None);
    }

    #[test]
    fn an_ordinary_track_is_preferred_to_one_for_the_hard_of_hearing() {
        let film = film(vec![
            heard_in("eng"),
            for_the_hard_of_hearing_in("fre"),
            written_in("fre"),
        ]);
        let chosen =
            subtitle_by_mode(&film, None, SubtitleMode::Always, Some("fre")).expect("a subtitle");
        assert!(matches!(
            &chosen.kind,
            TrackKind::Subtitle(details) if !details.is_hearing_impaired
        ));

        let only_that = self::film(vec![heard_in("eng"), for_the_hard_of_hearing_in("fre")]);
        assert_eq!(
            picked(&only_that, SubtitleMode::Always, Some("fre")),
            whole("fre")
        );
    }

    #[test]
    fn the_soundtrack_asked_for_is_the_one_heard() {
        let english = heard_in("eng");
        let mut french = heard_in("fre");
        french.is_default = false;
        let film = film(vec![english, french.clone(), written_in("fre")]);
        let french = film
            .tracks
            .iter()
            .find(|track| track.id == french.id)
            .expect("the French soundtrack");
        assert!(
            subtitle_by_mode(&film, Some(french), SubtitleMode::Smart, Some("fre")).is_none(),
            "the film is heard in French once French is asked for"
        );
        assert!(subtitle_by_mode(&film, None, SubtitleMode::Smart, Some("fre")).is_some());
    }
}
