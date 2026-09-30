//! Files sent into a library from the interface.
//!
//! A file is written beside a hidden name while it arrives and given its own
//! only once it is whole, so a scan never meets half a film and a broken
//! connection leaves nothing behind. It is written by the account the server
//! runs as, and given the group and the rights of the folder it lands in: a
//! folder shared with other machines stays shared with them, without anybody
//! going over the rights by hand afterwards.
//!
//! Never a name that is not one file, never a way out of the folder of the
//! library, never a file the library has no use for, never over a file that is
//! already there, and never into a folder the server may not write into.

use std::path::{Path, PathBuf};

use melyxar_core::error::ErrorCode;
use melyxar_core::id::{LibraryId, LibraryRootId, WorkId};
use melyxar_core::library::{Library, LibraryKind};
use melyxar_core::user::User;
use melyxar_library::music::album_folder;
use melyxar_library::upload::{
    WHILE_ARRIVING, clean_file_name, clean_folder, is_uploadable,
};
use tokio::io::AsyncWriteExt;

use crate::reach::may_read;
use crate::{AppError, AppState, Result};

/// The most one file may weigh: a film of a very long sitting, and no more.
pub const LARGEST: u64 = 64 * 1024 * 1024 * 1024;

/// Where a file is to go.
#[derive(Debug, Clone)]
pub enum Target {
    /// A folder of a library, the folder itself when none is named.
    Folder {
        root: LibraryRootId,
        /// Its names, separated by slashes, under the root.
        folder: String,
    },
    /// The folder the songs of an album are in.
    Album(WorkId),
}

/// What was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Uploaded {
    /// Where it is, under its root.
    pub relative_path: PathBuf,
    pub bytes: u64,
}

fn refused(code: ErrorCode, detail: &str) -> AppError {
    AppError::Domain(melyxar_core::Error::new(code, detail))
}

/// A file on its way: written to a hidden name, and a name of its own only
/// once [`Arrival::finish`] says it is whole. Dropped unfinished, it takes
/// what was written with it.
pub struct Arrival {
    library: Library,
    file: Option<tokio::fs::File>,
    temporary: PathBuf,
    kept_as: PathBuf,
    folder: PathBuf,
    relative_path: PathBuf,
    written: u64,
}

impl Drop for Arrival {
    fn drop(&mut self) {
        if self.file.is_some() {
            let _ = std::fs::remove_file(&self.temporary);
        }
    }
}

/// Makes ready to receive one file: everything that can be refused is refused
/// here, before anything is sent.
pub async fn begin(
    state: &AppState,
    who: &User,
    library_id: LibraryId,
    target: &Target,
    name: &str,
) -> Result<Arrival> {
    if !who.permissions.may_upload {
        return Err(refused(
            ErrorCode::Forbidden,
            "this account may not add files to the libraries",
        ));
    }
    may_read(who, library_id)?;
    let library = state
        .database()
        .list_libraries()
        .await?
        .into_iter()
        .find(|library| library.id == library_id)
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("library")))?;
    let name = clean_file_name(name)
        .ok_or_else(|| refused(ErrorCode::InvalidInput, "that is not the name of a file"))?;
    if !is_uploadable(library.kind, &name) {
        return Err(refused(
            ErrorCode::InvalidInput,
            "this library has no use for that kind of file",
        ));
    }

    let (root_path, inside) = where_it_goes(state, &library, target).await?;
    if !melyxar_library::check_root_access(&root_path).allows_writing() {
        return Err(refused(
            ErrorCode::PathNotAllowed,
            "the server may not write into this folder",
        ));
    }
    let folder = make_folder(&root_path, &inside).await?;
    let kept_as = folder.join(&name);
    if tokio::fs::try_exists(&kept_as).await? {
        return Err(refused(ErrorCode::Conflict, "a file of that name is already there"));
    }
    let temporary = folder.join(format!(".{name}{WHILE_ARRIVING}"));
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .await
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => {
                refused(ErrorCode::Conflict, "a file of that name is already arriving")
            }
            std::io::ErrorKind::PermissionDenied => {
                refused(ErrorCode::PathNotAllowed, "the server may not write into this folder")
            }
            _ => AppError::Directory(error),
        })?;
    Ok(Arrival {
        library,
        file: Some(file),
        temporary,
        kept_as,
        folder,
        relative_path: inside.join(&name),
        written: 0,
    })
}

/// The root a file lands under, and the folder under it.
async fn where_it_goes(
    state: &AppState,
    library: &Library,
    target: &Target,
) -> Result<(PathBuf, PathBuf)> {
    match target {
        Target::Folder { root, folder } => {
            let root = library
                .roots
                .iter()
                .find(|held| held.id == *root)
                .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("library root")))?;
            let inside = clean_folder(folder)
                .ok_or_else(|| refused(ErrorCode::InvalidInput, "that is not a folder of the library"))?;
            Ok((root.path.clone(), inside))
        }
        Target::Album(album) => {
            if library.kind != LibraryKind::Music {
                return Err(refused(ErrorCode::InvalidInput, "only an album of music has a folder"));
            }
            let database = state.database();
            let song = database
                .songs_of_album(*album)
                .await?
                .into_iter()
                .next()
                .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("album")))?;
            let on_disk = database
                .song_on_disk(song)
                .await?
                .filter(|on_disk| on_disk.library_id == library.id)
                .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("album")))?;
            let inside = album_folder(on_disk.relative_path.parent().unwrap_or(Path::new("")));
            Ok((on_disk.root_path, inside.to_path_buf()))
        }
    }
}

