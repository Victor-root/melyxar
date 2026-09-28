//! Subtitles from OpenSubtitles: the key and account the administrator gave,
//! the subtitle tracks of a copy, what is offered for it, and downloading or
//! taking away one.

use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use melyxar_app::online_subtitles::SubtitleOffer;
use melyxar_app::ratings::KeyTried;
use melyxar_app::AppState;
use melyxar_core::media::TrackKind;
use serde::{Deserialize, Serialize};

use crate::account::Viewer;
use crate::error::Result;
use crate::identifiers::{parse_source, parse_track};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/settings/opensubtitles",
            axum::routing::get(settings).put(set_account).delete(forget_account),
        )
        .route("/api/v1/playback/{id}/subtitles", axum::routing::get(tracks))
        .route(
            "/api/v1/playback/{id}/subtitles/online",
            axum::routing::get(offers).post(download),
        )
        .route(
            "/api/v1/playback/{id}/subtitles/online/{track}",
            axum::routing::delete(remove),
        )
}

/// Whether a key was given, and an account with it, never either one itself.
#[derive(Debug, Serialize)]
struct SettingsView {
    has_key: bool,
    signed_in: bool,
    /// What trying the key just typed came to: kept, refused or unreachable.
    tried: Option<&'static str>,
}

impl SettingsView {
    async fn of(state: &AppState, tried: Option<KeyTried>) -> Result<Json<Self>> {
        let standing = melyxar_app::online_subtitles::standing(state).await?;
        Ok(Json(Self {
            has_key: standing.has_key,
            signed_in: standing.signed_in,
            tried: tried.map(KeyTried::as_str),
        }))
    }
}

async fn settings(State(state): State<AppState>, _: crate::account::Administrator) -> Result<Json<SettingsView>> {
    SettingsView::of(&state, None).await
}

#[derive(Debug, Deserialize)]
struct AccountGiven {
    key: String,
    #[serde(default)]
    username: String,
    #[serde(default)]
    password: String,
}

async fn set_account(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    Json(given): Json<AccountGiven>,
) -> Result<Json<SettingsView>> {
    let tried =
        melyxar_app::online_subtitles::set_account(&state, &given.key, &given.username, &given.password).await?;
    SettingsView::of(&state, Some(tried)).await
}

async fn forget_account(
    State(state): State<AppState>,
    _: crate::account::Administrator,
) -> Result<Json<SettingsView>> {
    melyxar_app::online_subtitles::forget_account(&state).await?;
    SettingsView::of(&state, None).await
}

/// One subtitle track of a copy, and where it comes from.
#[derive(Debug, Serialize)]
struct TrackView {
    id: String,
    language: Option<String>,
    title: Option<String>,
    /// inside, beside or downloaded.
    origin: &'static str,
    hearing_impaired: bool,
}

async fn tracks(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<Vec<TrackView>>> {
    let tracks = melyxar_app::online_subtitles::tracks_of(&state, &who, parse_source(&id)?).await?;
    Ok(Json(
        tracks
            .iter()
            .filter_map(|track| match &track.kind {
                TrackKind::Subtitle(details) => Some(TrackView {
                    id: track.id.to_string(),
                    language: track.language.clone(),
                    title: track.title.clone(),
                    origin: if details.downloaded_file.is_some() {
                        "downloaded"
                    } else if details.is_external {
                        "beside"
                    } else {
                        "inside"
                    },
                    hearing_impaired: details.is_hearing_impaired,
                }),
                _ => None,
            })
            .collect(),
    ))
}

/// One subtitle offered, as the window lists it and sends it back to be
/// downloaded.
#[derive(Debug, Serialize, Deserialize)]
struct OfferView {
    file_id: i64,
    language: String,
    release: String,
    #[serde(default)]
    downloads: i64,
    #[serde(default)]
    hearing_impaired: bool,
    #[serde(default)]
    machine_translated: bool,
    #[serde(default)]
    trusted: bool,
}

impl From<SubtitleOffer> for OfferView {
    fn from(offer: SubtitleOffer) -> Self {
        Self {
            file_id: offer.file_id,
            language: offer.language,
            release: offer.release,
            downloads: offer.downloads,
            hearing_impaired: offer.hearing_impaired,
            machine_translated: offer.machine_translated,
            trusted: offer.trusted,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Languages {
    /// Two letters each, separated by commas.
    languages: String,
}

async fn offers(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Query(asked): Query<Languages>,
) -> Result<Json<Vec<OfferView>>> {
    let languages: Vec<String> = asked
        .languages
        .split(',')
        .map(|language| language.trim().to_string())
        .filter(|language| !language.is_empty())
        .collect();
    let offers = melyxar_app::online_subtitles::offers(&state, &who, parse_source(&id)?, &languages).await?;
    Ok(Json(offers.into_iter().map(OfferView::from).collect()))
}

#[derive(Debug, Serialize)]
struct FetchedView {
    track_id: String,
    /// How many more downloads today allows, when OpenSubtitles said.
    remaining: Option<i64>,
}

async fn download(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Json(offer): Json<OfferView>,
) -> Result<Json<FetchedView>> {
    let offer = SubtitleOffer {
        file_id: offer.file_id,
        language: offer.language,
        release: offer.release,
        downloads: offer.downloads,
        hearing_impaired: offer.hearing_impaired,
        machine_translated: offer.machine_translated,
        trusted: offer.trusted,
    };
    let fetched = melyxar_app::online_subtitles::download(&state, &who, parse_source(&id)?, &offer).await?;
    Ok(Json(FetchedView {
        track_id: fetched.track_id.to_string(),
        remaining: fetched.remaining,
    }))
}

async fn remove(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path((id, track)): Path<(String, String)>,
) -> Result<Json<()>> {
    melyxar_app::online_subtitles::remove(&state, &who, parse_source(&id)?, parse_track(&track)?).await?;
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
