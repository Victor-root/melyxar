//! Playing: what the answer is, and the bytes themselves.
//!
//! One route says how a film reaches this client and where the viewer stopped,
//! with the reasons behind the answer so a page can explain itself. The rest
//! hand over bytes: the file as it lies on the disk, a session of segments for
//! a film this client cannot open, and the words that go on top.
//!
//! Handing a file over is done by `tower-http`, which already speaks the part
//! of the protocol that matters here: a player asks for the stretch of the
//! film it is about to show rather than the whole thing, so moving the cursor
//! to the last ten minutes fetches the last ten minutes. Writing that by hand
//! would mean writing an entire specification by hand.

use crate::identifiers::{parse_source, parse_track, parse_work};
use axum::body::Body;
use axum::extract::{Path as RoutePath, State};
use axum::http::{header, HeaderValue, Request, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use melyxar_app::playback::{
    ClientProfile, PlayPlan, PlayRequest, Preparation, Session, SEGMENT_DURATION,
};
use melyxar_app::AppState;
use melyxar_core::id::MediaSourceId;
use melyxar_core::media::TrackKind;
use melyxar_core::time::{Millis, Timestamp};
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};
use crate::viewer;

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
        .route(
            "/api/v1/playback/{id}/subtitles/{track}",
            axum::routing::get(subtitle),
        )
        .route(
            "/api/v1/playback/{id}/thumbnails/{sheet}",
            axum::routing::get(thumbnail_sheet),
        )
        // One route for everything a session hands out. A router allows one
        // name per part of a path, and a segment is named after its number in
        // a way a player builds itself from the playlist.
        .route(
            "/api/v1/stream/{session}/{file}",
            axum::routing::get(session_file),
        )
        // A viewer who has paused asks for nothing, and a session is kept
        // alive by being asked for something. This is how they say they are
        // still in front of it, and its refusal is how a page learns the
        // session is gone and opens another.
        .route(
            "/api/v1/stream/{session}",
            axum::routing::post(still_watching).delete(close_session),
        )
}

// ---------------------------------------------------------------------------
// The answer
// ---------------------------------------------------------------------------

/// What the client says it can open, and what the viewer chose.
///
/// Read into a request in one place, because the plan a page is shown and the
/// session it then opens have to decide exactly the same thing. Written out
/// twice, a field added here and wired up on one road only gives a session
/// that produces something other than what the page announced, which is the
/// one failure of this whole area that nothing downstream can recover from.
#[derive(Debug, Clone, Default, Deserialize)]
struct PlanBody {
    #[serde(default)]
    profile: Option<ClientProfile>,
    #[serde(default)]
    audio_track_id: Option<String>,
    #[serde(default)]
    subtitle_track_id: Option<String>,
    /// A codec asked for directly, forced whenever this server allows it and
    /// the card can produce it. Absent leaves the choice to the usual
    /// negotiation.
    #[serde(default)]
    preferred_video_codec: Option<String>,
}

impl PlanBody {
    /// What was asked for, with the track identifiers read.
    fn asked_for(self, source_id: MediaSourceId) -> Result<PlayRequest> {
        Ok(PlayRequest {
            source_id,
            profile: self.profile,
            audio_track_id: self
                .audio_track_id
                .as_deref()
                .map(parse_track)
                .transpose()?,
            subtitle_track_id: self
                .subtitle_track_id
                .as_deref()
                .map(parse_track)
                .transpose()?,
            preferred_video_codec: self.preferred_video_codec,
        })
    }
}

/// The same question, plus where the picture will actually be started.
///
/// Only a session cares: the answer to a plan is the same wherever the viewer
/// is. Said here so the first segment produced is the one about to be watched,
/// which is what saves producing the opening of a film nobody is at.
#[derive(Debug, Default, Deserialize)]
struct OpenBody {
    #[serde(flatten)]
    wanted: PlanBody,
    /// Absent leaves it to what the server remembers of this viewer.
    #[serde(default)]
    start_at_seconds: Option<f64>,
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
    /// How the picture is being rebuilt, when it is. Absent when nothing is.
    rebuild: Option<RebuildView>,
    /// The little pictures shown while dragging along the bar, when this film
    /// has been read for them.
    thumbnails: Option<ThumbnailsView>,
    /// Where the film changes scene, when the file names them.
    ///
    /// Marked on the bar, so that a viewer sees the shape of the film rather
    /// than one unbroken line, and so that landing on the beginning of a scene
    /// is something the bar helps with. Empty for a film whose file names
    /// none, which is most of them.
    chapters: Vec<ChapterView>,
    /// Whether this viewer has marked the film as one they like.
    favourite: bool,
    /// What the file itself holds, beside what is being made of it.
    ///
    /// Every media server shows this while a film plays, and for a reason: a
    /// film that looks or sounds wrong is one question, "what is in the file
    /// and what is being done to it", and answering it used to mean reading
    /// the journal. The two halves have to sit side by side or neither means
    /// anything.
    film: FilmView,
}

