//! Declaring a library from a screen, and looking around the disk for it.
//!
//! The one corner of this server where a path arrives from whoever is looking
//! at a screen. What that is allowed to do is written down in the architecture
//! notes and is narrow on purpose: folders are named and counted, nothing is
//! opened, served or removed, and what is chosen becomes a declared root,
//! which is exactly what the configuration file used to give.
//!
//! Every one of these is an administrator's to do, and says so through the one
//! check that exists for it.

use axum::extract::{Path as UrlPath, Query, State};
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_core::id::{LibraryId, LibraryRootId};
use melyxar_core::library::LibraryKind;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/folders", axum::routing::get(folders))
        .route("/api/v1/libraries", axum::routing::post(create))
        .route("/api/v1/libraries/{id}", axum::routing::delete(remove))
        .route("/api/v1/libraries/{id}/name", axum::routing::put(rename))
        .route(
            "/api/v1/libraries/{id}/removal",
            axum::routing::get(what_removing_takes),
        )
        .route(
            "/api/v1/libraries/{id}/roots",
            axum::routing::post(add_root),
        )
        .route(
            "/api/v1/libraries/{id}/roots/{root}",
            axum::routing::delete(remove_root),
        )
        .route(
            "/api/v1/libraries/{id}/roots/{root}/label",
            axum::routing::put(rename_root),
        )
        .route(
            "/api/v1/libraries/{id}/roots/{root}/removal",
            axum::routing::get(what_removing_a_folder_takes),
        )
}

#[derive(Debug, Deserialize)]
struct Where {
    /// The folder to look inside. Left out means the top of the tree, which is
    /// where somebody with nothing typed in starts.
    path: Option<String>,
}

#[derive(Debug, Serialize)]
struct FolderView {
    name: String,
    path: String,
    /// How many videos sit directly inside, counted up to a bound. The one
    /// number that answers "is this the folder I mean".
    videos: usize,
    /// Whether that count stopped at the bound rather than at the end.
    more_videos: bool,
}

#[derive(Debug, Serialize)]
struct ListingView {
    /// The folder that was listed, in its one true form: what came in may have
    /// been written with a link or a dot in it.
    path: String,
    /// Where going up leads, absent at the top of the tree.
    parent: Option<String>,
    folders: Vec<FolderView>,
    /// Whether the list stopped at a bound rather than at the end.
    cut_short: bool,
}

/// The folders inside one folder, for somebody choosing where a library looks.
///
/// Never a file name, here or anywhere below.
async fn folders(
    _: crate::account::Administrator,
    Query(asked): Query<Where>,
) -> Result<Json<ListingView>> {
    let listing = melyxar_app::libraries::folders_in(std::path::Path::new(
        asked.path.as_deref().unwrap_or_default(),
    ))?;

    Ok(Json(ListingView {
        path: listing.path.to_string_lossy().to_string(),
        parent: listing
            .parent
            .map(|path| path.to_string_lossy().to_string()),
        folders: listing
            .folders
            .into_iter()
            .map(|folder| FolderView {
                name: folder.name,
                path: folder.path.to_string_lossy().to_string(),
                videos: folder.videos,
                more_videos: folder.more_videos,
            })
            .collect(),
        cut_short: listing.cut_short,
    }))
}

