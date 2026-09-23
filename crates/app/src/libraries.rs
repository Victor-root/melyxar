//! Declaring a library: what it is called, what it holds, and where it looks.
//!
//! Until now this lived in the configuration file alone, which meant a
//! terminal on the server and a restart to add a disk. The file still declares
//! libraries, and still creates any it names that are not here yet; what
//! changes is that it is no longer the only way.
//!
//! Every rule a library is made under lives here rather than in the layer
//! above, because a library declared from a file and one declared from a
//! screen have to be the same library, made under the same rules, or the two
//! ways of asking drift apart.

use std::path::{Path, PathBuf};

use melyxar_core::id::{LibraryId, LibraryRootId, MediaSourceId};
use melyxar_core::library::{Library, LibraryKind, LibraryRoot, RootAccess};
pub use melyxar_database::deletion::SetAsideFile;
pub use melyxar_database::libraries::{Removed, WouldGo};

use crate::{AppError, AppState};

/// Why a library, or a folder for one, was refused.
///
/// A word and never a sentence: the wording belongs to whatever is showing it,
/// in the language of whoever is reading. These are the refusals somebody
/// meets while filling a form in, so each one has to say which thing to put
/// right rather than that something was wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NameNeeded,
    NameTooLong,
    NameTaken,
    /// Something is already at work on this library, so taking it away would
    /// make that work fail for a reason nobody could read.
    SomethingIsRunning,
    /// A kind of library this server does not have.
    UnknownKind,
    /// A library has to look somewhere.
    NoFolder,
    /// A path that is not a whole path, which nothing can resolve.
    NotAWholePath,
    FolderMissing,
    NotAFolder,
    FolderUnreadable,
    /// A library, this one or another, already looks in that folder or in one
    /// that holds it: every film in it would be found twice.
    FolderAlreadyLookedIn,
    /// Two of the folders chosen at once are inside one another.
    FoldersNested,
    LanguageNotTwoLetters,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NameNeeded => "name_needed",
            Self::NameTooLong => "name_too_long",
            Self::NameTaken => "name_taken",
            Self::SomethingIsRunning => "something_is_running",
            Self::UnknownKind => "unknown_kind",
            Self::NoFolder => "no_folder",
            Self::NotAWholePath => "not_a_whole_path",
            Self::FolderMissing => "folder_missing",
            Self::NotAFolder => "not_a_folder",
            Self::FolderUnreadable => "folder_unreadable",
            Self::FolderAlreadyLookedIn => "folder_already_looked_in",
            Self::FoldersNested => "folders_nested",
            Self::LanguageNotTwoLetters => "language_not_two_letters",
        }
    }

    /// Every one of them, so that a test can cross from here to the words the
    /// interface shows and catch a refusal nobody worded.
    pub const ALL: [Self; 13] = [
        Self::NameNeeded,
        Self::NameTooLong,
        Self::NameTaken,
        Self::SomethingIsRunning,
        Self::UnknownKind,
        Self::NoFolder,
        Self::NotAWholePath,
        Self::FolderMissing,
        Self::NotAFolder,
        Self::FolderUnreadable,
        Self::FolderAlreadyLookedIn,
        Self::FoldersNested,
        Self::LanguageNotTwoLetters,
    ];
}

/// What can go wrong here: something somebody typed, or the server itself.
///
/// Kept apart on purpose. The first is a form to correct and is shown where it
/// was typed; the second is a failure and is shown as one.
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

/// A library somebody is asking for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asked {
    pub name: String,
    pub kind: LibraryKind,
    /// The language its films are described in, as a two letter code.
    pub language: String,
    /// The folders it looks in. Several is the ordinary case: a collection
    /// spread across four disks is one library, not four.
    pub roots: Vec<PathBuf>,
}

