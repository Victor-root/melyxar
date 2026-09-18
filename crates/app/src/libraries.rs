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

use melyxar_core::id::{LibraryId, LibraryRootId};
use melyxar_core::library::{Library, LibraryKind, LibraryRoot, RootAccess};

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
    pub const ALL: [Self; 12] = [
        Self::NameNeeded,
        Self::NameTooLong,
        Self::NameTaken,
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
    for path in &asked.roots {
        let path = folder_to_look_in(state, path).await?;
        if roots.iter().any(|(_, kept)| nested(&path, kept)) {
            return Err(Trouble::Refused(Refused::FoldersNested));
        }
        roots.push((label_for(state, &path).await?, path));
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

    let label = label_for(state, &path).await?;
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

/// Gives a folder the name it is called by in logs and on screens.
///
/// Its own name would do until a collection is spread across four disks, where
/// every one of them holds a folder called `Films` and four roots would answer
/// to one word. The folder above it is what tells them apart, which is exactly
/// what somebody reading a log needs.
///
/// A name already taken by another root gets the two together, and a number
/// after that: a label nobody can tell apart is a log line nobody can place.
async fn label_for(state: &AppState, path: &Path) -> Result<String> {
    let own = name_of_folder(path);
    let above = path.parent().map(name_of_folder).unwrap_or_default();
    let taken: Vec<String> = state
        .database()
        .roots_with_access()
        .await?
        .into_iter()
        .map(|entry| entry.root.label)
        .collect();

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
