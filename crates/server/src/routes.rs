//! The routes.
//!
//! Handlers translate and nothing more: they read a request, call a use case
//! and render the answer. No rule about media, playback or scanning lives
//! here, which is what lets a second entry point reuse the same behaviour.

use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::error::Result;

/// Version of the interface this server speaks.
///
/// Part of every path, so a later version can be served alongside rather than
/// replacing this one and breaking clients that have not caught up.
pub const API_VERSION: &str = "v1";

/// Whether an address belongs to this server's own surface rather than to a
/// page of the interface.
///
/// Read without regard to case, so that an address spelt `/API/...` is still
/// one of the surface: the gate shuts it like any other, for a reason rather
/// than because no route happens to match it, and one no route answers is
/// refused rather than handed the page.
pub(crate) fn on_the_surface(path: &str) -> bool {
    const THE_SURFACE: &[u8] = b"/api";
    let bytes = path.as_bytes();
    bytes.len() >= THE_SURFACE.len()
        && bytes[..THE_SURFACE.len()].eq_ignore_ascii_case(THE_SURFACE)
}

/// Builds the whole surface.
pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/v1/system/info", get(system_info))
        .route("/api/v1/system/health", get(health))
        .route("/api/v1/system/overview", get(overview))
        .route("/api/v1/system/measures", get(measures))
        .route("/api/v1/system/measures/history", get(measures_history))
        .route("/api/v1/system/diagnostics", get(diagnostics))
        .route("/api/v1/system/diagnostics/text", get(diagnostics_text))
        // One name, two verbs: reading what was said and forgetting it are the
        // same thing seen from either end. Two calls to `route` on one path
        // would take the server down as it starts.
        .route(
            "/api/v1/system/journal",
            get(journal).delete(forget_journal),
        )
        .route("/api/v1/system/journal/text", get(journal_text))
        .route(
            "/api/v1/system/cache/subtitles",
            axum::routing::delete(forget_converted_subtitles),
        )
        .route("/api/v1/public/branding", get(public_branding))
        .route("/api/v1/public/names", get(names_at_the_door))
        .merge(crate::account::router())
        .merge(crate::accounts::router())
        .merge(crate::activity::router())
        .merge(crate::calibration::router())
        .merge(crate::catalogue::router())
        .merge(crate::general::router())
        .merge(crate::images::router())
        .merge(crate::installing::router())
        .merge(crate::jobs::router())
        .merge(crate::libraries::router())
        .merge(crate::live::router())
        .merge(crate::deletion::router())
        .merge(crate::devices::router())
        .merge(crate::pictures::router())
        .merge(crate::page::router())
        .merge(crate::playback::router())
        .merge(crate::preferences::router())
        // Last: whatever no route above answered. An address of the surface
        // is not found; any other is a page of the interface, which reads the
        // address itself.
        .merge(crate::interface::router())
        .with_state(state)
}

/// What this server is and what it can do.
///
/// Deliberately says what is *available* rather than which version is running,
/// so a client adapts to a capability instead of comparing version numbers.
#[derive(Debug, Serialize)]
struct SystemInfo {
    server_name: String,
    version: &'static str,
    api_version: &'static str,
    /// Whether the setup wizard still has to run.
    setup_complete: bool,
    /// Whether playback can work at all. A library still browses without it.
    playback_available: bool,
    maintenance: bool,
    features: Features,
}

#[derive(Debug, Serialize)]
struct Features {
    /// Hardware paths the media tool carries.
    hardware_acceleration: Vec<String>,
    /// Whether wide gamut colour can be converted. Without it, such films
    /// cannot be shown with correct colours.
    wide_gamut_conversion: bool,
}

async fn system_info(State(state): State<AppState>) -> Result<Json<SystemInfo>> {
    let settings = state
        .database()
        .server_settings()
        .await
        .map_err(|error| crate::error::ServerError::internal(error.to_string()))?;

    Ok(Json(SystemInfo {
        server_name: settings.server_name,
        version: melyxar_core::BUILD,
        api_version: API_VERSION,
        // One account is what tells a server that has been set up from one
        // that has not, and it is what the wizard keys on.
        setup_complete: !melyxar_app::accounts::still_to_be_set_up(&state).await?,
        playback_available: state.can_play_media(),
        maintenance: settings.maintenance_enabled,
        features: Features {
            hardware_acceleration: state.hardware_acceleration_names(),
            wide_gamut_conversion: state.can_convert_wide_gamut(),
        },
    }))
}

