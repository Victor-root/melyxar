//! Settings of the server itself, rather than of a library or of a viewer.
//!
//! Whether wide gamut colour is ever converted for a client that cannot show
//! it: a real switch for a real problem, a processor too slow to rebuild a
//! picture whose only fault is its colour. And what the server is called, the
//! logo it wears and what stands behind its sign in screen. More
//! belongs here as branding and maintenance reach the interface, which is why
//! this is its own small module rather than a corner of another one.

use axum::extract::State;
use axum::{Json, Router};
use melyxar_app::ratings::KeyTried;
use melyxar_app::settings::TranscodingLimits;
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::error::Result;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/settings/playback",
            axum::routing::get(playback_settings).put(set_playback_settings),
        )
        .route("/api/v1/settings/server", axum::routing::get(server_settings))
        .route(
            "/api/v1/settings/server/name",
            axum::routing::put(rename_server).delete(forget_name),
        )
        .route(
            "/api/v1/settings/server/logo",
            axum::routing::put(choose_logo)
                .delete(remove_logo)
                // A picture straight off a phone is larger than what any other
                // request is allowed to carry, the same as a profile picture.
                .layer(axum::extract::DefaultBodyLimit::max(
                    melyxar_app::avatars::LARGEST,
                )),
        )
        .route(
            "/api/v1/settings/server/door/background",
            axum::routing::put(choose_door_background),
        )
        .route(
            "/api/v1/settings/server/theme",
            axum::routing::put(choose_default_theme),
        )
        .route(
            "/api/v1/settings/server/door/slogan",
            axum::routing::put(write_door_slogan),
        )
        .route(
            "/api/v1/settings/ratings",
            axum::routing::get(ratings_settings)
                .put(set_omdb_key)
                .delete(forget_omdb_key),
        )
        .route(
            "/api/v1/settings/server/door/picture",
            axum::routing::put(choose_door_picture)
                .delete(remove_door_picture)
                .layer(axum::extract::DefaultBodyLimit::max(
                    melyxar_app::avatars::LARGEST,
                )),
        )
}

/// What the server is called, what it would be called given back its own
/// name, and where its logo is, when it has one.
#[derive(Debug, Serialize)]
struct ServerView {
    server_name: String,
    default_name: &'static str,
    logo: Option<String>,
    logo_icon: Option<String>,
    /// Which drawn background the sign in screen wears when no picture is
    /// there.
    door_background: &'static str,
    /// Where the picture behind the sign in screen is, when there is one.
    door_picture: Option<String>,
    /// The line under the server's name, or nothing for Melyxar's own.
    door_slogan: Option<String>,
    longest_slogan: usize,
    /// The theme of whoever has not chosen one.
    default_theme: &'static str,
}

impl ServerView {
    async fn of(state: &AppState) -> Result<Json<Self>> {
        let identity = melyxar_app::server::identity(state).await?;
        let door = melyxar_app::server::door(state).await?;
        Ok(Json(Self {
            server_name: identity.name,
            default_name: melyxar_app::server::DEFAULT_NAME,
            logo: identity.logo.as_deref().map(crate::images::logo_url),
            logo_icon: identity
                .logo
                .as_deref()
                .map(crate::installing::logo_icon_url),
            door_background: door.background.as_str(),
            door_picture: door.picture.as_deref().map(crate::images::door_picture_url),
            door_slogan: door.slogan,
            longest_slogan: melyxar_app::server::LONGEST_SLOGAN,
            default_theme: melyxar_app::server::default_theme(state).await?.as_str(),
        }))
    }
}

#[derive(Debug, Deserialize)]
struct NameAsked {
    server_name: String,
}

async fn server_settings(
    State(state): State<AppState>,
    _: crate::account::Administrator,
) -> Result<Json<ServerView>> {
    ServerView::of(&state).await
}

/// Calls the server something else.
async fn rename_server(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    Json(asked): Json<NameAsked>,
) -> Result<Json<ServerView>> {
    melyxar_app::server::rename(&state, &asked.server_name).await?;
    ServerView::of(&state).await
}

/// Gives the server back the name it came with.
async fn forget_name(
    State(state): State<AppState>,
    _: crate::account::Administrator,
) -> Result<Json<ServerView>> {
    melyxar_app::server::forget_name(&state).await?;
    ServerView::of(&state).await
}

