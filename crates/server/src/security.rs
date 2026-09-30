//! How the way in is protected, for the administration.
//!
//! Translation only: the bounds are the app's to decide.

use axum::extract::State;
use axum::{Json, Router};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::account::Administrator;
use crate::error::Result;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/system/security/tries",
        axum::routing::get(tries).put(set_tries),
    )
}

#[derive(Debug, Serialize, Deserialize)]
struct TriesView {
    tries: i64,
}

async fn tries(_: Administrator, State(state): State<AppState>) -> Result<Json<TriesView>> {
    Ok(Json(TriesView {
        tries: melyxar_app::accounts::sign_in_tries(&state).await?,
    }))
}

async fn set_tries(
    _: Administrator,
    State(state): State<AppState>,
    Json(asked): Json<TriesView>,
) -> Result<Json<TriesView>> {
    Ok(Json(TriesView {
        tries: melyxar_app::accounts::set_sign_in_tries(&state, asked.tries).await?,
    }))
}
