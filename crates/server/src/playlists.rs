//! Each account's playlists: listed, opened, made, renamed, deleted, filled
//! and put in order, always those of whoever asks.

use axum::extract::{Path, State};
use axum::{Json, Router};
use melyxar_app::playlists::PlaylistSummary;
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::account::Viewer;
use crate::catalogue::{card_view, CardView};
use crate::error::{Result, ServerError};
use crate::identifiers::{parse_playlist, parse_work, parse_works};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/playlists", axum::routing::get(every_one).post(create))
        .route(
            "/api/v1/playlists/{id}",
            axum::routing::get(one).delete(delete),
        )
        .route("/api/v1/playlists/{id}/name", axum::routing::put(rename))
        .route("/api/v1/playlists/{id}/works", axum::routing::post(put))
        .route("/api/v1/playlists/{id}/order", axum::routing::put(reorder))
        .route("/api/v1/works/{id}/playlists", axum::routing::get(holding))
}

/// One playlist as the list shows it.
#[derive(Debug, Serialize)]
struct SummaryView {
    id: String,
    name: String,
    /// How many of its works this account can reach.
    count: i64,
    /// Its first work, whose poster it wears.
    cover: Option<CardView>,
}

impl From<&PlaylistSummary> for SummaryView {
    fn from(summary: &PlaylistSummary) -> Self {
        Self {
            id: summary.id.to_string(),
            name: summary.name.clone(),
            count: summary.held,
            cover: summary.cover.as_ref().map(card_view),
        }
    }
}

/// One playlist and its works, in its order.
#[derive(Debug, Serialize)]
struct PlaylistView {
    id: String,
    name: String,
    cards: Vec<CardView>,
}

async fn every_one(State(state): State<AppState>, Viewer(who): Viewer) -> Result<Json<Vec<SummaryView>>> {
    let all = melyxar_app::playlists::every_one(&state, &who).await?;
    Ok(Json(all.iter().map(SummaryView::from).collect()))
}

async fn one(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<PlaylistView>> {
    let held = melyxar_app::playlists::one(&state, &who, parse_playlist(&id)?)
        .await?
        .ok_or_else(|| ServerError::not_found("no such playlist"))?;
    Ok(Json(PlaylistView {
        id: held.id.to_string(),
        name: held.name,
        cards: held.cards.iter().map(card_view).collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct Made {
    name: String,
    /// The works it starts with.
    #[serde(default)]
    works: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MadeView {
    id: String,
}

async fn create(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Json(made): Json<Made>,
) -> Result<Json<MadeView>> {
    let id = melyxar_app::playlists::create(&state, &who, &made.name, &parse_works(&made.works)?).await?;
    Ok(Json(MadeView { id: id.to_string() }))
}

#[derive(Debug, Deserialize)]
struct Renamed {
    name: String,
}

async fn rename(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(renamed): Json<Renamed>,
) -> Result<Json<()>> {
    melyxar_app::playlists::rename(&state, &who, parse_playlist(&id)?, &renamed.name).await?;
    Ok(Json(()))
}

async fn delete(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<()>> {
    melyxar_app::playlists::delete(&state, &who, parse_playlist(&id)?).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct Put {
    works: Vec<String>,
    /// Put in, or taken out.
    in_it: bool,
}

async fn put(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(asked): Json<Put>,
) -> Result<Json<()>> {
    melyxar_app::playlists::put(&state, &who, parse_playlist(&id)?, &parse_works(&asked.works)?, asked.in_it)
        .await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct Order {
    works: Vec<String>,
}

async fn reorder(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(order): Json<Order>,
) -> Result<Json<()>> {
    melyxar_app::playlists::reorder(&state, &who, parse_playlist(&id)?, &parse_works(&order.works)?).await?;
    Ok(Json(()))
}

/// The playlists of this account a work is in.
#[derive(Debug, Serialize)]
struct HoldingView {
    playlists: Vec<String>,
}

async fn holding(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<HoldingView>> {
    let ids = melyxar_app::playlists::holding(&state, &who, parse_work(&id)?).await?;
    Ok(Json(HoldingView {
        playlists: ids.iter().map(ToString::to_string).collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }
}
