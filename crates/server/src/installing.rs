//! What a browser needs to install the interface as an application: a
//! manifest, and the icon drawn in the colour of the logo.
//!
//! A browser fetches both without the session's cookie, so they are open to
//! anybody, like the page itself, and say nothing about the library. The
//! colours travel in the address: the page writes the address of the
//! manifest from its own theme, and the manifest gives the addresses of the
//! icons in the same colours. A server given a logo of its own wears it
//! instead, under addresses named after the logo.

use axum::body::Body;
use axum::extract::{Path as RoutePath, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use melyxar_app::mark::{self, Colour};
use melyxar_app::server::LogoIcon;
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::error::ServerError;
use crate::interface::{look, Found};

/// Where the icons are served, one address per colour of the logo, and one
/// per colour of the logo and of the ground under it.
const ICONS: &str = "/api/v1/public/app/icon";

/// Where the icons of a logo of the server's own are served, named after the
/// logo: a new logo is a new address, which is what tells a browser to fetch
/// it again, and an installed application to change its icon.
const LOGO_ICONS: &str = "/api/v1/public/app/logo";

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
        .route(&format!("{LOGO_ICONS}/{{logo}}"), get(logo_alone))
        .route(
            &format!("{LOGO_ICONS}/{{logo}}/{{ground}}"),
            get(logo_on_ground),
        )
}

/// Where the icon of the server's own logo is, the logo filling its square:
/// what the page puts in its tab.
pub fn logo_icon_url(logo: &str) -> String {
    format!("{LOGO_ICONS}/{}", melyxar_app::server::stem_of(logo))
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
    // sign in screen shows, and wearing the logo it was given if it was.
    let settings = match state.database().server_settings().await {
        Ok(settings) => settings,
        Err(error) => return ServerError::internal(error.to_string()).into_response(),
    };
    let manifest = manifest_for(
        &settings.server_name,
        mark,
        ground,
        settings.logo_path.as_deref(),
    );
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

/// The manifest for these colours, or for the server's own logo. The same
/// address every time for the application itself, so a change of colour or of
/// logo updates the one installed rather than making another.
fn manifest_for(name: &str, mark: Colour, ground: Colour, logo: Option<&str>) -> Manifest {
    let ground_hex = ground.hex();
    let alone = match logo {
        Some(logo) => logo_icon_url(logo),
        None => format!("{ICONS}/{}", mark.hex()),
    };
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
                src: alone.clone(),
                sizes: ICON_SIZE,
                r#type: "image/png",
                purpose: "any",
            },
            Icon {
                src: format!("{alone}/{ground_hex}"),
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

async fn logo_alone(State(state): State<AppState>, RoutePath(logo): RoutePath<String>) -> Response {
    logo_icon(&state, &logo, LogoIcon::Whole, None).await
}

async fn logo_on_ground(
    State(state): State<AppState>,
    RoutePath((logo, ground)): RoutePath<(String, String)>,
) -> Response {
    match Colour::from_hex(&ground) {
        Some(ground) => logo_icon(&state, &logo, LogoIcon::Inset, Some(ground)).await,
        None => ServerError::invalid_input("a colour is six hexadecimal digits").into_response(),
    }
}

/// One icon of the server's own logo, laid on a ground when one is asked for.
/// Kept for ever: the address changes with the logo and with the ground.
async fn logo_icon(
    state: &AppState,
    logo: &str,
    icon: LogoIcon,
    ground: Option<Colour>,
) -> Response {
    let made = match melyxar_app::server::logo_icon(state, logo, icon).await {
        Ok(Some(made)) => made,
        Ok(None) => return ServerError::not_found("the server wears no such logo").into_response(),
        Err(error) => return ServerError::from(error).into_response(),
    };
    let icon = match ground {
        None => made,
        // Laying half a thousand pixels square on a ground takes a few
        // milliseconds: not for the threads that answer requests.
        Some(ground) => {
            match tokio::task::spawn_blocking(move || mark::laid_on(&made, ground)).await {
                Ok(Ok(laid)) => laid,
                Ok(Err(error)) => {
                    tracing::warn!(%error, "the icon of the server's logo could not be laid on its ground");
                    return ServerError::internal(error.to_string()).into_response();
                }
                Err(error) => return ServerError::internal(error.to_string()).into_response(),
            }
        }
    };
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, HeaderValue::from_static("image/png")),
            (
                header::CACHE_CONTROL,
                HeaderValue::from_static(crate::images::KEEP_FOR),
            ),
        ],
        Body::from(icon),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_manifest_names_its_icons_in_its_own_colours() {
        let mark = Colour::from_hex("1C7ED6").expect("a colour");
        let ground = Colour::from_hex("0c0d10").expect("a colour");
        let manifest =
            serde_json::to_value(manifest_for("Home Cinema", mark, ground, None)).expect("json");

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

    #[test]
    fn a_server_with_a_logo_of_its_own_wears_it_once_installed() {
        let mark = Colour::from_hex("1C7ED6").expect("a colour");
        let ground = Colour::from_hex("0c0d10").expect("a colour");
        let manifest = serde_json::to_value(manifest_for(
            "Home Cinema",
            mark,
            ground,
            Some("logo-abc.webp"),
        ))
        .expect("json");

        assert_eq!(
            manifest["icons"][0]["src"],
            "/api/v1/public/app/logo/logo-abc"
        );
        assert_eq!(
            manifest["icons"][1]["src"],
            "/api/v1/public/app/logo/logo-abc/0c0d10"
        );
    }
}
