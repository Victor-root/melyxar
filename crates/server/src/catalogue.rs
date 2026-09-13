//! Browsing: libraries, grids and detail pages.
//!
//! The shapes sent here are written for the screens that read them, never
//! taken from the database as they are. A grid is given exactly what a card
//! draws and nothing else, because a page of sixty cards carrying a synopsis
//! each is ten times the weight for text nobody reads there.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::Request;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use melyxar_app::browse::{BrowseRequest, WorkCard, WorkOrder, DEFAULT_PAGE};
use melyxar_app::detail::{Credit, Version, WorkDetail};
use melyxar_app::picture::StoredImage;
use melyxar_app::AppState;
use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::media::TrackKind;
use melyxar_core::time::Millis;
use melyxar_core::work::IdentificationNote;
use serde::{Deserialize, Serialize};
use tower::ServiceExt;
use tower_http::services::ServeFile;

use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/libraries", axum::routing::get(libraries))
        .route(
            "/api/v1/libraries/{id}/filters",
            axum::routing::get(filters),
        )
        .route("/api/v1/works", axum::routing::get(works))
        .route("/api/v1/works/{id}", axum::routing::get(work))
        .route(
            "/api/v1/works/{id}/trailers/{rank}",
            axum::routing::get(trailer),
        )
        .route("/api/v1/home", axum::routing::get(home))
}