/// Declares a library and starts a scan of it.
///
/// The scan follows at once, because somebody who has just said where their
/// films are is waiting to see them and not waiting to be told to press
/// another button. It is a job like any other: it shows, it counts, it stops.
pub async fn create(state: &AppState, asked: Asked) -> Result<Library> {
    let name = name_of(&asked.name)?;
    let language = language_of(&asked.language)?;
    if asked.roots.is_empty() {
        return Err(Trouble::Refused(Refused::NoFolder));
    }
    if state.database().library_by_name(&name).await?.is_some() {
        return Err(Trouble::Refused(Refused::NameTaken));
    }

    let mut roots: Vec<(String, PathBuf)> = Vec::new();
    let mut resolved: Vec<PathBuf> = Vec::new();
    for path in &asked.roots {
        let path = folder_to_look_in(state, path).await?;
        if resolved.iter().any(|kept| nested(&path, kept)) {
            return Err(Trouble::Refused(Refused::FoldersNested));
        }
        resolved.push(path.clone());
    }

    // Whether a folder's own name is worth using depends on whether another
    // folder of this very declaration answers to the same one: a root not
    // yet written to the database cannot otherwise be found colliding with
    // one being declared alongside it.
    let own_names: Vec<String> = resolved.iter().map(|path| name_of_folder(path)).collect();
    for (index, path) in resolved.into_iter().enumerate() {
        let already_taken = own_names[..index].contains(&own_names[index])
            || own_names[index + 1..].contains(&own_names[index]);
        roots.push((label_for(state, &path, already_taken).await?, path));
    }

    let library = state
        .database()
        .create_library(&name, asked.kind, &language, &roots)
        .await?;
    // Tested for real rather than read off the permission bits, and written
    // down now so the screen shows the state of a root it has just been given
    // rather than the most cautious guess.
    record_what_can_be_reached(state, &library).await;

    tracing::info!(
        library = library.name,
        kind = asked.kind.as_str(),
        roots = library.roots.len(),
        "a library was declared from a screen"
    );

    // A library nobody scans is an empty grid, which is indistinguishable from
    // a library declared wrongly.
    if let Err(error) = crate::scan::start_scan_and_identification(
        state,
        library.clone(),
        melyxar_core::job::JobPriority::REQUESTED,
        melyxar_core::refresh::RefreshMode::default(),
    )
    .await
    {
        tracing::warn!(library = library.name, %error, "the first scan would not start");
    }
    Ok(library)
}

/// Gives a library another folder to look in, and scans it.
pub async fn add_root(state: &AppState, library_id: LibraryId, path: &Path) -> Result<LibraryRoot> {
    let library = library_by_id(state, library_id).await?;
    let path = folder_to_look_in(state, path).await?;
    if library.roots.iter().any(|root| nested(&path, &root.path)) {
        return Err(Trouble::Refused(Refused::FolderAlreadyLookedIn));
    }

    let label = label_for(state, &path, false).await?;
    let root = state.database().add_root(library.id, &label, &path).await?;
    let access = melyxar_library::check_root_access(&root.path);
    state.database().set_root_access(root.id, access).await?;

    tracing::info!(
        library = library.name,
        root = root.label,
        access = access.as_str(),
        "a folder was added to a library from a screen"
    );

    let with_the_new_root = library_by_id(state, library_id).await?;
    if let Err(error) = crate::scan::start_scan_and_identification(
        state,
        with_the_new_root,
        melyxar_core::job::JobPriority::REQUESTED,
        melyxar_core::refresh::RefreshMode::default(),
    )
    .await
    {
        tracing::warn!(library = library.name, %error, "the scan of the new folder would not start");
    }
    Ok(root)
}

/// Calls a library something else.
///
/// The way a name typed wrongly is put right. Nothing else moves: the films,
/// the folders and everything anybody has watched belong to the library and
/// not to its name.
pub async fn rename(state: &AppState, library_id: LibraryId, name: &str) -> Result<Library> {
    let library = library_by_id(state, library_id).await?;
    let name = name_of(name)?;
    if name == library.name {
        return Ok(library);
    }
    if state.database().library_by_name(&name).await?.is_some() {
        return Err(Trouble::Refused(Refused::NameTaken));
    }

    state.database().rename_library(library.id, &name).await?;
    tracing::info!(was = library.name, now = name, "a library was renamed");
    library_by_id(state, library_id).await
}

/// What taking a library away would take with it.
///
/// Asked for just before the question is put, rather than read off a listing
/// that may be an hour old: the number somebody says yes to has to be the
/// number that goes.
pub async fn what_removing_takes(state: &AppState, library_id: LibraryId) -> Result<WouldGo> {
    library_by_id(state, library_id).await?;
    Ok(state
        .database()
        .what_would_go_with_a_library(library_id)
        .await?)
}

/// What taking one folder away from a library would take with it.
///
/// Fewer films than the folder holds whenever one of them is also held in
/// another folder of the library: that one stays, and loses a copy.
pub async fn what_removing_a_folder_takes(
    state: &AppState,
    library_id: LibraryId,
    root_id: LibraryRootId,
) -> Result<WouldGo> {
    let root = root_of(state, library_id, root_id).await?;
    Ok(state.database().what_would_go_with_a_root(root.id).await?)
}