/// What the file holds, as the analyser read it.
#[derive(Debug, Serialize)]
struct FilmView {
    container: Option<String>,
    size_bytes: i64,
    /// Everything in the file, per second, streams and container alike.
    overall_bitrate: Option<i64>,
    /// The picture being played, when the file holds one.
    picture: Option<PictureView>,
    /// The soundtrack being played, which is the one chosen rather than the
    /// first: a film with four languages has four, and only one is playing.
    sound: Option<SoundView>,
}

#[derive(Debug, Serialize)]
struct PictureView {
    codec: String,
    profile: Option<String>,
    /// The picture, margins off. What a viewer means by the size of a film.
    width: i32,
    height: i32,
    /// The frame the picture sits in, said only when the film declares that
    /// part of that frame is not the picture. The two being different is the
    /// whole reason such a film is handled apart, and nothing else on a screen
    /// says so.
    frame_width: Option<i32>,
    frame_height: Option<i32>,
    frame_rate: Option<f64>,
    bitrate: Option<i64>,
    /// hdr10, hlg or dolby_vision. Absent for an ordinary picture.
    hdr: Option<&'static str>,
    bit_depth: Option<i32>,
}

#[derive(Debug, Serialize)]
struct SoundView {
    codec: String,
    channels: i32,
    channel_layout: Option<String>,
    sample_rate: Option<i32>,
    bitrate: Option<i64>,
}

/// Everything a page needs to put a thumbnail under the cursor.
///
/// The sheets are handed over whole and the page cuts them itself: one request
/// covers a hundred thumbnails, and cutting a picture is something a browser
/// does without being asked twice.
#[derive(Debug, Serialize)]
struct ThumbnailsView {
    /// Where the sheets are. The page adds a slash, the number of the sheet it
    /// wants and `.jpg`.
    url: String,
    /// How far apart in the film two of them stand.
    every_seconds: f64,
    /// Size of one thumbnail on a sheet.
    width: u32,
    height: u32,
    /// How many stand across one sheet and how many down it.
    columns: u32,
    rows: u32,
    /// How many the film has. Past the last one there is nothing: a sheet is
    /// filled to the end with black whatever the film gave.
    counted: u32,
}

/// One place the film changes scene.
#[derive(Debug, Serialize)]
struct ChapterView {
    /// Where it begins, in seconds, which is where the mark goes and what a
    /// player is set to.
    at_second: f64,
    /// What the file calls it, when it calls it anything. A film whose
    /// chapters are numbered rather than named says nothing here, and the
    /// interface writes its own wording.
    title: Option<String>,
}

/// What is rebuilding the picture, and into what.
///
/// On the page rather than in a log alone: "the card is doing this, in this
/// codec, at this size" is the answer to the only question anybody asks about
/// a film that stutters, and looking for it used to mean a terminal.
#[derive(Debug, Serialize)]
struct RebuildView {
    /// card or processor.
    ///
    /// Which card is deliberately not said: that is a fact about the machine,
    /// and it belongs in the report and the log where the administrator reads
    /// it, not in an answer handed to every viewer.
    by: &'static str,
    codec: String,
    height: Option<i32>,
    /// Rate the picture is held to, in bits per second.
    bitrate: Option<i64>,
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
    /// Where to fetch the words. Absent for a soundtrack, and for a subtitle
    /// made of pictures: there is no text in one to hand over.
    url: Option<String>,
}

