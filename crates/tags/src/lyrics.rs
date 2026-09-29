//! The lyrics a music file carries inside it, read on their own when a song
//! is listened to rather than for every song of a scan.

use std::path::Path;

use lofty::config::ParseOptions;
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::ItemKey;

use crate::ReadError;

/// The lyrics written in a file, if any. Blocking, like every reading of a
/// file.
pub fn lyrics(path: &Path) -> Result<Option<String>, ReadError> {
    let tagged = Probe::open(path)
        .map_err(|error| ReadError::Unreadable(error.to_string()))?
        .options(ParseOptions::new().read_cover_art(false))
        .guess_file_type()
        .map_err(|error| ReadError::Unreadable(error.to_string()))?
        .read()
        .map_err(|error| ReadError::Unreadable(error.to_string()))?;
    Ok(tagged
        .tags()
        .iter()
        .find_map(|tag| tag.get_string(ItemKey::Lyrics))
        .map(str::trim)
        .filter(|words| !words.is_empty())
        .map(str::to_string))
}

#[cfg(test)]
mod tests {
    use lofty::config::WriteOptions;
    use lofty::file::AudioFile as _;
    use lofty::tag::{Tag, TagType};

    use super::*;

    #[test]
    fn the_lyrics_written_in_a_file_are_read_back_and_a_file_without_says_so() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let song = directory.path().join("song.flac");
        std::fs::copy("tests/fixtures/one-second.flac", &song).expect("copied");
        assert_eq!(lyrics(&song).expect("read"), None);

        let mut tagged = lofty::read_from_path(&song).expect("read");
        if tagged.primary_tag_mut().is_none() {
            tagged.insert_tag(Tag::new(TagType::VorbisComments));
        }
        let tag = tagged.primary_tag_mut().expect("a tag");
        tag.insert_text(ItemKey::Lyrics, "[00:01.00]First line\n[00:02.50]Second line\n".to_string());
        tagged
            .save_to_path(&song, WriteOptions::default())
            .expect("written");

        assert_eq!(
            lyrics(&song).expect("read").as_deref(),
            Some("[00:01.00]First line\n[00:02.50]Second line")
        );
    }
}
