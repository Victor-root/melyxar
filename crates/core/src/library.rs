//! Libraries and their root folders.
//!
//! A library groups one or more root folders holding the same kind of content.
//! The maintainer's films are spread across four disks, so several roots per
//! library is the normal case, not an edge case.

use std::path::PathBuf;

use crate::id::{LibraryId, LibraryRootId};
use crate::time::Millis;
use crate::work::{PlaybackState, WorkKind};

/// What a library holds. Stored explicitly so that no part of the code has to
/// assume "a film": music and television programmes reuse the same trunk with
/// their own domain tables.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryKind {
    /// Feature films.
    Movies,
    /// Fiction series, organised in seasons and episodes.
    Series,
    /// Animated series, which need their own provider and numbering.
    Anime,
    /// Documentaries and television programmes, shaped like series.
    Shows,
    /// Music, organised in artists, albums and tracks.
    Music,
    /// What people filmed and photographed themselves, videos and photos
    /// together, organised by the folders they were put in.
    HomeMedia,
}

impl LibraryKind {
    /// Every kind, in the order a person meets them until they choose another.
    pub const fn every() -> [Self; 6] {
        [
            Self::Movies,
            Self::Series,
            Self::Anime,
            Self::HomeMedia,
            Self::Shows,
            Self::Music,
        ]
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Movies => "movies",
            Self::Series => "series",
            Self::Anime => "anime",
            Self::Shows => "shows",
            Self::Music => "music",
            Self::HomeMedia => "home_media",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "movies" => Some(Self::Movies),
            "series" => Some(Self::Series),
            "anime" => Some(Self::Anime),
            "shows" => Some(Self::Shows),
            "music" => Some(Self::Music),
            "home_media" => Some(Self::HomeMedia),
            _ => None,
        }
    }

    /// Whether the kind is organised in seasons and episodes.
    pub fn is_episodic(self) -> bool {
        matches!(self, Self::Series | Self::Anime | Self::Shows)
    }

    /// Whether what it holds is in the catalogue of films and series the
    /// server looks things up in. What people filmed themselves is in none,
    /// and music is in a catalogue of its own, which that one knows nothing
    /// about: asking it for an album is asking for a film of the same name.
    pub fn is_catalogued(self) -> bool {
        !matches!(self, Self::HomeMedia | Self::Music)
    }

    /// Whether what it holds is played as video. Music is not, and none of the
    /// readings that go through a video applies to it: where a jump lands,
    /// the thumbnails of the bar, the subtitles inside the file.
    pub fn holds_videos(self) -> bool {
        !matches!(self, Self::Music)
    }

    /// The kind of work a file in this library stands for when nothing better
    /// is known about it.
    ///
    /// A film is one file to one work. In an episodic library this is only
    /// reached by a file whose name never said which episode it is: it is an
    /// episode all the same, belonging to no season, and it is met on its own
    /// in the grid rather than disappearing behind a series it was never
    /// attached to.
    pub fn work_kind(self) -> WorkKind {
        match self {
            Self::Movies => WorkKind::Movie,
            Self::Series | Self::Anime | Self::Shows => WorkKind::Episode,
            Self::Music => WorkKind::Song,
            Self::HomeMedia => WorkKind::Video,
        }
    }
}

/// What the server may actually do with a root folder, established by a real
/// access test rather than by reading the permission bits.
///
/// Reading the bits lies as soon as groups, network mounts or an unprivileged
/// container are involved, which is exactly the maintainer's setup. The four
/// states are kept apart on purpose: a missing mount must stop a scan, whereas
/// a read-only root is a perfectly normal state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootAccess {
    /// The path does not exist. Usually a mount that is not up.
    Missing,
    /// The path exists but the server cannot list it.
    Unreadable,
    /// Readable, not writable. The recommended default.
    ReadOnly,
    /// Readable and writable. Required only by deletion on disk and by
    /// writing companion files.
    ReadWrite,
}

impl RootAccess {
    /// A short code explaining the state, for an interface to word itself.
    ///
    /// A code rather than a sentence, so the wording belongs to the client and
    /// can be translated.
    pub fn explanation_code(self) -> &'static str {
        match self {
            Self::Missing => "root_missing_or_not_mounted",
            Self::Unreadable => "root_not_readable_by_server_user",
            Self::ReadOnly => "root_readable_only",
            Self::ReadWrite => "root_readable_and_writable",
        }
    }

    /// Whether the scanner may walk this root at all.
    pub fn is_usable(self) -> bool {
        matches!(self, Self::ReadOnly | Self::ReadWrite)
    }

    /// Whether features that touch the disk may be offered.
    pub fn allows_writing(self) -> bool {
        matches!(self, Self::ReadWrite)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Unreadable => "unreadable",
            Self::ReadOnly => "read_only",
            Self::ReadWrite => "read_write",
        }
    }
}

