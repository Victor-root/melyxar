//! The lyrics of a song chosen by hand, for an administrator: what LRCLIB
//! holds under an artist and a title, taking one of its entries as the words
//! of a song, forgetting what was taken or found so it is looked up again,
//! lining the stamps of the lines up with the words heard in the song, and
//! the words written in the editor.
//!
//! Reading the lyrics of a song is `music.rs`; this is only for changing them.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_app::music::lyrics::Offer;
use melyxar_core::music_lyrics::SyncedLine;
use melyxar_core::time::Millis;
use serde::{Deserialize, Serialize};

use crate::account::Viewer;
use crate::error::Result;
use crate::identifiers::parse_work;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/music/songs/{id}/lyrics/offers", get(offers))
        .route(
            "/api/v1/music/songs/{id}/lyrics/online",
            axum::routing::post(choose).delete(forget),
        )
        .route(
            "/api/v1/music/songs/{id}/lyrics/sync",
            axum::routing::post(synchronise).delete(unsynchronise),
        )
        .route("/api/v1/music/songs/{id}/lyrics/offset", axum::routing::put(set_offset))
        .route(
            "/api/v1/music/songs/{id}/lyrics/by_hand",
            axum::routing::put(write_by_hand).delete(set_by_hand_aside),
        )
        .route("/api/v1/music/songs/{id}/lyrics/versions", get(versions))
        .route(
            "/api/v1/music/songs/{id}/lyrics/versions/{version}",
            axum::routing::put(use_version).delete(delete_version),
        )
}

/// One entry of LRCLIB offered for a song, as the window lists it.
#[derive(Debug, Serialize)]
struct OfferView {
    id: i64,
    artist: String,
    title: String,
    album: Option<String>,
    /// How long the song it was stamped on lasts.
    duration_ms: Option<i64>,
    /// Stamped line by line, so it follows the song.
    synced: bool,
    /// Whether it has words at all.
    plain: bool,
    instrumental: bool,
    /// The lines that carry a moment, and the longest stretch without one in
    /// seconds: where stamped words leave the song running with nothing lit.
    synced_lines: u32,
    longest_gap_seconds: Option<u32>,
}

impl From<Offer> for OfferView {
    fn from(offer: Offer) -> Self {
        Self {
            id: offer.id,
            artist: offer.artist,
            title: offer.title,
            album: offer.album,
            duration_ms: offer.duration_ms,
            synced: offer.synced,
            plain: offer.plain,
            instrumental: offer.instrumental,
            synced_lines: offer.synced_lines,
            longest_gap_seconds: offer.longest_gap_seconds,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Asked {
    #[serde(default)]
    artist: String,
    title: String,
}

async fn offers(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Query(asked): Query<Asked>,
) -> Result<Json<Vec<OfferView>>> {
    let offers =
        melyxar_app::music::lyrics::offers(&state, &who, parse_work(&id)?, &asked.artist, &asked.title).await?;
    Ok(Json(offers.into_iter().map(OfferView::from).collect()))
}

#[derive(Debug, Deserialize)]
struct Chosen {
    /// The identifier of the entry, as LRCLIB numbers it.
    entry: i64,
}

async fn choose(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(chosen): Json<Chosen>,
) -> Result<Json<()>> {
    melyxar_app::music::lyrics::choose(&state, &who, parse_work(&id)?, chosen.entry).await?;
    Ok(Json(()))
}

async fn forget(State(state): State<AppState>, Viewer(who): Viewer, Path(id): Path<String>) -> Result<Json<()>> {
    melyxar_app::music::lyrics::forget(&state, &who, parse_work(&id)?).await?;
    Ok(Json(()))
}

#[derive(Debug, Serialize)]
struct StartedView {
    job_id: String,
}

/// Starts listening to the song to line its lines up with it.
async fn synchronise(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<StartedView>> {
    let job = melyxar_app::music::lyrics_sync::start(&state, &who, parse_work(&id)?).await?;
    Ok(Json(StartedView { job_id: job.id.to_string() }))
}

async fn unsynchronise(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<()>> {
    melyxar_app::music::lyrics_sync::forget(&state, &who, parse_work(&id)?).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct OffsetAsked {
    /// How much later every line is shown, sooner when negative.
    offset_ms: i64,
}

/// Moves every line of a song by hand, for every account.
async fn set_offset(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(asked): Json<OffsetAsked>,
) -> Result<Json<()>> {
    melyxar_app::music::lyrics::set_offset(&state, &who, parse_work(&id)?, asked.offset_ms).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct WrittenLine {
    at_ms: i64,
    text: String,
}

#[derive(Debug, Deserialize)]
struct Written {
    /// The words, kept when no line has a moment.
    #[serde(default)]
    plain: String,
    /// The lines with their moments, in any order.
    #[serde(default)]
    lines: Vec<WrittenLine>,
}

/// Keeps the words written in the editor, for every account and before any
/// other.
async fn write_by_hand(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(written): Json<Written>,
) -> Result<Json<()>> {
    let lines = written
        .lines
        .into_iter()
        .map(|line| SyncedLine { at: Millis::new(line.at_ms), text: line.text })
        .collect();
    melyxar_app::music::lyrics::write_by_hand(&state, &who, parse_work(&id)?, &written.plain, lines).await?;
    Ok(Json(()))
}

/// Sets the words written by hand aside: every version stays.
async fn set_by_hand_aside(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<()>> {
    melyxar_app::music::lyrics::set_by_hand_aside(&state, &who, parse_work(&id)?).await?;
    Ok(Json(()))
}

/// One version of the lyrics written by hand, as the list shows it.
#[derive(Debug, Serialize)]
struct VersionView {
    id: i64,
    written_at: String,
    in_use: bool,
    lines: u32,
    /// Whether its lines carry their moments.
    stamped: bool,
    opening: String,
}

async fn versions(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<Vec<VersionView>>> {
    let versions = melyxar_app::music::lyrics::versions(&state, &who, parse_work(&id)?).await?;
    Ok(Json(
        versions
            .into_iter()
            .map(|version| VersionView {
                id: version.id,
                written_at: melyxar_core::time::to_text(version.written_at),
                in_use: version.in_use,
                lines: version.lines,
                stamped: version.stamped,
                opening: version.opening,
            })
            .collect(),
    ))
}

/// Puts a version back in service.
async fn use_version(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path((id, version)): Path<(String, i64)>,
) -> Result<Json<()>> {
    melyxar_app::music::lyrics::use_version(&state, &who, parse_work(&id)?, version).await?;
    Ok(Json(()))
}

/// Deletes a version for good.
async fn delete_version(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path((id, version)): Path<(String, i64)>,
) -> Result<Json<()>> {
    melyxar_app::music::lyrics::delete_version(&state, &who, parse_work(&id)?, version).await?;
    Ok(Json(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }
}
