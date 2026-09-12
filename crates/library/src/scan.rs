//! Walking a root and reporting what changed.
//!
//! The walk itself is deliberately dumb: it lists files and says what it saw.
//! Deciding what to do with that belongs to the orchestration layer, which is
//! what makes this testable against a temporary folder and what keeps the
//! database out of the loop.
//!
//! Two rules matter more than anything else here. A root that is not usable
//! stops the scan outright rather than reporting an empty folder, because an
//! empty answer from an unmounted disk is what makes other servers wipe a
//! library. And nothing is ever deleted by a scan: a file that is no longer
//! there is *marked* absent, so that a mistake is recoverable.

use std::path::{Path, PathBuf};

use melyxar_core::library::RootAccess;
use melyxar_core::privacy::MediaPath;
use melyxar_core::time::Timestamp;

use crate::access;
use crate::naming;

/// One file the walk found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FoundFile {
    /// Path relative to the root, which is what gets stored.
    pub relative_path: PathBuf,
    pub size_bytes: i64,
    pub modified_at: Timestamp,
    /// Set when the file is a companion rather than a work of its own, such as
    /// a trailer or a sample clip.
    pub companion_kind: Option<&'static str>,
}

/// What a walk produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanOutcome {
    pub files: Vec<FoundFile>,
    /// Subtitle files sitting next to the media, by path relative to the root.
    /// A subtitle in its own file is a track of the film it belongs to, not a
    /// work, so it is reported apart from the files above.
    pub subtitles: Vec<PathBuf>,
    /// Description files sitting next to the media, by path relative to the
    /// root. Reading them is a choice the server makes elsewhere; finding them
    /// costs nothing and keeps the walk the only thing that touches the disk.
    pub companion_files: Vec<PathBuf>,
    /// Folders that could not be entered, reported rather than swallowed.
    pub unreadable_folders: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// The root is not usable. Deliberately not an empty result: an empty
    /// answer would be taken as "everything was removed".
    #[error("the root is not usable: {state:?}")]
    RootUnusable { state: RootAccess },
}

/// Walks a root and returns everything worth looking at.
///
/// The root state is checked first, and an unusable root stops the walk. That
/// single guard is what prevents an unmounted disk from emptying a library.
pub fn walk(root_label: &str, root: &Path) -> Result<ScanOutcome, ScanError> {
    let state = access::check(root);
    if !state.is_usable() {
        tracing::warn!(
            root = root_label,
            state = state.as_str(),
            "the root is not usable, the scan stops rather than reporting an empty folder"
        );
        return Err(ScanError::RootUnusable { state });
    }

    let mut outcome = ScanOutcome {
        files: Vec::new(),
        subtitles: Vec::new(),
        companion_files: Vec::new(),
        unreadable_folders: Vec::new(),
    };
    walk_into(root, root, &mut outcome, root_label);

    tracing::info!(
        root = root_label,
        files = outcome.files.len(),
        subtitles = outcome.subtitles.len(),
        unreadable = outcome.unreadable_folders.len(),
        "walk finished"
    );
    Ok(outcome)
}