async fn plan(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<String>,
    body: Option<Json<PlanBody>>,
) -> Result<Json<PlanView>> {
    let source_id = parse_source(&id)?;
    let body = body.map(|Json(body)| body).unwrap_or_default();

    let request = body.asked_for(source_id)?;

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
                url: None,
            }),
            TrackKind::Subtitle(details) => subtitles.push(TrackView {
                id: track.id.to_string(),
                language: track.language.clone(),
                title: track.title.clone(),
                codec: details.codec.clone(),
                is_default: track.is_default,
                channels: None,
                burns_in: Some(details.forces_full_transcode()),
                url: (!details.forces_full_transcode()).then(|| {
                    format!(
                        "/api/v1/playback/{}/subtitles/{}.vtt",
                        plan.source_id, track.id
                    )
                }),
            }),
            TrackKind::Video(_) => {}
        }
    }

    // Looked up by kind as well as by number. A subtitle sitting in a file of
    // its own is stream zero of that file, which is also the number of the
    // picture: matching on the number alone hands back the picture and leaves
    // the viewer with subtitles that never appear.
    let chosen = |index: Option<i32>, subtitle: bool| -> Option<String> {
        let index = index?;
        plan.tracks
            .iter()
            .find(|track| {
                track.stream_index == index
                    && matches!(track.kind, TrackKind::Subtitle(_)) == subtitle
            })
            .map(|track| track.id.to_string())
    };

    PlanView {
        url: format!("/api/v1/playback/{}/stream", plan.source_id),
        chosen_audio_id: chosen(plan.decision.audio_stream_index, false),
        chosen_subtitle_id: chosen(plan.decision.subtitle_stream_index, true),
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
        thumbnails: plan
            .thumbnails
            .filter(|made| made.counted > 0)
            .map(|made| ThumbnailsView {
                url: format!("/api/v1/playback/{}/thumbnails", plan.source_id),
                every_seconds: made.every.as_seconds_f64(),
                width: made.width,
                height: made.height,
                columns: made.columns,
                rows: made.rows,
                counted: made.counted,
            }),
        chapters: plan
            .chapters
            .iter()
            .map(|chapter| ChapterView {
                at_second: chapter.start.as_seconds_f64(),
                title: chapter.title.clone(),
            })
            .collect(),
        favourite: plan.favourite,
        rebuild: plan.rebuild.as_ref().map(|rebuild| RebuildView {
            by: match rebuild.on_a_card() {
                true => "card",
                false => "processor",
            },
            codec: rebuild.codec.clone(),
            height: rebuild.height,
            bitrate: rebuild.bitrate,
        }),
        film: film_view(plan),
    }
}

/// What the file holds, beside what is being made of it.
///
/// The soundtrack shown is the one being played rather than the first in the
/// file: a film carrying four languages holds four, and three of them have
/// nothing to do with what a viewer is hearing.
fn film_view(plan: &PlayPlan) -> FilmView {
    let picture = plan.tracks.iter().find_map(|track| match &track.kind {
        TrackKind::Video(details) => Some(PictureView {
            codec: details.codec.clone(),
            profile: details.profile.clone(),
            width: details.visible_width(),
            height: details.visible_height(),
            // Said only when the film declares that part of its frame is not
            // the picture. The two being different is the whole reason such a
            // film is handled apart, and nothing else on a screen says so.
            frame_width: details.margins.map(|_| details.width),
            frame_height: details.margins.map(|_| details.height),
            frame_rate: details.frame_rate,
            bitrate: details.bitrate,
            hdr: details.hdr.map(|hdr| hdr.as_word()),
            bit_depth: details.color.bit_depth,
        }),
        _ => None,
    });

    let sound = plan
        .tracks
        .iter()
        .find(|track| {
            matches!(track.kind, TrackKind::Audio(_))
                && plan
                    .decision
                    .audio_stream_index
                    .is_some_and(|index| track.stream_index == index)
        })
        .and_then(|track| match &track.kind {
            TrackKind::Audio(details) => Some(SoundView {
                codec: details.codec.clone(),
                channels: details.channels,
                channel_layout: details.channel_layout.clone(),
                sample_rate: details.sample_rate,
                bitrate: details.bitrate,
            }),
            _ => None,
        });

    FilmView {
        container: plan.container.clone(),
        size_bytes: plan.size_bytes,
        overall_bitrate: plan.overall_bitrate,
        picture,
        sound,
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

    crate::serve_the_file(&source.path, request).await
}

// ---------------------------------------------------------------------------
// The words on top of it
// ---------------------------------------------------------------------------

/// Hands over one subtitle track in the form a browser draws.
///
/// Converted on the first request and read from the cache afterwards, so a
/// viewer turning subtitles on waits once and nobody waits again.
async fn subtitle(
    State(state): State<AppState>,
    RoutePath((id, track)): RoutePath<(String, String)>,
    request: Request<Body>,
) -> Response {
    match serve_subtitle(&state, &id, &track, request).await {
        Ok(response) => response,
        // Not written down again here: every way this refuses already says so
        // for itself, with the codec and the stream it was about, which is
        // what a refusal has to carry to be worth reading.
        Err(error) => error.into_response(),
    }
}

async fn serve_subtitle(
    state: &AppState,
    id: &str,
    track: &str,
    request: Request<Body>,
) -> Result<Response> {
    let source_id = parse_source(id)?;
    // The name a client sends carries the suffix a browser expects to see on
    // the address; what it names is an identifier, never a path.
    let track_id = parse_track(track.strip_suffix(".vtt").unwrap_or(track))?;

    let path = melyxar_app::subtitles::as_web_vtt(state, source_id, track_id).await?;
    let mut response = crate::serve_the_file(&path, request).await?;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/vtt; charset=utf-8"),
    );
    Ok(response)
}

