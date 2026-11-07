//! Steering one device of the account from another: the devices that can be
//! reached and what plays there, and the order handed to one of them (see
//! `melyxar_app::remote`).

use axum::extract::{Path as RoutePath, State};
use axum::{Json, Router};
use melyxar_app::remote::{Order, Steerable, Told};
use melyxar_app::AppState;
use serde::Serialize;

use crate::account::{Viewer, Watcher};
use crate::error::Result;
use crate::identifiers::parse_device;
use crate::playback::{tracks_view, TrackView, TracksView};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/remote/devices", axum::routing::get(devices))
        .route("/api/v1/remote/devices/{device}", axum::routing::post(order))
        .route("/api/v1/remote/told", axum::routing::put(told))
        .route("/api/v1/remote/here", axum::routing::post(still_here))
}

/// One device a remote can steer.
#[derive(Debug, Serialize)]
pub(crate) struct SteerableView {
    device: String,
    /// What the browser said it was when it signed in, which a page makes
    /// readable.
    name: String,
    browser: Option<String>,
    playing: Option<PlayingView>,
    /// How loud it plays and the queue of its music, as it said.
    #[serde(flatten)]
    told: Told,
}

/// What plays on it, as the remote shows it.
#[derive(Debug, Serialize)]
struct PlayingView {
    work_id: String,
    title: String,
    kind: &'static str,
    series: Option<String>,
    season: Option<i32>,
    episode: Option<i32>,
    artist: Option<String>,
    album: Option<String>,
    picture: Option<String>,
    position_seconds: f64,
    duration_seconds: Option<f64>,
    paused: bool,
    /// A film's tracks, and those it plays: empty for a song, and for a film
    /// whose plan was not heard.
    audio: Vec<TrackView>,
    subtitles: Vec<TrackView>,
    chosen_audio_id: Option<String>,
    chosen_subtitle_id: Option<String>,
}

pub(crate) fn steerable_views(steerable: Vec<Steerable>) -> Vec<SteerableView> {
    steerable
        .into_iter()
        .map(|one| SteerableView {
            device: one.device.to_string(),
            name: one.name,
            browser: one.browser,
            told: one.told,
            playing: one.playing.map(|playing| {
                let TracksView {
                    audio,
                    subtitles,
                    chosen_audio_id,
                    chosen_subtitle_id,
                } = playing.plan.as_deref().map(tracks_view).unwrap_or(TracksView {
                    audio: Vec::new(),
                    subtitles: Vec::new(),
                    chosen_audio_id: None,
                    chosen_subtitle_id: None,
                });
                PlayingView {
                    work_id: playing.work.to_string(),
                    kind: playing.kind.as_str(),
                    series: playing.series,
                    season: playing.season,
                    episode: playing.episode,
                    artist: playing.artist,
                    album: playing.album,
                    picture: playing.picture.map(|path| format!("/api/v1/images/{path}")),
                    position_seconds: playing.position.as_seconds_f64(),
                    duration_seconds: playing
                        .plan
                        .as_ref()
                        .and_then(|plan| plan.duration)
                        .or(playing.length)
                        .map(|duration| duration.as_seconds_f64()),
                    paused: playing.paused,
                    title: playing.title,
                    audio,
                    subtitles,
                    chosen_audio_id,
                    chosen_subtitle_id,
                }
            }),
        })
        .collect()
}

/// The other devices of this account that can be steered from here.
async fn devices(
    State(state): State<AppState>,
    Watcher(watcher): Watcher,
) -> Result<Json<Vec<SteerableView>>> {
    let steerable = melyxar_app::remote::steerable(&state, watcher.user, watcher.device).await?;
    Ok(Json(steerable_views(steerable)))
}

/// What this device says of itself, for the remotes steering it.
async fn told(
    State(state): State<AppState>,
    Watcher(watcher): Watcher,
    Json(told): Json<Told>,
) -> Result<Json<serde_json::Value>> {
    melyxar_app::remote::tell(&state, watcher.device, told)?;
    Ok(Json(serde_json::json!({ "kept": true })))
}

/// Says this device is there, as its open line alone does not.
async fn still_here(
    State(state): State<AppState>,
    Watcher(watcher): Watcher,
) -> Json<serde_json::Value> {
    Json(serde_json::json!({ "kept": melyxar_app::remote::still_here(&state, watcher.device) }))
}

/// Hands an order to one of this account's devices.
async fn order(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    RoutePath(device): RoutePath<String>,
    Json(order): Json<Order>,
) -> Result<Json<serde_json::Value>> {
    melyxar_app::remote::send(&state, who.id, parse_device(&device)?, &order)?;
    Ok(Json(serde_json::json!({ "sent": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }
}
