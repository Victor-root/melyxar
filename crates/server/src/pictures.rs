//! The pictures one work wears, and choosing them by hand.
//!
//! Reading what a work has is open to whoever may read the work; changing it
//! is an administrator's doing, like every other correction to what the
//! library says about itself.
//!
//! Nothing here does any work of its own: what a picture costs to prepare, and
//! the rule that a choice made by hand is never undone, both live where the
//! pictures are prepared.

use axum::extract::{Path, State};
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_app::metadata::{MetadataProvider, PictureKind};
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};
use crate::identifiers::parse_work;
use crate::account::Viewer;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/works/{id}/pictures",
            axum::routing::get(held_pictures),
        )
        .route(
            "/api/v1/works/{id}/pictures/offered",
            axum::routing::get(offered_pictures),
        )
        // One name, two verbs: putting a picture of this kind on the work, and
        // taking the one that is there off it.
        .route(
            "/api/v1/works/{id}/pictures/{kind}",
            axum::routing::put(choose_picture).delete(forget_picture),
        )
}

/// One picture the work wears, in every size the interface serves.
#[derive(Debug, Serialize)]
struct HeldPictureView {
    kind: String,
    /// The largest size held, which is what a panel showing it wants: the
    /// point of the panel is judging the picture.
    url: String,
    /// How large the picture really is, which is what says whether it is worth
    /// replacing.
    width: Option<i32>,
    height: Option<i32>,
    /// Whether somebody chose this one by hand, in which case no run of the
    /// identification replaces it.
    by_hand: bool,
}

/// What the work wears now, by kind.
async fn held_pictures(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<Vec<HeldPictureView>>> {
    let work_id = parse_work(&id)?;
    melyxar_app::reach::may_read_the_work(&state, &who, work_id).await?;

    let by_hand = state
        .database()
        .locked_fields(work_id)
        .await
        .map_err(internal)?;
    let held = state
        .database()
        .images_of("work", &work_id.to_db_string())
        .await
        .map_err(internal)?;

    let mut views = Vec::new();
    for kind in PictureKind::every() {
        let named = kind.as_str();
        // The largest of the set, which is the one worth looking at here. They
        // come back largest first, and a kind with nothing in it is a kind the
        // work does not wear.
        let Some(largest) = held
            .iter()
            .filter(|image| image.image_kind == named)
            .max_by_key(|image| image.width.unwrap_or(0))
        else {
            continue;
        };
        views.push(HeldPictureView {
            kind: named.to_string(),
            url: format!("/api/v1/images/{}", largest.relative_path),
            width: largest.width,
            height: largest.height,
            by_hand: by_hand.iter().any(|field| field == named),
        });
    }
    Ok(Json(views))
}

/// One picture the provider offers, as it offers it.
#[derive(Debug, Serialize)]
struct OfferedPictureView {
    kind: String,
    /// What the provider calls it, which is what is sent back to choose it.
    path: String,
    /// Full address of the picture at the provider, so the panel can show it
    /// without this server fetching a single one of them.
    url: String,
    width: Option<i64>,
    height: Option<i64>,
    language: Option<String>,
    vote_average: f64,
    vote_count: i64,
}

/// Everything the provider holds for this work, to be chosen among.
async fn offered_pictures(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<OfferedPictureView>>> {
    let work_id = parse_work(&id)?;
    let provider = crate::jobs::provider_of(&state)?;

    let offered = melyxar_app::images::offered_pictures(&state, provider.as_ref(), work_id).await?;
    Ok(Json(
        offered
            .into_iter()
            .map(|picture| OfferedPictureView {
                kind: picture.kind.as_str().to_string(),
                url: provider.image_url(&picture.path),
                path: picture.path,
                width: picture.width,
                height: picture.height,
                language: picture.language,
                vote_average: picture.vote_average,
                vote_count: picture.vote_count,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
struct ChosenPicture {
    /// What the provider calls the picture, exactly as it was offered.
    path: String,
}

#[derive(Debug, Serialize)]
struct ChangedView {
    changed: bool,
}

/// Puts one picture chosen by hand on the work.
async fn choose_picture(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path((id, kind)): Path<(String, String)>,
    Json(chosen): Json<ChosenPicture>,
) -> Result<Json<ChangedView>> {
    let work_id = parse_work(&id)?;
    let kind = parse_kind(&kind)?;
    let provider = crate::jobs::provider_of(&state)?;

    let changed = melyxar_app::images::choose_picture(
        &state,
        provider.as_ref(),
        work_id,
        kind,
        &chosen.path,
    )
    .await?;
    Ok(Json(ChangedView { changed }))
}

/// Takes the picture of that kind off the work.
async fn forget_picture(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path((id, kind)): Path<(String, String)>,
) -> Result<Json<ChangedView>> {
    let work_id = parse_work(&id)?;
    melyxar_app::images::forget_picture(&state, work_id, parse_kind(&kind)?).await?;
    Ok(Json(ChangedView { changed: true }))
}

/// A kind of picture as the address writes it. A word nobody knows is refused
/// rather than read as a poster: quietly changing the wrong picture is worse
/// than changing none.
fn parse_kind(word: &str) -> Result<PictureKind> {
    PictureKind::parse(word)
        .ok_or_else(|| ServerError::invalid_input("that is not a kind of picture"))
}

fn internal(error: impl std::fmt::Display) -> ServerError {
    ServerError::internal(error.to_string())
}
