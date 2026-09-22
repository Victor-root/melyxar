//! Reading which way up a photo is meant to be seen, off the photo itself.

use std::io::BufReader;
use std::path::Path;

use melyxar_core::orientation::Orientation;

/// Which way up the photo at this path is meant to be seen.
///
/// Reads the file, so it is called away from the threads that answer
/// requests. A photo that says nothing, or cannot be read, is drawn as it is
/// stored: a wrong turn is worse than none.
pub fn orientation_of(path: &Path) -> Orientation {
    let Ok(file) = std::fs::File::open(path) else {
        return Orientation::AsStored;
    };
    let Ok(read) = exif::Reader::new().read_from_container(&mut BufReader::new(file)) else {
        return Orientation::AsStored;
    };
    read.get_field(exif::Tag::Orientation, exif::In::PRIMARY)
        .and_then(|field| field.value.get_uint(0))
        .map(Orientation::from_camera)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_with_nothing_to_say_is_drawn_as_it_is_stored() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("plain.jpg");
        std::fs::write(&path, b"not a photo at all").expect("written");
        assert_eq!(orientation_of(&path), Orientation::AsStored);
        assert_eq!(
            orientation_of(&directory.path().join("gone.jpg")),
            Orientation::AsStored
        );
    }

    #[test]
    fn the_turn_a_camera_wrote_is_read() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("turned.jpg");
        std::fs::write(&path, a_jpeg_turned(6)).expect("written");
        assert_eq!(orientation_of(&path), Orientation::TurnedRight);
    }

    /// The smallest file that carries a camera's note: the start of a JPEG and
    /// the block holding the note, with the one field that says the turn.
    fn a_jpeg_turned(value: u16) -> Vec<u8> {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"MM\x00\x2a\x00\x00\x00\x08");
        tiff.extend_from_slice(&1u16.to_be_bytes());
        tiff.extend_from_slice(&0x0112u16.to_be_bytes());
        tiff.extend_from_slice(&3u16.to_be_bytes());
        tiff.extend_from_slice(&1u32.to_be_bytes());
        tiff.extend_from_slice(&value.to_be_bytes());
        tiff.extend_from_slice(&[0, 0]);
        tiff.extend_from_slice(&0u32.to_be_bytes());

        let mut block = b"Exif\x00\x00".to_vec();
        block.extend_from_slice(&tiff);

        let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe1];
        jpeg.extend_from_slice(&((block.len() + 2) as u16).to_be_bytes());
        jpeg.extend_from_slice(&block);
        jpeg.extend_from_slice(&[0xff, 0xd9]);
        jpeg
    }
}
