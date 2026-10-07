//! Refusing a change another site asks for in somebody's name.
//!
//! The session cookie is kept to this site, which stops a site anywhere else
//! from riding on it. It does not stop a neighbour: another name under the
//! same domain, or another port of the same machine, counts as the same site
//! to that rule. So a request that changes something is refused when the
//! browser says it was not sent by this server's own pages.
//!
//! Asked of what the browser says about where a request comes from
//! (`Sec-Fetch-Site`), rather than of the address it was sent to: a proxy in
//! front may rewrite the name a request arrives under, and comparing names
//! would then refuse everything. A request that carries no such word, from a
//! program rather than a browser or from a browser too old to say, holds no
//! cookie a page elsewhere could have slipped in, and is answered as before.

use axum::extract::Request;
use axum::http::{Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use melyxar_core::error::ErrorCode;

use crate::error::ServerError;

/// Lets through what this server's own pages ask, and every reading.
pub async fn only_from_here(request: Request, next: Next) -> Response {
    let said = request
        .headers()
        .get("sec-fetch-site")
        .and_then(|value| value.to_str().ok());
    if refused(request.method(), said) {
        return ServerError::new(
            StatusCode::FORBIDDEN,
            ErrorCode::Forbidden,
            "a change asked by another site",
        )
        .into_response();
    }
    next.run(request).await
}

/// Whether a request is a change another site asked for. A reading changes
/// nothing, and is never refused here.
fn refused(method: &Method, sent_from: Option<&str>) -> bool {
    let changes = !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS);
    changes && sent_from.is_some_and(|from| from != "same-origin" && from != "none")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_change_asked_by_this_server_s_own_pages_goes_through() {
        assert!(!refused(&Method::POST, Some("same-origin")));
        assert!(!refused(&Method::DELETE, Some("same-origin")));
    }

    #[test]
    fn a_change_asked_by_a_neighbour_or_a_stranger_is_refused() {
        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            assert!(refused(&method, Some("same-site")), "{method}: a neighbouring name");
            assert!(refused(&method, Some("cross-site")), "{method}");
        }
    }

    #[test]
    fn a_reading_is_never_refused_wherever_it_comes_from() {
        // A poster shown on another page, or a link followed to this one.
        for method in [Method::GET, Method::HEAD, Method::OPTIONS] {
            assert!(!refused(&method, Some("cross-site")), "{method}");
        }
    }

    #[test]
    fn a_request_that_says_nothing_of_where_it_comes_from_is_answered() {
        // A program, or a browser too old to say: neither carries a cookie
        // a page elsewhere slipped in.
        assert!(!refused(&Method::POST, None));
        // Typed, or chosen from a bookmark: nobody else asked for it.
        assert!(!refused(&Method::POST, Some("none")));
    }
}
