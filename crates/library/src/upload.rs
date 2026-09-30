//! What may be sent into a library from the interface, and under what name:
//! judged before a single byte is written, so a name that would leave its
//! folder, hide from the scan or plant something a library has no use for is
//! refused whole.

use std::path::PathBuf;

use melyxar_core::library::LibraryKind;

use crate::naming;
use crate::sidecar;

/// The longest a name, or one folder of a path, may be, in bytes: what a disk
/// keeps.
pub const LONGEST_NAME: usize = 255;

/// The most folders one path may go down through.
const DEEPEST: usize = 8;

/// What is written while a file is still arriving. Hidden, so a scan that
/// walks past it takes it for nothing, and never a name somebody can send.
pub const WHILE_ARRIVING: &str = ".melyxar-upload";

/// A file name, as it will be written, when it is one a walk would find: one
/// name and not a path, not hidden, not what a system keeps for itself.
pub fn clean_file_name(name: &str) -> Option<String> {
    let name = name.trim();
    let fine = !name.is_empty()
        && name.len() <= LONGEST_NAME
        && !name.chars().any(|character| character.is_control() || "/\\".contains(character))
        && !naming::is_left_alone(name)
        && !name.ends_with(WHILE_ARRIVING)
        && name != "."
        && name != "..";
    fine.then(|| name.to_string())
}

/// A folder under a root, written as its names separated by slashes: nothing
/// for the root itself. Refused whole when any name of it is not one a file
/// could have, which is what keeps `..` and a way back up out of it.
pub fn clean_folder(folder: &str) -> Option<PathBuf> {
    let folder = folder.trim().trim_end_matches('/');
    if folder.is_empty() {
        return Some(PathBuf::new());
    }
    let names: Vec<&str> = folder.split('/').collect();
    if names.len() > DEEPEST {
        return None;
    }
    names
        .into_iter()
        .map(clean_file_name)
        .collect::<Option<PathBuf>>()
}

/// Whether a library of this kind has a use for a file of this name: the
/// media it holds, and what goes with it.
pub fn is_uploadable(kind: LibraryKind, file_name: &str) -> bool {
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_lowercase())
        .unwrap_or_default();
    match kind {
        LibraryKind::Music => {
            naming::is_audio_file(file_name)
                || extension == "lrc"
                || matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "webp")
        }
        LibraryKind::HomeMedia => {
            naming::is_photo_file(file_name) || naming::is_video_file(file_name)
        }
        _ => naming::is_video_file(file_name) || sidecar::is_subtitle_file(file_name),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_taken_as_it_is_or_refused() {
        assert_eq!(
            clean_file_name("  Quiet Harbour (2019).mkv "),
            Some("Quiet Harbour (2019).mkv".to_string())
        );
        for refused in [
            "",
            "   ",
            ".",
            "..",
            "../up.mkv",
            "a/b.mkv",
            "a\\b.mkv",
            ".hidden.mkv",
            "$RECYCLE.BIN",
            "film.mkv\0",
            "film.mkv.melyxar-upload",
        ] {
            assert_eq!(clean_file_name(refused), None, "{refused:?}");
        }
        assert_eq!(clean_file_name(&"a".repeat(LONGEST_NAME + 1)), None);
    }

    #[test]
    fn a_folder_never_leaves_the_root() {
        assert_eq!(clean_folder(""), Some(PathBuf::new()));
        assert_eq!(clean_folder("/"), Some(PathBuf::new()));
        assert_eq!(
            clean_folder("Amber Field/Early Days/"),
            Some(PathBuf::from("Amber Field/Early Days"))
        );
        for refused in ["..", "a/../b", "a//b", "/etc", "a/.hidden", "a\\b", "./a"] {
            let asked = if refused == "/etc" { "//etc" } else { refused };
            assert_eq!(clean_folder(asked), None, "{asked:?}");
        }
        assert_eq!(clean_folder(&["a"; 9].join("/")), None);
    }

    #[test]
    fn a_library_takes_the_media_it_holds_and_what_goes_with_it() {
        assert!(is_uploadable(LibraryKind::Movies, "Quiet Harbour.mkv"));
        assert!(is_uploadable(LibraryKind::Series, "Episode 1.fr.srt"));
        assert!(!is_uploadable(LibraryKind::Movies, "song.mp3"));
        assert!(!is_uploadable(LibraryKind::Movies, "run.exe"));
        assert!(is_uploadable(LibraryKind::Music, "01 - Tides.flac"));
        assert!(is_uploadable(LibraryKind::Music, "Tides.lrc"));
        assert!(is_uploadable(LibraryKind::Music, "cover.JPG"));
        assert!(!is_uploadable(LibraryKind::Music, "film.mkv"));
        assert!(is_uploadable(LibraryKind::HomeMedia, "holiday.jpg"));
        assert!(is_uploadable(LibraryKind::HomeMedia, "holiday.mp4"));
        assert!(!is_uploadable(LibraryKind::HomeMedia, "holiday.mp3"));
    }
}
