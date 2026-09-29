//! Each account's playlists of songs: listed, opened, made, renamed,
//! deleted, added to and put in order, always those of whoever asks.

use axum::extract::{Path, State};
use axum::routing::{get, put};
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_app::music::playlists::MusicPlaylistSummary;
use serde::{Deserialize, Serialize};

use crate::account::Viewer;
use crate::catalogue::{ImageView, image_view};
use crate::error::{Result, ServerError};
use crate::identifiers::{parse_playlist, parse_works};
use crate::music::{SongView, song_view};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/music/playlists", get(every_one).post(create))
        .route("/api/v1/music/playlists/{id}", get(one).delete(delete))
        .route("/api/v1/music/playlists/{id}/name", put(rename))
        .route("/api/v1/music/playlists/{id}/songs", put(set).post(add))
}

#[derive(Debug, Serialize)]
struct SummaryView {
    id: String,
    name: String,
    songs: i64,
    seconds: i64,
    cover: Vec<ImageView>,
}

impl From<&MusicPlaylistSummary> for SummaryView {
    fn from(summary: &MusicPlaylistSummary) -> Self {
        Self {
            id: summary.id.to_string(),
            name: summary.name.clone(),
            songs: summary.songs,
            seconds: (summary.duration_ms + 500) / 1_000,
            cover: summary.cover.iter().map(image_view).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct PlaylistView {
    id: String,
    name: String,
    /// Named apart from how many there are, as on an album.
    tracks: Vec<SongView>,
}

async fn every_one(
    State(state): State<AppState>,
    Viewer(who): Viewer,
) -> Result<Json<Vec<SummaryView>>> {
    let all = melyxar_app::music::playlists::every_one(&state, &who).await?;
    Ok(Json(all.iter().map(SummaryView::from).collect()))
}

async fn one(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<PlaylistView>> {
    let (name, songs) = melyxar_app::music::playlists::one(&state, &who, parse_playlist(&id)?)
        .await?
        .ok_or_else(|| ServerError::not_found("no such playlist"))?;
    Ok(Json(PlaylistView {
        id,
        name,
        tracks: songs.iter().map(song_view).collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct Made {
    name: String,
    #[serde(default)]
    songs: Vec<String>,
}

#[derive(Debug, Serialize)]
struct MadeView {
    id: String,
}

async fn create(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Json(made): Json<Made>,
) -> Result<Json<MadeView>> {
    let id =
        melyxar_app::music::playlists::create(&state, &who, &made.name, &parse_works(&made.songs)?)
            .await?;
    Ok(Json(MadeView { id: id.to_string() }))
}

#[derive(Debug, Deserialize)]
struct Renamed {
    name: String,
}

async fn rename(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(renamed): Json<Renamed>,
) -> Result<Json<()>> {
    melyxar_app::music::playlists::rename(&state, &who, parse_playlist(&id)?, &renamed.name)
        .await?;
    Ok(Json(()))
}

async fn delete(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<()>> {
    melyxar_app::music::playlists::delete(&state, &who, parse_playlist(&id)?).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct Songs {
    songs: Vec<String>,
}

async fn add(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(asked): Json<Songs>,
) -> Result<Json<()>> {
    melyxar_app::music::playlists::add(
        &state,
        &who,
        parse_playlist(&id)?,
        &parse_works(&asked.songs)?,
    )
    .await?;
    Ok(Json(()))
}

async fn set(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(asked): Json<Songs>,
) -> Result<Json<()>> {
    melyxar_app::music::playlists::set(
        &state,
        &who,
        parse_playlist(&id)?,
        &parse_works(&asked.songs)?,
    )
    .await?;
    Ok(Json(()))
}
