//! Serving the interface itself.
//!
//! The built interface lives in a folder of its own and is read from the disk
//! each time a file is asked for. That is what lets an update of the interface
//! alone skip rebuilding and restarting the server: put in the folder, it is
//! served from the next request on, and a film playing meanwhile is not cut.
//! It costs nothing anybody can measure: the whole of it is a couple of
//! megabytes the system keeps in memory once read, and a browser keeps for
//! ever everything whose name carries a fingerprint, so it asks for little.
//!
//! Only what the build names with a fingerprint is handed over with permission
//! to keep it for ever, because only such a name can promise never to stand
//! for different bytes. Everything else, the page and the files copied over
//! untouched, is asked for again every time and answered "unchanged" when it
//! is, which costs one small round trip and never a stale logo.

use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use melyxar_app::AppState;

use crate::images::safe_relative_path;

/// The page every address of the interface is answered with.
const PAGE: &str = "index.html";

/// How long a file named with a fingerprint may be kept.
const KEEP_FOR: &str = "public, max-age=31536000, immutable";

/// Everything else is asked for again every time. Cheap, because the answer is
/// almost always "unchanged" and carries no file with it.
const ALWAYS_ASK: &str = "no-cache";

/// Where the build puts what it names with a fingerprint.
///
/// The rest of what is served is copied over from `web/public` under the name
/// it was given, so `melyxar-shade-64.png` today and `melyxar-shade-64.png` tomorrow can
/// hold different pictures. Promising a browser it may keep such a file for a
/// year means a logo that changes is never seen to change.
const FINGERPRINTED: &str = "assets/";

pub fn router() -> Router<AppState> {
    Router::new().fallback(serve)
}

async fn serve(State(state): State<AppState>, asked: HeaderMap, uri: Uri) -> Response {
    from_the_folder(&state.config().directories.interface, uri.path(), &asked)
}

/// What one address is answered with, out of the folder the interface is in.
fn from_the_folder(folder: &Path, address: &str, asked: &HeaderMap) -> Response {
    // Anything under the interface that is not a file is one of its own
    // addresses, and those are answered with the page: the interface reads the
    // address itself and shows the right screen. A name that would reach out
    // of the folder is answered the same way, and never read.
    if let Some(relative) = safe_relative_path(address.trim_start_matches('/')) {
        if let Some(file) = file_response(folder, &relative, asked) {
            return file;
        }
    }
    match file_response(folder, Path::new(PAGE), asked) {
        Some(page) => page,
        None => (
            StatusCode::NOT_FOUND,
            "the interface is not installed in the folder this server reads it from",
        )
            .into_response(),
    }
}

/// One file of the interface, or nothing when there is no such file.
fn file_response(folder: &Path, relative: &Path, asked: &HeaderMap) -> Option<Response> {
    let name = relative.to_str()?;
    let held = asked
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());
    // Asked right here rather than sent to the pool kept for slow work: these
    // are a few small files on the server's own disk, which the system keeps
    // in memory once read, and the whole look takes a few microseconds.
    // Sending it away was measured at half again the time of the whole
    // answer, where reading it here matches what the interface cost when it
    // was built into the binary.
    let found = look(&folder.join(relative), held)?;

    let keeping = if name.starts_with(FINGERPRINTED) {
        KEEP_FOR
    } else {
        ALWAYS_ASK
    };
    let tag = match &found {
        Found::Held(tag) | Found::Read(tag, _) => tag,
    };
    let headers = [
        (header::CACHE_CONTROL, HeaderValue::from_static(keeping)),
        (
            header::ETAG,
            HeaderValue::from_str(tag).unwrap_or(HeaderValue::from_static("\"\"")),
        ),
    ];

    Some(match found {
        Found::Held(_) => (StatusCode::NOT_MODIFIED, headers).into_response(),
        Found::Read(_, bytes) => (
            StatusCode::OK,
            headers,
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static(content_type_of(name)),
            )],
            Body::from(bytes),
        )
            .into_response(),
    })
}

/// What the disk had for one file.
pub(crate) enum Found {
    /// The version the browser already holds, which was not read at all.
    Held(String),
    /// Another version, read whole.
    Read(String, Vec<u8>),
}

/// Looks for one file, and reads it only when the browser does not hold it
/// already: saying so is the whole point of asking again, a few dozen bytes
/// instead of the file, and never a stale one.
///
/// Opened once and described from what was opened rather than from the name:
/// an update swaps each file whole under the same name, and this way what is
/// described is always what is read.
pub(crate) fn look(path: &Path, held: Option<&str>) -> Option<Found> {
    let mut file = std::fs::File::open(path).ok()?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() {
        return None;
    }
    let tag = tag_of(&metadata);
    if already_held(held, &tag) {
        return Some(Found::Held(tag));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(metadata.len()).unwrap_or(0));
    file.read_to_end(&mut bytes).ok()?;
    Some(Found::Read(tag, bytes))
}

/// A name for this version of a file, from its size and the moment it was
/// written, which is how the usual web servers name theirs. Read from what
/// the system says about the file, so answering "unchanged" reads none of
/// it. The installer leaves a file alone when an update did not change it,
/// so an update that changed nothing does not look like a change.
fn tag_of(metadata: &std::fs::Metadata) -> String {
    let written = metadata
        .modified()
        .ok()
        .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |since| since.as_nanos());
    format!("\"{:x}-{:x}\"", metadata.len(), written)
}

