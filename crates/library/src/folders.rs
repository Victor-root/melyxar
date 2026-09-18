//! Looking around the disk for a folder to make a library of.
//!
//! The one place in this server where a path comes from whoever is looking at
//! a screen. Everywhere else a path is read from the configuration or from the
//! database, and the rule that a client never hands one over holds: nothing
//! here opens, serves or removes a file. A path arrives, it is made canonical,
//! the folders inside it are named, and that is all that leaves.
//!
//! What is deliberately not offered:
//!
//! * file names, ever. Only folders come back, and a count of the videos a
//!   folder holds, which is what tells `Films` from the disk it sits on
//!   without naming a single one of them;
//! * anything whose name begins with a dot, for the same reason the walk skips
//!   them: a network drive keeps folders for itself that are full of things
//!   that look exactly like films;
//! * anything a symbolic link points at, which is what keeps a link pointing
//!   back up the tree from being a loop.

use std::path::{Path, PathBuf};

use crate::naming;

/// How many folders one listing hands back.
///
/// A bound rather than a promise: nobody picks a library root out of a
/// thousand entries, and a folder holding that many is a folder somebody
/// typed their way into rather than one they are choosing from.
const MOST: usize = 500;

/// How many files are looked at when counting the videos of a folder.
///
/// The count is there to tell one folder from another, so it stops being
/// useful long before it stops being countable. A folder of ten thousand
/// films answers "more than a thousand" as usefully as it answers ten
/// thousand, and reading the whole directory to say so would make the
/// listing of a parent folder as slow as its slowest child.
const COUNTED_AT_MOST: usize = 1_000;

/// One folder, as somebody choosing sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Folder {
    /// The folder's own name, which is what a list shows.
    pub name: String,
    /// Its whole path, which is what asking for its contents needs and what
    /// becomes a root if it is chosen.
    pub path: PathBuf,
    /// How many videos sit directly inside it, counted up to a bound.
    ///
    /// The one number that answers "is this the folder I mean": a disk holds
    /// folders, a folder of films holds films.
    pub videos: usize,
    /// Whether that count stopped at the bound rather than at the end.
    pub more_videos: bool,
}

/// What one folder holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// The folder that was listed, made canonical: what came in may have been
    /// written with a link, a dot or a doubled separator in it, and what a
    /// screen shows and what is stored has to be the one true form.
    pub path: PathBuf,
    /// Where going up leads, or nothing at the top of the tree.
    pub parent: Option<PathBuf>,
    pub folders: Vec<Folder>,
    /// Whether the list stopped at the bound rather than at the end.
    pub cut_short: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FolderError {
    #[error("there is no such folder")]
    Missing,
    #[error("that is a file, not a folder")]
    NotAFolder,
    #[error("this server is not allowed to look inside that folder")]
    Unreadable,
}

/// The folders inside one folder.
///
/// An empty path means the top of the tree, which is where somebody with
/// nothing typed in starts.
pub fn folders_in(path: &Path) -> Result<Listing, FolderError> {
    let asked = if path.as_os_str().is_empty() {
        Path::new("/")
    } else {
        path
    };

    // Canonical before anything else. It resolves `..` and links, so what is
    // listed, what is shown and what is stored are the same folder, and a path
    // written three ways cannot become three roots for one disk.
    let path = std::fs::canonicalize(asked).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => FolderError::Missing,
        std::io::ErrorKind::PermissionDenied => FolderError::Unreadable,
        _ => FolderError::Missing,
    })?;
    if !path.is_dir() {
        return Err(FolderError::NotAFolder);
    }

    let entries = std::fs::read_dir(&path).map_err(|error| match error.kind() {
        std::io::ErrorKind::PermissionDenied => FolderError::Unreadable,
        std::io::ErrorKind::NotFound => FolderError::Missing,
        _ => FolderError::Unreadable,
    })?;

    let mut folders = Vec::new();
    let mut cut_short = false;
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().to_string();
        // Hidden, unreadable or not a folder at all: none of the three is
        // something somebody is choosing between.
        if name.starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if !kind.is_dir() || kind.is_symlink() {
            continue;
        }

        if folders.len() == MOST {
            cut_short = true;
            break;
        }
        let inside = entry.path();
        let (videos, more_videos) = videos_in(&inside);
        folders.push(Folder {
            name,
            path: inside,
            videos,
            more_videos,
        });
    }

    // By name, the way anybody reading a list expects, and without case
    // deciding where a folder lands.
    folders.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(Listing {
        parent: path.parent().map(Path::to_path_buf),
        path,
        folders,
        cut_short,
    })
}

