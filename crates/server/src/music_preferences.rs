//! The routes of what was chosen for music: by an account, and for a library
//! of it by an administrator. Read whole, and written whole, since there are
//! few enough of them to send every time.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_core::music_preferences::{
    FilmOnScreen, LONGEST_CROSSFADE_SECONDS, MusicLibraryOptions, MusicPreferences, VolumeMode,
    bounded_ceiling,
};
use serde::{Deserialize, Serialize};

use crate::account::{Administrator, Viewer};
use crate::error::{Result, ServerError};
use crate::identifiers::parse_library;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/music/preferences", get(read).put(write))
        .route(
            "/api/v1/music/{library}/options",
            get(read_library_options).put(write_library_options),
        )
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct MusicPreferencesView {
    /// stop or pause.
    film_on_screen: String,
    resume_queue: bool,
    /// Nothing for every song as it is.
    max_bitrate_kbps: Option<u32>,
    /// off, track or album.
    volume_mode: String,
    /// Nought for songs that follow one another without a gap.
    crossfade_seconds: u32,
    /// Absent from what an older screen sends, which keeps the preview on.
    #[serde(default = "shown")]
    tag_preview: bool,
}

fn shown() -> bool {
    true
}

fn answer(chosen: &MusicPreferences) -> Json<MusicPreferencesView> {
    Json(MusicPreferencesView {
        film_on_screen: chosen.film_on_screen.as_str().to_string(),
        resume_queue: chosen.resume_queue,
        max_bitrate_kbps: chosen.max_bitrate_kbps,
        volume_mode: chosen.volume_mode.as_str().to_string(),
        crossfade_seconds: chosen.crossfade_seconds,
        tag_preview: chosen.tag_preview,
    })
}

async fn read(
    State(state): State<AppState>,
    Viewer(who): Viewer,
) -> Result<Json<MusicPreferencesView>> {
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
        volume_mode: VolumeMode::parse(&body.volume_mode).ok_or_else(|| {
            ServerError::invalid_input("songs are levelled off, by track or by album")
        })?,
        crossfade_seconds: body.crossfade_seconds.min(LONGEST_CROSSFADE_SECONDS),
        tag_preview: body.tag_preview,
    })
}

#[derive(Debug, Serialize, Deserialize)]
struct LibraryOptionsView {
    lyrics_online: bool,
    #[serde(default)]
    tag_writing: bool,
    #[serde(default)]
    covers_online: bool,
    #[serde(default)]
    artist_photos_online: bool,
}

async fn read_library_options(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(library): Path<String>,
) -> Result<Json<LibraryOptionsView>> {
    let options =
        melyxar_app::music::preferences::library_options(&state, &who, parse_library(&library)?)
            .await?;
    Ok(Json(LibraryOptionsView {
        lyrics_online: options.lyrics_online,
        tag_writing: options.tag_writing,
        covers_online: options.covers_online,
        artist_photos_online: options.artist_photos_online,
    }))
}

async fn write_library_options(
    State(state): State<AppState>,
    _: Administrator,
    Viewer(who): Viewer,
    Path(library): Path<String>,
    Json(body): Json<LibraryOptionsView>,
) -> Result<Json<LibraryOptionsView>> {
    let options = melyxar_app::music::preferences::set_library_options(
        &state,
        &who,
        parse_library(&library)?,
        &MusicLibraryOptions {
            lyrics_online: body.lyrics_online,
            tag_writing: body.tag_writing,
            covers_online: body.covers_online,
            artist_photos_online: body.artist_photos_online,
        },
    )
    .await?;
    Ok(Json(LibraryOptionsView {
        lyrics_online: options.lyrics_online,
        tag_writing: options.tag_writing,
        covers_online: options.covers_online,
        artist_photos_online: options.artist_photos_online,
    }))
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
            volume_mode: "album".to_string(),
            crossfade_seconds: 60,
            tag_preview: true,
        });
        let Ok(chosen) = chosen else {
            panic!("a choice that exists is read");
        };
        assert_eq!(chosen.film_on_screen, FilmOnScreen::Pause);
        assert_eq!(chosen.max_bitrate_kbps, Some(320));
        assert_eq!(chosen.volume_mode, VolumeMode::Album);
        assert_eq!(chosen.crossfade_seconds, LONGEST_CROSSFADE_SECONDS);
        assert!(
            chosen_from(&MusicPreferencesView {
                film_on_screen: "louder".to_string(),
                resume_queue: true,
                max_bitrate_kbps: None,
                volume_mode: "track".to_string(),
                crossfade_seconds: 0,
                tag_preview: true,
            })
            .is_err()
        );
    }
}