/// Takes a library away, and everything the server knew about it.
///
/// **Not one file on the disk is touched.** The collection is not this
/// server's to remove: what goes is what it wrote down about it, which is the
/// films, their pages, their pictures, what was watched of them, the files as
/// rows and the folders as declared folders. Removing a film from the disk is
/// a different thing entirely, asked for film by film, and it never comes
/// through here.
///
/// Says how much went, which is the same count the screen showed before
/// anybody said yes.
pub async fn remove(state: &AppState, library_id: LibraryId) -> Result<Removed> {
    let library = library_by_id(state, library_id).await?;
    refuse_while_something_is_running_on(state, library_id).await?;

    // Read before the rows go: what is keyed on a file outside the database
    // cannot be found again once the row that named it is gone.
    let sources = state.database().source_ids_of_library(library_id).await?;
    let went = state.database().delete_library(library_id).await?;
    let sheets = forget_the_thumbnails_of(state, &sources).await;
    let pictures = forget_the_pictures(state, &went.swept.picture_paths).await;

    tell_what_went(
        &library.name,
        None,
        &went,
        pictures,
        sheets,
        "a library was taken away: its films, their pages and everything only \
         they pointed at are gone, and no file of the collection was touched",
    );
    Ok(went)
}

/// Takes one folder away from a library, with the films that were only in it.
///
/// No file on the disk is touched here either. A film also held in another
/// folder stays and loses that copy.
pub async fn remove_root(
    state: &AppState,
    library_id: LibraryId,
    root_id: LibraryRootId,
) -> Result<Removed> {
    let library = library_by_id(state, library_id).await?;
    let root = root_of(state, library_id, root_id).await?;
    refuse_while_something_is_running_on(state, library_id).await?;

    let sources = state.database().source_ids_of_root(root_id).await?;
    let went = state.database().delete_root(root_id).await?;
    let sheets = forget_the_thumbnails_of(state, &sources).await;
    let pictures = forget_the_pictures(state, &went.swept.picture_paths).await;
    if went.works > 0 {
        state.database().bump_library_version(library_id).await?;
    }

    tell_what_went(
        &library.name,
        Some(&root.label),
        &went,
        pictures,
        sheets,
        "a folder was taken away from a library: the films only it held, their \
         pages and everything only they pointed at are gone, and no file of \
         the collection was touched",
    );
    Ok(went)
}

/// The files of this library taken out of it while they stay on the disk.
pub async fn set_aside_files(state: &AppState, library_id: LibraryId) -> Result<Vec<SetAsideFile>> {
    library_by_id(state, library_id).await?;
    Ok(state.database().set_aside_files(library_id).await?)
}

/// Forgets that these files of this library were taken out of it, or all of
/// them when none is named, and scans the library so they come back at once.
/// Answers how many were taken back.
pub async fn take_back_set_aside(
    state: &AppState,
    library_id: LibraryId,
    files: Option<&[(LibraryRootId, PathBuf)]>,
) -> Result<u64> {
    let library = library_by_id(state, library_id).await?;
    let taken_back = state
        .database()
        .take_back_set_aside(library_id, files)
        .await?;
    if taken_back == 0 {
        return Ok(0);
    }
    state.database().bump_library_version(library_id).await?;
    tracing::info!(
        library = library.name,
        files = taken_back,
        "files taken out of a library were taken back"
    );
    if let Err(error) = crate::scan::start_scan_and_identification(
        state,
        library.clone(),
        melyxar_core::job::JobPriority::REQUESTED,
        melyxar_core::refresh::RefreshMode::default(),
    )
    .await
    {
        tracing::warn!(
            library = library.name,
            %error,
            "the scan that brings them back would not start; the next one will"
        );
    }
    Ok(taken_back)
}

/// Says what a removal took with it, in the one shape both removals use.
///
/// A library and one of its folders leave behind the same nine counts, and a
/// count added to one journal line and not the other is a removal that reads
/// differently depending on what was removed.
fn tell_what_went(
    library: &str,
    root: Option<&str>,
    went: &Removed,
    pictures: usize,
    sheets: usize,
    what: &'static str,
) {
    tracing::info!(
        library,
        root,
        works = went.works,
        files = went.files,
        people = went.swept.people,
        collections = went.swept.collections,
        genres = went.swept.genres,
        studios = went.swept.studios,
        picture_rows = went.swept.pictures,
        pictures_deleted = pictures,
        thumbnail_sheets_deleted = sheets,
        "{what}"
    );
}

/// Refuses while this library has work under way.
///
/// Taking a library out from under a scan makes it fail, and a job that fails
/// for a reason nobody can read is worse than a button that waits.
async fn refuse_while_something_is_running_on(
    state: &AppState,
    library_id: LibraryId,
) -> Result<()> {
    match something_is_running_on(state, library_id).await? {
        true => Err(Trouble::Refused(Refused::SomethingIsRunning)),
        false => Ok(()),
    }
}

