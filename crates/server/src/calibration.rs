//! Finding out what one device really decodes.
//!
//! The clips are the server's, and only somebody signed in is handed them. The
//! calibration itself belongs to a device rather than to an account, so a
//! laptop and a television never inherit each other's answer: the device names
//! itself with a value it made up and keeps to itself.

use axum::body::Body;
use axum::extract::{Path as RoutePath, State};
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use melyxar_app::calibration::{CodecResult, Readiness, CALIBRATION_VERSION};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::account::{ThisBrowser, Viewer};
use crate::error::Result;
use crate::identifiers::parse_client;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/calibration/clips",
            axum::routing::get(clips).post(prepare),
        )
        .route(
            "/api/v1/calibration/clips/{codec}/{height}",
            axum::routing::get(clip),
        )
        .route(
            "/api/v1/calibration/{client}",
            axum::routing::get(calibration)
                .post(record)
                .delete(forget),
        )
}

#[derive(Debug, Serialize)]
struct ClipView {
    codec: String,
    height: i32,
    frame_rate: f64,
    url: String,
}

#[derive(Debug, Serialize)]
struct ClipsView {
    /// Which recipe these clips are made by, handed back with the result.
    calibration_version: i32,
    /// not_prepared, preparing, failed or ready.
    state: &'static str,
    done: usize,
    total: usize,
    reason: Option<String>,
    clips: Vec<ClipView>,
}

/// Where the clips this account is measured on stand, and what they are once
/// they are ready.
async fn clips(State(state): State<AppState>, Viewer(who): Viewer) -> Result<Json<ClipsView>> {
    let readiness = melyxar_app::calibration::readiness(&state, &who).await?;
    let mut view = ClipsView {
        calibration_version: CALIBRATION_VERSION,
        state: "not_prepared",
        done: 0,
        total: 0,
        reason: None,
        clips: Vec::new(),
    };
    match readiness {
        Readiness::NotPrepared => {}
        Readiness::Preparing { done, total } => {
            view.state = "preparing";
            view.done = done;
            view.total = total;
        }
        Readiness::Failed(reason) => {
            view.state = "failed";
            view.reason = Some(reason);
        }
        Readiness::Ready(clips) => {
            view.state = "ready";
            view.clips = clips
                .into_iter()
                .map(|clip| ClipView {
                    url: format!("/api/v1/calibration/clips/{}/{}", clip.codec, clip.height),
                    codec: clip.codec,
                    height: clip.height,
                    frame_rate: clip.frame_rate,
                })
                .collect();
        }
    }
    Ok(Json(view))
}

/// Starts making whatever clips are missing.
async fn prepare(
    State(state): State<AppState>,
    Viewer(who): Viewer,
) -> Result<Json<serde_json::Value>> {
    melyxar_app::calibration::prepare(&state, &who).await?;
    Ok(Json(serde_json::json!({ "preparing": true })))
}

/// Hands one ready clip over whole.
async fn clip(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    RoutePath((codec, height)): RoutePath<(String, i32)>,
    request: Request<Body>,
) -> Response {
    let served = async {
        let path = melyxar_app::calibration::clip_file(&state, &who, &codec, height).await?;
        crate::serve_the_file(&path, request).await
    };
    match served.await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct RecordBody {
    calibration_version: i32,
    codecs: Vec<CodecResult>,
}

/// Keeps one device's whole calibration. A partial one is refused.
async fn record(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    browser: ThisBrowser,
    RoutePath(client): RoutePath<String>,
    Json(body): Json<RecordBody>,
) -> Result<Json<serde_json::Value>> {
    let client_id = parse_client(&client)?;
    melyxar_app::calibration::record(
        &state,
        &who,
        &asking(&browser),
        client_id,
        body.calibration_version,
        body.codecs,
    )
    .await?;
    Ok(Json(serde_json::json!({ "recorded": true })))
}

#[derive(Debug, Serialize)]
struct CalibrationView {
    calibration_version: i32,
    #[serde(with = "time::serde::rfc3339")]
    measured_at: melyxar_core::time::Timestamp,
    codecs: Vec<CodecResult>,
}

/// One device's calibration, or nothing when it has no whole one made the
/// way this server makes them now.
async fn calibration(
    State(state): State<AppState>,
    browser: ThisBrowser,
    RoutePath(client): RoutePath<String>,
) -> Result<Json<Option<CalibrationView>>> {
    let client_id = parse_client(&client)?;
    let kept = melyxar_app::calibration::calibration_of(&state, &asking(&browser), client_id).await?;
    Ok(Json(kept.map(|kept| CalibrationView {
        calibration_version: kept.calibration_version,
        measured_at: kept.measured_at,
        codecs: kept.codecs,
    })))
}

/// Forgets one device's calibration.
async fn forget(
    State(state): State<AppState>,
    browser: ThisBrowser,
    RoutePath(client): RoutePath<String>,
) -> Result<Json<serde_json::Value>> {
    let client_id = parse_client(&client)?;
    melyxar_app::calibration::forget(&state, &asking(&browser), client_id).await?;
    Ok(Json(serde_json::json!({ "forgotten": true })))
}

/// The browser behind a request, as the calibration asks about it.
fn asking(browser: &ThisBrowser) -> melyxar_app::calibration::Asking<'_> {
    melyxar_app::calibration::Asking {
        device: browser.device,
        client: browser.client.as_deref(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::PlaybackClientId;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    #[test]
    fn a_malformed_client_identifier_is_refused_rather_than_looked_up() {
        assert!(parse_client("../../etc/passwd").is_err());
        assert!(parse_client("not-an-identifier").is_err());
        assert!(parse_client(&PlaybackClientId::new().to_string()).is_ok());
    }
}
