//! A work's details written by hand, which only an administrator may do.
//!
//! What may be written and which fields a look up then leaves alone both live
//! with the use case; this only carries them.

use axum::extract::{Path, State};
use axum::{Json, Router};
use melyxar_app::hand_edits::{WrittenCredit, WrittenDetails, EDITABLE_FIELDS, ROLES};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};
use crate::identifiers::parse_work;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/works/{id}/details",
        axum::routing::get(read).put(write),
    )
}

/// What a work says about itself, which of it is locked, and every field that
/// may be.
#[derive(Debug, Serialize, Deserialize)]
struct DetailsView {
    title: String,
    tagline: Option<String>,
    overview: Option<String>,
    release_year: Option<i32>,
    /// Year, month, day.
    release_date: Option<String>,
    end_date: Option<String>,
    community_rating: Option<f64>,
    age_rating: Option<String>,
    genres: Vec<String>,
    studios: Vec<String>,
    /// Everybody credited, in the order the page shows them.
    credits: Vec<CreditView>,
    /// The fields no look up changes any more.
    locked: Vec<String>,
    /// The roles somebody may be credited in. Only sent.
    #[serde(default, skip_deserializing)]
    roles: Vec<&'static str>,
}

/// One person credited, and who they play when they act.
#[derive(Debug, Serialize, Deserialize)]
struct CreditView {
    name: String,
    role: String,
    character: Option<String>,
}

async fn read(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DetailsView>> {
    current(&state, parse_work(&id)?).await
}

async fn write(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(given): Json<DetailsView>,
) -> Result<Json<DetailsView>> {
    let work_id = parse_work(&id)?;
    if let Some(unknown) = given
        .locked
        .iter()
        .find(|field| !EDITABLE_FIELDS.contains(&field.as_str()))
    {
        return Err(ServerError::invalid_input(format!("no field goes by {unknown}")));
    }
    let details = WrittenDetails {
        title: given.title,
        tagline: given.tagline,
        overview: given.overview,
        release_year: given.release_year,
        release_date: given.release_date,
        end_date: given.end_date,
        community_rating: given.community_rating,
        age_rating_label: given.age_rating,
        genres: given.genres,
        studios: given.studios,
        credits: given
            .credits
            .into_iter()
            .map(|credit| WrittenCredit {
                name: credit.name,
                role: credit.role,
                character: credit.character,
            })
            .collect(),
    };
    if !melyxar_app::hand_edits::write(&state, work_id, details, &given.locked).await? {
        return Err(ServerError::not_found("work"));
    }
    current(&state, work_id).await
}

async fn current(state: &AppState, work_id: melyxar_core::id::WorkId) -> Result<Json<DetailsView>> {
    let held = melyxar_app::hand_edits::details_of(state, work_id)
        .await?
        .ok_or_else(|| ServerError::not_found("work"))?;
    Ok(Json(view(held.details, held.locked)))
}

fn view(details: WrittenDetails, locked: Vec<String>) -> DetailsView {
    DetailsView {
        title: details.title,
        tagline: details.tagline,
        overview: details.overview,
        release_year: details.release_year,
        release_date: details.release_date,
        end_date: details.end_date,
        community_rating: details.community_rating,
        age_rating: details.age_rating_label,
        genres: details.genres,
        studios: details.studios,
        credits: details
            .credits
            .into_iter()
            .map(|credit| CreditView {
                name: credit.name,
                role: credit.role,
                character: credit.character,
            })
            .collect(),
        locked,
        roles: ROLES.to_vec(),
    }
}
