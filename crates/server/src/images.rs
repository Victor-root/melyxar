//! Serving the pictures the server prepared.
//!
//! Two things matter here. A picture is served under a name earned by its
//! content, so it is handed over with permission to keep it for ever: a poster
//! fetched once is never fetched again, and a poster replaced arrives under
//! another name at once, without anyone clearing anything.
//!
//! And the path asked for is checked rather than trusted. A path is the one
//! place a request reaches into the file system, and a request that walks out
//! of the picture folder is a request that reads whatever it likes.

use axum::body::Body;
use axum::extract::{Path as RoutePath, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use melyxar_app::AppState;
use std::path::{Component, Path, PathBuf};

use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/images/{*path}", axum::routing::get(picture))
}

/// How long a client may keep a picture.
///
/// A year, and marked as never changing. It is safe because the name carries a
/// fingerprint of the content: a different picture is a different name, so
/// nothing stale can be held on to.
const KEEP_FOR: &str = "public, max-age=31536000, immutable";

async fn picture(State(state): State<AppState>, RoutePath(path): RoutePath<String>) -> Response {
    match read(&state, &path).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn read(state: &AppState, requested: &str) -> Result<Response> {
    let relative = safe_relative_path(requested)
        .ok_or_else(|| ServerError::invalid_input("that is not a picture path"))?;

    let full_path = state.config().directories.images().join(&relative);
    let bytes = tokio::fs::read(&full_path)
        .await
        .map_err(|_| ServerError::not_found("no picture there"))?;

    let content_type = content_type_of(&relative)
        .ok_or_else(|| ServerError::invalid_input("that is not a kind of picture served here"))?;

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::CACHE_CONTROL, HeaderValue::from_static(KEEP_FOR)),
        ],
        Body::from(bytes),
    )
        .into_response())
}

/// Turns what was asked for into a path inside the picture folder, or nothing.
///
/// Every part has to be a plain name. That rules out walking upwards, starting
/// from the root of the disk, and the Windows oddities, rather than trying to
/// recognise the shapes an attacker might use: a list of forbidden shapes is
/// always one shape short.
fn safe_relative_path(requested: &str) -> Option<PathBuf> {
    if requested.is_empty() || requested.contains('\0') {
        return None;
    }

    let mut safe = PathBuf::new();
    for component in Path::new(requested).components() {
        match component {
            Component::Normal(part) => {
                let name = part.to_str()?;
                // A name that is nothing but dots is the way up under another
                // guise on some systems.
                if name.chars().all(|c| c == '.') {
                    return None;
                }
                safe.push(name);
            }
            _ => return None,
        }
    }
    (!safe.as_os_str().is_empty()).then_some(safe)
}

/// What a picture is, from the end of its name.
///
/// Only the kinds this server writes are served. Anything else in the folder,
/// however it got there, is not handed out.
fn content_type_of(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "webp" => Some("image/webp"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_picture_path_is_allowed_through() {
        assert_eq!(
            safe_relative_path("works/01a0/poster-abc123-400.webp"),
            Some(PathBuf::from("works/01a0/poster-abc123-400.webp"))
        );
    }

    #[test]
    fn a_path_that_walks_upwards_is_refused() {
        for attempt in [
            "../melyxar.toml",
            "works/../../../etc/passwd",
            "..",
            "works/..",
            "./../secrets",
        ] {
            assert_eq!(
                safe_relative_path(attempt),
                None,
                "a request must not be able to read outside the picture folder: {attempt}"
            );
        }
    }

    #[test]
    fn a_path_starting_from_the_root_of_the_disk_is_refused() {
        assert_eq!(safe_relative_path("/etc/passwd"), None);
        assert_eq!(safe_relative_path("//etc/passwd"), None);
    }

    #[test]
    fn a_path_holding_something_that_is_not_a_name_is_refused() {
        assert_eq!(safe_relative_path(""), None);
        assert_eq!(safe_relative_path("works/\0/poster.webp"), None);
        assert_eq!(
            safe_relative_path("works/.../poster.webp"),
            None,
            "a run of dots is the way up wearing a hat"
        );
    }

    #[test]
    fn only_the_kinds_of_picture_this_server_writes_are_served() {
        assert_eq!(
            content_type_of(Path::new("poster-abc-400.webp")),
            Some("image/webp")
        );
        assert_eq!(content_type_of(Path::new("cover.JPG")), Some("image/jpeg"));
        assert_eq!(
            content_type_of(Path::new("melyxar.db")),
            None,
            "whatever else is in that folder is not handed out"
        );
        assert_eq!(content_type_of(Path::new("no-extension")), None);
    }

    #[test]
    fn a_picture_is_handed_over_with_permission_to_keep_it() {
        assert!(KEEP_FOR.contains("immutable"));
        assert!(
            KEEP_FOR.contains("max-age=31536000"),
            "the name carries the content, so holding on to it for ever is safe"
        );
    }
}
