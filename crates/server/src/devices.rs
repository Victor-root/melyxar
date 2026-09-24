//! The devices signed in to this server.
//!
//! Every one of them for an administrator, and one's own for everybody else:
//! a phone lost is signed out by whoever lost it, without asking anybody.
//! Which device may be signed out by whom is the use cases' to decide.

use axum::extract::{Path, State};
use axum::routing::{delete, get};
use axum::{Json, Router};
use melyxar_app::accounts::SignedInDevice;
use melyxar_app::AppState;
use melyxar_core::id::DeviceId;
use serde::Serialize;

use crate::account::{Administrator, ThisBrowser, Viewer};
use crate::error::Result;
use crate::identifiers::parse_device;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/devices", get(every_device))
        .route("/api/v1/devices/{id}", delete(sign_out_a_device))
        .route("/api/v1/me/devices", get(my_devices))
        .route("/api/v1/me/devices/{id}", delete(sign_out_my_device))
}

/// One device, as a list of them shows it.
#[derive(Debug, Serialize)]
struct DeviceView {
    id: String,
    user_id: String,
    user_name: String,
    /// Where the picture of the account is served, when it has one.
    user_avatar: Option<String>,
    /// What the browser said it was when it signed in, which the page turns
    /// into "Firefox on Windows".
    name: String,
    /// The browser its own page found, when it could tell better.
    browser: Option<String>,
    /// False for a session that ends when the browser is closed.
    remembered: bool,
    signed_in_at: String,
    /// Up to an hour behind the real last use.
    last_seen_at: String,
    /// Whether this is the device the list is being looked at from.
    is_this_one: bool,
}

fn device_view(device: &SignedInDevice, this_one: DeviceId) -> DeviceView {
    DeviceView {
        id: device.id.to_string(),
        user_id: device.user_id.to_string(),
        user_name: device.user_name.clone(),
        user_avatar: device.user_avatar.as_deref().map(crate::images::face_url),
        name: device.name.clone(),
        browser: device.browser.clone(),
        remembered: device.remembered == melyxar_app::accounts::Remembered::Yes,
        signed_in_at: melyxar_core::time::to_text(device.signed_in_at),
        last_seen_at: melyxar_core::time::to_text(device.last_seen_at),
        is_this_one: device.id == this_one,
    }
}

async fn every_device(
    _: Administrator,
    this: ThisBrowser,
    State(state): State<AppState>,
) -> Result<Json<Vec<DeviceView>>> {
    let devices = melyxar_app::accounts::every_device(&state).await?;
    Ok(Json(
        devices
            .iter()
            .map(|device| device_view(device, this.device))
            .collect(),
    ))
}

async fn sign_out_a_device(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    melyxar_app::accounts::sign_out_a_device(&state, &who, parse_device(&id)?).await?;
    Ok(Json(serde_json::json!({ "signed_out": true })))
}

async fn my_devices(
    Viewer(who): Viewer,
    this: ThisBrowser,
    State(state): State<AppState>,
) -> Result<Json<Vec<DeviceView>>> {
    let devices = melyxar_app::accounts::devices_of(&state, &who).await?;
    Ok(Json(
        devices
            .iter()
            .map(|device| device_view(device, this.device))
            .collect(),
    ))
}

async fn sign_out_my_device(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    melyxar_app::accounts::sign_out_my_device(&state, &who, parse_device(&id)?).await?;
    Ok(Json(serde_json::json!({ "signed_out": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }
}
