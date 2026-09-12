//! Serving the interface itself.
//!
//! The built interface travels inside the binary. That is what lets the
//! container hold nothing but one file: no folder of assets to install
//! alongside it, nothing to keep in step with it, and no way for the two to
//! drift apart.
//!
//! Everything the build names with a fingerprint is handed over with
//! permission to keep it for ever; the one file without a fingerprint, the
//! page itself, is handed over with none, since it is what points at the rest.

use axum::body::Body;
use axum::http::{header, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use melyxar_app::AppState;
use rust_embed::Embed;

/// The built interface.
///
/// In a development build these are read from the folder each time, so an
/// interface being worked on refreshes without rebuilding the server; in a
/// release build they are inside the binary.
#[derive(Embed)]
#[folder = "$CARGO_MANIFEST_DIR/../../web/dist"]
struct Interface;

/// How long a file named with a fingerprint may be kept.
const KEEP_FOR: &str = "public, max-age=31536000, immutable";

/// The page itself is asked for again every time: it is the one file that
/// says where all the others are.
const ALWAYS_ASK: &str = "no-cache";

pub fn router() -> Router<AppState> {
    Router::new().fallback(serve)
}

async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Anything under the interface that is not a file is one of its own
    // addresses, and those are answered with the page: the interface reads the
    // address itself and shows the right screen.
    match Interface::get(path) {
        Some(file) => file_response(path, file.data.into_owned()),
        None => match Interface::get("index.html") {
            Some(page) => page_response(page.data.into_owned()),
            None => (
                StatusCode::NOT_FOUND,
                "the interface was not built into this server",
            )
                .into_response(),
        },
    }
}

fn file_response(path: &str, bytes: Vec<u8>) -> Response {
    if path == "index.html" {
        return page_response(bytes);
    }

    let content_type = content_type_of(path);
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static(content_type)),
            (header::CACHE_CONTROL, HeaderValue::from_static(KEEP_FOR)),
        ],
        Body::from(bytes),
    )
        .into_response()
}

fn page_response(bytes: Vec<u8>) -> Response {
    (
        StatusCode::OK,
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/html; charset=utf-8"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static(ALWAYS_ASK)),
        ],
        Body::from(bytes),
    )
        .into_response()
}

/// What a file is, from the end of its name.
fn content_type_of(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "map" => "application/json",
        // A kind nobody planned for is handed over as plain bytes rather than
        // as something a browser would try to run.
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_interface_was_built_into_this_server() {
        assert!(
            Interface::get("index.html").is_some(),
            "the interface has to be built before the server is: run the build in web/"
        );
    }

    #[test]
    fn every_kind_the_build_produces_is_named_correctly() {
        assert_eq!(content_type_of("assets/index-abc123.js"), "text/javascript; charset=utf-8");
        assert_eq!(content_type_of("assets/index-abc123.css"), "text/css; charset=utf-8");
        assert_eq!(content_type_of("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type_of("logo.svg"), "image/svg+xml");
    }

    #[test]
    fn something_nobody_planned_for_is_not_handed_over_as_runnable() {
        assert_eq!(content_type_of("notes.txt"), "application/octet-stream");
        assert_eq!(content_type_of("no-extension"), "application/octet-stream");
    }

    #[test]
    fn the_page_is_asked_for_again_and_the_rest_is_kept() {
        assert_eq!(ALWAYS_ASK, "no-cache");
        assert!(
            KEEP_FOR.contains("immutable"),
            "a file named with a fingerprint never changes under that name"
        );
    }
}