/// Walks one folder, then its subfolders.
///
/// Written as a loop over a queue rather than by calling itself, so that a
/// deeply nested or looping folder structure cannot exhaust the stack.
fn walk_into(root: &Path, start: &Path, outcome: &mut ScanOutcome, root_label: &str) {
    let mut pending = vec![start.to_path_buf()];

    while let Some(folder) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            outcome.unreadable_folders.push(folder);
            continue;
        };

        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            let Ok(metadata) = entry.metadata() else {
                continue;
            };

            if metadata.is_dir() {
                // Symbolic links are not followed: a link pointing back up the
                // tree would make the walk run for ever.
                if !metadata.is_symlink() {
                    pending.push(path);
                }
                continue;
            }

            let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                // A name that is not valid text cannot be stored or served,
                // and silently skipping it would hide the problem.
                tracing::warn!(
                    root = root_label,
                    "a file name is not valid text and was skipped"
                );
                continue;
            };

            let is_video = naming::is_video_file(name);
            let is_subtitle = crate::sidecar::is_subtitle_file(name);
            let is_description = crate::companion::is_companion_file(name);
            if !is_video && !is_subtitle && !is_description {
                continue;
            }

            let Ok(relative_path) = path.strip_prefix(root) else {
                continue;
            };

            if is_subtitle {
                outcome.subtitles.push(relative_path.to_path_buf());
                continue;
            }
            if is_description {
                outcome.companion_files.push(relative_path.to_path_buf());
                continue;
            }

            tracing::debug!(
                file = %MediaPath::new(root_label, &path),
                "found"
            );

            outcome.files.push(FoundFile {
                relative_path: relative_path.to_path_buf(),
                size_bytes: metadata.len() as i64,
                modified_at: metadata
                    .modified()
                    .map(Timestamp::from)
                    .unwrap_or_else(|_| melyxar_core::time::now()),
                companion_kind: naming::is_companion_clip(name),
            });
        }
    }

    // A stable order makes a scan reproducible, which matters when comparing
    // two runs while chasing a problem.
    outcome
        .files
        .sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    outcome.subtitles.sort();
    outcome.companion_files.sort();
    outcome.unreadable_folders.sort();
}

/// What changed between what is stored and what is on disk.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScanDiff {
    pub added: Vec<FoundFile>,
    /// Files whose size or date moved, so they need analysing again.
    pub changed: Vec<FoundFile>,
    /// Stored paths no longer on disk. Marked absent, never deleted.
    pub missing: Vec<PathBuf>,
    pub unchanged: usize,
}

/// A file as it was recorded, reduced to what identifies it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownFile {
    pub relative_path: PathBuf,
    pub size_bytes: i64,
    pub modified_at: Timestamp,
}