/// The folder under the root, made as far as it is not there, each folder
/// made taking the group and the rights of the one it is in. Refused when it
/// would end up outside the root, which a link inside a library can arrange.
async fn make_folder(root: &Path, inside: &Path) -> Result<PathBuf> {
    let mut here = root.to_path_buf();
    for name in inside {
        let above = here.clone();
        here.push(name);
        if !tokio::fs::try_exists(&here).await? {
            tokio::fs::create_dir(&here).await.map_err(|error| match error.kind() {
                std::io::ErrorKind::PermissionDenied => {
                    refused(ErrorCode::PathNotAllowed, "the server may not write into this folder")
                }
                _ => AppError::Directory(error),
            })?;
            settle_like(&here, &above, true).await;
        }
    }
    let real_root = tokio::fs::canonicalize(root).await?;
    let real_folder = tokio::fs::canonicalize(&here).await?;
    if !real_folder.starts_with(&real_root) {
        return Err(refused(
            ErrorCode::PathNotAllowed,
            "that folder leads out of the library",
        ));
    }
    Ok(here)
}

/// Gives a file or a folder the group of the folder above it, and the rights
/// it allows: what is made by the account the server runs as is then as open
/// to the others as everything else in that folder is. Best effort: a group
/// the account does not belong to cannot be given, and what was made stays
/// what the account could make it.
#[cfg(unix)]
async fn settle_like(path: &Path, above: &Path, folder: bool) {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let (path, above) = (path.to_path_buf(), above.to_path_buf());
    let settled = tokio::task::spawn_blocking(move || {
        let reference = std::fs::metadata(&above)?;
        if std::fs::metadata(&path)?.gid() != reference.gid() {
            std::os::unix::fs::chown(&path, None, Some(reference.gid()))?;
        }
        let mode = match folder {
            true => reference.mode() & 0o7777,
            false => reference.mode() & 0o666,
        };
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode))
    })
    .await;
    if let Ok(Err(error)) | Err(error) = settled.map_err(std::io::Error::other) {
        tracing::debug!(%error, "the group and rights of the folder could not be given to what was sent");
    }
}

#[cfg(not(unix))]
async fn settle_like(_path: &Path, _above: &Path, _folder: bool) {}

impl Arrival {
    /// The next of the bytes.
    pub async fn push(&mut self, chunk: &[u8]) -> Result<()> {
        self.written += chunk.len() as u64;
        if self.written > LARGEST {
            return Err(refused(ErrorCode::InvalidInput, "that file is larger than any that is taken"));
        }
        let Some(file) = self.file.as_mut() else {
            return Err(refused(ErrorCode::Internal, "the file is already whole"));
        };
        file.write_all(chunk).await.map_err(|error| match error.kind() {
            std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded => {
                refused(ErrorCode::NoRoomLeft, "the disk has no room left")
            }
            _ => AppError::Directory(error),
        })
    }

