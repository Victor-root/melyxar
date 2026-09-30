//! Writing the tags of one music file, as the tag manager asks: the fields a
//! person edits set to what they chose, and taken away where they chose
//! nothing. Everything else the file carries, its pictures first, is left as
//! it was, and so is its sound.

use std::path::Path;

use lofty::config::WriteOptions;
use lofty::file::TaggedFileExt;
use lofty::tag::{ItemKey, Tag, TagExt};

/// The tags a person edits, as they want them written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditedTags {
    pub title: Option<String>,
    pub artists: Vec<String>,
    pub album: Option<String>,
    pub album_artists: Vec<String>,
    pub track: Option<u32>,
    pub disc: Option<u32>,
    pub year: Option<i32>,
    pub genres: Vec<String>,
    pub compilation: bool,
}

impl From<&crate::Tags> for EditedTags {
    /// What a file carries now, as the tag manager starts from it.
    fn from(tags: &crate::Tags) -> Self {
        Self {
            title: tags.title.clone(),
            artists: tags.artists.clone(),
            album: tags.album.clone(),
            album_artists: tags.album_artists.clone(),
            track: tags.track,
            disc: tags.disc,
            year: tags.year,
            genres: tags.genres.clone(),
            compilation: tags.compilation,
        }
    }
}

impl EditedTags {
    /// The fields that differ from what the file carried, by name.
    pub fn changed_from(&self, before: &Self) -> Vec<&'static str> {
        let mut changed = Vec::new();
        let mut differs = |name: &'static str, different: bool| {
            if different {
                changed.push(name);
            }
        };
        differs("title", self.title != before.title);
        differs("artists", self.artists != before.artists);
        differs("album", self.album != before.album);
        differs("album_artists", self.album_artists != before.album_artists);
        differs("track", self.track != before.track);
        differs("disc", self.disc != before.disc);
        differs("year", self.year != before.year);
        differs("genres", self.genres != before.genres);
        differs("compilation", self.compilation != before.compilation);
        changed
    }
}

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("the file could not be read to be written: {0}")]
    Unreadable(String),
    #[error("the tags could not be written: {0}")]
    Unwritable(String),
}

/// Several names as one value, the way the reader splits them again.
const BETWEEN_NAMES: &str = "; ";

/// Writes the edited tags into a file. Blocking, like every reading of one.
pub fn write(path: &Path, edited: &EditedTags) -> Result<(), WriteError> {
    let mut tagged =
        lofty::read_from_path(path).map_err(|error| WriteError::Unreadable(cause_of(&error)))?;
    let tag_type = tagged.primary_tag_type();
    if tagged.primary_tag().is_none() {
        tagged.insert_tag(Tag::new(tag_type));
    }
    let Some(tag) = tagged.primary_tag_mut() else {
        return Err(WriteError::Unwritable(
            "the file holds no tag it can be given".to_string(),
        ));
    };
    fill(tag, edited);
    tag.save_to_path(path, WriteOptions::default())
        .map_err(|error| WriteError::Unwritable(cause_of(&error)))
}

/// An error and what lay under it, in one sentence. The library says only
/// "failed to write to file" of an error that is the disk's, and the disk's
/// word, a permission refused or a disk read only, is the one that says what
/// to put right.
fn cause_of(error: &(dyn std::error::Error + 'static)) -> String {
    let mut said = error.to_string();
    let mut under = error.source();
    while let Some(cause) = under {
        said.push_str(": ");
        said.push_str(&cause.to_string());
        under = cause.source();
    }
    said
}

/// The edited fields set in a tag, each taken away where nothing was chosen.
fn fill(tag: &mut Tag, edited: &EditedTags) {
    let mut set = |key: ItemKey, value: Option<String>| {
        tag.remove_key(key);
        if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
            tag.insert_text(key, value.trim().to_string());
        }
    };
    let names = |names: &[String]| {
        let kept: Vec<&str> = names
            .iter()
            .map(|name| name.trim())
            .filter(|name| !name.is_empty())
            .collect();
        (!kept.is_empty()).then(|| kept.join(BETWEEN_NAMES))
    };
    set(ItemKey::TrackTitle, edited.title.clone());
    // One value for all the names: the several-valued key is taken away, or
    // the reader, which prefers it, would go on reading the old names.
    set(ItemKey::TrackArtists, None);
    set(ItemKey::TrackArtist, names(&edited.artists));
    set(ItemKey::AlbumTitle, edited.album.clone());
    set(ItemKey::AlbumArtists, None);
    set(ItemKey::AlbumArtist, names(&edited.album_artists));
    set(
        ItemKey::TrackNumber,
        edited.track.map(|track| track.to_string()),
    );
    set(
        ItemKey::DiscNumber,
        edited.disc.map(|disc| disc.to_string()),
    );
    // Written both ways: some forms keep a year of its own, and ID3 only
    // the date of the recording, which the reader falls back on.
    let year = edited.year.map(|year| year.to_string());
    set(ItemKey::RecordingDate, year.clone());
    set(ItemKey::Year, year);
    set(ItemKey::Genre, names(&edited.genres));
    set(
        ItemKey::FlagCompilation,
        edited.compilation.then(|| "1".to_string()),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_changed_is_named_field_by_field() {
        let before = EditedTags {
            title: Some("Tides".to_string()),
            track: Some(2),
            ..EditedTags::default()
        };
        let after = EditedTags {
            title: Some("Low Tide".to_string()),
            year: Some(2019),
            ..before.clone()
        };
        assert_eq!(after.changed_from(&before), vec!["title", "year"]);
        assert!(before.changed_from(&before).is_empty());
    }

    #[test]
    fn an_error_is_told_with_what_lay_under_it() {
        #[derive(Debug, thiserror::Error)]
        #[error("failed to write to file")]
        struct Outer(#[source] std::io::Error);

        let error = Outer(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "Permission denied (os error 13)",
        ));
        assert_eq!(
            cause_of(&error),
            "failed to write to file: Permission denied (os error 13)"
        );
    }
}
