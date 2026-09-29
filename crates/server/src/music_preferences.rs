//! The routes of what an account chose for its music: read whole, and
//! written whole, since there are few enough of them to send every time.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_core::music_preferences::{FilmOnScreen, MusicPreferences, bounded_ceiling};
use serde::{Deserialize, Serialize};

use crate::account::Viewer;
use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/music/preferences", get(read).put(write))
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct MusicPreferencesView {
    /// stop or pause.
    film_on_screen: String,
    resume_queue: bool,
    /// Nothing for every song as it is.
    max_bitrate_kbps: Option<u32>,
}

fn answer(chosen: &MusicPreferences) -> Json<MusicPreferencesView> {
    Json(MusicPreferencesView {
        film_on_screen: chosen.film_on_screen.as_str().to_string(),
        resume_queue: chosen.resume_queue,
        max_bitrate_kbps: chosen.max_bitrate_kbps,
    })
}

async fn read(State(state): State<AppState>, Viewer(who): Viewer) -> Result<Json<MusicPreferencesView>> {
    Ok(answer(
        &melyxar_app::music::preferences::preferences(&state, &who).await?,
    ))
}

async fn write(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Json(body): Json<MusicPreferencesView>,
) -> Result<Json<MusicPreferencesView>> {
    let chosen = chosen_from(&body)?;
    Ok(answer(
        &melyxar_app::music::preferences::set_preferences(&state, &who, &chosen).await?,
    ))
}

fn chosen_from(body: &MusicPreferencesView) -> Result<MusicPreferences> {
    Ok(MusicPreferences {
        film_on_screen: FilmOnScreen::parse(&body.film_on_screen).ok_or_else(|| {
            ServerError::invalid_input("a film either stops the music or pauses it")
        })?,
        resume_queue: body.resume_queue,
        max_bitrate_kbps: body.max_bitrate_kbps.map(bounded_ceiling),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn what_is_sent_is_read_back_with_its_ceiling_held_in_bounds() {
        let chosen = chosen_from(&MusicPreferencesView {
            film_on_screen: "pause".to_string(),
            resume_queue: false,
            max_bitrate_kbps: Some(9000),
        });
        let Ok(chosen) = chosen else {
            panic!("a choice that exists is read");
        };
        assert_eq!(chosen.film_on_screen, FilmOnScreen::Pause);
        assert_eq!(chosen.max_bitrate_kbps, Some(320));
        assert!(
            chosen_from(&MusicPreferencesView {
                film_on_screen: "louder".to_string(),
                resume_queue: true,
                max_bitrate_kbps: None,
            })
            .is_err()
        );
    }
}
