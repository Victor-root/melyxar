//! The lyrics of a song chosen by hand, for an administrator: what LRCLIB
//! holds under an artist and a title, taking one of its entries as the words
//! of a song, forgetting what was taken or found so it is looked up again, and
//! lining the stamps of the lines up with where the voice starts in the song.
//!
//! Reading the lyrics of a song is `music.rs`; this is only for changing them.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_app::music::lyrics::Offer;
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
}

/// One entry of LRCLIB offered for a song, as the window lists it.
#[derive(Debug, Serialize)]
struct OfferView {
    id: i64,
    artist: String,
    title: String,
    album: Option<String>,
    seconds: Option<u64>,
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
            seconds: offer.seconds,
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

/// What came of lining the lines up with the song.
#[derive(Debug, Serialize)]
struct SynchronisedView {
    /// aligned, already_fits, not_sure, too_few_lines, not_stamped or too_long.
    conclusion: &'static str,
    shift_ms: i64,
    lines: u32,
    lines_moved: u32,
    confidence: f32,
}

async fn synchronise(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<SynchronisedView>> {
    let outcome = melyxar_app::music::lyrics_sync::synchronise(&state, &who, parse_work(&id)?).await?;
    Ok(Json(SynchronisedView {
        conclusion: outcome.conclusion.as_str(),
        shift_ms: outcome.shift_ms,
        lines: outcome.lines,
        lines_moved: outcome.lines_moved,
        confidence: outcome.confidence,
    }))
}

async fn unsynchronise(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<()>> {
    melyxar_app::music::lyrics_sync::forget(&state, &who, parse_work(&id)?).await?;
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
