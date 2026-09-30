//! The tag manager: what the songs of an album carry in their files, what
//! would change, and writing it, renaming each file by a pattern where one
//! is given (see `docs/architecture/06-musique.md`).
//!
//! Only an account given the right does it, and only in a library that lets
//! its files be written. A copy of each file can be kept before it is
//! written. The files are renamed where they lie, never moved, and the scan
//! that follows reads them again under the same songs, so nothing kept about
//! them is lost.

use std::path::{Path, PathBuf};

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::job::JobPriority;
use melyxar_core::music_naming::{NameValues, name_from_pattern};
use melyxar_core::refresh::RefreshMode;
use melyxar_core::user::User;
use melyxar_database::music_tag_editing::SongOnDisk;
pub use melyxar_tags::EditedTags;

use crate::reach::may_read;
use crate::{AppError, AppState, Result};

/// What one song's file carries now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SongTags {
    pub song: WorkId,
    pub file_name: String,
    pub tags: EditedTags,
}

/// What a person wants a song's file to carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub song: WorkId,
    pub tags: EditedTags,
}

/// What writing would do to one song.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Planned {
    pub song: WorkId,
    pub file_name: String,
    /// The name it would be given, when the pattern changes it.
    pub new_file_name: Option<String>,
    /// The fields that would change, by name.
    pub changed: Vec<&'static str>,
    pub before: EditedTags,
    pub after: EditedTags,
}

/// What came of writing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Written {
    pub written: usize,
    /// The songs that could not be written, and why.
    pub failed: Vec<(WorkId, String)>,
}

/// Where the copies of the files are kept before they are written.
const BACKUPS: &str = "tag-backups";

/// What the songs of an album carry in their files, read from the files.
pub async fn album_tags(state: &AppState, who: &User, album: WorkId) -> Result<Vec<SongTags>> {
    let mut found = Vec::new();
    for song in state.database().songs_of_album(album).await? {
        let on_disk = writable(state, who, song).await?;
        found.push(SongTags {
            song,
            file_name: file_name_of(&on_disk.relative_path),
            tags: read_tags(on_disk.path()).await?,
        });
    }
    Ok(found)
}

/// What writing would do, without writing anything.
pub async fn plan(
    state: &AppState,
    who: &User,
    wanted: &[Wanted],
    pattern: Option<&str>,
) -> Result<Vec<Planned>> {
    let mut planned = Vec::with_capacity(wanted.len());
    for one in wanted {
        let on_disk = writable(state, who, one.song).await?;
        let before = read_tags(on_disk.path()).await?;
        let file_name = file_name_of(&on_disk.relative_path);
        let new_file_name = match pattern.map(str::trim).filter(|pattern| !pattern.is_empty()) {
            Some(pattern) => Some(renamed(pattern, &one.tags, &on_disk.relative_path)?),
            None => None,
        }
        .filter(|name| *name != file_name);
        planned.push(Planned {
            song: one.song,
            changed: one.tags.changed_from(&before),
            file_name,
            new_file_name,
            before,
            after: one.tags.clone(),
        });
    }
    Ok(planned)
}

/// Writes what is wanted into the files, renaming them by the pattern when
/// one is given, and keeping a copy of each first when asked. The library is
/// scanned again once it is done, which is what files the songs anew.
pub async fn write(
    state: &AppState,
    who: &User,
    wanted: &[Wanted],
    pattern: Option<&str>,
    keep_a_copy: bool,
) -> Result<Written> {
    let planned = plan(state, who, wanted, pattern).await?;
    let backups = state
        .config()
        .directories
        .data
        .join(BACKUPS)
        .join(melyxar_core::time::now().unix_timestamp().to_string());
    let mut written = Written::default();
    let mut libraries: Vec<LibraryId> = Vec::new();
    // A song that nothing changes is left alone, and so is its file: no copy
    // of it is kept and the library is not read again for it.
    for plan in planned
        .into_iter()
        .filter(|plan| !plan.changed.is_empty() || plan.new_file_name.is_some())
    {
        let Some(on_disk) = state.database().song_on_disk(plan.song).await? else {
            continue;
        };
        if !libraries.contains(&on_disk.library_id) {
            libraries.push(on_disk.library_id);
        }
        match write_one(
            state,
            &on_disk,
            &plan,
            keep_a_copy.then_some(backups.as_path()),
        )
        .await
        {
            Ok(()) => written.written += 1,
            Err(reason) => {
                tracing::warn!(file = %on_disk.path().display(), reason, "a song's tags could not be written");
                written.failed.push((plan.song, reason));
            }
        }
    }
    for library in libraries {
        read_the_library_again(state, library).await;
    }
    Ok(written)
}

/// Puts a picture beside the songs of an album as its cover, `cover.jpg` in
/// the album's folder, a disc's folder never. The picture is a JPEG, which
/// the page that sends it makes it.
pub async fn set_cover(
    state: &AppState,
    who: &User,
    album: WorkId,
    jpeg: Vec<u8>,
    keep_a_copy: bool,
) -> Result<()> {
    if !jpeg.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err(refused("a cover is sent as a JPEG picture"));
    }
    let songs = state.database().songs_of_album(album).await?;
    let Some(first) = songs.first() else {
        return Err(AppError::Domain(melyxar_core::Error::not_found("album")));
    };
    let on_disk = writable(state, who, *first).await?;
    let path = on_disk.path();
    let folder = path
        .parent()
        .map(melyxar_library::music::album_folder)
        .ok_or_else(|| refused("the album has no folder"))?
        .to_path_buf();
    let cover = folder.join("cover.jpg");
    if keep_a_copy && tokio::fs::try_exists(&cover).await.unwrap_or(false) {
        let relative = cover
            .strip_prefix(&on_disk.root_path)
            .unwrap_or(&cover)
            .to_path_buf();
        let backup = state
            .config()
            .directories
            .data
            .join(BACKUPS)
            .join(melyxar_core::time::now().unix_timestamp().to_string())
            .join(relative);
        copy_into(&cover, &backup).await.map_err(refused)?;
    }
    tokio::fs::write(&cover, jpeg)
        .await
        .map_err(|error| refused(format!("the cover could not be written: {error}")))?;
    read_the_library_again(state, on_disk.library_id).await;
    Ok(())
}

