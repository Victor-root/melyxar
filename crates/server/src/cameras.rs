//! The cameras of a library of cameras: declared by the administrator, listed
//! for the accounts granted the library, and handed live to their
//! applications.

use axum::extract::{Path as UrlPath, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use melyxar_app::cameras::{Declared, NewPassword};
use melyxar_app::AppState;
use melyxar_core::id::{CameraId, LibraryId};
use serde::{Deserialize, Serialize};

use crate::account::{Administrator, Viewer};
use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/libraries/{id}/cameras",
            axum::routing::get(cameras).post(declare),
        )
        .route(
            "/api/v1/cameras/{id}",
            axum::routing::put(change).delete(remove),
        )
        .route("/api/v1/cameras/{id}/live", axum::routing::get(live))
}

#[derive(Debug, Serialize)]
struct CameraView {
    id: String,
    name: String,
    /// Only for an administrator.
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    username: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    has_password: Option<bool>,
}

async fn cameras(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    UrlPath(id): UrlPath<String>,
) -> Result<Json<Vec<CameraView>>> {
    let shown = melyxar_app::cameras::of_the_library(&state, &who, library_id(&id)?).await?;
    Ok(Json(
        shown
            .into_iter()
            .map(|camera| CameraView {
                id: camera.id.to_string(),
                name: camera.name,
                address: camera.address,
                username: camera.username,
                has_password: camera.has_password,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
struct CameraBody {
    name: String,
    address: String,
    #[serde(default)]
    username: String,
    /// A new password. Absent keeps the one kept.
    #[serde(default)]
    password: Option<String>,
    /// Takes the kept password away.
    #[serde(default)]
    forget_password: bool,
}

impl CameraBody {
    fn declared(&self) -> Declared {
        Declared {
            name: self.name.clone(),
            address: self.address.clone(),
            username: self.username.clone(),
        }
    }
}

#[derive(Debug, Serialize)]
struct DeclaredView {
    id: String,
}

async fn declare(
    State(state): State<AppState>,
    _: Administrator,
    UrlPath(id): UrlPath<String>,
    Json(body): Json<CameraBody>,
) -> Result<Json<DeclaredView>> {
    let camera = melyxar_app::cameras::declare(
        &state,
        library_id(&id)?,
        &body.declared(),
        body.password.as_deref(),
    )
    .await?;
    Ok(Json(DeclaredView { id: camera.to_string() }))
}

async fn change(
    State(state): State<AppState>,
    _: Administrator,
    UrlPath(id): UrlPath<String>,
    Json(body): Json<CameraBody>,
) -> Result<Json<serde_json::Value>> {
    let password = match (&body.password, body.forget_password) {
        (_, true) => NewPassword::Forgotten,
        (Some(password), false) if !password.is_empty() => NewPassword::Replaced(password.clone()),
        _ => NewPassword::Kept,
    };
    melyxar_app::cameras::change(&state, camera_id(&id)?, &body.declared(), password).await?;
    Ok(Json(serde_json::json!({ "changed": true })))
}

async fn remove(
    State(state): State<AppState>,
    _: Administrator,
    UrlPath(id): UrlPath<String>,
) -> Result<Json<serde_json::Value>> {
    melyxar_app::cameras::remove(&state, camera_id(&id)?).await?;
    Ok(Json(serde_json::json!({ "removed": true })))
}

#[derive(Debug, Serialize)]
struct LiveView {
    address: String,
}

/// The address an application opens the camera at, account included. Kept
/// by nothing on the way: it holds a password.
async fn live(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    UrlPath(id): UrlPath<String>,
) -> Result<Response> {
    let address = melyxar_app::cameras::live_address(&state, &who, camera_id(&id)?).await?;
    let mut response = Json(LiveView { address }).into_response();
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

fn library_id(id: &str) -> Result<LibraryId> {
    id.parse()
        .map_err(|_| ServerError::invalid_input("the library identifier is malformed"))
}

fn camera_id(id: &str) -> Result<CameraId> {
    id.parse()
        .map_err(|_| ServerError::invalid_input("the camera identifier is malformed"))
}
