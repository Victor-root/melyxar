//! What a browser needs to install the interface as an application: a
//! manifest, and the icon drawn in the colour of the logo.
//!
//! A browser fetches both without the session's cookie, so they are open to
//! anybody, like the page itself, and say nothing about the library. The
//! colours travel in the address: the page writes the address of the
//! manifest from its own theme, and the manifest gives the addresses of the
//! icons in the same colours.

use axum::body::Body;
use axum::extract::{Path as RoutePath, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use melyxar_app::mark::{self, Colour};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::error::ServerError;
use crate::interface::{look, Found};

/// Where the icons are served, one address per colour of the logo, and one
/// per colour of the logo and of the ground under it.
const ICONS: &str = "/api/v1/public/app/icon";

/// The relief of the logo on its own, filling the square.
const RELIEF: &str = "melyxar-shade-512.png";

/// The relief drawn small enough in the middle of the square to survive any
/// shape a system cuts an icon to: the circle of four fifths of its width.
const RELIEF_INSET: &str = "melyxar-shade-inset-512.png";

/// How wide both reliefs are, and so the icons drawn from them.
const ICON_SIZE: &str = "512x512";

/// Asked again each time and answered "unchanged" when it is: the relief can
/// change under the same address with an update of the interface.
const ALWAYS_ASK: &str = "no-cache";

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/public/app/manifest", get(manifest))
        .route("/api/v1/public/app/icon/{mark}", get(icon_alone))
        .route(
            "/api/v1/public/app/icon/{mark}/{ground}",
            get(icon_on_ground),
        )
}

#[derive(Debug, Deserialize)]
struct Colours {
    mark: String,
    ground: String,
}

#[derive(Debug, Serialize)]
struct Manifest {
    id: &'static str,
    name: String,
    short_name: String,
    start_url: &'static str,
    scope: &'static str,
    display: &'static str,
    background_color: String,
    theme_color: String,
    icons: [Icon; 2],
}

#[derive(Debug, Serialize)]
struct Icon {
    src: String,
    sizes: &'static str,
    r#type: &'static str,
    purpose: &'static str,
}

async fn manifest(State(state): State<AppState>, Query(colours): Query<Colours>) -> Response {
    let (Some(mark), Some(ground)) = (
        Colour::from_hex(&colours.mark),
        Colour::from_hex(&colours.ground),
    ) else {
        return ServerError::invalid_input("a colour is six hexadecimal digits").into_response();
    };
    // Named after the server as its administrator named it, the name the
    // sign in screen shows.
    let name = match state.database().server_settings().await {
        Ok(settings) => settings.server_name,
        Err(error) => return ServerError::internal(error.to_string()).into_response(),
    };
    let manifest = manifest_for(&name, mark, ground);
    (
        [
            (
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/manifest+json"),
            ),
            (header::CACHE_CONTROL, HeaderValue::from_static(ALWAYS_ASK)),
        ],
        Json(manifest),
    )
        .into_response()
}

/// The manifest for these colours. The same address every time for the
/// application itself, so a change of colour updates the one installed rather
/// than making another.
fn manifest_for(name: &str, mark: Colour, ground: Colour) -> Manifest {
    let ground_hex = ground.hex();
    Manifest {
        id: "/",
        name: name.to_owned(),
        short_name: name.to_owned(),
        start_url: "/",
        scope: "/",
        display: "standalone",
        background_color: format!("#{ground_hex}"),
        theme_color: format!("#{ground_hex}"),
        icons: [
            Icon {
                src: format!("{ICONS}/{}", mark.hex()),
                sizes: ICON_SIZE,
                r#type: "image/png",
                purpose: "any",
            },
            Icon {
                src: format!("{ICONS}/{}/{ground_hex}", mark.hex()),
                sizes: ICON_SIZE,
                r#type: "image/png",
                purpose: "maskable",
            },
        ],
    }
}

async fn icon_alone(
    State(state): State<AppState>,
    RoutePath(mark): RoutePath<String>,
    asked: HeaderMap,
) -> Response {
    match Colour::from_hex(&mark) {
        Some(mark) => icon(&state, RELIEF, mark, None, &asked).await,
        None => ServerError::invalid_input("a colour is six hexadecimal digits").into_response(),
    }
}

async fn icon_on_ground(
    State(state): State<AppState>,
    RoutePath((mark, ground)): RoutePath<(String, String)>,
    asked: HeaderMap,
) -> Response {
    match (Colour::from_hex(&mark), Colour::from_hex(&ground)) {
        (Some(mark), Some(ground)) => icon(&state, RELIEF_INSET, mark, Some(ground), &asked).await,
        _ => ServerError::invalid_input("a colour is six hexadecimal digits").into_response(),
    }
}

/// One icon, drawn from the relief the interface carries. Named after the
/// relief's own version: the colours are in the address already, so the
/// relief is all that can change under it.
async fn icon(
    state: &AppState,
    relief: &'static str,
    mark: Colour,
    ground: Option<Colour>,
    asked: &HeaderMap,
) -> Response {
    let path = state.config().directories.interface.join(relief);
    let held = asked
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    // Drawing a square of half a thousand pixels and writing it out takes a
    // few milliseconds: not for the threads that answer requests.
    let drawn = tokio::task::spawn_blocking(move || {
        look(&path, held.as_deref()).map(|found| match found {
            Found::Held(tag) => Ok((tag, None)),
            Found::Read(tag, bytes) => {
                mark::draw(&bytes, mark, ground).map(|icon| (tag, Some(icon)))
            }
        })
    })
    .await;

    let (tag, icon) = match drawn {
        Ok(Some(Ok(drawn))) => drawn,
        Ok(None) => {
            return ServerError::not_found("the interface carries no relief of the logo")
                .into_response();
        }
        Ok(Some(Err(error))) => {
            tracing::warn!(relief, %error, "the icon of the application could not be drawn");
            return ServerError::internal(error.to_string()).into_response();
        }
        Err(error) => return ServerError::internal(error.to_string()).into_response(),
    };
    let headers = [
        (header::CACHE_CONTROL, HeaderValue::from_static(ALWAYS_ASK)),
        (
            header::ETAG,
            HeaderValue::from_str(&tag).unwrap_or(HeaderValue::from_static("\"\"")),
        ),
    ];
    match icon {
        None => (StatusCode::NOT_MODIFIED, headers).into_response(),
        Some(icon) => (
            StatusCode::OK,
            headers,
            [(header::CONTENT_TYPE, HeaderValue::from_static("image/png"))],
            Body::from(icon),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_names_its_icons_in_its_own_colours() {
        let mark = Colour::from_hex("1C7ED6").expect("a colour");
        let ground = Colour::from_hex("0c0d10").expect("a colour");
        let manifest =
            serde_json::to_value(manifest_for("Home Cinema", mark, ground)).expect("json");

        assert_eq!(manifest["id"], "/");
        assert_eq!(manifest["name"], "Home Cinema");
        assert_eq!(manifest["short_name"], "Home Cinema");
        assert_eq!(manifest["start_url"], "/");
        assert_eq!(manifest["display"], "standalone");
        assert_eq!(manifest["background_color"], "#0c0d10");
        assert_eq!(
            manifest["icons"][0]["src"],
            "/api/v1/public/app/icon/1c7ed6"
        );
        assert_eq!(manifest["icons"][0]["purpose"], "any");
        assert_eq!(
            manifest["icons"][1]["src"],
            "/api/v1/public/app/icon/1c7ed6/0c0d10"
        );
        assert_eq!(manifest["icons"][1]["purpose"], "maskable");
        assert_eq!(manifest["icons"][1]["type"], "image/png");
    }
}
