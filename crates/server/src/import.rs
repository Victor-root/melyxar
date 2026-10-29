//! Taking up what an Emby or Jellyfin server kept of each account, for an
//! administrator: a preview of the data folder named, then the choices
//! written in (see `melyxar_app::import`).

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_app::import::{Choices, Preview, Report};
use serde::Deserialize;

use crate::account::{Administrator, Viewer};
use crate::error::Result;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/import/preview", post(preview))
        .route("/api/v1/import/take_up", post(take_up))
}

/// Where the data folder of the other server, or its copy, is on this machine.
#[derive(Debug, Deserialize)]
struct From {
    path: String,
}

async fn preview(State(state): State<AppState>, _: Administrator, Json(from): Json<From>) -> Result<Json<Preview>> {
    Ok(Json(melyxar_app::import::preview(&state, &from.path).await?))
}

#[derive(Debug, Deserialize)]
struct Chosen {
    path: String,
    #[serde(flatten)]
    choices: Choices,
}

async fn take_up(
    State(state): State<AppState>,
    _: Administrator,
    Viewer(who): Viewer,
    Json(chosen): Json<Chosen>,
) -> Result<Json<Report>> {
    Ok(Json(melyxar_app::import::take_up(&state, &who, &chosen.path, &chosen.choices).await?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    #[test]
    fn the_choices_are_read_beside_the_path() {
        let chosen: Chosen = serde_json::from_str(
            r#"{"path":"/var/lib/emby","accounts":[
                {"becomes":"existing","key":"1","user":"0192f0a4-5c3b-7cc4-9a51-2c9e5c5b2a10"},
                {"becomes":"new","key":"2","name":"Ben","password":"quiet harbour lights"},
                {"becomes":"nothing","key":"3"}],
               "collections":["Rainy days"]}"#,
        )
        .unwrap();
        assert_eq!(chosen.path, "/var/lib/emby");
        assert_eq!(chosen.choices.accounts.len(), 3);
        assert_eq!(chosen.choices.collections, vec!["Rainy days"]);
    }
}
