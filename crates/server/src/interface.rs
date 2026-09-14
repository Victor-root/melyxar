//! Serving the interface itself.
//!
//! The built interface travels inside the binary. That is what lets the
//! container hold nothing but one file: no folder of assets to install
//! alongside it, nothing to keep in step with it, and no way for the two to
//! drift apart.
//!
//! Only what the build names with a fingerprint is handed over with permission
//! to keep it for ever, because only such a name can promise never to stand
//! for different bytes. Everything else, the page and the files copied over
//! untouched, is asked for again every time and answered "unchanged" when it
//! is, which costs one small round trip and never a stale logo.

use axum::body::Body;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use melyxar_app::AppState;
use rust_embed::{Embed, EmbeddedFile};

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

/// Everything else is asked for again every time. Cheap, because the answer is
/// almost always "unchanged" and carries no file with it.
const ALWAYS_ASK: &str = "no-cache";

/// Where the build puts what it names with a fingerprint.
///
/// The rest of what is served is copied over from `web/public` under the name
/// it was given, so `melyxar-64.png` today and `melyxar-64.png` tomorrow can
/// hold different pictures. Promising a browser it may keep such a file for a
/// year means a logo that changes is never seen to change.
const FINGERPRINTED: &str = "assets/";

pub fn router() -> Router<AppState> {
    Router::new().fallback(serve)
}

async fn serve(asked: HeaderMap, uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Anything under the interface that is not a file is one of its own
    // addresses, and those are answered with the page: the interface reads the
    // address itself and shows the right screen.
    match Interface::get(path) {
        Some(file) => file_response(path, &file, &asked),
        None => match Interface::get("index.html") {
            Some(page) => file_response("index.html", &page, &asked),
            None => (
                StatusCode::NOT_FOUND,
                "the interface was not built into this server",
            )
                .into_response(),
        },
    }
}

fn file_response(path: &str, file: &EmbeddedFile, asked: &HeaderMap) -> Response {
    let tag = etag_of(file);
    let keeping = if path.starts_with(FINGERPRINTED) {
        KEEP_FOR
    } else {
        ALWAYS_ASK
    };
    let headers = [
        (header::CACHE_CONTROL, HeaderValue::from_static(keeping)),
        (
            header::ETAG,
            HeaderValue::from_str(&tag).unwrap_or(HeaderValue::from_static("\"\"")),
        ),
    ];

    // The browser told us which version it holds, and it is this one. Saying so
    // is the whole point of asking again: a few dozen bytes instead of the
    // file, and never a stale one.
    if already_held(asked, &tag) {
        return (StatusCode::NOT_MODIFIED, headers).into_response();
    }

    (
        StatusCode::OK,
        headers,
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static(content_type_of(path)),
        )],
        Body::from(file.data.clone().into_owned()),
    )
        .into_response()
}

/// A name for these exact bytes, taken from what they are rather than from
/// when they were written: two servers built from one commit then answer the
/// same thing, and a rebuild that changed nothing does not look like a change.
fn etag_of(file: &EmbeddedFile) -> String {
    let hash = file.metadata.sha256_hash();
    let mut tag = String::with_capacity(18);
    tag.push('"');
    for byte in &hash[..8] {
        use std::fmt::Write;
        let _ = write!(tag, "{byte:02x}");
    }
    tag.push('"');
    tag
}

/// Whether the browser already holds this exact version.
///
/// A browser may list several, and may weaken a tag it stored; both are
/// answered by looking for ours among them rather than comparing the whole
/// line.
fn already_held(asked: &HeaderMap, tag: &str) -> bool {
    asked
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|held| {
            held.split(',')
                .map(|one| one.trim().trim_start_matches("W/"))
                .any(|one| one == tag || one == "*")
        })
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
        assert_eq!(
            content_type_of("assets/index-abc123.js"),
            "text/javascript; charset=utf-8"
        );
        assert_eq!(
            content_type_of("assets/index-abc123.css"),
            "text/css; charset=utf-8"
        );
        assert_eq!(content_type_of("index.html"), "text/html; charset=utf-8");
        assert_eq!(content_type_of("logo.svg"), "image/svg+xml");
    }

    #[test]
    fn something_nobody_planned_for_is_not_handed_over_as_runnable() {
        assert_eq!(content_type_of("notes.txt"), "application/octet-stream");
        assert_eq!(content_type_of("no-extension"), "application/octet-stream");
    }

    #[test]
    fn only_a_name_carrying_a_fingerprint_may_be_kept_for_ever() {
        // The defect this exists for: the logo and the icons are copied over
        // under a fixed name, so the same name holds different pictures the
        // day one of them changes. Told to keep such a file for a year, a
        // browser never shows the new one.
        for kept in ["assets/index-abc123.js", "assets/index-abc123.css"] {
            assert!(kept.starts_with(FINGERPRINTED), "{kept}");
        }
        for asked_again in [
            "index.html",
            "melyxar-64.png",
            "favicon-32.png",
            "apple-touch-icon.png",
            "melyxar-1024.webp",
        ] {
            assert!(!asked_again.starts_with(FINGERPRINTED), "{asked_again}");
        }
        assert_eq!(ALWAYS_ASK, "no-cache");
        assert!(KEEP_FOR.contains("immutable"));
    }

    #[test]
    fn a_browser_holding_this_very_version_is_told_so() {
        let tag = etag_of(&Interface::get("index.html").expect("the page is built in"));
        assert!(
            tag.starts_with('"') && tag.ends_with('"') && tag.len() == 18,
            "an entity tag is quoted: {tag}"
        );

        let mut asked = HeaderMap::new();
        assert!(!already_held(&asked, &tag), "nothing was claimed");

        asked.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(&tag).expect("a tag is a header value"),
        );
        assert!(already_held(&asked, &tag));

        // A browser may list several, and may mark one as weak on the way out.
        asked.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(&format!("\"0011223344556677\", W/{tag}"))
                .expect("a tag is a header value"),
        );
        assert!(already_held(&asked, &tag));

        asked.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_static("\"0011223344556677\""),
        );
        assert!(
            !already_held(&asked, &tag),
            "another version is not this one"
        );
    }

    #[test]
    fn two_different_files_are_never_given_the_same_name() {
        let page = etag_of(&Interface::get("index.html").expect("the page is built in"));
        let logo = etag_of(&Interface::get("melyxar-64.png").expect("the logo is built in"));
        assert_ne!(page, logo);
    }
}
