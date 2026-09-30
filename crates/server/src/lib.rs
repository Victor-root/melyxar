//! The HTTP surface.
//!
//! A translator and nothing more. Handlers read a request, call a use case and
//! render the answer; no rule about media, playback or scanning lives here.
//! That boundary is what lets a second entry point behave identically, and it
//! is what keeps a change of web framework confined to this one crate.

#![forbid(unsafe_code)]

pub mod account;
mod address;
mod door;
pub mod accounts;
pub mod activity;
pub mod calibration;
pub mod catalogue;
pub mod collections;
pub mod deletion;
pub mod details;
pub mod devices;
pub mod error;
pub mod general;
pub(crate) mod identifiers;
pub mod images;
pub mod installing;
pub mod interface;
pub mod jobs;
pub mod libraries;
pub mod live;
pub mod music;
pub mod music_playlists;
pub mod music_preferences;
pub mod music_tags;
pub mod online_subtitles;
pub mod page;
pub mod pictures;
pub mod playback;
pub mod playlists;
pub mod preferences;
pub mod routes;
pub mod security;
pub mod timing;
pub mod uploads;

use std::net::SocketAddr;

use melyxar_app::AppState;
use axum::http::header;
use tower_http::compression::CompressionLayer;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

pub use error::{ApiError, ServerError};
pub use routes::{router, API_VERSION};

/// Hands a file on the disk straight to the client.
///
/// Ranges, conditional requests and the sending itself are all the file
/// server's; what the routes add on top is only their own headers, so this is
/// the one place that knows how to reach it and what a failure to reach it
/// reads as.
pub(crate) async fn serve_the_file(
    path: &std::path::Path,
    request: axum::http::Request<axum::body::Body>,
) -> error::Result<axum::response::Response> {
    use axum::response::IntoResponse;
    use tower::ServiceExt;

    tower_http::services::ServeFile::new(path)
        .oneshot(request)
        .await
        .map(IntoResponse::into_response)
        .map_err(|error| ServerError::internal(error.to_string()))
}

/// Builds the application with the layers every response goes through.
pub fn build(state: AppState) -> axum::Router {
    routes::router(state.clone())
        // Card listings are mostly text and compress very well, which is what
        // keeps a page of a hundred cards small on the wire.
        .layer(CompressionLayer::new())
        // Outside the compression and inside the timer: nothing is worth
        // packing for somebody who is not going to be answered, and a refusal
        // is a request this server spent time on like any other.
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            account::at_the_gate,
        ))
        // Outside the gate, so somebody not signed in yet is sent to the
        // encrypted address before anything else.
        .layer(axum::middleware::from_fn_with_state(state, door::sent_to_encrypted))
        // Outside the compression, so the time reported is the time the client
        // waited and not the time before the answer was packed.
        .layer(axum::middleware::from_fn(timing::measured))
        .layer(guarded(header::X_CONTENT_TYPE_OPTIONS, "nosniff"))
        // No other site may frame these pages and trick a click on them.
        .layer(guarded(header::X_FRAME_OPTIONS, "DENY"))
        .layer(guarded(header::REFERRER_POLICY, "same-origin"))
        .layer(TraceLayer::new_for_http())
}

/// A header every response carries, unless a route already set its own.
fn guarded(name: header::HeaderName, value: &'static str) -> SetResponseHeaderLayer<header::HeaderValue> {
    SetResponseHeaderLayer::if_not_present(name, header::HeaderValue::from_static(value))
}

/// Serves until the shutdown signal fires.
///
/// The signal is passed in rather than installed here, so the binary decides
/// what counts as a shutdown and can close playback sessions before the
/// listener stops accepting.
pub async fn serve(
    address: SocketAddr,
    state: AppState,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind(address).await?;
    tracing::info!(%address, "listening");
    let closing = state.clone();
    let door = door::Door::new(listener, state.clone())?;
    axum::serve(door, build(state).into_make_service_with_connect_info::<door::Peer>())
        .with_graceful_shutdown(async move {
            shutdown.await;
            // A live line never ends on its own, and a stopping server waits
            // for every answer it started.
            melyxar_app::watching::close_every_line(&closing);
        })
        .await
}