// ---------------------------------------------------------------------------
// The little pictures of the bar
// ---------------------------------------------------------------------------

/// Hands over one sheet of thumbnails.
///
/// Made by a background pass of the scan, never here: reading a film for them
/// takes minutes, and a request that would take minutes is a request nobody
/// should be able to make.
async fn thumbnail_sheet(
    State(state): State<AppState>,
    RoutePath((id, sheet)): RoutePath<(String, String)>,
    request: Request<Body>,
) -> Response {
    match serve_sheet(&state, &id, &sheet, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn serve_sheet(
    state: &AppState,
    id: &str,
    sheet: &str,
    request: Request<Body>,
) -> Result<Response> {
    let source_id = parse_source(id)?;
    // The name a client sends carries the suffix a browser expects to see on
    // the address; what it names is a number, never a path.
    let number: u32 = sheet
        .strip_suffix(".jpg")
        .unwrap_or(sheet)
        .parse()
        .map_err(|_| ServerError::not_found("that sheet of thumbnails"))?;

    let path = melyxar_app::thumbnails::sheet_of(state, source_id, number).await?;
    let mut response = crate::serve_the_file(&path, request).await?;
    response
        .headers_mut()
        .insert(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"));
    // A sheet never changes: it is named after a film that was read once, and
    // a film read again is written down again with as many sheets as it gave.
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=604800, immutable"),
    );
    Ok(response)
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
    body: Option<Json<OpenBody>>,
) -> Result<Json<SessionView>> {
    let source_id = parse_source(&id)?;
    let body = body.map(|Json(body)| body).unwrap_or_default();

    let plan = melyxar_app::playback::plan(
        &state,
        viewer(&state).await?,
        &body.wanted.clone().asked_for(source_id)?,
    )
    .await?;

    let session = melyxar_app::playback::open_session(
        &state,
        &plan,
        body.start_at_seconds
            .filter(|seconds| seconds.is_finite())
            .map(melyxar_core::time::Millis::from_seconds_f64),
    )
    .await?;
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
    /// How far the preparation has got, which a page shows while waiting.
    Preparation,
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
        "preparation" => Some(Wanted::Preparation),
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
        Some(Wanted::Preparation) => preparation(&state, &id).await,
        None => ServerError::not_found("a session hands out nothing by that name").into_response(),
    }
}

/// Where the preparation has got to.
///
/// Named steps rather than a proportion alone: a page that says "reading the
/// film" says which part is slow, where a bar filling at an unknown rate says
/// only that something is happening.
#[derive(Debug, Serialize)]
struct PreparationView {
    /// starting, reading, producing or ready.
    step: &'static str,
    /// Seconds of film on the disk that a player can actually read.
    ///
    /// In seconds rather than in the pieces the film is cut into: how long a
    /// piece is, is this server's own business, and a viewer waiting knows
    /// what ten seconds of a film is and not what two sixths of one is.
    ready_seconds: f64,
    /// How many seconds make a comfortable start from where the tool was set
    /// going, which near the end of a film is whatever is left of it.
    wanted_seconds: f64,
    /// How hard the machine is working on this film, while it is working.
    ///
    /// Sent from here rather than from the plan because it is the one thing
    /// about a playback that changes every second. The plan says what was
    /// decided; this says whether the machine can do it.
    producing: Option<ProducingView>,
}

#[derive(Debug, Serialize)]
struct ProducingView {
    /// Pictures a second the tool says it is writing just now.
    pictures_a_second: f64,
    /// The same work against real time. Below one and the picture will stop.
    speed: f64,
}

async fn preparation(state: &AppState, id: &str) -> Response {
    match live_session(state, id).await {
        Ok(session) => Json(preparation_view(&session.preparation().await)).into_response(),
        Err(error) => error.into_response(),
    }
}

fn preparation_view(seen: &Preparation) -> PreparationView {
    let of = |segments: u32| f64::from(segments) * SEGMENT_DURATION.as_seconds_f64();
    PreparationView {
        step: seen.step.as_str(),
        ready_seconds: of(seen.ready),
        wanted_seconds: of(seen.wanted),
        producing: seen.producing.map(|working| ProducingView {
            pictures_a_second: working.pictures_a_second,
            speed: working.speed,
        }),
    }
}

/// The playlist, which the server writes and the tool never sees.
async fn playlist(state: &AppState, id: &str) -> Response {
    match live_session(state, id).await {
        Ok(session) => {
            let text = session.playlist_text();
            // The one step of opening a film that left no trace at all, which
            // is the step that says where the player was told to begin. A
            // player asking for a part of the film nobody is at can be read
            // two ways without this, and only one of them is the server's.
            tracing::debug!(
                session = %session.id,
                begins_at = text
                    .lines()
                    .find_map(|line| line.strip_prefix("#EXT-X-START:TIME-OFFSET=")),
                segments = session.playlist().segment_count(),
                "the playlist was handed over"
            );
            (
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
                text,
            )
                .into_response()
        }
        Err(error) => error.into_response(),
    }
}

/// The header every segment needs.
async fn header_file(state: &AppState, id: &str, request: Request<Body>) -> Response {
    match live_session(state, id).await {
        Ok(session) => match session.initialisation().await {
            Ok(path) => serve(path, request, "video/mp4").await,
            Err(error) => ServerError::from(error).into_response(),
        },
        Err(error) => error.into_response(),
    }
}

/// One segment of the film, produced now if it is not there yet.
async fn segment(state: &AppState, id: &str, index: u32, request: Request<Body>) -> Response {
    match live_session(state, id).await {
        Ok(session) => match session.segment(index).await {
            Ok(path) => serve(path, request, "video/iso.segment").await,
            Err(error) => ServerError::from(error).into_response(),
        },
        Err(error) => error.into_response(),
    }
}

/// Closes a session, which a player asks for when it is done with a film.
/// Keeps a session alive while the viewer is in front of it doing nothing.
///
/// Refused when the session is over, which is an answer the page acts on:
/// there is nothing to go back to, so it opens a new one where the viewer
/// stands rather than waiting for them to press play and find the film gone.
async fn still_watching(
    State(state): State<AppState>,
    RoutePath(id): RoutePath<String>,
) -> Result<Json<serde_json::Value>> {
    live_session(&state, &id).await?.still_watching().await;
    Ok(Json(serde_json::json!({ "still_there": true })))
}

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
    sessions.get(session_id).await.map_err(ServerError::from)
}

