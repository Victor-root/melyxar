//! The routes of the tag manager: what the songs of an album carry in their
//! files, what writing would change, writing it, an album's cover, and a
//! song cut short at either end, drawn as its wave to cut it on.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderName, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_app::music::tag_editing::{EditedTags, Planned, SongTags, Wanted, Wave};
use melyxar_core::time::Millis;
use serde::{Deserialize, Serialize};

use crate::account::Viewer;
use crate::error::Result;
use crate::identifiers::parse_work;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/music/albums/{id}/tags", get(album_tags))
        .route("/api/v1/music/tags/preview", post(preview))
        .route("/api/v1/music/tags/write", post(write))
        .route("/api/v1/music/songs/{id}/trim", post(trim))
        .route("/api/v1/music/songs/{id}/wave", get(wave))
        .route(
            "/api/v1/music/albums/{id}/cover",
            // A picture straight off a phone, the same as a profile picture.
            put(cover).layer(axum::extract::DefaultBodyLimit::max(
                melyxar_app::avatars::LARGEST,
            )),
        )
}

/// The tags of one song, as they are sent both ways.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct TagsView {
    title: Option<String>,
    #[serde(default)]
    artists: Vec<String>,
    album: Option<String>,
    #[serde(default)]
    album_artists: Vec<String>,
    track: Option<u32>,
    disc: Option<u32>,
    year: Option<i32>,
    #[serde(default)]
    genres: Vec<String>,
    #[serde(default)]
    compilation: bool,
}

impl From<&EditedTags> for TagsView {
    fn from(tags: &EditedTags) -> Self {
        Self {
            title: tags.title.clone(),
            artists: tags.artists.clone(),
            album: tags.album.clone(),
            album_artists: tags.album_artists.clone(),
            track: tags.track,
            disc: tags.disc,
            year: tags.year,
            genres: tags.genres.clone(),
            compilation: tags.compilation,
        }
    }
}

impl From<TagsView> for EditedTags {
    fn from(view: TagsView) -> Self {
        Self {
            title: view.title,
            artists: view.artists,
            album: view.album,
            album_artists: view.album_artists,
            track: view.track,
            disc: view.disc,
            year: view.year,
            genres: view.genres,
            compilation: view.compilation,
        }
    }
}

#[derive(Debug, Serialize)]
struct SongTagsView {
    song: String,
    file_name: String,
    tags: TagsView,
    /// How long it plays, in milliseconds, when known.
    duration_ms: Option<i64>,
}

impl From<&SongTags> for SongTagsView {
    fn from(found: &SongTags) -> Self {
        Self {
            song: found.song.to_string(),
            file_name: found.file_name.clone(),
            tags: TagsView::from(&found.tags),
            duration_ms: found.duration.map(Millis::get),
        }
    }
}

async fn album_tags(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<Vec<SongTagsView>>> {
    let found = melyxar_app::music::tag_editing::album_tags(&state, &who, parse_work(&id)?).await?;
    Ok(Json(found.iter().map(SongTagsView::from).collect()))
}

#[derive(Debug, Deserialize)]
struct WantedView {
    song: String,
    tags: TagsView,
}

#[derive(Debug, Deserialize)]
struct Asked {
    songs: Vec<WantedView>,
    /// How to name the files from their tags, or nothing to leave the names.
    pattern: Option<String>,
    #[serde(default)]
    keep_a_copy: bool,
}

impl Asked {
    fn wanted(&self) -> Result<Vec<Wanted>> {
        self.songs
            .iter()
            .map(|one| {
                Ok(Wanted {
                    song: parse_work(&one.song)?,
                    tags: EditedTags::from(one.tags.clone()),
                })
            })
            .collect()
    }
}

#[derive(Debug, Serialize)]
struct PlannedView {
    song: String,
    file_name: String,
    new_file_name: Option<String>,
    changed: Vec<&'static str>,
    before: TagsView,
    after: TagsView,
}

impl From<&Planned> for PlannedView {
    fn from(planned: &Planned) -> Self {
        Self {
            song: planned.song.to_string(),
            file_name: planned.file_name.clone(),
            new_file_name: planned.new_file_name.clone(),
            changed: planned.changed.clone(),
            before: TagsView::from(&planned.before),
            after: TagsView::from(&planned.after),
        }
    }
}

async fn preview(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Json(asked): Json<Asked>,
) -> Result<Json<Vec<PlannedView>>> {
    let planned = melyxar_app::music::tag_editing::plan(
        &state,
        &who,
        &asked.wanted()?,
        asked.pattern.as_deref(),
    )
    .await?;
    Ok(Json(planned.iter().map(PlannedView::from).collect()))
}

#[derive(Debug, Serialize)]
struct FailedView {
    song: String,
    reason: String,
}

#[derive(Debug, Serialize)]
struct WrittenView {
    written: usize,
    failed: Vec<FailedView>,
}

async fn write(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Json(asked): Json<Asked>,
) -> Result<Json<WrittenView>> {
    let written = melyxar_app::music::tag_editing::write(
        &state,
        &who,
        &asked.wanted()?,
        asked.pattern.as_deref(),
        asked.keep_a_copy,
    )
    .await?;
    Ok(Json(WrittenView {
        written: written.written,
        failed: written
            .failed
            .into_iter()
            .map(|(song, reason)| FailedView {
                song: song.to_string(),
                reason,
            })
            .collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct CoverQuery {
    #[serde(default)]
    keep_a_copy: bool,
}

async fn cover(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Query(query): Query<CoverQuery>,
    image: axum::body::Bytes,
) -> Result<Json<()>> {
    melyxar_app::music::tag_editing::set_cover(
        &state,
        &who,
        parse_work(&id)?,
        image.to_vec(),
        query.keep_a_copy,
    )
    .await?;
    Ok(Json(()))
}

/// Where a song is cut, in milliseconds from its start.
#[derive(Debug, Deserialize)]
struct TrimAsked {
    from_ms: i64,
    to_ms: i64,
    #[serde(default)]
    keep_a_copy: bool,
}

async fn trim(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(asked): Json<TrimAsked>,
) -> Result<Json<()>> {
    melyxar_app::music::tag_editing::trim(
        &state,
        &who,
        parse_work(&id)?,
        Millis::new(asked.from_ms),
        Millis::new(asked.to_ms),
        asked.keep_a_copy,
    )
    .await?;
    Ok(Json(()))
}

/// How loud a song is along its length, a byte a moment, with how many
/// moments a second in a header.
async fn wave(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Response> {
    let wave = melyxar_app::music::tag_editing::wave(&state, &who, parse_work(&id)?).await?;
    Ok(wave_reply(wave))
}

/// Never kept by the browser: the file changes each time the song is cut.
fn wave_reply(wave: Wave) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CACHE_CONTROL, "no-store".to_string()),
            (
                HeaderName::from_static("x-wave-moments-a-second"),
                wave.moments_a_second.to_string(),
            ),
        ],
        wave.levels,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_wave_goes_out_as_its_levels_never_kept_by_the_browser() {
        let reply = wave_reply(Wave {
            moments_a_second: 100,
            levels: vec![0, 128, 255],
        });
        assert_eq!(reply.headers()["x-wave-moments-a-second"], "100");
        assert_eq!(reply.headers()[header::CACHE_CONTROL], "no-store");
        let body = axum::body::to_bytes(reply.into_body(), usize::MAX)
            .await
            .expect("a body");
        assert_eq!(body.as_ref(), [0, 128, 255]);
    }
}
