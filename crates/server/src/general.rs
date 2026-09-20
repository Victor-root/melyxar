//! Settings of the server itself, rather than of a library or of a viewer.
//!
//! The first of these is whether wide gamut colour is ever converted for a
//! client that cannot show it: a real switch for a real problem, a processor
//! too slow to rebuild a picture whose only fault is its colour. More belongs
//! here as branding and maintenance reach the interface, which is why this is
//! its own small module rather than a corner of another one.

use axum::extract::State;
use axum::{Json, Router};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/settings/playback",
        axum::routing::get(playback_settings).put(set_playback_settings),
    )
}

#[derive(Debug, Serialize, Deserialize)]
struct PlaybackSettingsView {
    /// Never convert wide gamut colour, even where a client cannot show it
    /// correctly. Off by default. Dolby Vision without a compatible base
    /// layer is converted regardless, since left alone it looks broken rather
    /// than merely washed out.
    tone_mapping_disabled: bool,
}

/// What the server is set to do about wide gamut colour it cannot show a
/// client, and every film like it.
async fn playback_settings(
    State(state): State<AppState>,
    _: crate::account::Administrator,
) -> Result<Json<PlaybackSettingsView>> {
    Ok(Json(PlaybackSettingsView {
        tone_mapping_disabled: state
            .database()
            .tone_mapping_disabled()
            .await
            .map_err(|error| crate::error::ServerError::internal(error.to_string()))?,
    }))
}

/// Turns the conversion of wide gamut colour on or off for the whole server.
async fn set_playback_settings(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    Json(asked): Json<PlaybackSettingsView>,
) -> Result<Json<PlaybackSettingsView>> {
    state
        .database()
        .set_tone_mapping_disabled(asked.tone_mapping_disabled)
        .await
        .map_err(|error| crate::error::ServerError::internal(error.to_string()))?;

    tracing::debug!(
        tone_mapping_disabled = asked.tone_mapping_disabled,
        "the server's playback settings were changed"
    );
    Ok(Json(asked))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }
}
