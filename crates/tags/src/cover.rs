//! The picture a music file carries of its album.

use std::path::Path;

use lofty::config::ParseOptions;
use lofty::file::TaggedFileExt;
use lofty::picture::{MimeType, PictureType};
use lofty::probe::Probe;

use crate::ReadError;

/// A picture taken out of a file, as it was stored there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub data: Vec<u8>,
    /// What to name a file holding it, for whoever reads it next.
    pub extension: &'static str,
}

/// The front cover a file carries, and failing that the first picture it
/// carries at all: plenty of files hold one picture, marked as nothing in
/// particular, which is the cover all the same.
///
/// Read apart from the tags, since only one song of each album is ever asked
/// for its picture, and reading every picture of every song while filing them
/// would be most of the cost of a scan.
pub fn front_cover(path: &Path) -> Result<Option<Cover>, ReadError> {
    let tagged = Probe::open(path)
        .map_err(|error| ReadError::Unreadable(error.to_string()))?
        .options(ParseOptions::new().read_properties(false))
        .guess_file_type()
        .map_err(|error| ReadError::Unreadable(error.to_string()))?
        .read()
        .map_err(|error| match error.is_unknown_format() {
            true => ReadError::Unsupported,
            false => ReadError::Unreadable(error.to_string()),
        })?;

    let pictures: Vec<_> = tagged
        .tags()
        .iter()
        .flat_map(|tag| tag.pictures())
        .collect();
    let chosen = pictures
        .iter()
        .find(|picture| picture.pic_type() == PictureType::CoverFront)
        .or_else(|| pictures.first());
    Ok(chosen
        .filter(|picture| !picture.data().is_empty())
        .map(|picture| Cover {
            data: picture.data().to_vec(),
            extension: match picture.mime_type() {
                Some(MimeType::Png) => "png",
                Some(MimeType::Gif) => "gif",
                Some(MimeType::Bmp) => "bmp",
                Some(MimeType::Tiff) => "tiff",
                _ => "jpg",
            },
        }))
}