/// Minimal liveness answer, for a reverse proxy or a watchdog.
async fn health() -> &'static str {
    "ok"
}

async fn overview(
    _: crate::account::Administrator,
    State(state): State<AppState>,
) -> Result<Json<melyxar_app::overview::Overview>> {
    Ok(Json(melyxar_app::overview::collect(&state).await?))
}

async fn measures(
    _: crate::account::Administrator,
    State(state): State<AppState>,
) -> Json<melyxar_app::measures::Live> {
    Json(melyxar_app::measures::live(&state))
}

#[derive(Debug, Deserialize)]
struct HistoryAsked {
    over: melyxar_app::measures::Over,
}

async fn measures_history(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Query(asked): Query<HistoryAsked>,
) -> Result<Json<Vec<melyxar_app::measures::Point>>> {
    Ok(Json(melyxar_app::measures::history(&state, asked.over).await?))
}

async fn diagnostics(
    _: crate::account::Administrator,
    State(state): State<AppState>,
) -> Result<Json<melyxar_app::diagnostics::Diagnostics>> {
    Ok(Json(melyxar_app::diagnostics::collect(&state).await?))
}

/// The same report as plain text, exactly as the command line prints it.
///
/// So that somebody who cannot read a log copies one block from a screen and
/// it holds everything worth asking about. Rendered by the same code as the
/// command line, because two renderings of one report drift apart and the
/// second one is always the one nobody checked.
async fn diagnostics_text(
    _: crate::account::Administrator,
    State(state): State<AppState>,
) -> Result<([(&'static str, &'static str); 1], String)> {
    let report = melyxar_app::diagnostics::collect(&state).await?;
    Ok((
        [("content-type", "text/plain; charset=utf-8")],
        melyxar_app::diagnostics::render_text(&report),
    ))
}

/// What the server has been saying, sorted by tag.
///
/// The point of the whole thing: somebody testing is told "send me `playback`
/// and `subtitles`", ticks those two, and copies. Neither of them has to
/// explain a filter syntax or the name of a service.
#[derive(Debug, Deserialize)]
struct WhatIsWanted {
    /// Tags to keep, separated by commas. Absent keeps every tag.
    tags: Option<String>,
    /// Keep only lines holding this text.
    holding: Option<String>,
    /// At most this many, newest first. Absent means everything kept.
    most: Option<usize>,
}

impl WhatIsWanted {
    fn asked(&self) -> melyxar_core::journal::Wanted {
        melyxar_core::journal::Wanted {
            tags: self
                .tags
                .as_deref()
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(str::to_string)
                .collect(),
            holding: self
                .holding
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(str::to_string),
            most: self.most.unwrap_or(0),
        }
    }
}

#[derive(Debug, Serialize)]
struct JournalView {
    /// Every tag this server has really written, with how many lines carry it.
    /// What the screen puts on its buttons, rather than a list that drifts.
    tags: Vec<TagView>,
    lines: Vec<melyxar_core::journal::Line>,
}

#[derive(Debug, Serialize)]
struct TagView {
    name: &'static str,
    lines: usize,
}

async fn journal(_: crate::account::Administrator, Query(wanted): Query<WhatIsWanted>) -> Json<JournalView> {
    Json(JournalView {
        tags: melyxar_core::journal::tags()
            .into_iter()
            .map(|(name, lines)| TagView { name, lines })
            .collect(),
        lines: melyxar_core::journal::lines(&wanted.asked()),
    })
}

/// The same, as one block of text ready to paste.
async fn journal_text(
    _: crate::account::Administrator,
    Query(wanted): Query<WhatIsWanted>,
) -> ([(&'static str, &'static str); 1], String) {
    let lines = melyxar_core::journal::lines(&wanted.asked());
    let mut text = String::with_capacity(lines.len() * 120);
    // The build first: a journal and a question about a fix cannot be put
    // together without knowing which one wrote it.
    text.push_str(&format!(
        "melyxar {} ({})\n\n",
        env!("CARGO_PKG_VERSION"),
        melyxar_core::BUILD
    ));
    for line in &lines {
        text.push_str(&format!(
            "{}  {:<5} [{}] {}\n",
            melyxar_core::time::to_text(line.at),
            line.level,
            line.tag,
            line.message
        ));
    }
    ([("content-type", "text/plain; charset=utf-8")], text)
}

