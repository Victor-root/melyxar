//! The server's collections: listed, opened, made, renamed, deleted and
//! filled. Who may change them is the use case's affair.

use axum::extract::{Path, State};
use axum::{Json, Router};
use melyxar_app::collections::CollectionSummary;
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::account::Viewer;
use crate::catalogue::{card_view, CardView};
use crate::error::{Result, ServerError};
use crate::identifiers::{parse_collection, parse_work};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/collections",
            axum::routing::get(every_one).post(create),
        )
        .route(
            "/api/v1/collections/{id}",
            axum::routing::get(one).delete(delete),
        )
        .route("/api/v1/collections/{id}/name", axum::routing::put(rename))
        .route("/api/v1/collections/{id}/works", axum::routing::post(put))
        .route(
            "/api/v1/works/{id}/collections",
            axum::routing::get(holding),
        )
}

/// One collection as the list shows it.
#[derive(Debug, Serialize)]
struct SummaryView {
    id: String,
    name: String,
    made_by_hand: bool,
    /// How many of its works this account can reach.
    count: i64,
    /// Its first work, whose poster it wears.
    cover: Option<CardView>,
}

impl From<&CollectionSummary> for SummaryView {
    fn from(summary: &CollectionSummary) -> Self {
        Self {
            id: summary.id.to_string(),
            name: summary.name.clone(),
            made_by_hand: summary.made_by_hand,
            count: summary.held,
            cover: summary.cover.as_ref().map(card_view),
        }
    }
}

/// One collection and its works, in its order.
#[derive(Debug, Serialize)]
struct CollectionView {
    id: String,
    name: String,
    made_by_hand: bool,
    cards: Vec<CardView>,
}

async fn every_one(
    State(state): State<AppState>,
    Viewer(who): Viewer,
) -> Result<Json<Vec<SummaryView>>> {
    let all = melyxar_app::collections::every_one(&state, &who).await?;
    Ok(Json(all.iter().map(SummaryView::from).collect()))
}

async fn one(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<CollectionView>> {
    let held = melyxar_app::collections::one(&state, &who, parse_collection(&id)?)
        .await?
        .ok_or_else(|| ServerError::not_found("no such collection"))?;
    Ok(Json(CollectionView {
        id: held.id.to_string(),
        name: held.name,
        made_by_hand: held.made_by_hand,
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
    let works = made
        .works
        .iter()
        .map(|work| parse_work(work))
        .collect::<Result<Vec<_>>>()?;
    let id = melyxar_app::collections::create(&state, &who, &made.name, &works).await?;
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
    melyxar_app::collections::rename(&state, &who, parse_collection(&id)?, &renamed.name).await?;
    Ok(Json(()))
}

async fn delete(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<()>> {
    melyxar_app::collections::delete(&state, &who, parse_collection(&id)?).await?;
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
    let works = asked
        .works
        .iter()
        .map(|work| parse_work(work))
        .collect::<Result<Vec<_>>>()?;
    melyxar_app::collections::put(&state, &who, parse_collection(&id)?, &works, asked.in_it).await?;
    Ok(Json(()))
}

/// The collections made by hand a work is in.
#[derive(Debug, Serialize)]
struct HoldingView {
    collections: Vec<String>,
}

async fn holding(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<HoldingView>> {
    let ids = melyxar_app::collections::holding(&state, &who, parse_work(&id)?).await?;
    Ok(Json(HoldingView {
        collections: ids.iter().map(ToString::to_string).collect(),
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
