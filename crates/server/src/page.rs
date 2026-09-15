//! What the page saw, written into the journal.
//!
//! Half of what happens when a film starts happens in the browser, and the
//! journal showed none of it: the server could say it had asked for the
//! seventeenth minute while the page asked for the opening of the film, with
//! nothing anywhere saying which of the two was wrong.
//!
//! So the page may write here. What it may not do is choose what it is saying.
//! It sends one fact out of a list this module defines, and the server writes
//! the line itself. A page cannot tell the journal anything it has no
//! vocabulary for, whatever else ends up running in the browser, and reading
//! this file is enough to know exactly what can appear under the `page` tag.
//!
//! One fact carries wording rather than numbers, and only one: a library
//! naming what it could not do with a film this server produced, which is the
//! answer and cannot be a number. It is cut short here, and it is still a fact
//! this module named.
//!
//! The rest are numbers about a film playing, which is the half of a reading
//! the server cannot see at all: it knows what it produced and when it handed
//! it over, never whether any of it reached a screen.

use axum::extract::State;
use axum::{Json, Router};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::viewer;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/system/journal/page",
        axum::routing::post(what_the_page_saw),
    )
}

/// One fact, named by the server.
///
/// A closed list on purpose. Adding to it is a change to the server, never a
/// decision the browser can make on its own.
#[derive(Debug, Deserialize)]
#[serde(tag = "saw", rename_all = "snake_case")]
enum Seen {
    /// Where the library that feeds the segments in decided to begin, beside
    /// what the playlist had told it, and the first segment it asked for.
    ///
    /// The three together are the whole question: the playlist saying one
    /// thing and the library doing another is a production nobody watches in
    /// front of every picture.
    PlaybackBegan {
        /// The starting point read from the playlist, when it read one.
        playlist_said_second: Option<f64>,
        /// Where it settled on beginning.
        began_at_second: f64,
        /// The number of the first segment it asked for.
        first_segment: u32,
    },
    /// The viewer moved, and where from.
    ///
    /// Both ends, because a jump is the one moment the server and the page can
    /// be made to disagree on purpose, and because a film that stops after one
    /// stops at a place that only means something next to where it came from.
    ViewerJumped {
        from_second: f64,
        to_second: f64,
        /// Whether the film was playing when it happened.
        was_playing: bool,
    },
    /// The film stopped for want of something to show.
    ///
    /// What is held on either side of where it stopped is the whole question:
    /// nothing ahead is a film waiting for the server, something ahead is a
    /// film that will not read what it already has.
    PlaybackStalled {
        at_second: f64,
        /// The stretch the browser is holding around that moment, when it
        /// holds one at all.
        held_from_second: Option<f64>,
        held_to_second: Option<f64>,
        /// How many separate stretches it is holding. More than one means a
        /// hole, and a hole is what a film stops on.
        stretches: u32,
        /// How ready the browser says it is, on its own scale of nought to
        /// four.
        ready_state: u32,
        /// Pictures shown since the film began, as the browser counts them.
        pictures_shown: u32,
    },
    /// The film began moving again after having stopped.
    PlaybackPickedUpAgain { at_second: f64, waited_ms: u32 },
    /// The clock went on while the picture stood still.
    ///
    /// The two are told apart by counting pictures rather than by watching,
    /// because from the outside a frozen picture with the sound running and a
    /// film that stopped altogether look like the same complaint and are not
    /// the same fault.
    ThePictureStoodStill {
        at_second: f64,
        for_ms: u32,
        pictures_shown: u32,
        /// Pictures the browser decoded and threw away rather than showing.
        ///
        /// It separates the two ways of showing nothing: climbing, the browser
        /// is producing pictures and refusing them, which is a browser that
        /// cannot keep up; standing still beside a clock that runs, it is
        /// producing none at all, which is a browser that cannot start from
        /// what it was given.
        pictures_dropped: u32,
        /// What the browser was holding around that moment, which is the whole
        /// question: holding the beginning of the piece it was meant to be
        /// showing, it had everything it needed and showed nothing; holding
        /// only the part past where the viewer landed, it never had the one
        /// picture the rest are built on.
        held_from_second: Option<f64>,
        held_to_second: Option<f64>,
        stretches: u32,
        ready_state: u32,
    },
    /// The library gave up on this film, in its own words, and whether the
    /// browser's own reader was handed the playlist instead.
    ///
    /// The one place a page's own wording reaches the journal, because the
    /// wording is the answer: it is the library naming what it could not do
    /// with a film this server produced. Cut short by the server all the same,
    /// and it is still a fact this module named, not a message a page chose to
    /// send.
    PlaybackRefused {
        because: String,
        /// Whether the film went on playing, read by the browser itself.
        browser_took_over: bool,
    },
}

