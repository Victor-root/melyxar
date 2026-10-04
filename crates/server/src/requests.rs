//! Title requests: whether this account may ask, looking a title up, its own
//! requests, and for the administrator the switch, who may ask and the
//! decisions.
//!
//! Translation only: what may be asked and what becomes of a request is the
//! app's to decide.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use melyxar_app::requests::access::Asker;
use melyxar_app::requests::asking::Asking;
use melyxar_app::requests::deciding::Waiting;
use melyxar_app::requests::search::{Found, SeasonChoice};
use melyxar_app::requests::{catalogue_of, word_of, Catalogue, Decision, TitleRequest};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::account::{Administrator, Viewer};
use crate::error::{Result, ServerError};
use crate::identifiers::{parse_account, parse_request};
use crate::jobs::provider_of;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/requests", get(mine).post(ask))
        .route("/api/v1/requests/{id}", axum::routing::delete(withdraw))
        .route("/api/v1/requests/access", get(access))
        .route("/api/v1/requests/search", get(search))
        .route("/api/v1/requests/series/{id}/seasons", get(seasons))
        .route("/api/v1/system/requests", get(for_the_administrator))
        .route("/api/v1/system/requests/enabled", put(switch))
        .route("/api/v1/system/requests/askers/{id}", put(allow))
        .route("/api/v1/system/requests/decisions", post(decide))
}

fn parse_catalogue(word: &str) -> Result<Catalogue> {
    catalogue_of(word).ok_or_else(|| ServerError::invalid_input("no such catalogue"))
}

/// The full address of a poster the provider named, when there is a
/// provider to say it.
fn poster_of(state: &AppState, path: Option<&str>) -> Option<String> {
    use melyxar_app::metadata::MetadataProvider;
    let provider = state.metadata_provider()?;
    path.map(|path| provider.small_image_url(path))
}

#[derive(Debug, Serialize)]
struct AccessView {
    enabled: bool,
    may_ask: bool,
}

async fn access(Viewer(who): Viewer, State(state): State<AppState>) -> Result<Json<AccessView>> {
    let access = melyxar_app::requests::access::of(&state, &who).await?;
    Ok(Json(AccessView {
        enabled: access.enabled,
        may_ask: access.may_ask,
    }))
}

#[derive(Debug, Serialize)]
struct RequestView {
    id: String,
    user_name: String,
    catalogue: String,
    tmdb_id: String,
    title: String,
    year: Option<i32>,
    poster: Option<String>,
    overview: Option<String>,
    seasons: Vec<i32>,
    note: String,
    /// pending, accepted, refused, added.
    state: String,
    answer: String,
    /// The work it became, once arrived.
    work_id: Option<String>,
    created_at: String,
    decided_at: Option<String>,
}

fn request_view(state: &AppState, request: &TitleRequest) -> RequestView {
    RequestView {
        id: request.id.to_string(),
        user_name: request.user_name.clone(),
        catalogue: request.catalogue.clone(),
        tmdb_id: request.tmdb_id.clone(),
        title: request.title.clone(),
        year: request.year,
        poster: poster_of(state, request.poster_path.as_deref()),
        overview: request.overview.clone(),
        seasons: request.seasons.clone(),
        note: request.note.clone(),
        state: request.state.clone(),
        answer: request.answer.clone(),
        work_id: request.work_id.map(|work| work.to_string()),
        created_at: melyxar_core::time::to_text(request.created_at),
        decided_at: request.decided_at.map(melyxar_core::time::to_text),
    }
}

#[derive(Debug, Deserialize)]
struct Looked {
    query: String,
    /// The language the page reads in, which the answers are written in.
    language: String,
}

#[derive(Debug, Serialize)]
struct HeldView {
    /// The work to open, when this account may read where it is.
    work_id: Option<String>,
    seasons: Vec<i32>,
}

#[derive(Debug, Serialize)]
struct FoundView {
    catalogue: &'static str,
    tmdb_id: String,
    title: String,
    original_title: Option<String>,
    year: Option<i32>,
    overview: Option<String>,
    poster: Option<String>,
    held: Option<HeldView>,
    asked_by: usize,
    /// This account's own request for it, while it waits.
    mine: Option<String>,
}

fn found_view(found: Found) -> FoundView {
    let candidate = found.candidate;
    FoundView {
        catalogue: word_of(candidate.catalogue),
        tmdb_id: candidate.external_id,
        title: candidate.title,
        original_title: candidate.original_title,
        year: candidate.release_year,
        overview: candidate.overview,
        poster: found.poster,
        held: found.held.map(|held| HeldView {
            work_id: held.work_id.map(|work| work.to_string()),
            seasons: held.seasons,
        }),
        asked_by: found.asked_by,
        mine: found.mine.map(|id| id.to_string()),
    }
}

async fn search(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Query(looked): Query<Looked>,
) -> Result<Json<Vec<FoundView>>> {
    let provider = provider_of(&state)?;
    let found = melyxar_app::requests::search::look_for(
        &state,
        &provider,
        &who,
        &looked.query,
        &looked.language,
    )
    .await?;
    Ok(Json(found.into_iter().map(found_view).collect()))
}

#[derive(Debug, Deserialize)]
struct Language {
    language: String,
}