/// Makes the image sent the server's logo.
async fn choose_logo(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    image: axum::body::Bytes,
) -> Result<Json<ServerView>> {
    melyxar_app::server::set_logo(&state, &image).await?;
    ServerView::of(&state).await
}

/// Takes the server's logo away, which puts Melyxar's own back.
async fn remove_logo(
    State(state): State<AppState>,
    _: crate::account::Administrator,
) -> Result<Json<ServerView>> {
    melyxar_app::server::remove_logo(&state).await?;
    ServerView::of(&state).await
}

#[derive(Debug, Deserialize)]
struct DoorBackgroundAsked {
    door_background: String,
}

/// Chooses the drawn background the sign in screen wears when no picture is
/// there.
async fn choose_door_background(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    Json(asked): Json<DoorBackgroundAsked>,
) -> Result<Json<ServerView>> {
    let background = melyxar_app::server::LoginBackground::parse(&asked.door_background)
        .ok_or_else(|| {
            crate::error::ServerError::invalid_input("no background goes by that name")
        })?;
    melyxar_app::server::set_door_background(&state, background).await?;
    ServerView::of(&state).await
}

#[derive(Debug, Deserialize)]
struct ThemeAsked {
    default_theme: String,
}

/// Chooses the theme of whoever has not chosen one.
async fn choose_default_theme(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    Json(asked): Json<ThemeAsked>,
) -> Result<Json<ServerView>> {
    let theme = melyxar_core::user::ThemeMode::parse(&asked.default_theme)
        .filter(|theme| *theme != melyxar_core::user::ThemeMode::Server)
        .ok_or_else(|| crate::error::ServerError::invalid_input("no theme goes by that name"))?;
    melyxar_app::server::set_default_theme(&state, theme).await?;
    ServerView::of(&state).await
}

#[derive(Debug, Deserialize)]
struct SloganAsked {
    door_slogan: String,
}

/// Writes the line under the server's name; an empty one gives Melyxar's own
/// back.
async fn write_door_slogan(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    Json(asked): Json<SloganAsked>,
) -> Result<Json<ServerView>> {
    melyxar_app::server::set_door_slogan(&state, &asked.door_slogan).await?;
    ServerView::of(&state).await
}

/// Puts the image sent behind the sign in screen.
async fn choose_door_picture(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    image: axum::body::Bytes,
) -> Result<Json<ServerView>> {
    melyxar_app::server::set_door_picture(&state, &image).await?;
    ServerView::of(&state).await
}

/// Takes the picture away from behind the sign in screen.
async fn remove_door_picture(
    State(state): State<AppState>,
    _: crate::account::Administrator,
) -> Result<Json<ServerView>> {
    melyxar_app::server::remove_door_picture(&state).await?;
    ServerView::of(&state).await
}

/// Where the ratings from elsewhere stand: when IMDb's file was last
/// fetched, and whether a key for OMDb was given, never the key itself.
#[derive(Debug, Serialize)]
struct RatingsView {
    #[serde(with = "time::serde::rfc3339::option")]
    imdb_fetched_at: Option<melyxar_core::time::Timestamp>,
    has_omdb_key: bool,
    /// What trying the key just typed came to: kept, refused or unreachable.
    tried: Option<&'static str>,
}

impl RatingsView {
    async fn of(state: &AppState, tried: Option<KeyTried>) -> Result<Json<Self>> {
        Ok(Json(Self {
            imdb_fetched_at: melyxar_app::ratings::imdb_fetched_at(state).await,
            has_omdb_key: melyxar_app::ratings::has_omdb_key(state).await?,
            tried: tried.map(KeyTried::as_str),
        }))
    }
}

async fn ratings_settings(
    State(state): State<AppState>,
    _: crate::account::Administrator,
) -> Result<Json<RatingsView>> {
    RatingsView::of(&state, None).await
}

#[derive(Debug, Deserialize)]
struct KeyGiven {
    omdb_key: String,
}

/// Tries the OMDb key typed, and keeps it once OMDb has taken it.
async fn set_omdb_key(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    Json(given): Json<KeyGiven>,
) -> Result<Json<RatingsView>> {
    let tried = melyxar_app::ratings::set_omdb_key(&state, &given.omdb_key).await?;
    RatingsView::of(&state, Some(tried)).await
}

