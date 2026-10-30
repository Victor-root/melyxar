//! Maintenance: where it stands, for anybody, and switching it, for an
//! administrator; and the answer given to whoever it turns away.

use axum::extract::State;
use axum::http::{header, HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use melyxar_app::maintenance::{Seen, Wanted};
use melyxar_app::AppState;
use melyxar_core::error::ErrorCode;
use serde::Deserialize;

use crate::account::{Administrator, Viewer};
use crate::error::{ApiError, Result};

/// Where maintenance stands, which a door, a page or a television asks
/// before anybody has signed in.
pub(crate) const STANDING: &str = "/api/v1/public/maintenance";

pub fn router() -> Router<AppState> {
    Router::new()
        .route(STANDING, axum::routing::get(standing))
        .route("/api/v1/system/maintenance", axum::routing::put(switch))
}

async fn standing(State(state): State<AppState>) -> Json<Seen> {
    Json(melyxar_app::maintenance::seen(&state))
}

#[derive(Debug, Deserialize)]
struct Switched {
    on: bool,
    #[serde(default)]
    delay_minutes: i64,
    #[serde(default, with = "time::serde::rfc3339::option")]
    until: Option<melyxar_core::time::Timestamp>,
    #[serde(default)]
    message: Option<String>,
}

async fn switch(
    _: Administrator,
    Viewer(by): Viewer,
    State(state): State<AppState>,
    Json(switched): Json<Switched>,
) -> Result<Json<Seen>> {
    let wanted = Wanted {
        on: switched.on,
        delay_minutes: switched.delay_minutes,
        until: switched.until,
        message: switched.message,
    };
    Ok(Json(melyxar_app::maintenance::set(&state, &by, wanted).await?))
}

/// The addresses of the surface still answered to everybody during a
/// maintenance: what a door asks once somebody has signed in (who it is, so a
/// page knows whether it is turned away or is the administrator's, the code
/// of a television, the name of the device); the live line, which says the
/// moment it ends; and where a film was left, so the last position is not
/// lost. The addresses open to anybody stay open, the door and where
/// maintenance stands among them.
pub(crate) fn still_answered(path: &str, method: &Method) -> bool {
    match path {
        "/api/v1/me" | "/api/v1/me/tv-code" | "/api/v1/system/live" => method == Method::GET,
        "/api/v1/me/tv-code/check" | "/api/v1/playback/progress" => method == Method::POST,
        "/api/v1/me/browser" => method == Method::PUT,
        _ => false,
    }
}

/// What whoever maintenance turns away is answered: the code every client
/// keys its screen on, and when to ask again once the administrator said
/// when it should end. Not a failure of the server, so nothing is written
/// in the log for each one.
pub(crate) fn turned_away(state: &AppState) -> Response {
    let mut response = (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiError::new(ErrorCode::Maintenance)),
    )
        .into_response();
    if let Some(seconds) = melyxar_app::maintenance::retry_after_seconds(state) {
        response
            .headers_mut()
            .insert(header::RETRY_AFTER, HeaderValue::from(seconds));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    #[test]
    fn only_what_a_door_and_a_turned_away_page_need_is_still_answered() {
        assert!(still_answered("/api/v1/me", &Method::GET));
        assert!(still_answered("/api/v1/system/live", &Method::GET));
        assert!(still_answered("/api/v1/playback/progress", &Method::POST));
        assert!(still_answered("/api/v1/me/tv-code/check", &Method::POST));
        assert!(!still_answered("/api/v1/me", &Method::PUT));
        assert!(!still_answered("/api/v1/me/tv-code", &Method::PUT));
        assert!(!still_answered("/api/v1/home", &Method::GET));
        assert!(!still_answered("/api/v1/playback/watching", &Method::POST));
    }
}