    /// The file whole: put on the disk, given its name, and its group and
    /// rights. Nothing is sent afterwards.
    pub async fn finish(mut self) -> Result<Uploaded> {
        let Some(mut file) = self.file.take() else {
            return Err(refused(ErrorCode::Internal, "the file is already whole"));
        };
        let put_away = async {
            file.flush().await?;
            file.sync_all().await?;
            drop(file);
            if tokio::fs::try_exists(&self.kept_as).await? {
                return Err(refused(ErrorCode::Conflict, "a file of that name is already there"));
            }
            tokio::fs::rename(&self.temporary, &self.kept_as).await?;
            Ok(())
        }
        .await;
        if put_away.is_err() {
            let _ = tokio::fs::remove_file(&self.temporary).await;
        }
        put_away?;
        settle_like(&self.kept_as, &self.folder, false).await;
        tracing::info!(
            library = self.library.name,
            file = %self.relative_path.display(),
            bytes = self.written,
            "a file was added from the interface"
        );
        Ok(Uploaded {
            relative_path: self.relative_path.clone(),
            bytes: self.written,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::user::Permissions;

    async fn state_with_a_library(
        directory: &Path,
        kind: melyxar_core::library::LibraryKind,
    ) -> (AppState, Library, User) {
        let config = melyxar_config::Config {
            directories: melyxar_config::Directories {
                data: directory.join("data"),
                cache: directory.join("cache"),
                transcodes: directory.join("cache/transcodes"),
                ..Default::default()
            },
            ..melyxar_config::Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        let state = AppState::new(config, database, None, None);
        let disk = directory.join("disk");
        std::fs::create_dir_all(&disk).expect("a disk");
        let library = crate::libraries::create(
            &state,
            crate::libraries::Asked {
                name: "Everything".to_string(),
                kind,
                language: "en".to_string(),
                roots: vec![disk],
                options: melyxar_core::library::LibraryOptions::default(),
            },
        )
        .await
        .expect("a library");
        let who = state
            .database()
            .create_user("uploader", Some("a stored form"), &Permissions::administrator())
            .await
            .expect("an account");
        (state, library, who)
    }

    async fn send(
        state: &AppState,
        who: &User,
        library: &Library,
        target: &Target,
        name: &str,
        bytes: &[u8],
    ) -> Result<Uploaded> {
        let mut arriving = begin(state, who, library.id, target, name).await?;
        for chunk in bytes.chunks(3) {
            arriving.push(chunk).await?;
        }
        arriving.finish().await
    }

    #[tokio::test]
    async fn a_file_lands_whole_under_its_name_and_a_second_one_never_replaces_it() {
        let directory = tempfile::tempdir().expect("directory");
        let (state, library, who) =
            state_with_a_library(directory.path(), LibraryKind::Movies).await;
        let target = Target::Folder {
            root: library.roots[0].id,
            folder: "Quiet Harbour (2019)".to_string(),
        };
        let uploaded = send(&state, &who, &library, &target, "Quiet Harbour (2019).mkv", b"a film")
            .await
            .expect("sent");
        assert_eq!(uploaded.bytes, 6);
        let there = directory.path().join("disk/Quiet Harbour (2019)/Quiet Harbour (2019).mkv");
        assert_eq!(std::fs::read(&there).expect("written"), b"a film");
        let hidden: Vec<_> = std::fs::read_dir(there.parent().unwrap())
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with('.'))
            .collect();
        assert!(hidden.is_empty(), "nothing is left beside it");

        let again = send(&state, &who, &library, &target, "Quiet Harbour (2019).mkv", b"other").await;
        assert!(matches!(
            again,
            Err(AppError::Domain(ref error)) if error.code == ErrorCode::Conflict
        ));
        assert_eq!(std::fs::read(&there).unwrap(), b"a film", "the first is untouched");
    }

    #[tokio::test]
    async fn what_would_leave_the_library_or_has_no_use_there_is_refused_before_a_byte_is_written() {
        let directory = tempfile::tempdir().expect("directory");
        let (state, library, who) =
            state_with_a_library(directory.path(), LibraryKind::Movies).await;
        let root = library.roots[0].id;
        for (folder, name) in [
            ("../elsewhere", "a.mkv"),
            ("", "../a.mkv"),
            ("", "song.mp3"),
            ("", ".hidden.mkv"),
        ] {
            let target = Target::Folder { root, folder: folder.to_string() };
            let refused = begin(&state, &who, library.id, &target, name).await;
            assert!(
                matches!(refused, Err(AppError::Domain(ref error)) if error.code == ErrorCode::InvalidInput),
                "{folder:?} {name:?}"
            );
        }
        assert!(std::fs::read_dir(directory.path().join("disk")).unwrap().next().is_none());
    }

    #[tokio::test]
    async fn only_an_account_with_the_right_may_send() {
        let directory = tempfile::tempdir().expect("directory");
        let (state, library, _admin) =
            state_with_a_library(directory.path(), LibraryKind::Movies).await;
        let viewer = state
            .database()
            .create_user("viewer", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("an account");
        let target = Target::Folder { root: library.roots[0].id, folder: String::new() };
        let refused = begin(&state, &viewer, library.id, &target, "a.mkv").await;
        assert!(matches!(
            refused,
            Err(AppError::Domain(ref error)) if error.code == ErrorCode::Forbidden
        ));
    }

    #[tokio::test]
    async fn a_file_that_never_finishes_leaves_nothing_behind() {
        let directory = tempfile::tempdir().expect("directory");
        let (state, library, who) =
            state_with_a_library(directory.path(), LibraryKind::Movies).await;
        let target = Target::Folder { root: library.roots[0].id, folder: String::new() };
        let mut arriving = begin(&state, &who, library.id, &target, "half.mkv").await.expect("ready");
        arriving.push(b"half of it").await.expect("pushed");
        drop(arriving);
        assert!(std::fs::read_dir(directory.path().join("disk")).unwrap().next().is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn what_is_sent_takes_the_rights_of_the_folder_it_lands_in() {
        use std::os::unix::fs::PermissionsExt;
        let directory = tempfile::tempdir().expect("directory");
        let (state, library, who) =
            state_with_a_library(directory.path(), LibraryKind::Movies).await;
        let disk = directory.path().join("disk");
        std::fs::set_permissions(&disk, std::fs::Permissions::from_mode(0o775)).unwrap();
        let target = Target::Folder { root: library.roots[0].id, folder: "New Folder".to_string() };
        send(&state, &who, &library, &target, "a.mkv", b"x").await.expect("sent");
        let mode = |path: PathBuf| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(disk.join("New Folder")), 0o775, "a folder made is as open as the one it is in");
        assert_eq!(mode(disk.join("New Folder/a.mkv")), 0o664);
    }
}