/// One root folder of a library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryRoot {
    pub id: LibraryRootId,
    pub library_id: LibraryId,
    /// Short label shown in logs and in the interface instead of the path.
    pub label: String,
    pub path: PathBuf,
}

/// What a library has been told to do: with the files that arrive in it, and
/// with what each account plays in it.
///
/// The three heavy readings of a film are each a switch, on to begin with:
/// what a library does not want, its tasks pass by and nothing of it waits.
/// When they are done is one more switch. Off, they are left to the scheduled
/// tasks, where nobody is waiting on them; on, a file is read through as soon
/// as it arrives, which is what somebody with a small library and a machine to
/// spare wants. The quick readings, the look up and where a jump can land,
/// always follow an arrival and have no switch: nobody gains by going without.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LibraryOptions {
    /// Pull the subtitles made of words out of the films that carry them.
    pub extract_subtitles: bool,
    /// Make the thumbnails of the playback bar.
    pub make_thumbnails: bool,
    /// Listen to the seasons for their opening and closing titles. Only ever
    /// asked of a library of series.
    pub detect_openings: bool,
    /// Write subtitles by listening to the sound of the videos that have none.
    /// Only ever asked of a library of personal videos, and off to begin with:
    /// it is the heaviest thing the server does to a file.
    pub generate_subtitles: bool,
    /// Do the heavy readings above as soon as a file arrives, rather than
    /// leaving them to the scheduled tasks.
    pub process_on_arrival: bool,
    /// Watch the library's folders, and scan again as soon as something in
    /// them changes, rather than waiting for somebody or the night to ask.
    pub watch_in_real_time: bool,
    /// Keep where each account stopped, so a work is picked up from there.
    pub keeps_resume_points: bool,
    /// Keep which works each account has watched, by hand or by playing them
    /// through.
    pub keeps_watched_marks: bool,
}

impl Default for LibraryOptions {
    /// Every heavy reading wanted and left to the scheduled tasks, the
    /// folders left unwatched, and every play remembered.
    fn default() -> Self {
        Self {
            extract_subtitles: true,
            make_thumbnails: true,
            detect_openings: true,
            generate_subtitles: false,
            process_on_arrival: false,
            watch_in_real_time: false,
            keeps_resume_points: true,
            keeps_watched_marks: true,
        }
    }
}

impl LibraryOptions {
    /// What of a play this library keeps, from the state and position the
    /// rules of resuming left it in. Nothing when it keeps neither.
    ///
    /// Without resume points, the position is let go of. Without watched
    /// marks, a work played through is simply not started again: there is
    /// nothing left to carry on, and nothing to mark.
    pub fn kept_of(
        self,
        (state, position): (PlaybackState, Millis),
    ) -> Option<(PlaybackState, Millis)> {
        if !self.keeps_resume_points && !self.keeps_watched_marks {
            return None;
        }
        let position = match self.keeps_resume_points {
            true => position,
            false => Millis::ZERO,
        };
        let state = match (state, self.keeps_watched_marks) {
            (PlaybackState::Watched, false) => PlaybackState::NotStarted,
            (state, _) => state,
        };
        Some((state, position))
    }
}

