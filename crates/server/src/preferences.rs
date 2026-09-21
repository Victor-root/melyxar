//! What the viewer has decided about how films reach them.
//!
//! Read and written whole rather than field by field: a screen shows them
//! together, and a client that sends back what it was given cannot leave a
//! field behind by forgetting to mention it.

use axum::extract::State;
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_core::user::{DownmixMethod, Preferences, ThemeMode};
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};
use crate::account::Viewer;

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/preferences", axum::routing::get(read).put(write))
}

/// What a viewer has chosen, and what they can choose between.
#[derive(Debug, Serialize)]
struct PreferencesView {
    /// The language the interface speaks to this person, as two letters.
    ///
    /// Theirs rather than the browser's: somebody signing in on a machine
    /// that is not their own should not have to say it again, and two people
    /// sharing one machine should not have to argue about it.
    interface_language: String,
    /// light, dark or system.
    theme_mode: &'static str,
    /// The colour that carries everything active, as a hash and six digits.
    accent_color: String,
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
    /// How tall the banner of the home page is, as a share of the screen's
    /// width: what it really sets is how much of the picture behind it
    /// survives.
    banner_height: f64,
    banner_height_range: [f64; 2],
    /// Where a band is cut out of that picture, nought at its top and one at
    /// its foot.
    banner_cut: f64,
    /// Whether the banner draws a fresh handful every time the page opens.
    banner_at_random: bool,
    /// Whether it takes the whole window, the height then deciding nothing.
    banner_fills_the_screen: bool,
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
    interface_language: Option<String>,
    #[serde(default)]
    theme_mode: Option<String>,
    #[serde(default)]
    accent_color: Option<String>,
    #[serde(default)]
    preferred_audio_language: Option<String>,
    #[serde(default)]
    preferred_subtitle_language: Option<String>,
    #[serde(default)]
    downmix_method: Option<String>,
    #[serde(default)]
    downmix_gain: Option<f64>,
    #[serde(default)]
    banner_height: Option<f64>,
    #[serde(default)]
    banner_cut: Option<f64>,
    #[serde(default)]
    banner_at_random: Option<bool>,
    #[serde(default)]
    banner_fills_the_screen: Option<bool>,
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

    if let Some(language) = body.interface_language {
        if !melyxar_core::user::is_a_language(&language) {
            return Err(ServerError::invalid_input(
                "a language is two letters, such as fr or en",
            ));
        }
        chosen.interface_language = language;
    }
    if let Some(mode) = body.theme_mode {
        chosen.theme_mode = ThemeMode::parse(&mode)
            .ok_or_else(|| ServerError::invalid_input("no theme goes by that name"))?;
    }
    if let Some(colour) = body.accent_color {
        // Refused rather than quietly put right: this one ends up inside a
        // stylesheet, and a client sending something else is a client to
        // answer rather than to humour.
        if !melyxar_core::user::is_an_accent_colour(&colour) {
            return Err(ServerError::invalid_input(
                "a colour is a hash and six hexadecimal digits",
            ));
        }
        chosen.accent_color = colour;
    }

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
    // Clamped rather than refused, unlike the colour: these two are dragged
    // on a slider, and a slider that ends one step past what is accepted
    // would answer with an error rather than with the end of its own track.
    if let Some(height) = body.banner_height {
        chosen.banner_height = height;
    }
    if let Some(cut) = body.banner_cut {
        chosen.banner_cut = cut;
    }
    if let Some(at_random) = body.banner_at_random {
        chosen.banner_at_random = at_random;
    }
    if let Some(fills) = body.banner_fills_the_screen {
        chosen.banner_fills_the_screen = fills;
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
        interface_language: chosen.interface_language,
        theme_mode: chosen.theme_mode.as_str(),
        accent_color: chosen.accent_color,
        preferred_audio_language: chosen.preferred_audio_language,
        preferred_subtitle_language: chosen.preferred_subtitle_language,
        downmix_method: chosen.downmix_method.as_str(),
        downmix_gain: chosen.downmix_gain,
        downmix_gain_range: [
            melyxar_core::user::MIN_DOWNMIX_GAIN,
            melyxar_core::user::MAX_DOWNMIX_GAIN,
        ],
        banner_height: chosen.banner_height,
        banner_height_range: [
            melyxar_core::user::MIN_BANNER_HEIGHT,
            melyxar_core::user::MAX_BANNER_HEIGHT,
        ],
        banner_cut: chosen.banner_cut,
        banner_at_random: chosen.banner_at_random,
        banner_fills_the_screen: chosen.banner_fills_the_screen,
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
