//! Which graphics card converts films, on a machine that may carry several.
//!
//! Every card was proved at start-up, each through its own path. Only one does
//! the work: putting two in one chain would mean handing every picture from
//! one to the other, which nothing does quickly. The administrator chooses
//! it; until then, or when the chosen one is not there any more, it is the one
//! that keeps the most away from the processor.
//!
//! The choice is read each time a film is opened, so a new one applies to the
//! next film without anything being restarted.

use melyxar_ffmpeg::Card;
use serde::Serialize;

use crate::{AppError, AppState, Result};

/// The card that converts films right now, when this machine has one.
pub async fn in_use(state: &AppState) -> Result<Option<Card>> {
    let Some(capabilities) = state.capabilities() else {
        return Ok(None);
    };
    if capabilities.cards().is_empty() {
        return Ok(None);
    }
    let chosen = state.database().transcoding_card().await?;
    Ok(capabilities.card(chosen.as_deref()).cloned())
}

/// One card that passed its trials, as the administration shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardOffered {
    /// What it is chosen by.
    pub key: String,
    pub name: String,
    /// The path it is driven by: vaapi, cuda.
    pub way: &'static str,
    /// The codecs it was proved to write, and to read.
    pub writes: Vec<String>,
    pub reads: Vec<String>,
    pub converts_wide_gamut: bool,
    pub paints_picture_subtitles: bool,
}

impl CardOffered {
    fn of(card: &Card) -> Self {
        Self {
            key: card.key.clone(),
            name: card.name.clone(),
            way: card.way.as_str(),
            writes: card.encoders.keys().cloned().collect(),
            reads: card.decoders.iter().cloned().collect(),
            converts_wide_gamut: card.can_tone_map(),
            paints_picture_subtitles: card.picture_subtitle_layout().is_some(),
        }
    }
}

/// The cards there are to choose from, and where the choice stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardChoice {
    pub cards: Vec<CardOffered>,
    /// The key that was chosen, when one was.
    pub chosen: Option<String>,
    /// The key of the card that converts now.
    pub in_use: Option<String>,
}

impl CardChoice {
    /// Whether a choice was kept for a card that did not pass here this time.
    pub fn chosen_is_missing(&self) -> bool {
        self.chosen
            .as_ref()
            .is_some_and(|chosen| !self.cards.iter().any(|card| &card.key == chosen))
    }
}

/// Where the choice of a card stands.
pub async fn choice(state: &AppState) -> Result<CardChoice> {
    let chosen = state.database().transcoding_card().await?;
    let cards = state
        .capabilities()
        .map(melyxar_ffmpeg::Capabilities::cards)
        .unwrap_or_default();
    let in_use = state
        .capabilities()
        .and_then(|capabilities| capabilities.card(chosen.as_deref()))
        .map(|card| card.key.clone());
    Ok(CardChoice {
        cards: cards.iter().map(CardOffered::of).collect(),
        chosen,
        in_use,
    })
}

/// Chooses the card that converts films, or leaves it to the server with
/// nothing. Only a card that passed here can be chosen.
pub async fn choose(state: &AppState, key: Option<&str>) -> Result<CardChoice> {
    if let Some(key) = key {
        let passed = state
            .capabilities()
            .is_some_and(|capabilities| capabilities.cards().iter().any(|card| card.key == key));
        if !passed {
            return Err(AppError::Domain(melyxar_core::Error::invalid_input(
                "no card that passed its trials here goes by that key",
            )));
        }
    }
    state.database().set_transcoding_card(key).await?;
    let choice = choice(state).await?;
    tracing::info!(
        chosen = key,
        in_use = choice.in_use.as_deref(),
        "the card that converts films was chosen"
    );
    Ok(choice)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(key: &str) -> CardOffered {
        CardOffered {
            key: key.to_string(),
            name: "a card".to_string(),
            way: "vaapi",
            writes: vec!["h264".to_string()],
            reads: Vec::new(),
            converts_wide_gamut: false,
            paints_picture_subtitles: false,
        }
    }

    #[test]
    fn a_choice_kept_for_a_card_that_is_gone_is_said_to_be_missing() {
        let gone = CardChoice {
            cards: vec![offered("vaapi:0000:03:00.0")],
            chosen: Some("cuda:0000:0c:00.0".to_string()),
            in_use: Some("vaapi:0000:03:00.0".to_string()),
        };
        assert!(gone.chosen_is_missing());

        let there = CardChoice {
            chosen: Some("vaapi:0000:03:00.0".to_string()),
            ..gone.clone()
        };
        assert!(!there.chosen_is_missing());

        let none = CardChoice {
            chosen: None,
            ..gone
        };
        assert!(!none.chosen_is_missing());
    }

    #[tokio::test]
    async fn a_card_that_did_not_pass_here_cannot_be_chosen() {
        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        let state = AppState::new(melyxar_config::Config::default(), database, None, None);

        assert!(choose(&state, Some("cuda:0000:0c:00.0")).await.is_err());
        assert_eq!(
            state.database().transcoding_card().await.expect("read"),
            None,
            "nothing is kept for a card that is not there"
        );

        let left = choose(&state, None).await.expect("left to the server");
        assert!(left.cards.is_empty());
        assert_eq!(left.in_use, None);
        assert!(in_use(&state).await.expect("read").is_none());
    }
}