/// Forgets the OMDb key, which stops the critics being asked.
async fn forget_omdb_key(
    State(state): State<AppState>,
    _: crate::account::Administrator,
) -> Result<Json<RatingsView>> {
    melyxar_app::ratings::forget_omdb_key(&state).await?;
    RatingsView::of(&state, None).await
}

#[derive(Debug, Serialize, Deserialize)]
struct PlaybackSettingsView {
    /// Never convert wide gamut colour, even where a client cannot show it
    /// correctly. Off by default. Dolby Vision without a compatible base
    /// layer is converted regardless, since left alone it looks broken rather
    /// than merely washed out.
    tone_mapping_disabled: bool,
    /// How many films the server may convert at once. None, the default, is
    /// no ceiling at all.
    #[serde(default)]
    max_transcoding_sessions: Option<u32>,
    /// How much of the disk the segments of those films may fill, in
    /// megabytes. None, the default, is no ceiling at all.
    #[serde(default)]
    transcode_cache_megabytes: Option<u32>,
    /// How far behind each viewer those segments stay whatever the ceiling
    /// says, in seconds.
    transcode_kept_behind_seconds: u32,
    /// The codecs a converted film may come out in, best first.
    transcode_video_codecs: Vec<String>,
}

/// What the server is set to do about wide gamut colour it cannot show a
/// client, and what it allows the films it converts.
async fn playback_settings(
    State(state): State<AppState>,
    _: crate::account::Administrator,
) -> Result<Json<PlaybackSettingsView>> {
    let database = state.database();
    Ok(Json(PlaybackSettingsView::of(
        database
            .tone_mapping_disabled()
            .await
            .map_err(|error| crate::error::ServerError::internal(error.to_string()))?,
        database
            .transcoding_limits()
            .await
            .map_err(|error| crate::error::ServerError::internal(error.to_string()))?,
    )))
}

/// Changes how the whole server plays films: the conversion of wide gamut
/// colour, and what it allows the films it converts.
async fn set_playback_settings(
    State(state): State<AppState>,
    _: crate::account::Administrator,
    Json(asked): Json<PlaybackSettingsView>,
) -> Result<Json<PlaybackSettingsView>> {
    let database = state.database();
    database
        .set_tone_mapping_disabled(asked.tone_mapping_disabled)
        .await
        .map_err(|error| crate::error::ServerError::internal(error.to_string()))?;
    // Brought back into range rather than refused, and answered as kept, so
    // the screen shows what the server really holds.
    let limits = database
        .set_transcoding_limits(TranscodingLimits {
            most_at_once: asked.max_transcoding_sessions,
            cache_megabytes: asked.transcode_cache_megabytes,
            kept_behind_seconds: asked.transcode_kept_behind_seconds,
            video_codecs: asked.transcode_video_codecs,
        })
        .await
        .map_err(|error| crate::error::ServerError::internal(error.to_string()))?;

    tracing::debug!(
        tone_mapping_disabled = asked.tone_mapping_disabled,
        max_transcoding_sessions = limits.most_at_once,
        transcode_cache_megabytes = limits.cache_megabytes,
        transcode_kept_behind_seconds = limits.kept_behind_seconds,
        "the server's playback settings were changed"
    );
    Ok(Json(PlaybackSettingsView::of(
        asked.tone_mapping_disabled,
        limits,
    )))
}

impl PlaybackSettingsView {
    /// The settings as the screen reads them.
    fn of(tone_mapping_disabled: bool, limits: TranscodingLimits) -> Self {
        Self {
            tone_mapping_disabled,
            max_transcoding_sessions: limits.most_at_once,
            transcode_cache_megabytes: limits.cache_megabytes,
            transcode_kept_behind_seconds: limits.kept_behind_seconds,
            transcode_video_codecs: limits.video_codecs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    /// The words live in the interface and the reasons in the app crate, so
    /// only a test crossing from one to the other catches a refusal nobody
    /// worded.
    #[test]
    fn every_reason_a_server_name_is_refused_for_has_words_in_both_languages() {
        let words = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/i18n.ts"),
        )
        .expect("the words of the interface");

        for refused in melyxar_app::server::Refused::ALL {
            let key = format!("refused.server.{}", refused.as_str());
            assert_eq!(
                words.matches(&format!("\"{key}\":")).count(),
                2,
                "{key} needs a sentence in English and one in French"
            );
        }
    }
}