#[derive(Debug, Serialize)]
struct SeasonView {
    number: i32,
    episodes: i32,
    held: bool,
}

async fn seasons(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(asked): Query<Language>,
) -> Result<Json<Vec<SeasonView>>> {
    let provider = provider_of(&state)?;
    let seasons =
        melyxar_app::requests::search::seasons_of(&state, &provider, &who, &id, &asked.language)
            .await?;
    Ok(Json(
        seasons
            .into_iter()
            .map(|season: SeasonChoice| SeasonView {
                number: season.number,
                episodes: season.episodes,
                held: season.held,
            })
            .collect(),
    ))
}

async fn mine(Viewer(who): Viewer, State(state): State<AppState>) -> Result<Json<Vec<RequestView>>> {
    let requests = melyxar_app::requests::asking::mine(&state, &who).await?;
    Ok(Json(requests.iter().map(|request| request_view(&state, request)).collect()))
}

#[derive(Debug, Deserialize)]
struct Asked {
    catalogue: String,
    tmdb_id: String,
    #[serde(default)]
    seasons: Vec<i32>,
    #[serde(default)]
    note: String,
    language: String,
}

async fn ask(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Json(asked): Json<Asked>,
) -> Result<Json<RequestView>> {
    let provider = provider_of(&state)?;
    let asking = Asking {
        catalogue: parse_catalogue(&asked.catalogue)?,
        tmdb_id: asked.tmdb_id,
        seasons: asked.seasons,
        note: asked.note,
    };
    let request =
        melyxar_app::requests::asking::ask(&state, &provider, &who, asking, &asked.language).await?;
    Ok(Json(request_view(&state, &request)))
}

async fn withdraw(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<()>> {
    melyxar_app::requests::asking::withdraw(&state, &who, parse_request(&id)?).await?;
    Ok(Json(()))
}

#[derive(Debug, Serialize)]
struct AskerView {
    id: String,
    name: String,
    administrator: bool,
    may_ask: bool,
}

#[derive(Debug, Serialize)]
struct WaitingView {
    catalogue: &'static str,
    tmdb_id: String,
    accepted: bool,
    /// Every open request for the title, oldest first.
    requests: Vec<RequestView>,
}

#[derive(Debug, Serialize)]
struct AdministrationView {
    enabled: bool,
    askers: Vec<AskerView>,
    waiting: Vec<WaitingView>,
}

async fn for_the_administrator(
    _: Administrator,
    State(state): State<AppState>,
) -> Result<Json<AdministrationView>> {
    let enabled = melyxar_app::requests::access::is_on(&state).await?;
    let askers = melyxar_app::requests::access::askers(&state).await?;
    let waiting = melyxar_app::requests::deciding::waiting(&state).await?;
    Ok(Json(AdministrationView {
        enabled,
        askers: askers
            .into_iter()
            .map(|asker: Asker| AskerView {
                id: asker.id.to_string(),
                name: asker.name,
                administrator: asker.administrator,
                may_ask: asker.may_ask,
            })
            .collect(),
        waiting: waiting
            .iter()
            .map(|title: &Waiting| WaitingView {
                catalogue: word_of(title.catalogue),
                tmdb_id: title.tmdb_id.clone(),
                accepted: title.accepted(),
                requests: title
                    .requests
                    .iter()
                    .map(|request| request_view(&state, request))
                    .collect(),
            })
            .collect(),
    }))
}

#[derive(Debug, Deserialize)]
struct Switched {
    enabled: bool,
}

async fn switch(
    _: Administrator,
    State(state): State<AppState>,
    Json(switched): Json<Switched>,
) -> Result<Json<()>> {
    melyxar_app::requests::access::switch(&state, switched.enabled).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct Allowed {
    may_ask: bool,
}

async fn allow(
    _: Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(allowed): Json<Allowed>,
) -> Result<Json<()>> {
    melyxar_app::requests::access::allow(&state, parse_account(&id)?, allowed.may_ask).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct Decided {
    catalogue: String,
    tmdb_id: String,
    /// accepted, refused or added.
    decision: String,
    #[serde(default)]
    answer: String,
}

async fn decide(
    _: Administrator,
    State(state): State<AppState>,
    Json(decided): Json<Decided>,
) -> Result<Json<()>> {
    let decision = match decided.decision.as_str() {
        "accepted" => Decision::Accepted,
        "refused" => Decision::Refused,
        "added" => Decision::Added,
        _ => return Err(ServerError::invalid_input("no such decision")),
    };
    melyxar_app::requests::deciding::decide(
        &state,
        parse_catalogue(&decided.catalogue)?,
        &decided.tmdb_id,
        decision,
        &decided.answer,
    )
    .await?;
    Ok(Json(()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    /// A refusal the interface has no words for reaches the screen as its
    /// key.
    #[test]
    fn every_reason_a_request_is_refused_for_has_words_in_both_languages() {
        let words = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/i18n.ts"),
        )
        .expect("the words of the interface");

        for refused in melyxar_app::requests::Refused::ALL {
            let key = format!("refused.requests.{}", refused.as_str());
            assert_eq!(
                words.matches(&format!("\"{key}\":")).count(),
                2,
                "{key} needs a sentence in English and one in French"
            );
        }
    }
}
