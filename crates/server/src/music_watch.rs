//! What the page saw of the two decks of the player of music, written into
//! the journal under the `music` tag.
//!
//! Twice a second song was heard playing over the first, on a phone, with
//! nothing anywhere saying which deck held which or when the next one was
//! started: the server sends the sound and never hears it. Like `page`, this
//! is a closed list of facts the server names itself.

use axum::{Json, Router};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::identifiers::parse_work;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/system/journal/music",
        axum::routing::post(what_the_page_saw),
    )
}

/// When the two decks were written down.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Moment {
    /// The next song has just taken over from the one before.
    HandedOver,
    /// Both decks have been sounding together for longer than any fade.
    BothSounding,
}

impl Moment {
    fn as_word(self) -> &'static str {
        match self {
            Self::HandedOver => "handed over",
            Self::BothSounding => "both sounding",
        }
    }
}

/// One fact, named by the server.
#[derive(Debug, Deserialize)]
#[serde(tag = "saw", rename_all = "snake_case")]
enum Seen {
    /// Both decks, as the page last heard them.
    Decks {
        moment: Moment,
        /// The deck the page takes for the one playing.
        live_deck: u8,
        /// The crossfade asked for, in seconds.
        fade_seconds: f64,
        /// Seconds into what each deck plays, and its length when known.
        deck0_second: f64,
        #[serde(default)]
        deck0_length: Option<f64>,
        deck0_sounding: bool,
        deck1_second: f64,
        #[serde(default)]
        deck1_length: Option<f64>,
        deck1_sounding: bool,
        /// How long both have been sounding together, in seconds.
        together_seconds: f64,
    },
}

#[derive(Debug, Deserialize)]
struct FromThePage {
    song: String,
    #[serde(flatten)]
    seen: Seen,
}

#[derive(Debug, Serialize)]
struct Written {
    written: bool,
}

async fn what_the_page_saw(
    _: crate::account::Viewer,
    Json(said): Json<FromThePage>,
) -> Result<Json<Written>> {
    // The identifier is a name from outside: read as one, written down only
    // once it parses.
    let song = tracing::field::display(parse_work(&said.song)?);
    match said.seen {
        Seen::Decks {
            moment,
            live_deck,
            fade_seconds,
            deck0_second,
            deck0_length,
            deck0_sounding,
            deck1_second,
            deck1_length,
            deck1_sounding,
            together_seconds,
        } => tracing::debug!(
            song,
            moment = moment.as_word(),
            live_deck,
            fade_seconds,
            deck0_second,
            deck0_length,
            deck0_sounding,
            deck1_second,
            deck1_length,
            deck1_sounding,
            together_seconds,
            "the two decks of the music"
        ),
    }
    Ok(Json(Written { written: true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    #[test]
    fn the_decks_are_read_and_anything_else_is_refused() {
        let song = "01a0f0c6-c1fa-769d-9f83-3467c8c4586b";
        let decks = format!(
            r#"{{"song":"{song}","saw":"decks","moment":"both_sounding","live_deck":1,"fade_seconds":0,
                "deck0_second":143.2,"deck0_length":null,"deck0_sounding":true,
                "deck1_second":61.5,"deck1_length":212.4,"deck1_sounding":true,"together_seconds":2.5}}"#
        );
        match serde_json::from_str::<FromThePage>(&decks).expect("read").seen {
            Seen::Decks {
                moment,
                live_deck,
                deck0_length,
                deck1_length,
                together_seconds,
                ..
            } => {
                assert_eq!(moment.as_word(), "both sounding");
                assert_eq!(live_deck, 1);
                assert_eq!(deck0_length, None);
                assert_eq!(deck1_length, Some(212.4));
                assert_eq!(together_seconds, 2.5);
            }
        }
        for refused in [
            format!(r#"{{"song":"{song}","saw":"anything_else","message":"hi"}}"#),
            decks.replace("both_sounding", "whenever"),
        ] {
            assert!(serde_json::from_str::<FromThePage>(&refused).is_err(), "{refused}");
        }
    }
}
