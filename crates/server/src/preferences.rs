//! What the viewer has decided about how films reach them.
//!
//! Read and written whole rather than field by field: a screen shows them
//! together, and a client that sends back what it was given cannot leave a
//! field behind by forgetting to mention it.

use axum::extract::State;
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_core::user::{DownmixMethod, Preferences};
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};
use crate::account::Viewer;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/preferences", axum::routing::get(read).put(write))
}

/// What a viewer has chosen, and what they can choose between.
#[derive(Debug, Serialize)]
struct PreferencesView {
    /// Preferred soundtrack language, as a three letter code. Absent means no
    /// preference, and the file decides.
    preferred_audio_language: Option<String>,
    preferred_subtitle_language: Option<String>,
    /// none, centre_and_bass_split, night_dialogue, intensity_preserving or
    /// broadcast_standard.
    downmix_method: &'static str,
    downmix_gain: f64,
    /// The range the gain is kept inside, so a screen can draw a slider that
    /// cannot be dragged somewhere the server would refuse.
    downmix_gain_range: [f64; 2],
    /// Every fold this server knows how to perform.
    downmix_methods: Vec<&'static str>,
    /// The languages the library actually holds, which is what a picker
    /// offers: a list of every language in the world ends with someone
    /// choosing one no film in the house carries.
    audio_languages: Vec<String>,
    subtitle_languages: Vec<String>,
}

/// What a client sends back. Every field is optional so a screen can change
/// one thing without having to know the others.
#[derive(Debug, Default, Deserialize)]
struct PreferencesBody {
    #[serde(default)]
    preferred_audio_language: Option<String>,
    #[serde(default)]
    preferred_subtitle_language: Option<String>,
    #[serde(default)]
    downmix_method: Option<String>,
    #[serde(default)]
    downmix_gain: Option<f64>,
}

async fn read(
    State(state): State<AppState>,
    Viewer(who): Viewer,
) -> Result<Json<PreferencesView>> {
    let chosen = melyxar_app::preferences::of(&state, who.id).await?;
    view(&state, chosen).await
}

async fn write(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Json(body): Json<PreferencesBody>,
) -> Result<Json<PreferencesView>> {
    let mut chosen = melyxar_app::preferences::of(&state, who.id).await?;

    // An empty answer means no preference, which is a choice of its own and
    // not the same as leaving the field out.
    if let Some(language) = body.preferred_audio_language {
        chosen.preferred_audio_language = some_language(language);
    }
    if let Some(language) = body.preferred_subtitle_language {
        chosen.preferred_subtitle_language = some_language(language);
    }
    if let Some(method) = body.downmix_method {
        chosen.downmix_method = DownmixMethod::parse(&method)
            .ok_or_else(|| ServerError::invalid_input("no fold to stereo goes by that name"))?;
    }
    if let Some(gain) = body.downmix_gain {
        chosen.downmix_gain = gain;
    }

    let kept = melyxar_app::preferences::save(&state, who.id, chosen).await?;
    view(&state, kept).await
}

fn some_language(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

async fn view(state: &AppState, chosen: Preferences) -> Result<Json<PreferencesView>> {
    let available = melyxar_app::preferences::languages_available(state).await?;
    Ok(Json(PreferencesView {
        preferred_audio_language: chosen.preferred_audio_language,
        preferred_subtitle_language: chosen.preferred_subtitle_language,
        downmix_method: chosen.downmix_method.as_str(),
        downmix_gain: chosen.downmix_gain,
        downmix_gain_range: [
            melyxar_core::user::MIN_DOWNMIX_GAIN,
            melyxar_core::user::MAX_DOWNMIX_GAIN,
        ],
        downmix_methods: DownmixMethod::every()
            .iter()
            .map(|one| one.as_str())
            .collect(),
        audio_languages: available.audio,
        subtitle_languages: available.subtitle,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    #[test]
    fn an_empty_answer_means_no_preference_rather_than_an_empty_language() {
        // Stored as an empty string, it would be looked for among the tracks
        // and never found, which is a preference that silently does nothing.
        assert_eq!(some_language(String::new()), None);
        assert_eq!(some_language("   ".to_string()), None);
        assert_eq!(some_language("fre".to_string()), Some("fre".to_string()));
        assert_eq!(some_language(" fre ".to_string()), Some("fre".to_string()));
    }
}
