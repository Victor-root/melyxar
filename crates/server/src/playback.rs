//! Playing: what the answer is, and the bytes themselves.
//!
//! Two routes. One says how a film reaches this client and where the viewer
//! stopped, with the reasons behind the answer so a page can explain itself.
//! The other hands over the file.
//!
//! Handing it over is done by `tower-http`, which already speaks the part of
//! the protocol that matters here: a player asks for the stretch of the film
//! it is about to show rather than the whole thing, so moving the cursor to
//! the last ten minutes fetches the last ten minutes. Writing that by hand
//! would mean writing an entire specification by hand.

use axum::body::Body;
use axum::extract::{Path as RoutePath, State};
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use melyxar_app::playback::{ClientProfile, PlayPlan, PlayRequest, Session, StreamingError};
use melyxar_app::AppState;
use melyxar_core::id::{MediaSourceId, TrackId, UserId, WorkId};
use melyxar_core::media::TrackKind;
use melyxar_core::time::{Millis, Timestamp};
use serde::{Deserialize, Serialize};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/playback/{id}/plan", axum::routing::post(plan))
        .route("/api/v1/playback/{id}/stream", axum::routing::get(stream))
        .route(
            "/api/v1/playback/progress",
            axum::routing::post(record_progress),
        )
        .route(
            "/api/v1/playback/tracks",
            axum::routing::post(remember_tracks),
        )
        .route(
            "/api/v1/playback/{id}/session",
            axum::routing::post(open_session),
        )
        // One route for the three things a session hands out. A router allows
        // one name per part of a path, and a segment is named after its
        // number in a way a player builds itself from the playlist.
        .route(
            "/api/v1/stream/{session}/{file}",
            axum::routing::get(session_file),
        )
        .route(
            "/api/v1/stream/{session}",
            axum::routing::delete(close_session),
        )
}

// ---------------------------------------------------------------------------
// The answer
// ---------------------------------------------------------------------------

