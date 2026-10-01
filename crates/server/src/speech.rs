//! The models that listen to personal videos: which one is in use, which are
//! on the disk, and the buttons that fetch and forget them.
//!
//! Administrators only. The address a model is fetched from is never part of a
//! request: a request names a model among the few the server offers, and the
//! address is fixed in the code that fetches it.

use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::account::Administrator;
use crate::error::Result;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/system/speech", get(status).put(choose))
        .route("/api/v1/system/speech/models/{id}", delete(forget))
        .route("/api/v1/system/speech/models/{id}/download", post(download))
}

#[derive(Debug, Serialize)]
struct StatusView {
    /// Whether the tool that listens is on this machine.
    tool_found: bool,
    /// The model in use, once one is chosen and on the disk.
    chosen: Option<&'static str>,
    models: Vec<ModelView>,
}

#[derive(Debug, Serialize)]
struct ModelView {
    id: &'static str,
    bytes: u64,
    downloaded: bool,
    downloading: bool,
}

impl From<melyxar_app::speech::Status> for StatusView {
    fn from(status: melyxar_app::speech::Status) -> Self {
        Self {
            tool_found: status.tool_found,
            chosen: status.chosen,
            models: status
                .models
                .into_iter()
                .map(|model| ModelView {
                    id: model.id,
                    bytes: model.bytes,
                    downloaded: model.downloaded,
                    downloading: model.downloading,
                })
                .collect(),
        }
    }
}

async fn status(_: Administrator, State(state): State<AppState>) -> Result<Json<StatusView>> {
    Ok(Json(melyxar_app::speech::status(&state).await?.into()))
}

#[derive(Debug, Deserialize)]
struct Chosen {
    /// Nothing to stop listening.
    model: Option<String>,
}

async fn choose(
    _: Administrator,
    State(state): State<AppState>,
    Json(chosen): Json<Chosen>,
) -> Result<Json<StatusView>> {
    melyxar_app::speech::choose(&state, chosen.model.as_deref()).await?;
    Ok(Json(melyxar_app::speech::status(&state).await?.into()))
}

async fn forget(
    _: Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<StatusView>> {
    melyxar_app::speech::forget(&state, &id).await?;
    Ok(Json(melyxar_app::speech::status(&state).await?.into()))
}

#[derive(Debug, Serialize)]
struct StartedView {
    job_id: String,
}

async fn download(
    _: Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<StartedView>> {
    let job = melyxar_app::speech::download(&state, &id).await?;
    Ok(Json(StartedView {
        job_id: job.id.to_string(),
    }))
}