/// Forgets what was said, so that what is copied after a test is about that
/// test and nothing else.
#[derive(Debug, Serialize)]
struct ForgottenView {
    forgotten: usize,
}

async fn forget_journal(_: crate::account::Administrator) -> Json<ForgottenView> {
    Json(ForgottenView {
        forgotten: melyxar_core::journal::forget(),
    })
}

/// Throws away the subtitles already converted, so the slow path can be tried
/// again. A converted track is served in a millisecond and proves nothing
/// about the minute it took to get there.
async fn forget_converted_subtitles(
    _: crate::account::Administrator,
    State(state): State<AppState>,
) -> Result<Json<ForgottenView>> {
    Ok(Json(ForgottenView {
        forgotten: melyxar_app::subtitles::forget_what_was_converted(&state).await?,
    }))
}

/// What the door needs to draw itself, before anyone has signed in.
///
/// The sign in page needs the name and the mark of the server before an
/// account exists, so this one route answers without authentication. It
/// therefore says nothing else: no account list, no library names, no version,
/// nothing about the machine.
///
/// Whether this server has been set up is part of it, and is the one thing
/// besides the name and the mark that the page has to know: a brand new server
/// shows somebody the screen that makes the first account, and a server that
/// has been set up shows them the one that asks for a password. Saying so
/// gives nothing away, since whoever asks can find out by trying either door.
#[derive(Debug, Serialize)]
struct PublicBranding {
    server_name: String,
    /// Where the logo the administrator gave the server is, when there is one.
    logo: Option<String>,
    /// Where that logo is as a square icon, for the tab of the browser.
    logo_icon: Option<String>,
    login_background_path: Option<String>,
    /// Which drawn background the screen wears when no picture was put there.
    /// A word rather than a number, so that what arrives at the screen says
    /// what it means and a third one costs nobody a renumbering.
    login_background_style: &'static str,
    setup_complete: bool,
}

async fn public_branding(State(state): State<AppState>) -> Result<Json<PublicBranding>> {
    let settings = state
        .database()
        .server_settings()
        .await
        .map_err(|error| crate::error::ServerError::internal(error.to_string()))?;

    Ok(Json(PublicBranding {
        server_name: settings.server_name,
        logo: settings.logo_path.as_deref().map(crate::images::logo_url),
        logo_icon: settings
            .logo_path
            .as_deref()
            .map(crate::installing::logo_icon_url),
        login_background_path: settings.login_background_path,
        login_background_style: settings.login_background.as_str(),
        setup_complete: !melyxar_app::accounts::still_to_be_set_up(&state).await?,
    }))
}

/// The names the sign in screen offers, for whoever has not signed in yet.
///
/// A deliberate disclosure, and the only one on this server: it tells anybody
/// who can reach it which names exist here. That is the trade the list is, and
/// it buys back the thing it costs, because a screen that offers the names is
/// a screen where signing in is one press and a password rather than
/// remembering how you spelt your own name.
///
/// It is bounded twice over. An administrator turns the whole list off for the
/// server, and each person turns themselves off for their own account, and
/// either way this answers an empty list rather than a refusal: a screen that
/// is told nothing simply shows no names, which is also what a brand new
/// server with no accounts answers.
///
/// Names and their pictures, nothing else. No identifier, no rights, nothing
/// that says whether a name is an administrator: what is drawn is a row of
/// faces with a name under each, and everything beyond that would be given
/// away for no gain.
#[derive(Debug, Serialize)]
struct NamesAtTheDoor {
    names: Vec<NameAtTheDoor>,
}

#[derive(Debug, Serialize)]
struct NameAtTheDoor {
    name: String,
    /// Where its picture is served, when it has one.
    avatar: Option<String>,
}

async fn names_at_the_door(State(state): State<AppState>) -> Result<Json<NamesAtTheDoor>> {
    let settings = state
        .database()
        .server_settings()
        .await
        .map_err(|error| crate::error::ServerError::internal(error.to_string()))?;

    if !settings.show_user_picker {
        return Ok(Json(NamesAtTheDoor { names: Vec::new() }));
    }

    let names = state
        .database()
        .names_at_the_door()
        .await
        .map_err(|error| crate::error::ServerError::internal(error.to_string()))?;
    Ok(Json(NamesAtTheDoor {
        names: names
            .into_iter()
            .map(|offered| NameAtTheDoor {
                avatar: offered.avatar_path.as_deref().map(crate::images::face_url),
                name: offered.name,
            })
            .collect(),
    }))
}
