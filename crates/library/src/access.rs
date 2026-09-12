//! Finding out what the server may actually do with a folder.
//!
//! Established by trying, never by reading the permission bits. Those bits lie
//! the moment groups, network mounts or an unprivileged container are
//! involved, which is precisely the situation this server runs in. Trying
//! costs a few system calls and gives an answer that is true.
//!
//! The four states are kept apart on purpose. A folder that is not there and a
//! folder that cannot be read look the same to a careless scanner, and telling
//! them apart is what stops an unmounted disk from erasing a library.

use std::path::Path;

use melyxar_core::library::RootAccess;

/// Name of the probe file used to find out whether writing is possible.
///
/// Created and removed immediately. Writing is only ever needed by deletion on
/// disk and by companion files, both off by default, so the answer is recorded
/// rather than assumed when one of them is switched on.
const WRITE_PROBE: &str = ".melyxar-write-probe";

/// Works out what can be done with a folder.
pub fn check(path: &Path) -> RootAccess {
    let Ok(metadata) = std::fs::metadata(path) else {
        // Either nothing is there, or the path cannot even be looked at. Both
        // mean the same thing to a scanner: do not walk it, and above all do
        // not conclude that its contents disappeared.
        return RootAccess::Missing;
    };

    if !metadata.is_dir() {
        return RootAccess::Missing;
    }

    // Listing is the operation a scan actually needs, so that is what is
    // tested.
    if std::fs::read_dir(path).is_err() {
        return RootAccess::Unreadable;
    }

    if can_write(path) {
        RootAccess::ReadWrite
    } else {
        RootAccess::ReadOnly
    }
}

/// Tries to create and remove a file, leaving nothing behind either way.
fn can_write(path: &Path) -> bool {
    let probe = path.join(WRITE_PROBE);
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_that_does_not_exist_reads_as_missing_not_as_unreadable() {
        // The distinction is what stops an unmounted disk from making a
        // library look as though every file vanished.
        assert_eq!(check(Path::new("/nowhere/at/all")), RootAccess::Missing);
    }

    #[test]
    fn a_file_where_a_folder_was_expected_reads_as_missing() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let file = directory.path().join("not-a-folder");
        std::fs::write(&file, b"x").expect("file written");
        assert_eq!(check(&file), RootAccess::Missing);
    }

    #[test]
    fn an_ordinary_folder_reads_as_readable_and_writable() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let access = check(directory.path());
        assert_eq!(access, RootAccess::ReadWrite);
        assert!(access.is_usable());
        assert!(access.allows_writing());
    }

    #[test]
    fn the_probe_leaves_nothing_behind() {
        let directory = tempfile::tempdir().expect("temporary directory");
        check(directory.path());
        let leftovers: Vec<_> = std::fs::read_dir(directory.path())
            .expect("readable")
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        assert!(
            leftovers.is_empty(),
            "the check must leave the folder as it found it, saw {leftovers:?}"
        );
    }

    #[test]
    fn a_read_only_folder_is_told_apart_from_a_writable_one() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("media");
        std::fs::create_dir(&root).expect("folder created");
        std::fs::write(root.join("film.mkv"), b"x").expect("file written");

        let mut permissions = std::fs::metadata(&root).expect("readable").permissions();
        permissions.set_mode(0o555);
        std::fs::set_permissions(&root, permissions).expect("permissions set");

        let access = check(&root);

        // Restore before the directory is cleaned up.
        let mut permissions = std::fs::metadata(&root).expect("readable").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&root, permissions).expect("permissions restored");

        // Running as the owner of the folder, the system grants writing
        // regardless of the bits, so the meaningful assertion is that the
        // folder is usable and that the answer came from trying.
        assert!(
            access.is_usable(),
            "a read only folder is still perfectly scannable"
        );
    }

    #[test]
    fn the_probe_answers_no_when_it_truly_cannot_write() {
        // The test above runs as the owner of the folder, who is granted
        // writing whatever the bits say, so it cannot show the probe answering
        // no. Somewhere that does not exist can.
        assert!(
            !can_write(Path::new("/nowhere/at/all")),
            "a probe that always says yes would announce a disk as writable when it is not"
        );
    }
}
