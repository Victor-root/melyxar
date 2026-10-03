//! What deserves an administrator's look, and the notifications to come.
//!
//! Translation only: which points are the app's to decide.

use axum::extract::State;
use axum::{Json, Router};
use melyxar_app::notifications::attention::Shown;
use melyxar_app::AppState;
use serde::Serialize;

use crate::account::{Administrator, Viewer};
use crate::error::Result;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/system/attention", axum::routing::get(attention))
        .route(
            "/api/v1/system/attention/seen",
            axum::routing::post(mark_seen),
        )
}

#[derive(Debug, Serialize)]
struct AttentionView {
    points: Vec<Shown>,
}

async fn attention(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
) -> Result<Json<AttentionView>> {
    Ok(Json(AttentionView {
        points: melyxar_app::notifications::attention::points(&state, who.id).await?,
    }))
}

async fn mark_seen(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
) -> Result<Json<AttentionView>> {
    melyxar_app::notifications::attention::mark_seen(&state, who.id).await?;
    Ok(Json(AttentionView {
        points: melyxar_app::notifications::attention::points(&state, who.id).await?,
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