/// Hands over a trailer sitting next to the film.
///
/// Named by its place in the list the detail page was given, so nothing a
/// client sends is ever treated as a path. A trailer hosted elsewhere never
/// comes through here: it is watched where it lives, and this server does not
/// go and fetch someone else's video.
async fn trailer(
    State(state): State<AppState>,
    Path((id, rank)): Path<(String, usize)>,
    request: Request<Body>,
) -> Response {
    match serve_trailer(&state, &id, rank, request).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn serve_trailer(
    state: &AppState,
    id: &str,
    rank: usize,
    request: Request<Body>,
) -> Result<Response> {
    let work_id: WorkId = id
        .parse()
        .map_err(|_| ServerError::invalid_input("the work identifier is malformed"))?;
    let detail = melyxar_app::detail::work_detail(state, work_id)
        .await?
        .ok_or_else(|| ServerError::not_found("no work with that identifier"))?;

    let file = detail
        .trailers
        .get(rank)
        .and_then(|trailer| trailer.local.as_ref())
        .ok_or_else(|| ServerError::not_found("no trailer of this work sits on the disk"))?;

    ServeFile::new(&file.path)
        .oneshot(request)
        .await
        .map(IntoResponse::into_response)
        .map_err(|error| ServerError::internal(error.to_string()))
}

// ---------------------------------------------------------------------------
// Libraries
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct LibraryView {
    id: String,
    name: String,
    kind: &'static str,
    /// What the grid of this library holds, so a menu can show a count without
    /// asking for a page first.
    works: i64,
    /// Bumped whenever anything in the library moves. A client keeps it and
    /// asks whether it changed instead of fetching everything again.
    version: i64,
    roots: Vec<RootView>,
}

#[derive(Debug, Serialize)]
struct RootView {
    label: String,
    /// What the server may actually do with this folder, told by trying.
    access: &'static str,
    /// Stable code the interface turns into a sentence in its own language.
    explanation_code: &'static str,
}

async fn libraries(State(state): State<AppState>) -> Result<Json<Vec<LibraryView>>> {
    let summaries = melyxar_app::catalogue::libraries(&state).await?;
    Ok(Json(
        summaries
            .into_iter()
            .map(|library| LibraryView {
                id: library.id.to_string(),
                name: library.name,
                kind: library.kind.as_str(),
                works: library.works,
                version: library.version,
                roots: library
                    .roots
                    .into_iter()
                    .map(|root| RootView {
                        label: root.label,
                        access: root.access.as_str(),
                        explanation_code: root.access.explanation_code(),
                    })
                    .collect(),
            })
            .collect(),
    ))
}

#[derive(Debug, Serialize)]
struct FiltersView {
    /// Only what is actually in the library: offering a genre nobody has leads
    /// to an empty grid and looks like a fault.
    genres: Vec<CountedView>,
    decades: Vec<CountedDecade>,
}

#[derive(Debug, Serialize)]
struct CountedView {
    name: String,
    works: i64,
}

#[derive(Debug, Serialize)]
struct CountedDecade {
    decade: i32,
    works: i64,
}

async fn filters(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<FiltersView>> {
    let found = melyxar_app::catalogue::filters(&state, Some(parse_library(&id)?)).await?;
    Ok(Json(FiltersView {
        genres: found
            .genres
            .into_iter()
            .map(|(name, works)| CountedView { name, works })
            .collect(),
        decades: found
            .decades
            .into_iter()
            .map(|(decade, works)| CountedDecade { decade, works })
            .collect(),
    }))
}

// ---------------------------------------------------------------------------
// Grids
// ---------------------------------------------------------------------------

#[derive(Debug, Default, Deserialize)]
struct BrowseParams {
    library: Option<String>,
    order: Option<WorkOrder>,
    #[serde(default)]
    descending: bool,
    /// The last card of the page before. A position rather than a number, so
    /// a film added while someone reads never doubles or hides a card.
    after: Option<String>,
    limit: Option<i64>,
    genre: Option<String>,
    decade: Option<i32>,
    search: Option<String>,
    #[serde(default)]
    unidentified: bool,
}

#[derive(Debug, Serialize)]
struct PageView {
    cards: Vec<CardView>,
    /// What to pass as `after` for the next page. Absent on the last one.
    next: Option<String>,
}

#[derive(Debug, Serialize)]
struct CardView {
    id: String,
    title: String,
    year: Option<i32>,
    runtime_minutes: Option<i64>,
    rating: Option<f64>,
    /// pending, identified, unidentified or manual. A card shows a marker for
    /// anything but identified, so a film nobody recognised stays visible
    /// instead of being quietly set aside.
    identification: &'static str,
    /// Why the last look up did not name it, when one has run and failed.
    /// Absent for a film nobody has looked up yet, which is itself the answer.
    identification_note: Option<&'static str>,
    /// Drawn under the picture while it loads, so a grid has colour from the
    /// first moment rather than grey holes.
    color: Option<String>,
    poster: Vec<ImageView>,
}

#[derive(Debug, Serialize, PartialEq)]
struct ImageView {
    url: String,
    width: Option<i32>,
    height: Option<i32>,
}

async fn works(
    State(state): State<AppState>,
    Query(params): Query<BrowseParams>,
) -> Result<Json<PageView>> {
    let request = BrowseRequest {
        library_id: params.library.as_deref().map(parse_library).transpose()?,
        order: params.order.unwrap_or(WorkOrder::Title),
        descending: params.descending,
        after: params.after.as_deref().map(parse_work).transpose()?,
        limit: params.limit.unwrap_or(DEFAULT_PAGE),
        genre: params.genre,
        decade: params.decade,
        search: params.search.filter(|value| !value.trim().is_empty()),
        unidentified_only: params.unidentified,
    };

    let page = melyxar_app::catalogue::browse(&state, &request).await?;
    Ok(Json(PageView {
        cards: page.cards.iter().map(card_view).collect(),
        next: page.next.map(|id| id.to_string()),
    }))
}

fn card_view(card: &WorkCard) -> CardView {
    CardView {
        id: card.id.to_string(),
        title: card.title.clone(),
        year: card.release_year,
        runtime_minutes: card.runtime.map(whole_minutes),
        rating: card.community_rating,
        identification: card.identification.as_str(),
        identification_note: card.identification_note.map(IdentificationNote::as_str),
        color: card.dominant_color.clone(),
        poster: card.poster.iter().map(image_view).collect(),
    }
}

fn image_view(image: &StoredImage) -> ImageView {
    ImageView {
        url: format!("/api/v1/images/{}", image.relative_path),
        width: image.width,
        height: image.height,
    }
}

/// Runtime as a page shows it. Whole minutes, rounded rather than cut: a film
/// of a hundred and nineteen minutes and fifty seconds is two hours.
fn whole_minutes(duration: Millis) -> i64 {
    (duration.get() + 30_000) / 60_000
}

// ---------------------------------------------------------------------------
// Home
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct HomeParams {
    library: Option<String>,
}

#[derive(Debug, Serialize)]
struct HomeView {
    /// What a home page leads with, newest first.
    recently_added: Vec<CardView>,
    works: i64,
    /// Whether anything is still waiting to be looked up, so the page can say
    /// so rather than showing untitled films with no explanation.
    awaiting_identification: i64,
}

async fn home(
    State(state): State<AppState>,
    Query(params): Query<HomeParams>,
) -> Result<Json<HomeView>> {
    let library_id = params.library.as_deref().map(parse_library).transpose()?;
    let page = melyxar_app::catalogue::home(&state, library_id).await?;

    Ok(Json(HomeView {
        recently_added: page.recently_added.cards.iter().map(card_view).collect(),
        works: page.works,
        awaiting_identification: page.awaiting_identification,
    }))
}

// ---------------------------------------------------------------------------
// One work
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct WorkView {
    id: String,
    library_id: String,
    kind: &'static str,
    title: String,
    tagline: Option<String>,
    overview: Option<String>,
    year: Option<i32>,
    runtime_minutes: Option<i64>,
    rating: Option<f64>,
    age_rating: Option<String>,
    identification: &'static str,
    identification_note: Option<&'static str>,
    color: Option<String>,
    genres: Vec<String>,
    studios: Vec<String>,
    collection: Option<String>,
    cast: Vec<CreditView>,
    crew: Vec<CreditView>,
    poster: Vec<ImageView>,
    backdrop: Vec<ImageView>,
    versions: Vec<VersionView>,
    trailers: Vec<TrailerView>,
    external_ids: Vec<ExternalIdView>,
}

#[derive(Debug, Serialize)]
struct CreditView {
    name: String,
    role: String,
    character: Option<String>,
    /// The face shown next to the name. Empty for anyone whose picture has not
    /// been fetched, and the page shows an initial instead.
    photo: Vec<ImageView>,
}

#[derive(Debug, Serialize)]
struct VersionView {
    id: String,
    /// The short line shown above a play button.
    summary: String,
    size_bytes: i64,
    duration_minutes: Option<i64>,
    container: Option<String>,
    /// False while nothing has looked inside the file yet.
    analysed: bool,
    /// True when the file is not on disk. Such a version is shown as
    /// unavailable rather than offered and failing when pressed.
    missing: bool,
    video: Vec<VideoTrackView>,
    audio: Vec<AudioTrackView>,
    subtitles: Vec<SubtitleTrackView>,
    chapters: usize,
}

#[derive(Debug, Serialize)]
struct VideoTrackView {
    codec: String,
    width: i32,
    height: i32,
    /// hdr10, hlg or dolby_vision. Absent for an ordinary picture.
    hdr: Option<&'static str>,
    frame_rate: Option<f64>,
}

#[derive(Debug, Serialize)]
struct AudioTrackView {
    codec: String,
    language: Option<String>,
    channels: i32,
    channel_layout: Option<String>,
    is_default: bool,
}

#[derive(Debug, Serialize)]
struct SubtitleTrackView {
    codec: String,
    language: Option<String>,
    is_forced: bool,
    is_hearing_impaired: bool,
    /// True for a file sitting next to the film rather than a stream inside it.
    is_external: bool,
    /// True when showing it means burning it into the picture, which costs a
    /// full conversion. Worth knowing before pressing play.
    burns_in: bool,
}

#[derive(Debug, Serialize)]
struct TrailerView {
    name: Option<String>,
    /// Set when playing it leaves this server.
    remote_url: Option<String>,
    /// Set when the file sits next to the film.
    local: bool,
    /// Where to fetch a local one. Absent for a link, which is watched where
    /// it lives: this server does not go and fetch someone else's video.
    url: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExternalIdView {
    provider: String,
    id: String,
}

async fn work(State(state): State<AppState>, Path(id): Path<String>) -> Result<Json<WorkView>> {
    let work_id = parse_work(&id)?;
    let detail = melyxar_app::detail::work_detail(&state, work_id)
        .await?
        .ok_or_else(|| ServerError::not_found("no work with that identifier"))?;

    Ok(Json(work_view(&detail)))
}

fn work_view(detail: &WorkDetail) -> WorkView {
    let images_of = |kind: &str| -> Vec<ImageView> {
        detail
            .images
            .iter()
            .filter(|image| image.image_kind == kind)
            .map(image_view)
            .collect()
    };

    WorkView {
        id: detail.work.id.to_string(),
        library_id: detail.work.library_id.to_string(),
        kind: detail.work.kind.as_str(),
        title: detail.work.title.clone(),
        tagline: detail.tagline.clone(),
        overview: detail.overview.clone(),
        year: detail.work.release_year,
        // The runtime of the film, not the length of any one copy of it. A
        // file a few seconds short is still that film, and a page that shows
        // the file length shows a different number for each version.
        runtime_minutes: detail.work.runtime.map(whole_minutes),
        rating: detail.work.community_rating,
        age_rating: detail.work.age_rating_label.clone(),
        identification: detail.work.identification.as_str(),
        identification_note: detail.work.identification_note.map(IdentificationNote::as_str),
        color: detail.work.dominant_color.clone(),
        genres: detail.genres.clone(),
        studios: detail.studios.clone(),
        collection: detail.collection.clone(),
        cast: detail
            .credits
            .iter()
            .filter(|credit| credit.role == "actor")
            .map(credit_view)
            .collect(),
        crew: detail
            .credits
            .iter()
            .filter(|credit| credit.role != "actor")
            .map(credit_view)
            .collect(),
        poster: images_of("poster"),
        backdrop: images_of("backdrop"),
        versions: detail.versions.iter().map(version_view).collect(),
        trailers: detail
            .trailers
            .iter()
            .enumerate()
            .map(|(rank, trailer)| TrailerView {
                name: trailer.name.clone(),
                remote_url: trailer.remote_url.clone(),
                local: trailer.local.is_some(),
                // Named by its place in the list this very answer carries,
                // which is what keeps a path on someone's disk out of a page
                // and out of anything a client could send back.
                url: trailer
                    .local
                    .is_some()
                    .then(|| format!("/api/v1/works/{}/trailers/{rank}", detail.work.id)),
            })
            .collect(),
        external_ids: detail
            .external_ids
            .iter()
            .map(|(provider, id)| ExternalIdView {
                provider: provider.clone(),
                id: id.clone(),
            })
            .collect(),
    }
}

fn credit_view(credit: &Credit) -> CreditView {
    CreditView {
        name: credit.name.clone(),
        role: credit.role.clone(),
        character: credit.character.clone(),
        photo: credit.photo.iter().map(image_view).collect(),
    }
}

fn version_view(version: &Version) -> VersionView {
    let mut video = Vec::new();
    let mut audio = Vec::new();
    let mut subtitles = Vec::new();

    for track in &version.tracks {
        match &track.kind {
            TrackKind::Video(details) => video.push(VideoTrackView {
                codec: details.codec.clone(),
                width: details.width,
                height: details.height,
                hdr: details.hdr.map(|hdr| match hdr {
                    melyxar_core::media::HdrFormat::Hdr10 => "hdr10",
                    melyxar_core::media::HdrFormat::Hlg => "hlg",
                    melyxar_core::media::HdrFormat::DolbyVision { .. } => "dolby_vision",
                }),
                frame_rate: details.frame_rate,
            }),
            TrackKind::Audio(details) => audio.push(AudioTrackView {
                codec: details.codec.clone(),
                language: track.language.clone(),
                channels: details.channels,
                channel_layout: details.channel_layout.clone(),
                is_default: track.is_default,
            }),
            TrackKind::Subtitle(details) => subtitles.push(SubtitleTrackView {
                codec: details.codec.clone(),
                language: track.language.clone(),
                is_forced: track.is_forced,
                is_hearing_impaired: details.is_hearing_impaired,
                is_external: details.is_external,
                burns_in: details.forces_full_transcode(),
            }),
        }
    }

    VersionView {
        id: version.source_id.to_string(),
        summary: version.summary(),
        size_bytes: version.size_bytes,
        duration_minutes: version.duration.map(whole_minutes),
        container: version.container.clone(),
        analysed: version.analysed,
        missing: version.missing_since.is_some(),
        chapters: version.chapters.len(),
        video,
        audio,
        subtitles,
    }
}

fn parse_library(value: &str) -> Result<LibraryId> {
    value
        .parse()
        .map_err(|_| ServerError::invalid_input("the library identifier is malformed"))
}

fn parse_work(value: &str) -> Result<WorkId> {
    value
        .parse()
        .map_err(|_| ServerError::invalid_input("the work identifier is malformed"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::{MediaSourceId, TrackId};
    use melyxar_core::media::{
        AudioDetails, ColorInfo, HdrFormat, SubtitleDetails, SubtitleLayout, Track, VideoDetails,
    };

    fn version_with(tracks: Vec<Track>) -> Version {
        Version {
            source_id: MediaSourceId::new(),
            relative_path: "Quiet.Harbour.2019.mkv".to_string(),
            size_bytes: 12_000_000_000,
            missing_since: None,
            container: Some("matroska,webm".to_string()),
            duration: Some(Millis::new(7_190_000)),
            overall_bitrate: None,
            analysed: true,
            tracks,
            chapters: Vec::new(),
        }
    }

    fn wide_gamut_video() -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 0,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Video(VideoDetails {
                codec: "hevc".to_string(),
                profile: None,
                level: None,
                width: 3840,
                height: 2160,
                aspect_ratio: None,
                is_interlaced: false,
                frame_rate: Some(23.976),
                bitrate: None,
                pixel_format: None,
                reference_frames: None,
                color: ColorInfo::default(),
                hdr: Some(HdrFormat::DolbyVision { profile: Some(5) }),
            }),
        }
    }

    fn picture_subtitle() -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 1,
            language: Some("fre".to_string()),
            title: None,
            is_default: false,
            is_forced: true,
            kind: TrackKind::Subtitle(SubtitleDetails {
                codec: "hdmv_pgs_subtitle".to_string(),
                layout: SubtitleLayout::Bitmap,
                is_hearing_impaired: false,
                is_external: false,
                external_relative_path: None,
            }),
        }
    }

    fn soundtrack() -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index: 2,
            language: Some("fre".to_string()),
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "eac3".to_string(),
                profile: None,
                channels: 6,
                channel_layout: Some("5.1".to_string()),
                sample_rate: Some(48_000),
                bit_depth: None,
                bitrate: None,
                loudness: Default::default(),
            }),
        }
    }

    #[test]
    fn a_version_carries_what_the_page_needs_to_warn_about() {
        let view = version_view(&version_with(vec![
            wide_gamut_video(),
            soundtrack(),
            picture_subtitle(),
        ]));

        assert_eq!(view.video[0].hdr, Some("dolby_vision"));
        assert!(
            view.subtitles[0].burns_in,
            "a viewer should know before pressing play that this one costs a full conversion"
        );
        assert_eq!(view.audio[0].channel_layout.as_deref(), Some("5.1"));
        assert!(!view.missing);
    }

    #[test]
    fn a_runtime_is_rounded_the_way_a_person_would_round_it() {
        assert_eq!(whole_minutes(Millis::new(7_190_000)), 120);
        assert_eq!(whole_minutes(Millis::new(60_000)), 1);
        assert_eq!(whole_minutes(Millis::new(29_000)), 0);
    }

    #[test]
    fn a_file_that_is_gone_is_shown_as_gone_rather_than_offered() {
        let mut version = version_with(vec![wide_gamut_video()]);
        version.missing_since = Some(melyxar_core::time::now());
        assert!(version_view(&version).missing);
    }

    #[test]
    fn an_image_is_addressed_by_a_name_that_changes_with_its_content() {
        let image = StoredImage {
            owner_kind: "work".to_string(),
            owner_id: "w".to_string(),
            image_kind: "poster".to_string(),
            relative_path: "works/w/poster-abc123-400.webp".to_string(),
            width: Some(400),
            height: Some(600),
            fingerprint: "abc123".to_string(),
            dominant_color: None,
        };
        assert_eq!(
            image_view(&image),
            ImageView {
                url: "/api/v1/images/works/w/poster-abc123-400.webp".to_string(),
                width: Some(400),
                height: Some(600),
            }
        );
    }

    #[test]
    fn a_malformed_identifier_is_refused_rather_than_looked_up() {
        assert!(parse_work("not-an-identifier").is_err());
        assert!(parse_library("").is_err());
    }
}
