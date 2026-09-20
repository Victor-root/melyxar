//! The HTTP surface.
//!
//! A translator and nothing more. Handlers read a request, call a use case and
//! render the answer; no rule about media, playback or scanning lives here.
//! That boundary is what lets a second entry point behave identically, and it
//! is what keeps a change of web framework confined to this one crate.

#![forbid(unsafe_code)]

pub mod calibration;
pub mod catalogue;
pub mod error;
pub mod general;
pub(crate) mod identifiers;
pub mod images;
pub mod interface;
pub mod jobs;
pub mod libraries;
pub mod page;
pub mod playback;
pub mod preferences;
pub mod routes;
pub mod timing;

use std::net::SocketAddr;

use melyxar_app::AppState;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

pub use error::{ApiError, ServerError};
pub use routes::{router, API_VERSION};

/// Who is watching.
///
/// There is no signing in yet, so this is the account the server created for
/// itself at first start. When accounts arrive it is read from the request,
/// and nothing else changes.
///
/// Asked for by more than the playback routes: a home page showing what
/// somebody left halfway has to know whose halfway it is.
pub(crate) async fn viewer(state: &AppState) -> error::Result<melyxar_core::id::UserId> {
    state
        .database()
        .user_by_name(melyxar_app::startup::DEFAULT_ACCOUNT_NAME)
        .await
        .map_err(|error| ServerError::internal(error.to_string()))?
        .map(|(user, _)| user.id)
        .ok_or_else(|| ServerError::internal("this server has no account at all"))
}

/// Whoever is asking, when what they are asking for is an administrator's.
///
/// The check is written here now, at the one place it belongs, even though
/// there is nobody else to be yet: the account the server made for itself is
/// an administrator, so today it always passes. The day signing in arrives,
/// `viewer` above starts answering something else and this begins to bite
/// without anything around it moving.
pub(crate) async fn administrator(state: &AppState) -> error::Result<melyxar_core::id::UserId> {
    let (user, _) = state
        .database()
        .user_by_name(melyxar_app::startup::DEFAULT_ACCOUNT_NAME)
        .await
        .map_err(|error| ServerError::internal(error.to_string()))?
        .ok_or_else(|| ServerError::internal("this server has no account at all"))?;

    if !user.permissions.is_administrator {
        return Err(ServerError::new(
            axum::http::StatusCode::FORBIDDEN,
            melyxar_core::error::ErrorCode::Forbidden,
            "this is an administrator's to do",
        ));
    }
    Ok(user.id)
}

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
    routes::router(state)
        // Card listings are mostly text and compress very well, which is what
        // keeps a page of a hundred cards small on the wire.
        .layer(CompressionLayer::new())
        // Outside the compression, so the time reported is the time the client
        // waited and not the time before the answer was packed.
        .layer(axum::middleware::from_fn(timing::measured))
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
