//! A device waiting at the door, let in by an account signed in elsewhere:
//! the device's own questions, open to anybody, and the page that lets it in,
//! for whoever is signed in.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use melyxar_app::accounts::Remembered;
use melyxar_app::linking::Standing;
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::account::{answered_with_a_session, what_asked, Viewer};
use crate::address::Caller;
use crate::error::{Result, ServerError};
use crate::identifiers::parse_account;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/public/link", axum::routing::post(ask))
        .route("/api/v1/public/link/standing", axum::routing::post(standing))
        .route("/api/v1/link/{code}", axum::routing::get(named).post(let_in))
}

#[derive(Debug, Deserialize)]
struct Asking {
    /// The server as the device reaches it, which the QR code leads to.
    address: String,
    #[serde(default)]
    client: Option<String>,
    /// What the device calls itself, which the page letting it in shows:
    /// a television's model, a browser and its system. Its own words when
    /// it gives none.
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Serialize)]
struct AskedView {
    code: String,
    secret: String,
    lasts_seconds: u64,
    /// The QR code a row at a time, "1" for a dark square: a few hundred
    /// characters rather than a list of booleans ten times its size.
    squares: Vec<String>,
}

async fn ask(State(state): State<AppState>, headers: HeaderMap, Json(asking): Json<Asking>) -> Result<Json<AskedView>> {
    let name = asking
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(|name| name.chars().take(melyxar_app::accounts::LONGEST_BROWSER_NAME).collect())
        .unwrap_or_else(|| what_asked(&headers));
    let asked = melyxar_app::linking::ask(&state, &name, asking.client.as_deref(), &asking.address)?;
    Ok(Json(AskedView {
        squares: asked
            .squares
            .iter()
            .map(|row| row.iter().map(|dark| if *dark { '1' } else { '0' }).collect())
            .collect(),
        code: asked.code,
        secret: asked.secret.as_text().to_string(),
        lasts_seconds: asked.lasts_seconds,
    }))
}

#[derive(Debug, Deserialize)]
struct Holding {
    /// Sent in the body rather than the address, so no log of addresses
    /// ever holds it.
    secret: String,
}

#[derive(Debug, Serialize)]
struct WaitingView {
    seconds_left: u64,
}

/// Whether the device was let in: still waiting, its session, or nothing
/// left to wait on.
async fn standing(
    State(state): State<AppState>,
    Caller { encrypted, .. }: Caller,
    Json(holding): Json<Holding>,
) -> Result<Response> {
    match melyxar_app::linking::standing(&state, &holding.secret).await? {
        Standing::Waiting { seconds_left } => Ok(Json(WaitingView { seconds_left }).into_response()),
        Standing::Opened(opened) => Ok(answered_with_a_session(*opened, Remembered::Yes, encrypted)),
        Standing::Gone => Err(ServerError::not_found("no device waits with that secret")),
    }
}

#[derive(Debug, Serialize)]
struct NamedView {
    device: String,
}

async fn named(Viewer(who): Viewer, State(state): State<AppState>, Path(code): Path<String>) -> Result<Json<NamedView>> {
    Ok(Json(NamedView {
        device: melyxar_app::linking::device_named(&state, &who, &code)?,
    }))
}

#[derive(Debug, Default, Deserialize)]
struct LettingIn {
    /// Another account than one's own, for an administrator.
    #[serde(default)]
    account: Option<String>,
}

async fn let_in(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(code): Path<String>,
    Json(letting): Json<LettingIn>,
) -> Result<Json<serde_json::Value>> {
    let on = letting.account.as_deref().map(parse_account).transpose()?;
    melyxar_app::linking::let_in(&state, &who, &code, on).await?;
    Ok(Json(serde_json::json!({ "let_in": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }
}