/// A library as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    pub id: LibraryId,
    pub name: String,
    pub kind: LibraryKind,
    /// Preferred metadata language, as a two letter code.
    pub metadata_language: String,
    /// What a scan of this library is allowed to do in one sitting.
    pub options: LibraryOptions,
    pub roots: Vec<LibraryRoot>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_kinds_round_trip_through_their_stored_form() {
        for kind in [
            LibraryKind::Movies,
            LibraryKind::Series,
            LibraryKind::Anime,
            LibraryKind::Shows,
            LibraryKind::Music,
            LibraryKind::HomeMedia,
        ] {
            assert_eq!(LibraryKind::parse(kind.as_str()), Some(kind));
        }
    }

    #[test]
    fn a_library_nobody_configured_wants_every_reading_and_leaves_them_to_the_schedule() {
        // Ticked on a large collection, reading every film as it arrives turns
        // an import of minutes into one of days: off is the answer for the
        // library that needs the setting at all.
        let usual = LibraryOptions::default();
        assert!(usual.extract_subtitles && usual.make_thumbnails && usual.detect_openings);
        assert!(!usual.process_on_arrival);
    }

    #[test]
    fn a_library_nobody_configured_remembers_every_play() {
        let usual = LibraryOptions::default();
        let halfway = (PlaybackState::InProgress, Millis::new(600_000));
        assert_eq!(usual.kept_of(halfway), Some(halfway));
        let through = (PlaybackState::Watched, Millis::ZERO);
        assert_eq!(usual.kept_of(through), Some(through));
    }

    #[test]
    fn a_library_keeps_only_what_it_was_asked_to_keep_of_a_play() {
        let halfway = (PlaybackState::InProgress, Millis::new(600_000));
        let through = (PlaybackState::Watched, Millis::ZERO);

        let marks_only = LibraryOptions {
            keeps_resume_points: false,
            ..LibraryOptions::default()
        };
        assert_eq!(
            marks_only.kept_of(halfway),
            Some((PlaybackState::InProgress, Millis::ZERO)),
            "started, but with nowhere to be picked up from"
        );
        assert_eq!(marks_only.kept_of(through), Some(through));

        let resume_only = LibraryOptions {
            keeps_watched_marks: false,
            ..LibraryOptions::default()
        };
        assert_eq!(resume_only.kept_of(halfway), Some(halfway));
        assert_eq!(
            resume_only.kept_of(through),
            Some((PlaybackState::NotStarted, Millis::ZERO)),
            "played through is nothing left to carry on, and no mark"
        );

        let neither = LibraryOptions {
            keeps_resume_points: false,
            keeps_watched_marks: false,
            ..LibraryOptions::default()
        };
        assert_eq!(neither.kept_of(halfway), None);
        assert_eq!(neither.kept_of(through), None);
    }

    #[test]
    fn an_unknown_kind_is_rejected_rather_than_guessed() {
        assert_eq!(LibraryKind::parse("photos"), None);
    }

    #[test]
    fn episodic_kinds_are_the_three_shaped_like_series() {
        assert!(LibraryKind::Series.is_episodic());
        assert!(LibraryKind::Anime.is_episodic());
        assert!(LibraryKind::Shows.is_episodic());
        assert!(!LibraryKind::Movies.is_episodic());
        assert!(!LibraryKind::Music.is_episodic());
        assert!(!LibraryKind::HomeMedia.is_episodic());
    }

    #[test]
    fn every_kind_holds_videos_but_music() {
        for kind in LibraryKind::every() {
            assert_eq!(kind.holds_videos(), kind != LibraryKind::Music, "{kind:?}");
        }
    }

    #[test]
    fn films_and_series_are_in_the_catalogue_and_music_and_home_media_are_not() {
        for kind in LibraryKind::every() {
            let apart = matches!(kind, LibraryKind::HomeMedia | LibraryKind::Music);
            assert_eq!(kind.is_catalogued(), !apart, "{kind:?}");
        }
    }

    /// The interface turns these into a sentence of its own, so they are part
    /// of what this server promises and never rewritten lightly.
    #[test]
    fn the_state_of_a_root_is_named_the_way_the_interface_expects() {
        assert_eq!(RootAccess::Missing.as_str(), "missing");
        assert_eq!(RootAccess::Unreadable.as_str(), "unreadable");
        assert_eq!(RootAccess::ReadOnly.as_str(), "read_only");
        assert_eq!(RootAccess::ReadWrite.as_str(), "read_write");
    }

    #[test]
    fn only_mounted_and_readable_roots_may_be_scanned() {
        assert!(!RootAccess::Missing.is_usable());
        assert!(!RootAccess::Unreadable.is_usable());
        assert!(RootAccess::ReadOnly.is_usable());
        assert!(RootAccess::ReadWrite.is_usable());
    }

    #[test]
    fn only_a_writable_root_may_offer_features_touching_the_disk() {
        assert!(!RootAccess::ReadOnly.allows_writing());
        assert!(RootAccess::ReadWrite.allows_writing());
    }

    #[test]
    fn every_state_has_a_code_an_interface_can_word_in_its_own_language() {
        let codes: Vec<&str> = [
            RootAccess::Missing,
            RootAccess::Unreadable,
            RootAccess::ReadOnly,
            RootAccess::ReadWrite,
        ]
        .into_iter()
        .map(RootAccess::explanation_code)
        .collect();

        assert!(codes.iter().all(|code| !code.is_empty()));
        let mut unique = codes.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(
            unique.len(),
            codes.len(),
            "two states sharing a code would be two states a reader cannot tell apart"
        );
    }
}