/// Hands over a file the session produced.
async fn serve(path: std::path::PathBuf, request: Request<Body>, kind: &str) -> Response {
    let mut response = match crate::serve_the_file(&path, request).await {
        Ok(response) => response,
        Err(error) => return error.into_response(),
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
    let work_id = parse_work(&body.work_id)?;

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
    let work_id = parse_work(&body.work_id)?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::{TrackId, WorkId};

    #[test]
    fn what_a_player_sends_to_open_a_session_is_read_whole() {
        // The tracks and the starting point arrive in one flat object, and a
        // body that will not be read is not refused: it becomes the default,
        // silently, and the film then starts from its opening for no reason
        // anybody can see.
        let body: OpenBody = serde_json::from_str(
            r#"{"profile":null,"audio_track_id":"2","subtitle_track_id":"3",
                "preferred_video_codec":"av1","start_at_seconds":1024.5}"#,
        )
        .expect("the body of a session is read");

        assert_eq!(body.wanted.audio_track_id.as_deref(), Some("2"));
        assert_eq!(body.wanted.subtitle_track_id.as_deref(), Some("3"));
        assert_eq!(body.wanted.preferred_video_codec.as_deref(), Some("av1"));
        assert_eq!(body.start_at_seconds, Some(1024.5));
    }

    #[test]
    fn a_body_that_names_no_codec_leaves_the_choice_to_the_usual_negotiation() {
        let body: PlanBody = serde_json::from_str("{}").expect("an empty body is read");
        assert_eq!(body.preferred_video_codec, None);
    }

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        // Built at start-up, so a route a router refuses brings the whole
        // server down rather than failing one request. That is exactly what
        // happened, and only when it was run.
        let _ = router();
    }

    #[test]
    fn a_subtitle_is_asked_for_by_a_name_that_ends_the_way_a_browser_expects() {
        // The suffix belongs to the address, not to what it names: a client
        // sends an identifier, and it is read as one.
        assert!(parse_track(
            "01a09a58-ebee-75e5-aac2-00082f6b86de.vtt"
                .strip_suffix(".vtt")
                .expect("the suffix is there")
        )
        .is_ok());
        assert!(parse_track("../../etc/passwd").is_err());
    }

    /// A film with a picture and a subtitle sitting in a file of its own.
    ///
    /// Both are stream zero: the picture of the film, the subtitle of its own
    /// file. That is the ordinary shape of a film with an .srt beside it.
    fn film_with_a_separate_subtitle() -> PlayPlan {
        use melyxar_core::media::{SubtitleDetails, SubtitleLayout, Track, VideoDetails};

        let source_id = MediaSourceId::new();
        let picture = Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Video(VideoDetails {
                codec: "h264".into(),
                profile: None,
                level: None,
                width: 1920,
                height: 1080,
                margins: None,
                aspect_ratio: None,
                is_interlaced: false,
                frame_rate: Some(24.0),
                bitrate: None,
                pixel_format: None,
                reference_frames: None,
                color: Default::default(),
                hdr: None,
            }),
        };
        let words = Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
            language: Some("fre".into()),
            title: None,
            is_default: false,
            is_forced: false,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "subrip".into(),
                layout: SubtitleLayout::Text,
                is_hearing_impaired: false,
                is_external: true,
                external_relative_path: Some("Quiet.Harbour.2019.fr.srt".into()),
            }),
        };

        PlayPlan {
            source_id,
            work_id: WorkId::new(),
            path: "Quiet.Harbour.2019.mkv".into(),
            size_bytes: 12_000,
            container: Some("matroska,webm".into()),
            overall_bitrate: Some(8_000_000),
            duration: Some(Millis::new(7_200_000)),
            decision: melyxar_app::playback::PlaybackDecision {
                method: melyxar_app::playback::PlaybackMethod::Remux,
                video: melyxar_app::playback::StreamAction::Copy,
                audio: melyxar_app::playback::StreamAction::Copy,
                subtitles: melyxar_app::playback::SubtitleDelivery::External,
                audio_stream_index: None,
                subtitle_stream_index: Some(0),
                video_stream_index: Some(0),
                scale_to_height: None,
                bitrate_ceiling: None,
                tone_map: false,
                reasons: Vec::new(),
            },
            resume_from: None,
            tracks: vec![picture.clone(), words.clone()],
            downmix: Default::default(),
            downmix_gain: melyxar_core::user::DEFAULT_DOWNMIX_GAIN,
            rebuild: None,
            thumbnails: None,
            favourite: false,
            chapters: vec![
                melyxar_core::media::Chapter {
                    ordinal: 0,
                    start: Millis::ZERO,
                    title: Some("Cold open".into()),
                    thumbnail_path: None,
                },
                melyxar_core::media::Chapter {
                    ordinal: 1,
                    start: Millis::new(754_500),
                    title: None,
                    thumbnail_path: None,
                },
            ],
        }
    }

    #[test]
    fn a_bar_is_told_where_the_film_changes_scene() {
        // Marked on the bar rather than fetched by it: they are drawn from the
        // first frame the bar exists, and marks that appear a moment later are
        // a bar that jumps under the hand. In seconds, which is what a bar is
        // drawn in and what a player is set to; a chapter the file numbered
        // rather than named says nothing, and the interface writes its own
        // wording.
        let view = plan_view(&film_with_a_separate_subtitle());

        assert_eq!(view.chapters.len(), 2);
        assert_eq!(view.chapters[0].at_second, 0.0);
        assert_eq!(view.chapters[0].title.as_deref(), Some("Cold open"));
        assert_eq!(view.chapters[1].at_second, 754.5);
        assert_eq!(view.chapters[1].title, None);
    }

    #[test]
    fn the_chosen_subtitle_is_the_subtitle_and_not_the_picture() {
        // A subtitle in a file of its own is stream zero of that file, which
        // is also the number of the picture. Matching on the number alone
        // hands back the picture, and the viewer gets subtitles that never
        // appear with no hint as to why.
        let plan = film_with_a_separate_subtitle();
        let view = plan_view(&plan);

        assert_eq!(
            view.chosen_subtitle_id.as_deref(),
            Some(view.subtitles[0].id.as_str())
        );
        assert_ne!(
            view.chosen_subtitle_id.as_deref(),
            Some(plan.tracks[0].id.to_string().as_str()),
            "the picture is not a subtitle"
        );
    }

    #[test]
    fn the_frame_is_said_only_when_it_is_not_the_picture() {
        // A film that says part of its frame is not the picture is two shapes,
        // and the difference is the whole reason it is handled apart. Saying
        // the frame on every film would put the same two numbers twice on
        // every screen and teach nobody anything.
        let ordinary = plan_view(&film_with_a_separate_subtitle());
        let picture = ordinary.film.picture.expect("this film holds a picture");
        assert_eq!((picture.width, picture.height), (1920, 1080));
        assert_eq!(
            (picture.frame_width, picture.frame_height),
            (None, None),
            "nothing is cut off this one, so there is no frame to name"
        );

        let mut cut = film_with_a_separate_subtitle();
        if let TrackKind::Video(details) = &mut cut.tracks[0].kind {
            details.width = 3840;
            details.height = 2160;
            details.margins = Some(melyxar_core::media::Margins {
                top: 276,
                bottom: 276,
                left: 0,
                right: 0,
            });
        }
        let picture = plan_view(&cut)
            .film
            .picture
            .expect("this film holds a picture");
        assert_eq!(
            (picture.width, picture.height),
            (3840, 1608),
            "the picture is what is left once the margins are off"
        );
        assert_eq!(
            (picture.frame_width, picture.frame_height),
            (Some(3840), Some(2160)),
            "and the frame around it is said beside it"
        );
    }

    #[test]
    fn the_soundtrack_shown_is_the_one_being_played() {
        // A film carrying four languages holds four soundtracks, and three of
        // them have nothing to do with what a viewer is hearing.
        use melyxar_core::media::{AudioDetails, Track};

        let mut plan = film_with_a_separate_subtitle();
        let source_id = plan.source_id;
        let sound = |index: i32, codec: &str, channels: i32| Track {
            id: TrackId::new(),
            source_id,
            stream_index: index,
            language: None,
            title: None,
            is_default: index == 1,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: codec.into(),
                profile: None,
                channels,
                channel_layout: None,
                sample_rate: Some(48_000),
                bit_depth: None,
                bitrate: None,
                loudness: Default::default(),
            }),
        };
        plan.tracks.push(sound(1, "ac3", 6));
        plan.tracks.push(sound(2, "aac", 2));
        plan.decision.audio_stream_index = Some(2);

        let heard = plan_view(&plan)
            .film
            .sound
            .expect("a soundtrack is playing");
        assert_eq!(heard.codec, "aac");
        assert_eq!(heard.channels, 2);
    }

    #[test]
    fn how_hard_the_machine_is_working_is_sent_only_once_the_tool_has_said() {
        use melyxar_app::playback::{PreparationStep, Producing};

        let waiting = preparation_view(&Preparation {
            step: PreparationStep::Reading,
            ready: 0,
            wanted: 6,
            producing: None,
        });
        assert!(
            waiting.producing.is_none(),
            "a tool that has not spoken has no rate, and nothing is not a rate"
        );

        let working = preparation_view(&Preparation {
            step: PreparationStep::Producing,
            ready: 2,
            wanted: 6,
            producing: Some(Producing {
                pictures_a_second: 38.4,
                speed: 1.6,
            }),
        });
        let said = working.producing.expect("the tool said how it was doing");
        assert_eq!(said.pictures_a_second, 38.4);
        assert_eq!(said.speed, 1.6);
    }

    #[test]
    fn how_far_the_preparation_has_got_is_said_in_seconds_of_film() {
        use melyxar_app::playback::PreparationStep;

        // A count of pieces means nothing to anybody watching a ring turn, and
        // how long a piece is, is this server's own business. Said in seconds,
        // a page never has to know that figure and never has to be changed
        // when it does.
        let seen = preparation_view(&Preparation {
            step: PreparationStep::Producing,
            ready: 2,
            wanted: 6,
            producing: None,
        });
        assert_eq!(seen.ready_seconds, 2.0 * SEGMENT_DURATION.as_seconds_f64());
        assert_eq!(seen.wanted_seconds, 6.0 * SEGMENT_DURATION.as_seconds_f64());
    }

    #[test]
    fn a_subtitle_made_of_words_carries_the_address_of_its_words() {
        let view = plan_view(&film_with_a_separate_subtitle());
        let words = &view.subtitles[0];
        assert_eq!(words.burns_in, Some(false));
        assert_eq!(
            words.url.as_deref(),
            Some(
                format!(
                    "/api/v1/playback/{}/subtitles/{}.vtt",
                    view.url
                        .trim_start_matches("/api/v1/playback/")
                        .trim_end_matches("/stream"),
                    words.id
                )
                .as_str()
            )
        );
    }

    #[test]
    fn a_subtitle_made_of_pictures_carries_no_address() {
        // There is no text in one to hand over: it is drawn into the film.
        use melyxar_core::media::SubtitleLayout;

        let mut plan = film_with_a_separate_subtitle();
        if let TrackKind::Subtitle(details) = &mut plan.tracks[1].kind {
            details.layout = SubtitleLayout::Bitmap;
        }

        let view = plan_view(&plan);
        assert_eq!(view.subtitles[0].burns_in, Some(true));
        assert_eq!(view.subtitles[0].url, None);
    }

    #[test]
    fn a_film_read_for_its_bar_says_where_its_sheets_are_and_how_to_cut_them() {
        use melyxar_core::thumbnails::Thumbnails;

        let mut plan = film_with_a_separate_subtitle();
        plan.thumbnails = Some(Thumbnails {
            every: Millis::new(10_000),
            width: 320,
            height: 180,
            columns: 10,
            rows: 10,
            counted: 720,
            sheets: 8,
        });

        let view = plan_view(&plan);
        let shown = view.thumbnails.expect("the film has them");
        assert_eq!(
            shown.url,
            format!("/api/v1/playback/{}/thumbnails", plan.source_id),
            "the page adds the number of the sheet it wants"
        );
        assert_eq!(shown.every_seconds, 10.0);
        assert_eq!((shown.width, shown.height), (320, 180));
        assert_eq!((shown.columns, shown.rows), (10, 10));
        assert_eq!(shown.counted, 720);
    }

    #[test]
    fn a_film_nobody_has_read_for_its_bar_offers_no_pictures_at_all() {
        // Rather than an address that answers nothing: a page told there are
        // thumbnails asks for one on every movement of the cursor.
        use melyxar_core::thumbnails::Thumbnails;

        assert!(plan_view(&film_with_a_separate_subtitle())
            .thumbnails
            .is_none());

        let mut nothing_in_it = film_with_a_separate_subtitle();
        nothing_in_it.thumbnails = Some(Thumbnails {
            every: Millis::new(10_000),
            width: 0,
            height: 0,
            columns: 10,
            rows: 10,
            counted: 0,
            sheets: 0,
        });
        assert!(
            plan_view(&nothing_in_it).thumbnails.is_none(),
            "a file that holds no picture has been read and has none"
        );
    }

    #[test]
    fn a_sheet_is_asked_for_by_a_number_and_never_by_a_path() {
        let number = |sheet: &str| -> Option<u32> {
            sheet.strip_suffix(".jpg").unwrap_or(sheet).parse().ok()
        };
        assert_eq!(number("0000.jpg"), Some(0));
        assert_eq!(number("12.jpg"), Some(12));
        assert_eq!(number("../../etc/passwd"), None);
        assert_eq!(number("../0000.jpg"), None);
    }

    #[test]
    fn a_session_hands_out_four_things_and_nothing_else() {
        assert_eq!(wanted_from("playlist.m3u8"), Some(Wanted::Playlist));
        assert_eq!(wanted_from("init.mp4"), Some(Wanted::Header));
        assert_eq!(wanted_from("preparation"), Some(Wanted::Preparation));
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