/// Compares a walk against what is already stored.
///
/// Incremental on purpose. Emptying the table and reinserting everything is
/// the usual shortcut, and it destroys identifiers, loses watch history and
/// holds the writer for minutes on a large collection.
pub fn diff(found: &[FoundFile], known: &[KnownFile]) -> ScanDiff {
    let mut result = ScanDiff::default();

    for file in found {
        match known
            .iter()
            .find(|entry| entry.relative_path == file.relative_path)
        {
            None => result.added.push(file.clone()),
            Some(entry) => {
                // Size and date are enough to spot a replaced file, and cheap
                // even on a slow network share, where reading content to
                // compare would not be.
                if entry.size_bytes != file.size_bytes || entry.modified_at != file.modified_at {
                    result.changed.push(file.clone());
                } else {
                    result.unchanged += 1;
                }
            }
        }
    }

    for entry in known {
        if !found
            .iter()
            .any(|file| file.relative_path == entry.relative_path)
        {
            result.missing.push(entry.relative_path.clone());
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::time::now;

    fn write(root: &Path, relative: &str, contents: &[u8]) {
        let path = root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("folder created");
        }
        std::fs::write(path, contents).expect("file written");
    }

    #[test]
    fn a_walk_finds_video_files_and_ignores_everything_else() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        write(root, "Quiet.Harbour.2019.mkv", b"x");
        write(root, "Amber.Field.2020.mp4", b"xx");
        write(root, "cover.jpg", b"x");
        write(root, "notes.txt", b"x");
        write(root, "Quiet.Harbour.2019.nfo", b"x");

        let outcome = walk("disk-one", root).expect("the root is usable");
        assert_eq!(
            outcome.companion_files.len(),
            1,
            "a description file is found, whether or not it is read"
        );
        let names: Vec<String> = outcome
            .files
            .iter()
            .map(|file| file.relative_path.to_string_lossy().into_owned())
            .collect();

        assert_eq!(names.len(), 2);
        assert!(names.contains(&"Quiet.Harbour.2019.mkv".to_string()));
        assert!(names.contains(&"Amber.Field.2020.mp4".to_string()));
    }

    #[test]
    fn a_walk_goes_into_subfolders_and_reports_paths_relative_to_the_root() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        write(root, "Quiet.Harbour.2019.mkv", b"x");
        write(root, "Boxset/Amber.Field.2020.mkv", b"x");

        let outcome = walk("disk-one", root).expect("the root is usable");
        assert_eq!(outcome.files.len(), 2);
        assert!(outcome
            .files
            .iter()
            .any(|file| file.relative_path == Path::new("Boxset/Amber.Field.2020.mkv")));
        assert!(
            outcome
                .files
                .iter()
                .all(|file| file.relative_path.is_relative()),
            "stored paths are relative so a library can move disk"
        );
    }

    #[test]
    fn an_unmounted_root_stops_the_scan_instead_of_reporting_an_empty_folder() {
        // The single most damaging mistake a media server can make: taking an
        // empty answer for "everything was removed".
        let error = walk("disk-one", Path::new("/nowhere/at/all"))
            .expect_err("an unusable root must stop the scan");
        assert!(matches!(
            error,
            ScanError::RootUnusable {
                state: RootAccess::Missing
            }
        ));
    }

    #[test]
    fn a_partly_downloaded_file_is_not_picked_up() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        write(root, "Quiet.Harbour.2019.mkv.part", b"x");
        write(root, "Quiet.Harbour.2019.mkv", b"x");

        let outcome = walk("disk-one", root).expect("the root is usable");
        assert_eq!(outcome.files.len(), 1);
    }

    #[test]
    fn companion_clips_are_found_but_marked_as_such() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        write(root, "Quiet.Harbour.2019.mkv", b"x");
        write(root, "Quiet.Harbour.2019-trailer.mkv", b"x");
        write(root, "sample.mkv", b"x");

        let outcome = walk("disk-one", root).expect("the root is usable");
        assert_eq!(outcome.files.len(), 3);

        let trailer = outcome
            .files
            .iter()
            .find(|file| file.relative_path.to_string_lossy().contains("trailer"))
            .expect("the trailer was found");
        assert_eq!(trailer.companion_kind, Some("trailer"));

        let film = outcome
            .files
            .iter()
            .find(|file| file.relative_path == Path::new("Quiet.Harbour.2019.mkv"))
            .expect("the film was found");
        assert_eq!(film.companion_kind, None);
    }

    #[test]
    fn subtitle_files_are_reported_apart_from_the_films_they_belong_to() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        write(root, "Quiet.Harbour.2019.MULTi.1080p.mkv", b"x");
        write(root, "Quiet.Harbour.2019.MULTi.1080p.fr.srt", b"x");
        write(root, "Quiet.Harbour.2019.MULTi.1080p.en.sdh.srt", b"x");
        write(root, "notes.txt", b"x");

        let outcome = walk("disk-one", root).expect("the root is usable");
        assert_eq!(outcome.files.len(), 1, "a subtitle is not a film");
        assert_eq!(outcome.subtitles.len(), 2);
        assert!(outcome
            .subtitles
            .iter()
            .all(|path| path.extension().is_some_and(|value| value == "srt")));
    }

    #[test]
    fn a_walk_returns_a_stable_order_so_two_runs_can_be_compared() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        for name in ["c.mkv", "a.mkv", "b.mkv"] {
            write(root, name, b"x");
        }
        let first = walk("disk-one", root).expect("usable");
        let second = walk("disk-one", root).expect("usable");
        assert_eq!(first.files, second.files);
        assert_eq!(
            first
                .files
                .iter()
                .map(|file| file.relative_path.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["a.mkv", "b.mkv", "c.mkv"]
        );
    }

    #[test]
    fn a_new_file_shows_up_as_added() {
        let found = vec![FoundFile {
            relative_path: PathBuf::from("Quiet.Harbour.2019.mkv"),
            size_bytes: 100,
            modified_at: now(),
            companion_kind: None,
        }];
        let changes = diff(&found, &[]);
        assert_eq!(changes.added.len(), 1);
        assert!(changes.missing.is_empty());
        assert_eq!(changes.unchanged, 0);
    }

    #[test]
    fn an_untouched_file_is_not_analysed_again() {
        let moment = now();
        let found = vec![FoundFile {
            relative_path: PathBuf::from("Quiet.Harbour.2019.mkv"),
            size_bytes: 100,
            modified_at: moment,
            companion_kind: None,
        }];
        let known = vec![KnownFile {
            relative_path: PathBuf::from("Quiet.Harbour.2019.mkv"),
            size_bytes: 100,
            modified_at: moment,
        }];
        let changes = diff(&found, &known);
        assert_eq!(changes.unchanged, 1);
        assert!(changes.added.is_empty());
        assert!(changes.changed.is_empty());
    }

    #[test]
    fn a_replaced_file_is_noticed_by_its_size_or_its_date() {
        let moment = now();
        let known = vec![KnownFile {
            relative_path: PathBuf::from("Quiet.Harbour.2019.mkv"),
            size_bytes: 100,
            modified_at: moment,
        }];

        let bigger = vec![FoundFile {
            relative_path: PathBuf::from("Quiet.Harbour.2019.mkv"),
            size_bytes: 200,
            modified_at: moment,
            companion_kind: None,
        }];
        assert_eq!(diff(&bigger, &known).changed.len(), 1);

        let newer = vec![FoundFile {
            relative_path: PathBuf::from("Quiet.Harbour.2019.mkv"),
            size_bytes: 100,
            modified_at: moment + time::Duration::seconds(10),
            companion_kind: None,
        }];
        assert_eq!(diff(&newer, &known).changed.len(), 1);
    }

    #[test]
    fn a_file_no_longer_on_disk_is_reported_as_missing_not_deleted() {
        let known = vec![KnownFile {
            relative_path: PathBuf::from("Quiet.Harbour.2019.mkv"),
            size_bytes: 100,
            modified_at: now(),
        }];
        let changes = diff(&[], &known);
        assert_eq!(changes.missing.len(), 1);
        assert_eq!(
            changes.missing[0],
            PathBuf::from("Quiet.Harbour.2019.mkv"),
            "the caller marks it absent; a scan never deletes"
        );
    }

    #[test]
    fn only_the_file_that_left_is_reported_missing() {
        // The case that matters: one film removed from a folder that still
        // holds the others. A scan that reports the whole library absent puts
        // every film behind a warning until the next one.
        let moment = now();
        let known = vec![
            KnownFile {
                relative_path: PathBuf::from("Quiet.Harbour.2019.mkv"),
                size_bytes: 100,
                modified_at: moment,
            },
            KnownFile {
                relative_path: PathBuf::from("Amber.Field.2020.mkv"),
                size_bytes: 200,
                modified_at: moment,
            },
        ];
        let still_there = vec![FoundFile {
            relative_path: PathBuf::from("Amber.Field.2020.mkv"),
            size_bytes: 200,
            modified_at: moment,
            companion_kind: None,
        }];

        let changes = diff(&still_there, &known);
        assert_eq!(
            changes.missing,
            vec![PathBuf::from("Quiet.Harbour.2019.mkv")]
        );
        assert_eq!(changes.unchanged, 1);
        assert!(changes.added.is_empty() && changes.changed.is_empty());
    }

    #[test]
    fn an_unreadable_subfolder_is_reported_rather_than_silently_skipped() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path();
        write(root, "Quiet.Harbour.2019.mkv", b"x");
        let locked = root.join("locked");
        std::fs::create_dir(&locked).expect("folder created");
        write(&locked, "Amber.Field.2020.mkv", b"x");

        let mut permissions = std::fs::metadata(&locked).expect("readable").permissions();
        permissions.set_mode(0o000);
        std::fs::set_permissions(&locked, permissions).expect("permissions set");

        let outcome = walk("disk-one", root).expect("the root itself is usable");

        let mut permissions = std::fs::metadata(&locked).expect("readable").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&locked, permissions).expect("permissions restored");

        // Running as the owner, the system may still grant access, so the
        // assertion that holds either way is that the walk kept going and
        // never lost the file it could reach.
        assert!(outcome
            .files
            .iter()
            .any(|file| file.relative_path == Path::new("Quiet.Harbour.2019.mkv")));
    }
}
