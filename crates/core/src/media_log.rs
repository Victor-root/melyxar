//! Formatting a file for a log line.
//!
//! A log line about one file names it by its file name, and a path is named
//! by the root label its library was given rather than repeated in full at
//! every call site. There is no redaction here: this journal is the
//! maintainer's own, read by nobody else, so a real name is simply useful
//! when something has to be found on disk.

use std::fmt;
use std::path::Path;

/// The name of the file this path points to, with nothing above it.
pub fn file_name_of(path: &Path) -> &str {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

/// Wraps a path together with the label of the library root it was found
/// under, for one log line.
pub struct MediaPath<'a> {
    root_label: &'a str,
    path: &'a Path,
}

impl<'a> MediaPath<'a> {
    pub fn new(root_label: &'a str, path: &'a Path) -> Self {
        Self { root_label, path }
    }
}

impl fmt::Display for MediaPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.root_label, self.path.display())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn the_name_of_a_file_leaves_out_the_path_above_it() {
        let path = PathBuf::from("/somewhere/deep/inside/Something.Long.mkv");
        assert_eq!(file_name_of(&path), "Something.Long.mkv");
    }

    #[test]
    fn a_labeled_path_carries_its_root_and_its_full_path() {
        let path = PathBuf::from("/somewhere/deep/inside/Something.Long.mkv");
        let shown = MediaPath::new("disk-one", &path).to_string();
        assert_eq!(shown, "disk-one:/somewhere/deep/inside/Something.Long.mkv");
    }
}
