//! Deleting works: out of the library, and off the disk when asked.
//!
//! See the decisions on deleting a work for what goes and why. The one rule
//! that is never bent: only the files the library knows for this work are
//! ever deleted, and only inside the folder of the library they sit under.

use std::path::{Component, Path, PathBuf};

use melyxar_core::id::WorkId;
use melyxar_core::library::RootAccess;
use melyxar_core::user::User;
use melyxar_database::libraries::Removed;

pub use melyxar_database::deletion::{FileOfAWork, FileRole, WhatDeletingTakes};

use crate::AppError;
use crate::AppState;

/// Why a deletion was not done, in a word the interface turns into a
/// sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// Something is at work on this library, and taking a work out from
    /// under it would make it fail for a reason nobody could read.
    SomethingIsRunning,
    /// The disk holding a file cannot be written to, so nothing was deleted.
    DiskNotWritable,
    /// Some files could not be deleted. The work stays in the library.
    FilesResisted,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SomethingIsRunning => "something_is_running",
            Self::DiskNotWritable => "disk_not_writable",
            Self::FilesResisted => "files_resisted",
        }
    }

    /// Every one of them, so a test can check each has words on the screen.
    pub const ALL: [Self; 3] = [
        Self::SomethingIsRunning,
        Self::DiskNotWritable,
        Self::FilesResisted,
    ];
}

/// What can go wrong: a refusal to put into words, or the server itself.
#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error("refused: {}", .0.as_str())]
    Refused(Refused),
    #[error(transparent)]
    Failed(#[from] AppError),
}

impl From<melyxar_database::DatabaseError> for Trouble {
    fn from(error: melyxar_database::DatabaseError) -> Self {
        Self::Failed(AppError::from(error))
    }
}

type Result<T> = std::result::Result<T, Trouble>;

/// What deleting these works would take with them, for somebody allowed to
/// delete them. Asked just before the question is put, so the count somebody
/// says yes to is the count that goes. A work already gone is left out; when
/// none of them is there, there is nothing to ask about.
pub async fn what_deleting_takes(
    state: &AppState,
    who: &User,
    work_ids: &[WorkId],
) -> Result<WhatDeletingTakes> {
    may_delete(who, false)?;
    let going = state.database().what_deleting_takes(work_ids).await?;
    for &library_id in &going.library_ids {
        crate::reach::may_read(who, library_id)?;
    }
    match going.works {
        0 => Err(Trouble::Failed(
            melyxar_core::Error::not_found("work").into(),
        )),
        _ => Ok(going),
    }
}

/// Deletes these works and everything under them from the library, and
/// their files from the disk when `from_the_disk` says so. All of them or
/// none: every library and every disk is asked before anything goes.
///
/// Off the disk first: when a file resists, the works stay in the library
/// rather than leaving a file on the disk the library no longer knows. Kept
/// on the disk, their copies are set aside so the scan does not bring them
/// back.
pub async fn delete(
    state: &AppState,
    who: &User,
    work_ids: &[WorkId],
    from_the_disk: bool,
) -> Result<Deleted> {
    let going = what_deleting_takes(state, who, work_ids).await?;
    may_delete(who, from_the_disk)?;
    for &library_id in &going.library_ids {
        if crate::libraries::something_is_running_on(state, library_id).await? {
            return Err(Trouble::Refused(Refused::SomethingIsRunning));
        }
    }

    // Named before they go, for the line that says they went.
    let titles = crate::activity::titles_of(state, work_ids).await?;

    let off_the_disk = match from_the_disk {
        true => Some(delete_off_the_disk(&going.files).await?),
        false => None,
    };

    let database = state.database();
    let removed = database.delete_works(work_ids, !from_the_disk).await?;
    let sheets = crate::libraries::forget_the_thumbnails_of(state, &going.sources).await;
    let pictures = crate::libraries::forget_the_pictures(state, &removed.swept.picture_paths).await;
    for &library_id in &going.library_ids {
        database.bump_library_version(library_id).await?;
    }

    tracing::info!(
        who = %who.name,
        asked = work_ids.len(),
        works = removed.works,
        files = removed.files,
        off_the_disk = from_the_disk,
        pictures_deleted = pictures,
        thumbnail_sheets_deleted = sheets,
        "works were deleted"
    );
    crate::activity::record(
        state,
        crate::activity::Event::WorksDeleted {
            user: who.id,
            user_name: who.name.clone(),
            titles,
            works: removed.works,
            from_the_disk,
        },
    )
    .await;
    Ok(Deleted {
        removed,
        off_the_disk,
    })
}

/// What a deletion did.
#[derive(Debug)]
pub struct Deleted {
    pub removed: Removed,
    /// How many files were found gone from the disk once it was done, when
    /// the disk was asked to lose them.
    pub off_the_disk: Option<usize>,
}

/// Whether this account may delete, and off the disk when that is asked.
fn may_delete(who: &User, from_the_disk: bool) -> Result<()> {
    let allowed =
        who.permissions.may_delete && (!from_the_disk || who.permissions.may_delete_from_disk);
    match allowed {
        true => Ok(()),
        false => Err(Trouble::Failed(
            melyxar_core::Error::forbidden("this account may not delete that").into(),
        )),
    }
}