/// Whether this library has work under way. Every kind of work this server
/// does to a library is asked about, rather than the scan alone: the readings
/// of the upkeep are just as much in the middle of it.
pub(crate) async fn something_is_running_on(
    state: &AppState,
    library_id: LibraryId,
) -> std::result::Result<bool, melyxar_database::DatabaseError> {
    let target = library_id.to_string();
    for kind in [
        melyxar_core::job::JobKind::ScanLibrary,
        melyxar_core::job::JobKind::IdentifyWork,
        melyxar_core::job::JobKind::ReadKeyFrames,
        melyxar_core::job::JobKind::GenerateThumbnails,
    ] {
        if state
            .database()
            .has_unfinished_job(kind, Some(&target))
            .await?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Throws away the sheets of thumbnails of files that are no longer known.
///
/// A library of three hundred films leaves as many folders of sheets, and
/// nothing would ever go looking for them again. Answers how many folders
/// really went, so the journal states it rather than implying it.
///
/// Never a failure of the removal: the rows are gone either way, and a cache
/// that could not be swept is a cache, not a library.
pub(crate) async fn forget_the_thumbnails_of(state: &AppState, sources: &[MediaSourceId]) -> usize {
    let mut gone = 0;
    for source in sources {
        let folder = state
            .config()
            .directories
            .thumbnails()
            .join(source.to_string());
        match tokio::fs::remove_dir_all(&folder).await {
            Ok(()) => gone += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::debug!(%error, "sheets of thumbnails left behind in the cache")
            }
        }
    }
    gone
}

/// Throws away the pictures of everything that has just gone.
///
/// The posters, the backdrops, the title images and the faces of a cast, in
/// every size they were prepared in. This is the heaviest thing a removal
/// leaves behind and the one that would grow without bound: a film's pictures
/// are of no use to anybody once the film is not here, and nothing would ever
/// come looking for them again.
///
/// Each file is named rather than its folder swept, because the row that named
/// it is the only thing that ever knew it was ours. The folder is then removed
/// if the last file in it has gone, which is what stops the cache filling with
/// empty folders.
///
/// Never a failure of the removal, for the same reason as the sheets above.
pub(crate) async fn forget_the_pictures(state: &AppState, paths: &[String]) -> usize {
    let root = state.config().directories.images();
    let mut gone = 0;
    let mut folders: Vec<std::path::PathBuf> = Vec::new();

    for path in paths {
        let file = root.join(path);
        // A path from the database, but never trusted as one: a row that
        // pointed outside the cache must not let a removal reach outside it.
        if !file.starts_with(&root) {
            tracing::warn!("a stored picture sat outside the cache and was left alone");
            continue;
        }
        match tokio::fs::remove_file(&file).await {
            Ok(()) => gone += 1,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                tracing::debug!(%error, "a picture was left behind in the cache");
                continue;
            }
        }
        if let Some(folder) = file.parent().map(Path::to_path_buf) {
            if !folders.contains(&folder) {
                folders.push(folder);
            }
        }
    }

    // Only ever the ones that are now empty: remove_dir refuses the rest, which
    // is exactly the check wanted and one nothing can race.
    for folder in folders {
        let _ = tokio::fs::remove_dir(&folder).await;
    }
    gone
}

/// Gives a folder the name it is called by in logs and on screens.
///
/// **Its own name, whenever that name is worth anything.** For most people
/// that is every root they will ever add: a folder called `Movie` says what
/// it holds, and the folder above it is only ever where a disk happens to be
/// mounted, which is `mnt` on every root anyone will ever declare and tells
/// nobody anything.
///
/// Only when that name collides, because a collection is spread across four
/// disks and every one of them holds a folder called `Films`, does the folder
/// above it take over: that is what tells the four apart, and it is what
/// somebody reading a log needs at that point. `already_taken` says whether
/// this call already knows the own name collides with another root being
/// declared in the same breath, which a root not yet written to the database
/// could not otherwise be found colliding with.
///
/// A name still taken after that gets the two together, and a number after
/// that: a label nobody can tell apart is a log line nobody can place.
async fn label_for(state: &AppState, path: &Path, already_taken: bool) -> Result<String> {
    let own = name_of_folder(path);
    let above = path.parent().map(name_of_folder).unwrap_or_default();
    let taken: Vec<String> = state
        .database()
        .roots_with_access()
        .await?
        .into_iter()
        .map(|entry| entry.root.label)
        .collect();

    if !own.is_empty() && !already_taken && !taken.contains(&own) {
        return Ok(own);
    }

    let wanted = if above.is_empty() { own.clone() } else { above };
    let mut candidates = vec![wanted.clone()];
    if !own.is_empty() && own != wanted {
        candidates.push(format!("{wanted} {own}"));
    }
    for number in 2..100 {
        candidates.push(format!("{wanted} {number}"));
    }

    Ok(candidates
        .into_iter()
        .find(|candidate| !candidate.is_empty() && !taken.contains(candidate))
        // Nothing is left to fall back on only if a hundred roots answer to
        // the same word, which is a library nobody could read anyway.
        .unwrap_or(wanted))
}