/// How much of the library's wording is kept.
///
/// Long enough for a type, a detail and a sentence, which is what those
/// refusals are made of. A journal is read by eye.
const ENOUGH_OF_A_REFUSAL: usize = 300;

/// Which session the fact is about, and the fact.
#[derive(Debug, Deserialize)]
struct FromThePage {
    session: String,
    #[serde(flatten)]
    seen: Seen,
}

#[derive(Debug, Serialize)]
struct Written {
    written: bool,
}

/// Keeps what is worth reading of a refusal, on a boundary between characters.
fn cut_short(words: &str) -> &str {
    match words.char_indices().nth(ENOUGH_OF_A_REFUSAL) {
        Some((at, _)) => &words[..at],
        None => words,
    }
}

async fn what_the_page_saw(
    State(state): State<AppState>,
    Json(said): Json<FromThePage>,
) -> Result<Json<Written>> {
    // A viewer of this server, like every other route. Nothing here is worth
    // reading, but a journal anybody passing by can write into is a journal
    // that stops being worth reading too.
    viewer(&state).await?;

    // The identifier is a name from outside: read as one, never used as one.
    // Written down as the page sent it only once it parses.
    let session = said
        .session
        .parse::<melyxar_app::playback::SessionId>()
        .map_err(|_| {
            crate::error::ServerError::invalid_input("the session identifier is malformed")
        })?;

    match said.seen {
        Seen::PlaybackBegan {
            playlist_said_second,
            began_at_second,
            first_segment,
        } => tracing::debug!(
            %session,
            playlist_said_second,
            began_at_second,
            first_segment,
            "the page began the film here"
        ),
        Seen::ViewerJumped {
            from_second,
            to_second,
            was_playing,
        } => tracing::debug!(
            %session,
            from_second,
            to_second,
            was_playing,
            "the viewer jumped"
        ),
        Seen::PlaybackStalled {
            at_second,
            held_from_second,
            held_to_second,
            stretches,
            ready_state,
            pictures_shown,
        } => tracing::debug!(
            %session,
            at_second,
            held_from_second,
            held_to_second,
            stretches,
            ready_state,
            pictures_shown,
            "the film stopped for want of something to show"
        ),
        Seen::PlaybackPickedUpAgain {
            at_second,
            waited_ms,
        } => tracing::debug!(%session, at_second, waited_ms, "the film picked up again"),
        Seen::ThePictureStoodStill {
            at_second,
            for_ms,
            pictures_shown,
            pictures_dropped,
            held_from_second,
            held_to_second,
            stretches,
            ready_state,
        } => tracing::debug!(
            %session,
            at_second,
            for_ms,
            pictures_shown,
            pictures_dropped,
            held_from_second,
            held_to_second,
            stretches,
            ready_state,
            "the clock went on while the picture stood still"
        ),
        Seen::PlaybackRefused {
            because,
            browser_took_over,
        } => tracing::warn!(
            %session,
            because = cut_short(&because),
            browser_took_over,
            "the library gave up on this film"
        ),
    }

    Ok(Json(Written { written: true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_can_only_say_what_this_module_has_a_name_for() {
        let said: FromThePage = serde_json::from_str(
            r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9","saw":"playback_began",
                "playlist_said_second":1040.993,"began_at_second":0,"first_segment":0}"#,
        )
        .expect("a fact this module names is read");

        assert_eq!(said.session, "01a0a143-2ab0-748d-8924-e3208b7930c9");
        match said.seen {
            Seen::PlaybackBegan {
                playlist_said_second,
                began_at_second,
                first_segment,
            } => {
                assert_eq!(playlist_said_second, Some(1040.993));
                assert_eq!(began_at_second, 0.0);
                assert_eq!(first_segment, 0);
            }
            other => panic!("read as the wrong fact: {other:?}"),
        }
    }

    #[test]
    fn a_page_can_say_what_only_it_can_see_of_a_film_playing() {
        // The server knows what it produced and when it handed it over. It
        // does not know whether any of it reached a screen, and a film that
        // stops in the middle of itself is read from here or from nowhere.
        for (tried, what) in [
            (
                r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9","saw":"viewer_jumped",
                    "from_second":612.5,"to_second":600.0,"was_playing":true}"#,
                "a jump",
            ),
            (
                r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9","saw":"playback_stalled",
                    "at_second":612.5,"held_from_second":600.0,"held_to_second":612.6,
                    "stretches":2,"ready_state":1,"pictures_shown":14703}"#,
                "a film that stopped",
            ),
            (
                r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9",
                    "saw":"playback_picked_up_again","at_second":612.6,"waited_ms":4200}"#,
                "a film that picked up again",
            ),
            (
                r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9",
                    "saw":"the_picture_stood_still","at_second":612.5,"for_ms":5000,
                    "pictures_shown":14703,"pictures_dropped":2,"held_from_second":600.0,
                    "held_to_second":612.6,"stretches":1,"ready_state":4}"#,
                "a picture that stood still",
            ),
        ] {
            serde_json::from_str::<FromThePage>(tried)
                .unwrap_or_else(|error| panic!("{what} is a fact this module names: {error}"));
        }

        // A stretch the browser holds nothing in is nothing, never a nought:
        // a film holding nothing at all around where it stopped and a film
        // holding the opening of itself are the two answers, and they are not
        // the same answer.
        let held_nothing: FromThePage = serde_json::from_str(
            r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9","saw":"playback_stalled",
                "at_second":612.5,"held_from_second":null,"held_to_second":null,
                "stretches":0,"ready_state":0,"pictures_shown":14703}"#,
        )
        .expect("read");
        match held_nothing.seen {
            Seen::PlaybackStalled {
                held_from_second,
                stretches,
                ..
            } => {
                assert_eq!(held_from_second, None);
                assert_eq!(stretches, 0);
            }
            other => panic!("read as the wrong fact: {other:?}"),
        }
    }

    #[test]
    fn a_refusal_is_cut_short_by_the_server_and_never_by_the_page() {
        let long = "x".repeat(ENOUGH_OF_A_REFUSAL * 3);
        assert_eq!(cut_short(&long).len(), ENOUGH_OF_A_REFUSAL);
        assert_eq!(cut_short("short enough"), "short enough");
        // On a boundary between characters, so a journal stays readable.
        let accented = "é".repeat(ENOUGH_OF_A_REFUSAL * 2);
        assert_eq!(cut_short(&accented).chars().count(), ENOUGH_OF_A_REFUSAL);
    }

    #[test]
    fn anything_else_a_page_might_try_to_say_is_refused() {
        // The whole guarantee: a page has no way of putting its own words in
        // the journal, so nothing that happens outside Melyxar can reach it.
        for tried in [
            r#"{"session":"x","saw":"anything_else","message":"a page somebody visited"}"#,
            r#"{"session":"x","message":"a page somebody visited"}"#,
            r#"{"session":"x","saw":"playback_began","began_at_second":0}"#,
        ] {
            assert!(
                serde_json::from_str::<FromThePage>(tried).is_err(),
                "read as a fact: {tried}"
            );
        }
    }
}