/// Whether the server may write to every folder these files sit under,
/// found out by trying, now: what the question offers and what the deletion
/// refuses are the same answer.
pub async fn disks_take_writes(files: &[FileOfAWork]) -> bool {
    let mut roots: Vec<PathBuf> = files.iter().map(|file| file.root_path.clone()).collect();
    roots.sort();
    roots.dedup();
    tokio::task::spawn_blocking(move || {
        roots
            .iter()
            .all(|root| melyxar_library::check_root_access(root) == RootAccess::ReadWrite)
    })
    .await
    .unwrap_or(false)
}

/// Deletes these files, then every folder that deleting them emptied, and
/// answers how many are now gone from the disk.
async fn delete_off_the_disk(files: &[FileOfAWork]) -> Result<usize> {
    // Every disk is asked first, so that one that cannot be written to
    // refuses the whole deletion before a single file has gone.
    if !disks_take_writes(files).await {
        return Err(Trouble::Refused(Refused::DiskNotWritable));
    }

    let files = files.to_vec();
    let tally = tokio::task::spawn_blocking(move || delete_every_one(&files))
        .await
        .unwrap_or(Tally { gone: 0, resisted: 1 });
    match tally.resisted {
        0 => Ok(tally.gone),
        _ => Err(Trouble::Refused(Refused::FilesResisted)),
    }
}

/// What became of the files a deletion was asked to take off the disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tally {
    /// Looked for once it was done, and not found.
    gone: usize,
    /// Still there, or never touched.
    resisted: usize,
}

/// Deletes each file, then looks for it again: a file counts as gone only
/// once the disk no longer has it, whatever the deletion answered. A file
/// already gone before is gone.
fn delete_every_one(files: &[FileOfAWork]) -> Tally {
    let mut tally = Tally { gone: 0, resisted: 0 };
    for file in files {
        if !stays_inside(&file.relative_path) {
            tracing::warn!(
                file = %file.relative_path.display(),
                "a file named outside the folder of its library was left alone"
            );
            tally.resisted += 1;
            continue;
        }
        let path = file.path();
        match std::fs::remove_file(&path) {
            Ok(()) => {
                tracing::info!(file = %path.display(), "a file was deleted off the disk");
                empty_folders_above(&file.root_path, &file.relative_path);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::warn!(file = %path.display(), %error, "a file could not be deleted");
            }
        }
        match std::fs::symlink_metadata(&path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => tally.gone += 1,
            _ => {
                tracing::warn!(file = %path.display(), "a file is still on the disk after its deletion");
                tally.resisted += 1;
            }
        }
    }
    tally
}

/// Whether a path under the folder of a library stays under it: nothing but
/// plain names, so nothing walks up and nothing starts from the root.
fn stays_inside(relative: &Path) -> bool {
    relative.components().next().is_some()
        && relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

/// Takes away the folders above a deleted file that are empty now, from the
/// nearest up, and stops at the first that still holds anything. The folder
/// of the library itself is never one of them.
fn empty_folders_above(root: &Path, relative: &Path) {
    let mut folder = relative.parent();
    while let Some(inside) = folder.filter(|inside| inside.components().next().is_some()) {
        // Only ever succeeds on a folder with nothing at all in it, hidden
        // files included, which is exactly the rule.
        if std::fs::remove_dir(root.join(inside)).is_err() {
            return;
        }
        tracing::info!(folder = %root.join(inside).display(), "an emptied folder was deleted");
        folder = inside.parent();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_that_could_leave_its_folder_is_never_followed() {
        assert!(stays_inside(Path::new("Films/Quiet Harbour.mkv")));
        assert!(!stays_inside(Path::new("../elsewhere.mkv")));
        assert!(!stays_inside(Path::new("Films/../../elsewhere.mkv")));
        assert!(!stays_inside(Path::new("/etc/passwd")));
        assert!(!stays_inside(Path::new("")));
    }

    #[test]
    fn deleting_a_file_takes_the_folders_it_emptied_and_no_other() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().join("Series");
        let season = root.join("Signal/Saison 1");
        std::fs::create_dir_all(&season).expect("folders");
        std::fs::create_dir_all(root.join("Signal/Saison 2")).expect("folders");
        std::fs::write(season.join("e1.mkv"), b"x").expect("file");
        std::fs::write(season.join("e2.mkv"), b"x").expect("file");

        let file = |name: &str| FileOfAWork {
            root_id: melyxar_core::id::LibraryRootId::new(),
            root_path: root.clone(),
            relative_path: PathBuf::from(format!("Signal/Saison 1/{name}")),
            role: melyxar_database::deletion::FileRole::Copy,
        };

        assert_eq!(delete_every_one(&[file("e1.mkv")]), Tally { gone: 1, resisted: 0 });
        assert!(season.exists(), "a folder still holding a file stays");

        assert_eq!(
            delete_every_one(&[file("e2.mkv"), file("gone.mkv")]),
            Tally { gone: 2, resisted: 0 },
            "a file already gone is gone"
        );
        assert!(!season.exists(), "the season it emptied goes");
        assert!(
            root.join("Signal").exists(),
            "a folder still holding another season stays"
        );
        assert!(root.exists(), "the folder of the library is never touched");
    }
}