/// Whether the browser already holds this exact version.
///
/// A browser may list several, and may weaken a tag it stored; both are
/// answered by looking for ours among them rather than comparing the whole
/// line.
fn already_held(held: Option<&str>, tag: &str) -> bool {
    held.is_some_and(|held| {
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
            "melyxar-shade-64.png",
            "favicon-32.png",
            "melyxar-shade-inset-512.png",
            "melyxar-1024.webp",
        ] {
            assert!(!asked_again.starts_with(FINGERPRINTED), "{asked_again}");
        }
        assert_eq!(ALWAYS_ASK, "no-cache");
        assert!(KEEP_FOR.contains("immutable"));
    }

    /// A folder holding an interface: the page, one file with a fingerprint
    /// and one without.
    fn an_installed_interface() -> tempfile::TempDir {
        let folder = tempfile::tempdir().expect("temporary folder");
        std::fs::create_dir_all(folder.path().join("web/assets")).expect("folders");
        std::fs::write(folder.path().join("web/index.html"), "the page").expect("page");
        std::fs::write(folder.path().join("web/assets/index-abc123.js"), "the code").expect("code");
        std::fs::write(folder.path().join("web/melyxar-shade-64.png"), "the logo").expect("logo");
        folder
    }

    async fn answer(
        folder: &Path,
        address: &str,
        asked: &HeaderMap,
    ) -> (StatusCode, HeaderMap, String) {
        let response = from_the_folder(&folder.join("web"), address, asked);
        let status = response.status();
        let headers = response.headers().clone();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("a body");
        (status, headers, String::from_utf8_lossy(&body).into_owned())
    }

    #[tokio::test]
    async fn each_file_is_served_from_the_folder_with_what_it_is() {
        let folder = an_installed_interface();
        let nothing = HeaderMap::new();

        let (status, headers, body) =
            answer(folder.path(), "/assets/index-abc123.js", &nothing).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "the code");
        assert_eq!(
            headers[header::CONTENT_TYPE],
            "text/javascript; charset=utf-8"
        );
        assert_eq!(headers[header::CACHE_CONTROL], KEEP_FOR);

        let (_, headers, body) = answer(folder.path(), "/melyxar-shade-64.png", &nothing).await;
        assert_eq!(body, "the logo");
        assert_eq!(headers[header::CACHE_CONTROL], ALWAYS_ASK);
    }

    #[tokio::test]
    async fn an_address_of_the_interface_is_answered_with_the_page() {
        let folder = an_installed_interface();
        for address in [
            "/",
            "/library/01ab",
            "/assets",
            "/assets/",
            "/nothing-here.js",
        ] {
            let (status, headers, body) = answer(folder.path(), address, &HeaderMap::new()).await;
            assert_eq!(status, StatusCode::OK, "{address}");
            assert_eq!(body, "the page", "{address}");
            assert_eq!(headers[header::CACHE_CONTROL], ALWAYS_ASK, "{address}");
        }
    }

    #[tokio::test]
    async fn a_name_reaching_out_of_the_folder_is_never_read() {
        let folder = an_installed_interface();
        std::fs::write(folder.path().join("secret.toml"), "a secret").expect("secret");
        for address in [
            "/../secret.toml",
            "/assets/../../secret.toml",
            "//secret.toml",
            "/./../secret.toml",
        ] {
            let (_, _, body) = answer(folder.path(), address, &HeaderMap::new()).await;
            assert_eq!(body, "the page", "{address}");
        }
    }

    #[tokio::test]
    async fn an_interface_put_in_the_folder_is_served_from_the_next_request() {
        // The whole point of the folder: an update of the interface alone
        // neither rebuilds nor restarts anything.
        let folder = an_installed_interface();
        let (_, before, _) = answer(folder.path(), "/", &HeaderMap::new()).await;

        std::fs::write(folder.path().join("web/index.html"), "the new page").expect("page");
        let (_, after, body) = answer(folder.path(), "/", &HeaderMap::new()).await;
        assert_eq!(body, "the new page");
        assert_ne!(before[header::ETAG], after[header::ETAG]);
    }

    #[tokio::test]
    async fn a_browser_holding_this_very_version_is_told_so() {
        let folder = an_installed_interface();
        let (_, first, _) = answer(folder.path(), "/", &HeaderMap::new()).await;
        let tag = first[header::ETAG].to_str().expect("a tag").to_string();
        assert!(
            tag.starts_with('"') && tag.ends_with('"'),
            "an entity tag is quoted: {tag}"
        );

        let mut asked = HeaderMap::new();
        asked.insert(
            header::IF_NONE_MATCH,
            HeaderValue::from_str(&tag).expect("a header"),
        );
        let (status, _, body) = answer(folder.path(), "/", &asked).await;
        assert_eq!(status, StatusCode::NOT_MODIFIED);
        assert!(body.is_empty());

        // A browser may list several, and may mark one as weak on the way out.
        assert!(already_held(Some(&format!("\"0011\", W/{tag}")), &tag));

        asked.insert(header::IF_NONE_MATCH, HeaderValue::from_static("\"0011\""));
        let (status, _, body) = answer(folder.path(), "/", &asked).await;
        assert_eq!(status, StatusCode::OK, "another version is not this one");
        assert_eq!(body, "the page");
    }

    #[tokio::test]
    async fn no_interface_installed_is_said_rather_than_a_blank_page() {
        let folder = tempfile::tempdir().expect("temporary folder");
        let (status, _, body) = answer(folder.path(), "/", &HeaderMap::new()).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert!(body.contains("not installed"));
    }
}