fn name_of_folder(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

/// Whether one of these two folders holds the other, or they are the same.
///
/// Both ways round on purpose: a library told to look in a disk and in a
/// folder of that disk would find every film in it twice, and it does not
/// matter which of the two was named first.
fn nested(one: &Path, other: &Path) -> bool {
    one.starts_with(other) || other.starts_with(one)
}

/// Checks a folder can really be looked in, and hands back its true form.
///
/// Made canonical first: a path written with a link, a dot or a doubled
/// separator in it is the same folder as the one already declared, and two
/// spellings of one folder would be two roots finding every film twice.
///
/// Tested by trying rather than by reading the permission bits, which lie as
/// soon as groups, network mounts or an unprivileged container are involved,
/// and that is exactly this server.
async fn folder_to_look_in(state: &AppState, path: &Path) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(Trouble::Refused(Refused::NotAWholePath));
    }
    let path = std::fs::canonicalize(path).map_err(|_| Trouble::Refused(Refused::FolderMissing))?;
    if !path.is_dir() {
        return Err(Trouble::Refused(Refused::NotAFolder));
    }

    let access = melyxar_library::check_root_access(&path);
    if !access.is_usable() {
        return Err(Trouble::Refused(match access {
            RootAccess::Missing => Refused::FolderMissing,
            _ => Refused::FolderUnreadable,
        }));
    }

    // Against every root of every library, not only of this one: two libraries
    // looking in one folder is the same film twice in two grids, and the
    // second one would carry its own history of what was watched.
    for entry in state.database().roots_with_access().await? {
        if nested(&path, &entry.root.path) {
            return Err(Trouble::Refused(Refused::FolderAlreadyLookedIn));
        }
    }
    Ok(path)
}

/// Tests every root of a library and writes down what it found.
///
/// Never a failure of the thing that asked for it: a library is declared, and
/// a root whose state could not be written down is a screen showing the most
/// cautious guess until the next start, which is a great deal better than a
/// library that was refused.
async fn record_what_can_be_reached(state: &AppState, library: &Library) {
    for root in &library.roots {
        let access = melyxar_library::check_root_access(&root.path);
        if let Err(error) = state.database().set_root_access(root.id, access).await {
            tracing::warn!(root = root.label, %error, "what a root allows could not be written down");
        }
    }
}

async fn library_by_id(state: &AppState, library_id: LibraryId) -> Result<Library> {
    state
        .database()
        .list_libraries()
        .await?
        .into_iter()
        .find(|library| library.id == library_id)
        .ok_or_else(|| Trouble::Failed(AppError::Domain(melyxar_core::Error::not_found("library"))))
}

/// The name a library is to be called, or a refusal saying why not.
fn name_of(asked: &str) -> Result<String> {
    let name = asked.trim();
    if name.is_empty() {
        return Err(Trouble::Refused(Refused::NameNeeded));
    }
    if name.chars().count() > 60 {
        return Err(Trouble::Refused(Refused::NameTooLong));
    }
    Ok(name.to_string())
}

/// A language code a provider can be asked in: two letters, which is what it
/// takes and what a library has always been declared with.
fn language_of(asked: &str) -> Result<String> {
    let language = asked.trim().to_lowercase();
    if language.len() != 2 || !language.chars().all(|letter| letter.is_ascii_lowercase()) {
        return Err(Trouble::Refused(Refused::LanguageNotTwoLetters));
    }
    Ok(language)
}

/// What the screen that adds a library shows: the folders inside one folder.
///
/// The one place a path comes from whoever is looking at a screen. Nothing is
/// opened, served or removed: a path arrives, it is made canonical, and the
/// folders inside it are named. See the decision in the architecture notes.
pub fn folders_in(path: &Path) -> Result<melyxar_library::Listing> {
    melyxar_library::folders_in(path).map_err(|error| {
        Trouble::Refused(match error {
            melyxar_library::FolderError::Missing => Refused::FolderMissing,
            melyxar_library::FolderError::NotAFolder => Refused::NotAFolder,
            melyxar_library::FolderError::Unreadable => Refused::FolderUnreadable,
        })
    })
}