/// What the client says it can open, and what the viewer chose.
#[derive(Debug, Default, Deserialize)]
struct PlanBody {
    #[serde(default)]
    profile: Option<ClientProfile>,
    #[serde(default)]
    audio_track_id: Option<String>,
    #[serde(default)]
    subtitle_track_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct PlanView {
    /// Where to fetch the film itself.
    url: String,
    /// The tracks this answer was worked out for, whether the viewer chose
    /// them, they were remembered, or the file decided. A page shows them as
    /// the current choice rather than guessing which one that is.
    chosen_audio_id: Option<String>,
    chosen_subtitle_id: Option<String>,
    /// direct_play, remux, transcode_audio or full_transcode.
    method: &'static str,
    /// Whether anything has to be decoded, which is what a limit applies to.
    expensive: bool,
    /// Every reason behind the answer, as codes a page turns into sentences.
    reasons: Vec<serde_json::Value>,
    duration_minutes: Option<i64>,
    /// Where this viewer stopped last time, in seconds, when they did.
    resume_from_seconds: Option<f64>,
    audio: Vec<TrackView>,
    subtitles: Vec<TrackView>,
}

#[derive(Debug, Serialize)]
struct TrackView {
    id: String,
    language: Option<String>,
    /// What the file itself calls this track, when it says.
    title: Option<String>,
    codec: String,
    is_default: bool,
    /// Audio only.
    channels: Option<i32>,
    /// Subtitles only: showing it means rebuilding the picture.
    burns_in: Option<bool>,
}

async fn plan(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<String>,
    body: Option<Json<PlanBody>>,
) -> Result<Json<PlanView>> {
    let source_id = parse_source(&id)?;
    let body = body.map(|Json(body)| body).unwrap_or_default();

    let request = PlayRequest {
        source_id,
        profile: body.profile,
        audio_track_id: body
            .audio_track_id
            .as_deref()
            .map(parse_track)
            .transpose()?,
        subtitle_track_id: body
            .subtitle_track_id
            .as_deref()
            .map(parse_track)
            .transpose()?,
    };

    let plan = melyxar_app::playback::plan(&state, viewer(&state).await?, &request).await?;
    Ok(Json(plan_view(&plan)))
}

fn plan_view(plan: &PlayPlan) -> PlanView {
    let mut audio = Vec::new();
    let mut subtitles = Vec::new();
    for track in &plan.tracks {
        match &track.kind {
            TrackKind::Audio(details) => audio.push(TrackView {
                id: track.id.to_string(),
                language: track.language.clone(),
                title: track.title.clone(),
                codec: details.codec.clone(),
                is_default: track.is_default,
                channels: Some(details.channels),
                burns_in: None,
            }),
            TrackKind::Subtitle(details) => subtitles.push(TrackView {
                id: track.id.to_string(),
                language: track.language.clone(),
                title: track.title.clone(),
                codec: details.codec.clone(),
                is_default: track.is_default,
                channels: None,
                burns_in: Some(details.forces_full_transcode()),
            }),
            TrackKind::Video(_) => {}
        }
    }

    let chosen = |index: Option<i32>| -> Option<String> {
        let index = index?;
        plan.tracks
            .iter()
            .find(|track| track.stream_index == index)
            .map(|track| track.id.to_string())
    };

    PlanView {
        url: format!("/api/v1/playback/{}/stream", plan.source_id),
        chosen_audio_id: chosen(plan.decision.audio_stream_index),
        chosen_subtitle_id: chosen(plan.decision.subtitle_stream_index),
        method: plan.decision.method.as_str(),
        expensive: plan.decision.method.is_expensive(),
        reasons: plan
            .decision
            .reasons
            .iter()
            .map(|reason| serde_json::to_value(reason).unwrap_or(serde_json::Value::Null))
            .collect(),
        duration_minutes: plan.duration.map(whole_minutes),
        // Seconds rather than milliseconds: it is what a player is set to.
        resume_from_seconds: plan.resume_from.map(|position| position.as_seconds_f64()),
        audio,
        subtitles,
    }
}

fn whole_minutes(duration: Millis) -> i64 {
    (duration.get() + 30_000) / 60_000
}

// ---------------------------------------------------------------------------
// The film itself
// ---------------------------------------------------------------------------

/// Hands over the file.
///
/// Only files this server recorded are served, and only by the identifier it
/// gave them: nothing a client sends is ever treated as a path.
async fn stream(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<String>,
    request: Request<Body>,
) -> Response {
    match serve_file(&state, &id, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn serve_file(state: &AppState, id: &str, request: Request<Body>) -> Result<Response> {
    let source_id = parse_source(id)?;
    let source = state
        .database()
        .playable_source(source_id)
        .await
        .map_err(|error| ServerError::internal(error.to_string()))?
        .ok_or_else(|| ServerError::not_found("no file with that identifier"))?;

    if source.missing {
        return Err(ServerError::not_found(
            "the file is not on the disk at the moment",
        ));
    }

    ServeFile::new(&source.path)
        .oneshot(request)
        .await
        .map(IntoResponse::into_response)
        .map_err(|error| ServerError::internal(error.to_string()))
}

// ---------------------------------------------------------------------------
// A film being converted as it is watched
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct SessionView {
    id: String,
    /// What a player is pointed at. The playlist lists the whole film before
    /// any of it has been produced, so a viewer can jump anywhere at once.
    playlist_url: String,
    duration_minutes: Option<i64>,
    /// Where this viewer stopped last time, in seconds, when they did.
    resume_from_seconds: Option<f64>,
}

/// Opens a session for a film this client cannot play as it is.
async fn open_session(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<String>,
    body: Option<Json<PlanBody>>,
) -> Result<Json<SessionView>> {
    let source_id = parse_source(&id)?;
    let body = body.map(|Json(body)| body).unwrap_or_default();

    let plan = melyxar_app::playback::plan(
        &state,
        viewer(&state).await?,
        &PlayRequest {
            source_id,
            profile: body.profile,
            audio_track_id: body
                .audio_track_id
                .as_deref()
                .map(parse_track)
                .transpose()?,
            subtitle_track_id: body
                .subtitle_track_id
                .as_deref()
                .map(parse_track)
                .transpose()?,
        },
    )
    .await?;

    let session = melyxar_app::playback::open_session(&state, &plan).await?;
    Ok(Json(SessionView {
        id: session.id.to_string(),
        playlist_url: format!("/api/v1/stream/{}/playlist.m3u8", session.id),
        duration_minutes: plan.duration.map(whole_minutes),
        resume_from_seconds: plan.resume_from.map(|position| position.as_seconds_f64()),
    }))
}

/// What a player is asking a session for.
#[derive(Debug, PartialEq, Eq)]
enum Wanted {
    /// The list of every segment, which the server writes.
    Playlist,
    /// The header every segment needs.
    Header,
    /// One segment, by its number.
    Segment(u32),
}

/// Reads the name a player asked for.
///
/// Only the three shapes a session produces are recognised. Anything else is
/// refused rather than looked for: this is a name from outside, and the only
/// safe way to use one is to not use it at all.
fn wanted_from(name: &str) -> Option<Wanted> {
    match name {
        "playlist.m3u8" => Some(Wanted::Playlist),
        "init.mp4" => Some(Wanted::Header),
        other => other
            .strip_prefix("segment-")
            .and_then(|rest| rest.strip_suffix(".m4s"))
            .and_then(|number| number.parse().ok())
            .map(Wanted::Segment),
    }
}

async fn session_file(
    State(state): State<AppState>,
    RoutePath((id, name)): RoutePath<(String, String)>,
    request: Request<Body>,
) -> Response {
    match wanted_from(&name) {
        Some(Wanted::Playlist) => playlist(&state, &id).await,
        Some(Wanted::Header) => header_file(&state, &id, request).await,
        Some(Wanted::Segment(index)) => segment(&state, &id, index, request).await,
        None => ServerError::not_found("a session hands out nothing by that name").into_response(),
    }
}

/// The playlist, which the server writes and the tool never sees.
async fn playlist(state: &AppState, id: &str) -> Response {
    match live_session(state, id).await {
        Ok(session) => (
            StatusCode::OK,
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("application/vnd.apple.mpegurl"),
                ),
                // The film is already cut: the list never changes, but it
                // belongs to a session that will not outlive the evening.
                (header::CACHE_CONTROL, HeaderValue::from_static("no-store")),
            ],
            session.playlist().to_text(),
        )
            .into_response(),
        Err(error) => error.into_response(),
    }
}

/// The header every segment needs.
async fn header_file(state: &AppState, id: &str, request: Request<Body>) -> Response {
    match live_session(state, id).await {
        Ok(session) => match session.initialisation().await {
            Ok(path) => serve(path, request, "video/mp4").await,
            Err(error) => streaming_failure(error).into_response(),
        },
        Err(error) => error.into_response(),
    }
}

/// One segment of the film, produced now if it is not there yet.
async fn segment(state: &AppState, id: &str, index: u32, request: Request<Body>) -> Response {
    match live_session(state, id).await {
        Ok(session) => match session.segment(index).await {
            Ok(path) => serve(path, request, "video/iso.segment").await,
            Err(error) => streaming_failure(error).into_response(),
        },
        Err(error) => error.into_response(),
    }
}

/// Closes a session, which a player asks for when it is done with a film.
async fn close_session(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<String>,
) -> Result<Json<serde_json::Value>> {
    let session_id = id
        .parse()
        .map_err(|_| ServerError::invalid_input("the session identifier is malformed"))?;
    if let Some(sessions) = state.sessions() {
        sessions.close(session_id).await;
    }
    Ok(Json(serde_json::json!({ "closed": true })))
}

async fn live_session(state: &AppState, id: &str) -> Result<std::sync::Arc<Session>> {
    let session_id = id
        .parse()
        .map_err(|_| ServerError::invalid_input("the session identifier is malformed"))?;
    let sessions = state
        .sessions()
        .ok_or_else(|| ServerError::not_found("this server converts nothing"))?;
    sessions.get(session_id).await.map_err(streaming_failure)
}

/// Turns a streaming failure into something a client can act on.
fn streaming_failure(error: StreamingError) -> ServerError {
    use StreamingError as Failure;
    match error {
        Failure::NoSuchSession => {
            // The session was swept away while nobody was watching. A player
            // that comes back asks for a new one rather than failing outright.
            ServerError::not_found("that session is over")
        }
        Failure::NoSuchSegment => ServerError::not_found("that segment is not part of this film"),
        Failure::TooManyAtOnce => {
            ServerError::busy("this server is already converting all it can at once")
        }
        other => ServerError::internal(other.to_string()),
    }
}

/// Hands over a file the session produced.
async fn serve(path: std::path::PathBuf, request: Request<Body>, kind: &str) -> Response {
    let mut response = match ServeFile::new(&path).oneshot(request).await {
        Ok(response) => response.into_response(),
        Err(error) => return ServerError::internal(error.to_string()).into_response(),
    };
    if let Ok(value) = HeaderValue::from_str(kind) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    // A segment belongs to one session and outlives nothing.
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

// ---------------------------------------------------------------------------
// Where the viewer got to
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct ProgressBody {
    work_id: String,
    position_seconds: f64,
    /// When the client measured it, as a browser writes a date. A report
    /// arriving after a fresher one is refused, so a client that was away
    /// cannot make the point go backwards.
    #[serde(default, with = "time::serde::rfc3339::option")]
    reported_at: Option<Timestamp>,
}

#[derive(Debug, Serialize)]
struct ProgressView {
    /// False when a fresher report was already here, which is not a failure.
    kept: bool,
}

async fn record_progress(
    State(state): State<AppState>,
    Json(body): Json<ProgressBody>,
) -> Result<Json<ProgressView>> {
    let work_id: WorkId = body
        .work_id
        .parse()
        .map_err(|_| ServerError::invalid_input("the work identifier is malformed"))?;

    let kept = melyxar_app::playback::record_position(
        &state,
        viewer(&state).await?,
        work_id,
        Millis::from_seconds_f64(body.position_seconds),
        body.reported_at.unwrap_or_else(melyxar_core::time::now),
    )
    .await?;

    Ok(Json(ProgressView { kept }))
}

#[derive(Debug, Deserialize)]
struct TracksBody {
    work_id: String,
    source_id: String,
    #[serde(default)]
    audio_track_id: Option<String>,
    #[serde(default)]
    subtitle_track_id: Option<String>,
}

/// Remembers what a viewer chose, so the next time starts the same way.
async fn remember_tracks(
    State(state): State<AppState>,
    Json(body): Json<TracksBody>,
) -> Result<Json<serde_json::Value>> {
    let work_id: WorkId = body
        .work_id
        .parse()
        .map_err(|_| ServerError::invalid_input("the work identifier is malformed"))?;
    let source_id = parse_source(&body.source_id)?;

    // The tracks are read back from the file rather than taken on trust: what
    // is remembered has to be a track this film really holds.
    let tracks = state
        .database()
        .tracks_of_source(source_id)
        .await
        .map_err(|error| ServerError::internal(error.to_string()))?;
    let find = |wanted: Option<String>| -> Result<Option<&melyxar_core::media::Track>> {
        match wanted {
            None => Ok(None),
            Some(value) => {
                let id = parse_track(&value)?;
                Ok(tracks.iter().find(|track| track.id == id))
            }
        }
    };

    melyxar_app::playback::remember_chosen_tracks(
        &state,
        viewer(&state).await?,
        work_id,
        find(body.audio_track_id)?,
        find(body.subtitle_track_id)?,
    )
    .await?;

    Ok(Json(serde_json::json!({ "remembered": true })))
}

/// Who is watching.
///
/// There is no signing in yet, so this is the account the server created for
/// itself at first start. When accounts arrive it is read from the request,
/// and nothing else here changes.
async fn viewer(state: &AppState) -> Result<UserId> {
    state
        .database()
        .user_by_name(melyxar_app::startup::DEFAULT_ACCOUNT_NAME)
        .await
        .map_err(|error| ServerError::internal(error.to_string()))?
        .map(|(user, _)| user.id)
        .ok_or_else(|| ServerError::internal("this server has no account at all"))
}

fn parse_source(value: &str) -> Result<MediaSourceId> {
    value
        .parse()
        .map_err(|_| ServerError::invalid_input("the file identifier is malformed"))
}

fn parse_track(value: &str) -> Result<TrackId> {
    value
        .parse()
        .map_err(|_| ServerError::invalid_input("the track identifier is malformed"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        // Built at start-up, so a route a router refuses brings the whole
        // server down rather than failing one request. That is exactly what
        // happened, and only when it was run.
        let _ = router();
    }

    #[test]
    fn a_session_hands_out_three_things_and_nothing_else() {
        assert_eq!(wanted_from("playlist.m3u8"), Some(Wanted::Playlist));
        assert_eq!(wanted_from("init.mp4"), Some(Wanted::Header));
        assert_eq!(wanted_from("segment-0.m4s"), Some(Wanted::Segment(0)));
        assert_eq!(wanted_from("segment-1799.m4s"), Some(Wanted::Segment(1799)));

        // A name from outside is only ever recognised, never used.
        assert_eq!(wanted_from("segment-.m4s"), None);
        assert_eq!(wanted_from("segment--1.m4s"), None);
        assert_eq!(wanted_from("segment-abc.m4s"), None);
        assert_eq!(wanted_from("../../etc/passwd"), None);
        assert_eq!(wanted_from("init.mp4.bak"), None);
        assert_eq!(wanted_from(""), None);
    }

    #[test]
    fn a_malformed_identifier_is_refused_rather_than_looked_up() {
        assert!(parse_source("not-an-identifier").is_err());
        assert!(parse_track("").is_err());
    }

    #[test]
    fn a_runtime_is_rounded_the_way_a_person_would_round_it() {
        assert_eq!(whole_minutes(Millis::new(7_190_000)), 120);
        assert_eq!(whole_minutes(Millis::new(29_000)), 0);
    }
}
