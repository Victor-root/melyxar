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
        .route("/api/v1/libraries/{id}/name", axum::routing::put(rename))
        .route(
            "/api/v1/libraries/{id}/roots",
            axum::routing::post(add_root),
        )
        .route(
            "/api/v1/libraries/{id}/roots/{root}/label",
            axum::routing::put(rename_root),
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
    State(state): State<AppState>,
    Query(asked): Query<Where>,
) -> Result<Json<ListingView>> {
    crate::administrator(&state).await?;
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
    /// One of the stored library kinds: movies, series, anime, shows, music.
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
    Json(asked): Json<NewLibrary>,
) -> Result<Json<DeclaredView>> {
    crate::administrator(&state).await?;
    let kind = LibraryKind::parse(&asked.kind)
        .ok_or_else(|| ServerError::invalid_input("there is no library of that kind"))?;

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
    UrlPath(id): UrlPath<String>,
    Json(asked): Json<NewName>,
) -> Result<Json<NamedView>> {
    crate::administrator(&state).await?;
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
    /// What it is called in logs and on screens, worked out from the folder
    /// above it so that four disks each holding a folder called Films do not
    /// all answer to one word.
    label: String,
}

/// Gives a library another folder to look in, and scans it.
async fn add_root(
    State(state): State<AppState>,
    UrlPath(id): UrlPath<String>,
    Json(asked): Json<NewRoot>,
) -> Result<Json<RootView>> {
    crate::administrator(&state).await?;
    let root = melyxar_app::libraries::add_root(
        &state,
        library_id(&id)?,
        std::path::Path::new(&asked.path),
    )
    .await?;

    Ok(Json(RootView {
        id: root.id.to_string(),
        label: root.label,
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
    UrlPath((id, root)): UrlPath<(String, String)>,
    Json(asked): Json<NewLabel>,
) -> Result<Json<RootView>> {
    crate::administrator(&state).await?;
    let root_id: LibraryRootId = root
        .parse()
        .map_err(|_| ServerError::invalid_input("the folder identifier is malformed"))?;
    let root = melyxar_app::libraries::root_of(&state, library_id(&id)?, root_id).await?;

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
    }))
}

fn library_id(id: &str) -> Result<LibraryId> {
    id.parse()
        .map_err(|_| ServerError::invalid_input("the library identifier is malformed"))
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
}