/// Whether the root identifier belongs to the library named, which is what
/// keeps a root of one library from being renamed through another.
pub async fn root_of(
    state: &AppState,
    library_id: LibraryId,
    root_id: LibraryRootId,
) -> Result<LibraryRoot> {
    library_by_id(state, library_id)
        .await?
        .roots
        .into_iter()
        .find(|root| root.id == root_id)
        .ok_or_else(|| {
            Trouble::Failed(AppError::Domain(melyxar_core::Error::not_found(
                "library root",
            )))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories};

    async fn state_on(directory: &Path) -> AppState {
        let config = Config {
            directories: Directories {
                data: directory.join("data"),
                cache: directory.join("cache"),
                transcodes: directory.join("cache/transcodes"),
            },
            ..Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        AppState::new(config, database, None, None)
    }

    fn folder(inside: &Path, name: &str) -> PathBuf {
        let made = inside.join(name);
        std::fs::create_dir_all(&made).expect("folder made");
        made
    }

    /// Waits for the work a declaration set going.
    ///
    /// Declaring a library scans it, and a library with work under way is one
    /// the server refuses to take away. A test that takes one away has to let
    /// that work end first, exactly as somebody pressing the button would.
    ///
    /// Empty is not the same as ended. Declaring a library sets off a chain:
    /// the scan, then the look up, then the readings, each started only once
    /// the one before it has finished. Between two of them the list of
    /// unfinished work is momentarily empty, and a removal landing in that gap
    /// is refused because the next job appeared while it was being refused.
    /// Measured, not argued: this failed roughly once in ten whole runs, and
    /// always on a loaded machine, which is exactly when that gap is widest.
    ///
    /// So the list is watched until it has been empty for several rounds in a
    /// row rather than once, and whatever the caller then does is done through
    /// `once_it_is_allowed` below, which asks again rather than believing the
    /// first refusal.
    async fn once_nothing_is_running(state: &AppState) {
        let mut quiet = 0;
        while quiet < ROUNDS_OF_QUIET {
            if state
                .database()
                .unfinished_jobs()
                .await
                .expect("read")
                .is_empty()
            {
                quiet += 1;
            } else {
                quiet = 0;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    }

    /// How many quiet rounds mean the chain is really over rather than between
    /// two of its links.
    const ROUNDS_OF_QUIET: usize = 5;

    /// Does something the server refuses while work is under way, asking again
    /// until it is allowed.
    ///
    /// The only wait that cannot be raced: a chain always ends, so a refusal
    /// that comes from work being under way always stops coming.
    async fn once_it_is_allowed<T, F, Fut>(what: F) -> T
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        for _ in 0..HOWEVER_LONG_A_CHAIN_TAKES {
            match what().await {
                Ok(done) => return done,
                Err(Trouble::Refused(Refused::SomethingIsRunning)) => {
                    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
                }
                Err(error) => panic!("refused for a reason that will not pass: {error:?}"),
            }
        }
        panic!("the work under way never ended");
    }

    /// Long enough for a first scan of one file and the look up behind it,
    /// short enough that a test which will never pass says so.
    const HOWEVER_LONG_A_CHAIN_TAKES: usize = 1_500;

    fn asked(name: &str, roots: Vec<PathBuf>) -> Asked {
        Asked {
            name: name.to_string(),
            kind: LibraryKind::Movies,
            language: "fr".to_string(),
            roots,
        }
    }

    #[tokio::test]
    async fn a_library_declared_from_a_screen_is_the_one_a_file_would_have_made() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;
        let films = folder(directory.path(), "Films");

        let library = create(&state, asked("Films", vec![films.clone()]))
            .await
            .expect("the library is declared");

        assert_eq!(library.name, "Films");
        assert_eq!(library.kind, LibraryKind::Movies);
        assert_eq!(library.metadata_language, "fr");
        assert_eq!(library.roots.len(), 1);
        assert_eq!(
            library.roots[0].path,
            std::fs::canonicalize(&films).expect("canonical")
        );
        assert_eq!(
            state.database().roots_with_access().await.expect("read")[0].access,
            RootAccess::ReadWrite,
            "what a root allows is tested by trying, and written down at once"
        );
    }

    #[tokio::test]
    async fn two_libraries_never_look_in_one_folder_or_inside_one_another() {
        // Both ways round: a library told to look in a disk and in a folder of
        // that disk would find every film in it twice, and the second copy
        // would carry its own history of what was watched.
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;
        let disk = folder(directory.path(), "disk");
        let films = folder(&disk, "Films");

        create(&state, asked("Films", vec![films.clone()]))
            .await
            .expect("the first is declared");

        assert!(
            create(&state, asked("Again", vec![films.clone()]))
                .await
                .is_err(),
            "the same folder twice"
        );
        assert!(
            create(&state, asked("Above", vec![disk.clone()]))
                .await
                .is_err(),
            "a folder holding one already declared"
        );
        assert!(
            create(&state, asked("Below", vec![folder(&films, "Extras")]))
                .await
                .is_err(),
            "a folder inside one already declared"
        );

        // And the two ways of writing one path are one folder.
        let written_the_long_way = films.join("..").join("Films");
        assert!(
            create(&state, asked("Elsewhere", vec![written_the_long_way]))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn a_library_refuses_what_it_could_not_work_with_and_says_why() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;
        let films = folder(directory.path(), "Films");

        // A refusal is a word the screen showing it words itself, never a
        // sentence: whoever reads it is not reading English.
        let why = |trouble: Trouble| match trouble {
            Trouble::Refused(refused) => refused,
            Trouble::Failed(error) => panic!("this is a form to correct, not a failure: {error}"),
        };

        assert_eq!(
            why(create(&state, asked("  ", vec![films.clone()]))
                .await
                .expect_err("a library needs a name")),
            Refused::NameNeeded
        );
        assert_eq!(
            why(create(&state, asked("Films", Vec::new()))
                .await
                .expect_err("a library has to look somewhere")),
            Refused::NoFolder
        );
        assert_eq!(
            why(create(
                &state,
                asked("Films", vec![directory.path().join("nowhere")]),
            )
            .await
            .expect_err("a folder that is not there")),
            Refused::FolderMissing
        );

        let not_a_folder = directory.path().join("a-file.txt");
        std::fs::write(&not_a_folder, b"x").expect("file written");
        assert_eq!(
            why(create(&state, asked("Films", vec![not_a_folder]))
                .await
                .expect_err("a file is not a folder")),
            Refused::NotAFolder
        );
        assert_eq!(
            why(create(
                &state,
                Asked {
                    language: "klingon".into(),
                    ..asked("Films", vec![films.clone()])
                },
            )
            .await
            .expect_err("a language is two letters")),
            Refused::LanguageNotTwoLetters
        );

        create(&state, asked("Films", vec![films]))
            .await
            .expect("and the good one goes through");
        assert_eq!(
            state.database().list_libraries().await.expect("read").len(),
            1,
            "nothing a refusal touched was left behind"
        );
    }

    #[tokio::test]
    async fn a_second_library_of_the_same_name_is_refused_and_renaming_is_how_a_typo_is_fixed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;

        let library = create(
            &state,
            asked("Flims", vec![folder(directory.path(), "Films")]),
        )
        .await
        .expect("declared, with the name typed wrongly");
        assert!(
            create(
                &state,
                asked("flims", vec![folder(directory.path(), "Other")]),
            )
            .await
            .is_err(),
            "a name is taken whatever the case it was typed in"
        );

        let renamed = rename(&state, library.id, "Films").await.expect("renamed");
        assert_eq!(renamed.name, "Films");
        assert_eq!(
            renamed.roots, library.roots,
            "nothing but the name moves: the folders and everything found \
             through them belong to the library"
        );
    }

    #[tokio::test]
    async fn a_folder_with_a_name_of_its_own_is_called_by_that_name_and_not_by_where_it_is_mounted()
    {
        // The ordinary case, and the one the bug was in: a single disk mounted
        // under /mnt, holding one folder called Movie. The folder above it is
        // only ever where the disk happens to sit, and every root anyone will
        // ever declare on this machine shares it: it says nothing.
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;
        let mnt = folder(directory.path(), "mnt");
        let movie = folder(&mnt, "Movie");

        let library = create(&state, asked("Films", vec![movie]))
            .await
            .expect("declared");
        assert_eq!(library.roots[0].label, "Movie");

        // A second, differently named folder under the same mount point keeps
        // its own name too: nothing about "mnt" being shared makes it collide.
        let series = folder(&mnt, "Series");
        let root = add_root(&state, library.id, &series).await.expect("added");
        assert_eq!(root.label, "Series");
    }

    #[tokio::test]
    async fn every_root_is_called_something_a_log_can_be_read_by() {
        // Four disks each holding a folder called Films: their own name would
        // answer to one word four times over, so the folder above them is what
        // tells them apart.
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;
        let one = folder(&folder(directory.path(), "one"), "Films");
        let two = folder(&folder(directory.path(), "two"), "Films");

        let library = create(&state, asked("Films", vec![one, two]))
            .await
            .expect("declared");
        let labels: Vec<&str> = library
            .roots
            .iter()
            .map(|root| root.label.as_str())
            .collect();
        assert!(labels.contains(&"one"));
        assert!(labels.contains(&"two"));

        // And a third disk that really does answer to a name already taken
        // gets something of its own rather than a second "one".
        let again = folder(&folder(&folder(directory.path(), "deep"), "one"), "Films");
        let root = add_root(&state, library.id, &again).await.expect("added");
        assert_ne!(root.label, "one");
        assert!(!root.label.is_empty());
    }

    #[tokio::test]
    async fn taking_a_library_away_leaves_every_file_exactly_where_it_was() {
        // The promise this whole thing rests on: the collection is not this
        // server's to remove. It reads it.
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;
        let films = folder(directory.path(), "Films");
        let film = films.join("Quiet Harbour 2019.mkv");
        std::fs::write(&film, b"a film").expect("film written");

        let library = create(&state, asked("Films", vec![films.clone()]))
            .await
            .expect("declared");
        once_nothing_is_running(&state).await;
        once_it_is_allowed(|| remove(&state, library.id)).await;

        assert!(
            state
                .database()
                .list_libraries()
                .await
                .expect("read")
                .is_empty(),
            "the library is gone from what the server knows"
        );
        assert!(film.exists(), "and the film is still on the disk");
        assert_eq!(
            std::fs::read(&film).expect("still readable"),
            b"a film",
            "untouched, to the byte"
        );
        assert!(films.is_dir(), "and so is the folder it sits in");
    }

    #[tokio::test]
    async fn the_pictures_of_a_library_that_is_gone_leave_the_cache_with_it() {
        // Years of use is what this is really about: posters, title images and
        // the faces of every cast are the heaviest thing a removal would
        // otherwise leave behind, and nothing would ever come looking for them.
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;
        let films = folder(directory.path(), "Films");
        std::fs::write(films.join("Quiet Harbour 2019.mkv"), b"a film").expect("film written");

        let library = create(&state, asked("Films", vec![films]))
            .await
            .expect("declared");
        once_nothing_is_running(&state).await;

        let work = state
            .database()
            .recent_works(library.id, 1)
            .await
            .expect("read")
            .first()
            .expect("the scan found the film")
            .id;

        // A poster, as the identification would have left one.
        let relative = format!("works/{work}/poster-abc-400.webp");
        let poster = state.config().directories.images().join(&relative);
        std::fs::create_dir_all(poster.parent().expect("a folder")).expect("folder made");
        std::fs::write(&poster, b"a poster").expect("poster written");
        state
            .database()
            .replace_images(
                "work",
                &work.to_string(),
                "poster",
                &[melyxar_database::images::StoredImage {
                    owner_kind: "work".to_string(),
                    owner_id: work.to_string(),
                    image_kind: "poster".to_string(),
                    relative_path: relative,
                    width: Some(400),
                    height: Some(600),
                    fingerprint: "abc".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("poster recorded");

        let went = once_it_is_allowed(|| remove(&state, library.id)).await;

        assert_eq!(went.swept.pictures, 1, "the poster row went");
        assert!(!poster.exists(), "and so did the file it named");
        assert!(
            !poster.parent().expect("a folder").exists(),
            "and the folder it was the last thing in"
        );
    }

    #[tokio::test]
    async fn taking_a_folder_away_leaves_its_files_where_they_are_too() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;
        let films = folder(directory.path(), "Films");
        let more = folder(directory.path(), "More");
        std::fs::write(more.join("Amber Field 2021.mkv"), b"another film").expect("film written");

        let library = create(&state, asked("Films", vec![films]))
            .await
            .expect("declared");
        let root = add_root(&state, library.id, &more).await.expect("added");
        once_nothing_is_running(&state).await;

        once_it_is_allowed(|| remove_root(&state, library.id, root.id)).await;

        assert_eq!(
            state
                .database()
                .library_roots(library.id)
                .await
                .expect("read")
                .len(),
            1,
            "the library keeps the folder it still looks in"
        );
        assert!(more.join("Amber Field 2021.mkv").exists());
    }

    #[tokio::test]
    async fn nothing_is_taken_away_while_something_is_at_work_on_it() {
        // Taking a library out from under a scan makes that scan fail for a
        // reason nobody could read.
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;
        let library = create(
            &state,
            asked("Films", vec![folder(directory.path(), "Films")]),
        )
        .await
        .expect("declared");
        once_nothing_is_running(&state).await;

        let running = state
            .database()
            .create_job(
                melyxar_core::job::JobKind::GenerateThumbnails,
                melyxar_core::job::JobPriority::BACKGROUND,
                Some(&library.id.to_string()),
            )
            .await
            .expect("a reading is under way");

        assert!(matches!(
            remove(&state, library.id).await,
            Err(Trouble::Refused(Refused::SomethingIsRunning))
        ));
        assert_eq!(
            state.database().list_libraries().await.expect("read").len(),
            1,
            "and nothing was taken away"
        );

        state
            .database()
            .finish_job(running.id, melyxar_core::job::JobState::Succeeded, None)
            .await
            .expect("the reading ended");
        once_it_is_allowed(|| remove(&state, library.id)).await;
    }

    #[tokio::test]
    async fn a_folder_added_to_a_library_is_checked_the_same_way_as_a_first_one() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let state = state_on(directory.path()).await;
        let films = folder(directory.path(), "Films");
        let library = create(&state, asked("Films", vec![films.clone()]))
            .await
            .expect("declared");

        assert!(
            add_root(&state, library.id, &films).await.is_err(),
            "the folder it already looks in"
        );
        assert!(
            add_root(&state, library.id, &folder(&films, "Extras"))
                .await
                .is_err(),
            "a folder inside it"
        );

        let elsewhere = folder(directory.path(), "Elsewhere");
        add_root(&state, library.id, &elsewhere)
            .await
            .expect("another folder altogether");
        assert_eq!(
            state
                .database()
                .library_roots(library.id)
                .await
                .expect("read")
                .len(),
            2
        );
    }
}
