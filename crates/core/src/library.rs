//! Libraries and their root folders.
//!
//! A library groups one or more root folders holding the same kind of content.
//! The maintainer's films are spread across four disks, so several roots per
//! library is the normal case, not an edge case.

use std::path::PathBuf;

use crate::id::{LibraryId, LibraryRootId};

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
}

impl LibraryKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Movies => "movies",
            Self::Series => "series",
            Self::Anime => "anime",
            Self::Shows => "shows",
            Self::Music => "music",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "movies" => Some(Self::Movies),
            "series" => Some(Self::Series),
            "anime" => Some(Self::Anime),
            "shows" => Some(Self::Shows),
            "music" => Some(Self::Music),
            _ => None,
        }
    }

    /// Whether the kind is organised in seasons and episodes.
    pub fn is_episodic(self) -> bool {
        matches!(self, Self::Series | Self::Anime | Self::Shows)
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

/// A library as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    pub id: LibraryId,
    pub name: String,
    pub kind: LibraryKind,
    /// Preferred metadata language, as a two letter code.
    pub metadata_language: String,
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
        ] {
            assert_eq!(LibraryKind::parse(kind.as_str()), Some(kind));
        }
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
