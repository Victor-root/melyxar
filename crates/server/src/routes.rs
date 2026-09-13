//! The routes.
//!
//! Handlers translate and nothing more: they read a request, call a use case
//! and render the answer. No rule about media, playback or scanning lives
//! here, which is what lets a second entry point reuse the same behaviour.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use melyxar_app::AppState;
use serde::Serialize;

use crate::error::Result;

/// Version of the interface this server speaks.
///
/// Part of every path, so a later version can be served alongside rather than
/// replacing this one and breaking clients that have not caught up.
pub const API_VERSION: &str = "v1";

/// Builds the whole surface.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/system/info", get(system_info))
        .route("/api/v1/system/health", get(health))
        .route("/api/v1/system/diagnostics", get(diagnostics))
        .route("/api/v1/system/diagnostics/text", get(diagnostics_text))
        .route("/api/v1/public/branding", get(public_branding))
        .merge(crate::catalogue::router())
        .merge(crate::images::router())
        .merge(crate::jobs::router())
        .merge(crate::playback::router())
        .merge(crate::preferences::router())
        // Last: anything that is not an address of the interface proper is
        // answered with the interface, which reads the address itself.
        .merge(crate::interface::router())
        .with_state(state)
}

/// What this server is and what it can do.
///
/// Deliberately says what is *available* rather than which version is running,
/// so a client adapts to a capability instead of comparing version numbers.
#[derive(Debug, Serialize)]
struct SystemInfo {
    server_name: String,
    version: &'static str,
    api_version: &'static str,
    /// Whether the setup wizard still has to run.
    setup_complete: bool,
    /// Whether playback can work at all. A library still browses without it.
    playback_available: bool,
    maintenance: bool,
    features: Features,
}

#[derive(Debug, Serialize)]
struct Features {
    /// Hardware paths the media tool carries.
    hardware_acceleration: Vec<String>,
    /// Whether wide gamut colour can be converted. Without it, such films
    /// cannot be shown with correct colours.
    wide_gamut_conversion: bool,
}

async fn system_info(State(state): State<AppState>) -> Result<Json<SystemInfo>> {
    let settings = state
        .database()
        .server_settings()
        .await
        .map_err(|error| crate::error::ServerError::internal(error.to_string()))?;

    Ok(Json(SystemInfo {
        server_name: settings.server_name,
        version: melyxar_core::BUILD,
        api_version: API_VERSION,
        // A server whose only account still has no password has not been set
        // up, which is what the wizard keys on.
        setup_complete: state
            .database()
            .user_by_name(melyxar_app::startup::DEFAULT_ACCOUNT_NAME)
            .await
            .map_err(|error| crate::error::ServerError::internal(error.to_string()))?
            .is_none_or(|(_, password)| password.is_some()),
        playback_available: state.can_play_media(),
        maintenance: settings.maintenance_enabled,
        features: Features {
            hardware_acceleration: state.hardware_acceleration_names(),
            wide_gamut_conversion: state.can_convert_wide_gamut(),
        },
    }))
}

/// Minimal liveness answer, for a reverse proxy or a watchdog.
async fn health() -> &'static str {
    "ok"
}

async fn diagnostics(
    State(state): State<AppState>,
) -> Result<Json<melyxar_app::diagnostics::Diagnostics>> {
    Ok(Json(melyxar_app::diagnostics::collect(&state).await?))
}

/// The same report as plain text, exactly as the command line prints it.
///
/// So that somebody who cannot read a log copies one block from a screen and
/// it holds everything worth asking about. Rendered by the same code as the
/// command line, because two renderings of one report drift apart and the
/// second one is always the one nobody checked.
async fn diagnostics_text(
    State(state): State<AppState>,
) -> Result<([(&'static str, &'static str); 1], String)> {
    let report = melyxar_app::diagnostics::collect(&state).await?;
    Ok((
        [("content-type", "text/plain; charset=utf-8")],
        melyxar_app::diagnostics::render_text(&report),
    ))
}

/// The visual identity, before anyone has signed in.
///
/// The sign-in page needs the name and the logo before an account exists, so
/// this one route answers without authentication. It therefore says nothing
/// else: no account list, no library names, no version.
#[derive(Debug, Serialize)]
struct PublicBranding {
    server_name: String,
    logo_path: Option<String>,
    login_background_path: Option<String>,
}

async fn public_branding(State(state): State<AppState>) -> Result<Json<PublicBranding>> {
    let settings = state
        .database()
        .server_settings()
        .await
        .map_err(|error| crate::error::ServerError::internal(error.to_string()))?;

    Ok(Json(PublicBranding {
        server_name: settings.server_name,
        logo_path: settings.logo_path,
        login_background_path: settings.login_background_path,
    }))
}