#[derive(Debug, Deserialize)]
struct NewLibrary {
    name: String,
    /// One of the stored library kinds: movies, series, anime, shows,
    /// home_media, music.
    kind: String,
    /// The language its films are described in, as a two letter code.
    metadata_language: String,
    /// The folders it looks in. Several is the ordinary case.
    roots: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DeclaredView {
    id: String,
    name: String,
    /// The job that is already reading those folders, so a screen can follow
    /// it rather than leaving somebody looking at an empty grid.
    scanning: bool,
}

/// Declares a library, and sets a scan of it going.
async fn create(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    Json(asked): Json<NewLibrary>,
) -> Result<Json<DeclaredView>> {
    let kind = LibraryKind::parse(&asked.kind).ok_or(melyxar_app::libraries::Trouble::Refused(
        melyxar_app::libraries::Refused::UnknownKind,
    ))?;

    let library = melyxar_app::libraries::create(
        &state,
        melyxar_app::libraries::Asked {
            name: asked.name,
            kind,
            language: asked.metadata_language,
            roots: asked.roots.into_iter().map(Into::into).collect(),
        },
    )
    .await?;

    Ok(Json(DeclaredView {
        id: library.id.to_string(),
        name: library.name,
        scanning: true,
    }))
}

#[derive(Debug, Deserialize)]
struct NewName {
    name: String,
}

#[derive(Debug, Serialize)]
struct NamedView {
    name: String,
}

/// Calls a library something else. Nothing but the name moves.
async fn rename(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    UrlPath(id): UrlPath<String>,
    Json(asked): Json<NewName>,
) -> Result<Json<NamedView>> {
    let library = melyxar_app::libraries::rename(&state, library_id(&id)?, &asked.name).await?;
    Ok(Json(NamedView { name: library.name }))
}

#[derive(Debug, Deserialize)]
struct NewRoot {
    path: String,
}

#[derive(Debug, Serialize)]
struct RootView {
    id: String,
    /// What it is called in logs, worked out from its own name or, when that
    /// collides, from the folder above it: four disks each holding a folder
    /// called Films would otherwise all answer to one word.
    label: String,
    /// Its whole path on the server's disk. Shown only here, on the screen
    /// where roots are managed, for the administrator who has to tell two
    /// roots apart by more than a label they gave one themselves.
    path: String,
}

/// Gives a library another folder to look in, and scans it.
async fn add_root(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    UrlPath(id): UrlPath<String>,
    Json(asked): Json<NewRoot>,
) -> Result<Json<RootView>> {
    let root = melyxar_app::libraries::add_root(
        &state,
        library_id(&id)?,
        std::path::Path::new(&asked.path),
    )
    .await?;

    Ok(Json(RootView {
        id: root.id.to_string(),
        label: root.label,
        path: root.path.to_string_lossy().into_owned(),
    }))
}

#[derive(Debug, Deserialize)]
struct NewLabel {
    label: String,
}

/// Calls one folder of a library something else.
///
/// The label is what every log line and every screen shows instead of the
/// path, so one that reads badly is worth being able to put right.
async fn rename_root(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    UrlPath((id, root)): UrlPath<(String, String)>,
    Json(asked): Json<NewLabel>,
) -> Result<Json<RootView>> {
    let root = melyxar_app::libraries::root_of(&state, library_id(&id)?, root_id(&root)?).await?;

    let label = asked.label.trim();
    if label.is_empty() {
        return Err(ServerError::invalid_input(
            "a folder needs a name, since that is what logs show instead of its path",
        ));
    }
    state
        .database()
        .rename_root(root.id, label)
        .await
        .map_err(|error| ServerError::internal(error.to_string()))?;

    tracing::debug!(was = root.label, now = label, "a folder was renamed");
    Ok(Json(RootView {
        id: root.id.to_string(),
        label: label.to_string(),
        path: root.path.to_string_lossy().into_owned(),
    }))
}

#[derive(Debug, Serialize)]
struct WouldGoView {
    /// Films the server would stop knowing about. Not one of them leaves the
    /// disk.
    works: i64,
    /// The files behind them, as rows: the files themselves stay where they
    /// are.
    files: i64,
}

impl From<melyxar_app::libraries::WouldGo> for WouldGoView {
    fn from(went: melyxar_app::libraries::WouldGo) -> Self {
        Self {
            works: went.works,
            files: went.files,
        }
    }
}

/// What a removal really took, as the two numbers the screen said out loud.
///
/// The rest of what went with it, the people nobody credits any more and the
/// pictures of films that are not here, is in the journal: it is the answer to
/// "is this thing cleaning up after itself", not to "what am I about to lose".
impl From<melyxar_app::libraries::Removed> for WouldGoView {
    fn from(went: melyxar_app::libraries::Removed) -> Self {
        Self {
            works: went.works,
            files: went.files,
        }
    }
}

/// How much a library holds, asked just before the question is put.
async fn what_removing_takes(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    UrlPath(id): UrlPath<String>,
) -> Result<Json<WouldGoView>> {
    Ok(Json(
        melyxar_app::libraries::what_removing_takes(&state, library_id(&id)?)
            .await?
            .into(),
    ))
}

/// The same for one folder of a library.
async fn what_removing_a_folder_takes(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    UrlPath((id, root)): UrlPath<(String, String)>,
) -> Result<Json<WouldGoView>> {
    Ok(Json(
        melyxar_app::libraries::what_removing_a_folder_takes(
            &state,
            library_id(&id)?,
            root_id(&root)?,
        )
        .await?
        .into(),
    ))
}

/// Takes a library away. No file on the disk is touched.
async fn remove(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    UrlPath(id): UrlPath<String>,
) -> Result<Json<WouldGoView>> {
    Ok(Json(
        melyxar_app::libraries::remove(&state, library_id(&id)?)
            .await?
            .into(),
    ))
}

/// Takes one folder away from a library. No file on the disk is touched.
async fn remove_root(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    UrlPath((id, root)): UrlPath<(String, String)>,
) -> Result<Json<WouldGoView>> {
    Ok(Json(
        melyxar_app::libraries::remove_root(&state, library_id(&id)?, root_id(&root)?)
            .await?
            .into(),
    ))
}

fn library_id(id: &str) -> Result<LibraryId> {
    id.parse()
        .map_err(|_| ServerError::invalid_input("the library identifier is malformed"))
}

fn root_id(id: &str) -> Result<LibraryRootId> {
    id.parse()
        .map_err(|_| ServerError::invalid_input("the folder identifier is malformed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        // A path written wrongly takes the server down as it starts, which is
        // the one failure a test here can catch before the maintainer does.
        let _ = router();
    }

    #[test]
    fn a_library_identifier_that_is_not_one_is_refused_rather_than_guessed_at() {
        assert!(library_id("not-an-identifier").is_err());
    }

    /// Every refusal somebody can meet while declaring a library has words in
    /// both languages.
    ///
    /// The words live in the interface and the reasons live in the app crate,
    /// so nothing but a test crossing from one to the other can catch a
    /// refusal nobody worded. One with no words reaches the screen as
    /// `refused.library.folder_already_looked_in`, in front of somebody who is
    /// in the middle of filling a form in.
    #[test]
    fn every_reason_a_library_is_refused_for_has_words_in_both_languages() {
        let words = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/i18n.ts"),
        )
        .expect("the words of the interface");

        for refused in melyxar_app::libraries::Refused::ALL {
            let key = format!("refused.library.{}", refused.as_str());
            assert_eq!(
                words.matches(&format!("\"{key}\":")).count(),
                2,
                "{key} needs a sentence in English and one in French"
            );
        }
    }
}
