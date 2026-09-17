//! Finding out what one client really decodes.
//!
//! Nothing here belongs to an account: a calibration is a fact about a
//! machine, kept apart so a laptop and a television never inherit each
//! other's answer. The client names itself with a value it made up and keeps
//! to itself; nothing here checks it against anything.

use axum::extract::{Path as RoutePath, State};
use axum::{Json, Router};
use melyxar_app::calibration::CodecCalibration;
use melyxar_app::AppState;
use melyxar_core::id::PlaybackClientId;
use melyxar_core::time::now;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/calibration/session",
            axum::routing::post(open_session),
        )
        .route(
            "/api/v1/calibration/verdict",
            axum::routing::post(record_verdict),
        )
        .route(
            "/api/v1/calibration/{client}",
            axum::routing::get(profile).delete(forget),
        )
}

#[derive(Debug, Deserialize)]
struct OpenBody {
    codec: String,
    height: i32,
}

#[derive(Debug, Serialize)]
struct SessionView {
    id: String,
    playlist_url: String,
}

/// Opens a session that rebuilds the reference film into one codec, at one
/// height, for a client to watch and measure.
///
/// Nameless on purpose: the same reference film, codec and height is the same
/// question whoever asks it, and nothing about a client is known until the
/// verdict comes back.
async fn open_session(
    State(state): State<AppState>,
    Json(body): Json<OpenBody>,
) -> Result<Json<SessionView>> {
    let session =
        melyxar_app::calibration::open_calibration_session(&state, &body.codec, body.height)
            .await?;
    Ok(Json(SessionView {
        id: session.id.to_string(),
        playlist_url: format!("/api/v1/stream/{}/playlist.m3u8", session.id),
    }))
}

#[derive(Debug, Deserialize)]
struct VerdictBody {
    client_id: String,
    codec: String,
    calibration_version: i32,
    usable: bool,
    tested_height: i32,
    dropped_share: f64,
    shown_share: f64,
}

/// Records what one client measured for one codec.
async fn record_verdict(
    State(state): State<AppState>,
    Json(body): Json<VerdictBody>,
) -> Result<Json<serde_json::Value>> {
    let client_id = parse_client(&body.client_id)?;
    melyxar_app::calibration::record_calibration(
        &state,
        client_id,
        &CodecCalibration {
            codec: body.codec,
            calibration_version: body.calibration_version,
            usable: body.usable,
            tested_height: body.tested_height,
            dropped_share: body.dropped_share,
            shown_share: body.shown_share,
            measured_at: now(),
        },
    )
    .await?;
    Ok(Json(serde_json::json!({ "recorded": true })))
}

#[derive(Debug, Serialize)]
struct CalibrationView {
    codec: String,
    calibration_version: i32,
    usable: bool,
    tested_height: i32,
    dropped_share: f64,
    shown_share: f64,
}

/// Everything measured for one client so far.
async fn profile(
    State(state): State<AppState>,
    RoutePath(client): RoutePath<String>,
) -> Result<Json<Vec<CalibrationView>>> {
    let client_id = parse_client(&client)?;
    let calibrations = melyxar_app::calibration::calibration_profile(&state, client_id).await?;
    Ok(Json(
        calibrations
            .into_iter()
            .map(|calibration| CalibrationView {
                codec: calibration.codec,
                calibration_version: calibration.calibration_version,
                usable: calibration.usable,
                tested_height: calibration.tested_height,
                dropped_share: calibration.dropped_share,
                shown_share: calibration.shown_share,
            })
            .collect(),
    ))
}

/// Forgets everything measured for one client, all codecs at once.
async fn forget(
    State(state): State<AppState>,
    RoutePath(client): RoutePath<String>,
) -> Result<Json<serde_json::Value>> {
    let client_id = parse_client(&client)?;
    melyxar_app::calibration::forget_calibration(&state, client_id).await?;
    Ok(Json(serde_json::json!({ "forgotten": true })))
}

fn parse_client(value: &str) -> Result<PlaybackClientId> {
    value
        .parse()
        .map_err(|_| ServerError::invalid_input("the client identifier is malformed"))
}

#[cfg(test)]
mod tests {
    use super::*;

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
