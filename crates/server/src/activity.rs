//! The activity journal, for the administration.
//!
//! Translation only: which lines are the app's to decide.

use axum::extract::{Query, State};
use axum::{Json, Router};
use melyxar_app::activity::Category;
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::account::Administrator;
use crate::error::{Result, ServerError};

/// The most lines one page of the journal carries.
const LARGEST_PAGE: i64 = 200;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/system/activity", axum::routing::get(activity))
        .route(
            "/api/v1/system/activity/kept",
            axum::routing::get(kept_days).put(keep_days),
        )
}

#[derive(Debug, Deserialize)]
struct ActivityAsked {
    /// Families, comma separated: access, playback, library, server. Every
    /// family when absent.
    #[serde(default)]
    families: Option<String>,
    /// The last line already shown, for the page after it.
    #[serde(default)]
    before: Option<String>,
    #[serde(default)]
    most: Option<i64>,
}

#[derive(Debug, Serialize)]
struct LineView {
    id: String,
    at: String,
    kind: String,
    /// information, attention or trouble.
    level: &'static str,
    user_id: Option<String>,
    /// Where the account's picture is served, when it has one.
    user_avatar: Option<String>,
    work_id: Option<String>,
    /// What the browser said it was.
    device: Option<String>,
    /// Everything else the line says: names, titles, figures.
    details: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ActivityView {
    lines: Vec<LineView>,
    /// Whether there are older lines than these.
    more: bool,
}

async fn activity(
    _: Administrator,
    State(state): State<AppState>,
    Query(asked): Query<ActivityAsked>,
) -> Result<Json<ActivityView>> {
    let families = asked
        .families
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .filter(|word| !word.is_empty())
        .map(|word| {
            Category::from_word(word)
                .ok_or_else(|| ServerError::invalid_input("no such family of the journal"))
        })
        .collect::<Result<Vec<_>>>()?;
    let before = asked
        .before
        .as_deref()
        .map(|id| {
            id.parse()
                .map_err(|_| ServerError::invalid_input("the line identifier is malformed"))
        })
        .transpose()?;
    let most = asked.most.unwrap_or(50).clamp(1, LARGEST_PAGE);

    // One more than asked, to know whether there is a page after.
    let mut lines = melyxar_app::activity::page(&state, &families, before, most + 1).await?;
    let more = lines.len() > usize::try_from(most).unwrap_or(usize::MAX);
    lines.truncate(usize::try_from(most).unwrap_or(usize::MAX));
    Ok(Json(ActivityView {
        more,
        lines: lines
            .into_iter()
            .map(|line| LineView {
                id: line.id.to_string(),
                at: melyxar_core::time::to_text(line.at),
                kind: line.kind,
                level: line.level.as_str(),
                user_id: line.user.map(|user| user.to_string()),
                user_avatar: line.avatar.as_deref().map(crate::images::face_url),
                work_id: line.work.map(|work| work.to_string()),
                device: line.device,
                details: line.details,
            })
            .collect(),
    }))
}

#[derive(Debug, Serialize, Deserialize)]
struct KeptView {
    days: i64,
}

async fn kept_days(_: Administrator, State(state): State<AppState>) -> Result<Json<KeptView>> {
    Ok(Json(KeptView {
        days: melyxar_app::activity::kept_days(&state).await?,
    }))
}

async fn keep_days(
    _: Administrator,
    State(state): State<AppState>,
    Json(asked): Json<KeptView>,
) -> Result<Json<KeptView>> {
    Ok(Json(KeptView {
        days: melyxar_app::activity::keep_days(&state, asked.days).await?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }
}