async fn write_one(
    state: &AppState,
    on_disk: &SongOnDisk,
    plan: &Planned,
    backups: Option<&Path>,
) -> std::result::Result<(), String> {
    let path = on_disk.path();
    if let Some(backups) = backups {
        copy_into(&path, &backups.join(&on_disk.relative_path)).await?;
    }
    if !plan.changed.is_empty() {
        let tags = plan.after.clone();
        let target = path.clone();
        tokio::task::spawn_blocking(move || melyxar_tags::write(&target, &tags))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
    }
    if let Some(name) = &plan.new_file_name {
        let renamed = on_disk.relative_path.with_file_name(name);
        let to = on_disk.root_path.join(&renamed);
        if tokio::fs::try_exists(&to).await.unwrap_or(false) {
            return Err(format!("a file named {name} is already there"));
        }
        tokio::fs::rename(&path, &to)
            .await
            .map_err(|error| format!("the file could not be renamed: {error}"))?;
        state
            .database()
            .rename_source_file(on_disk.source_id, &renamed)
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

/// A copy of a file, into its folder of copies.
async fn copy_into(from: &Path, to: &Path) -> std::result::Result<(), String> {
    if let Some(folder) = to.parent() {
        tokio::fs::create_dir_all(folder)
            .await
            .map_err(|error| format!("no copy could be kept: {error}"))?;
    }
    tokio::fs::copy(from, to)
        .await
        .map(|_| ())
        .map_err(|error| format!("no copy could be kept: {error}"))
}

/// The song's file, once it is sure this account may write into it.
async fn writable(state: &AppState, who: &User, song: WorkId) -> Result<SongOnDisk> {
    if !who.permissions.may_edit_tags {
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Forbidden,
            "this account may not edit the tags of songs",
        )));
    }
    let on_disk = state
        .database()
        .song_on_disk(song)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("song")))?;
    may_read(who, on_disk.library_id)?;
    if !state
        .database()
        .music_library_options(on_disk.library_id)
        .await?
        .tag_writing
    {
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Forbidden,
            "this library does not let its files be written",
        )));
    }
    Ok(on_disk)
}

async fn read_tags(path: PathBuf) -> Result<EditedTags> {
    let read = tokio::task::spawn_blocking(move || melyxar_tags::read(&path))
        .await
        .map_err(|error| refused(error.to_string()))?
        .map_err(|error| refused(error.to_string()))?;
    Ok(EditedTags::from(&read.tags))
}

/// The name a pattern gives a file, keeping its extension.
fn renamed(pattern: &str, tags: &EditedTags, relative_path: &Path) -> Result<String> {
    let extension = relative_path
        .extension()
        .map(|extension| extension.to_string_lossy().into_owned())
        .unwrap_or_default();
    name_from_pattern(pattern, &name_values(tags), &extension)
        .ok_or_else(|| refused("the pattern names something no song has, or gives an empty name"))
}

/// What a pattern can name a file by, from the tags it is being given.
fn name_values(tags: &EditedTags) -> NameValues {
    NameValues {
        title: tags.title.clone(),
        artist: (!tags.artists.is_empty()).then(|| tags.artists.join(", ")),
        album: tags.album.clone(),
        album_artist: (!tags.album_artists.is_empty()).then(|| tags.album_artists.join(", ")),
        track: tags.track,
        disc: tags.disc,
        year: tags.year,
    }
}

fn file_name_of(relative_path: &Path) -> String {
    relative_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// A scan of the library, which reads the written files again under the
/// same songs.
async fn read_the_library_again(state: &AppState, library: LibraryId) {
    let started = match crate::identify::library_of(state, library).await {
        Ok(library) => crate::scan::start_scan_and_identification(
            state,
            library,
            JobPriority::REQUESTED,
            RefreshMode::NewAndUpdatedFiles,
        )
        .await
        .map(|_| ()),
        Err(error) => Err(error),
    };
    if let Err(error) = started {
        tracing::warn!(%error, "the library could not be read again after its tags were written");
    }
}

fn refused(reason: impl Into<String>) -> AppError {
    AppError::Domain(melyxar_core::Error::invalid_input(reason))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_file_is_renamed_by_the_tags_it_is_given_and_keeps_its_extension() {
        let tags = EditedTags {
            title: Some("Quiet Harbour".to_string()),
            artists: vec!["Amber Field".to_string()],
            track: Some(1),
            ..EditedTags::default()
        };
        assert_eq!(
            renamed(
                "{track} - {artist} - {title}",
                &tags,
                Path::new("A/old name.flac")
            )
            .expect("named"),
            "01 - Amber Field - Quiet Harbour.flac"
        );
        assert!(renamed("{mood}", &tags, Path::new("A/x.flac")).is_err());
    }
}