/// How many videos sit directly in a folder, counted up to a bound.
///
/// Answers the count and whether it stopped short. A folder that cannot be
/// read counts as none: this is a hint on a list, and a folder nobody can open
/// holds nothing anybody here can play either.
fn videos_in(folder: &Path) -> (usize, bool) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return (0, false);
    };

    let mut counted = 0;
    for entry in entries.filter_map(Result::ok).take(COUNTED_AT_MOST) {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || !naming::is_video_file(&name) {
            continue;
        }
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        counted += 1;
        if counted == COUNTED_AT_MOST {
            return (counted, true);
        }
    }
    (counted, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A folder holding what the test names: folders, and files inside them.
    fn build(inside: &Path, tree: &[(&str, &[&str])]) {
        for (folder, files) in tree {
            let made = inside.join(folder);
            std::fs::create_dir_all(&made).expect("folder made");
            for file in *files {
                std::fs::write(made.join(file), b"x").expect("file written");
            }
        }
    }

    #[test]
    fn only_folders_come_back_and_never_a_file_name() {
        // The whole promise of this module: somebody choosing a library root
        // is shown folders, and nothing here ever names a film.
        let directory = tempfile::tempdir().expect("temporary directory");
        build(
            directory.path(),
            &[
                ("Films", &["Quiet Harbour 2019.mkv", "Amber Field 2021.mkv"]),
                ("Series", &[]),
            ],
        );
        std::fs::write(directory.path().join("a-file.txt"), b"x").expect("file written");

        let listing = folders_in(directory.path()).expect("the folder is readable");
        let names: Vec<&str> = listing
            .folders
            .iter()
            .map(|folder| folder.name.as_str())
            .collect();
        assert_eq!(names, vec!["Films", "Series"]);
        assert!(!listing.cut_short);

        let films = &listing.folders[0];
        assert_eq!(
            (films.videos, films.more_videos),
            (2, false),
            "the count is what tells a folder of films from the disk it sits on"
        );
        assert_eq!(listing.folders[1].videos, 0);
    }

    #[test]
    fn what_is_hidden_and_what_is_a_link_are_both_left_alone() {
        // The hidden folders are the ones a network drive keeps for itself,
        // full of things that look exactly like films. A link is what makes a
        // walk run for ever.
        let directory = tempfile::tempdir().expect("temporary directory");
        build(
            directory.path(),
            &[("Films", &[".hidden.mkv", "Quiet Harbour 2019.mkv"])],
        );
        build(directory.path(), &[(".@__thumb", &["Quiet Harbour.mkv"])]);
        std::os::unix::fs::symlink(
            directory.path().join("Films"),
            directory.path().join("Link"),
        )
        .expect("link made");

        let listing = folders_in(directory.path()).expect("the folder is readable");
        let names: Vec<&str> = listing
            .folders
            .iter()
            .map(|folder| folder.name.as_str())
            .collect();
        assert_eq!(names, vec!["Films"]);
        assert_eq!(
            listing.folders[0].videos, 1,
            "a hidden file is not counted either"
        );
    }

    #[test]
    fn a_path_written_any_way_at_all_lists_the_one_folder_it_means() {
        // What is shown and what is stored have to be the same folder: a path
        // written three ways would otherwise become three roots for one disk.
        let directory = tempfile::tempdir().expect("temporary directory");
        build(directory.path(), &[("Films", &[]), ("Series", &[])]);

        let the_long_way = directory.path().join("Films").join("..").join("Series");
        let listing = folders_in(&the_long_way).expect("the folder is readable");
        assert_eq!(
            listing.path,
            std::fs::canonicalize(directory.path().join("Series")).expect("canonical")
        );
        assert_eq!(
            listing.parent,
            Some(std::fs::canonicalize(directory.path()).expect("canonical"))
        );
    }

    #[test]
    fn a_folder_that_is_not_there_says_so_rather_than_answering_nothing() {
        let directory = tempfile::tempdir().expect("temporary directory");
        assert_eq!(
            folders_in(&directory.path().join("nowhere")),
            Err(FolderError::Missing)
        );

        std::fs::write(directory.path().join("a-file.txt"), b"x").expect("file written");
        assert_eq!(
            folders_in(&directory.path().join("a-file.txt")),
            Err(FolderError::NotAFolder),
            "a file is not a folder, and saying which is what lets a screen say it"
        );
    }

    #[test]
    fn nothing_typed_in_starts_at_the_top_of_the_tree() {
        let listing = folders_in(Path::new("")).expect("the top of the tree is readable");
        assert_eq!(listing.path, Path::new("/"));
        assert_eq!(listing.parent, None, "there is nothing above the top");
    }
}
