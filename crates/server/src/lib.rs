//! The HTTP surface.
//!
//! A translator and nothing more. Handlers read a request, call a use case and
//! render the answer; no rule about media, playback or scanning lives here.
//! That boundary is what lets a second entry point behave identically, and it
//! is what keeps a change of web framework confined to this one crate.

#![forbid(unsafe_code)]

pub mod catalogue;
pub mod error;
pub mod images;
pub mod jobs;
pub mod routes;

use std::net::SocketAddr;

use melyxar_app::AppState;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

pub use error::{ApiError, ServerError};
pub use routes::{router, API_VERSION};

/// Builds the application with the layers every response goes through.
pub fn build(state: AppState) -> axum::Router {
    routes::router(state)
        // Card listings are mostly text and compress very well, which is what
        // keeps a page of a hundred cards small on the wire.
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
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
    axum::serve(listener, build(state))
        .with_graceful_shutdown(shutdown)
        .await
}
