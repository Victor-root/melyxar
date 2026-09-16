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
        /// What moved the film.
        ///
        /// A click and a drag look identical from here and are not the same
        /// gesture at all: a drag holds the library back for as long as the
        /// hand is down and sets it going once, a click holds it back and sets
        /// it going again in the same breath, and clicking along the bar does
        /// that over and over. Read in the maintainer's journal, a run of
        /// segments asked for backwards was put down to a drag, and the
        /// maintainer says he clicks. Nothing in the line could say which, and
        /// a fault chased on the wrong gesture is a fault fixed in the wrong
        /// place.
        moved_by: HowItMoved,
    },
    /// The shape of the picture the browser ended up with.
    ///
    /// The other end of what the server writes when it decides a playback. The
    /// server knows the shape the film holds and the shape it asked for; only
    /// the browser knows the shape that came out, and a film shown stretched
    /// is exactly the two of them disagreeing with nothing in between to say
    /// where it was lost. Counted in the browser's own terms, which are
    /// already the shape the picture is meant to be shown at, whatever shape
    /// its pixels were stored in.
    ThePictureArrived {
        /// How wide and how tall the browser says the picture is meant to be.
        across: u32,
        down: u32,
        /// The box it is being drawn in, which is the page's doing and not the
        /// film's: a picture the right shape in a box of the wrong one is a
        /// fault on this side of the wire.
        drawn_across: u32,
        drawn_down: u32,
    },
    /// A jump put a picture back on the screen, and how long that took.
    ///
    /// The one thing nobody could measure before. The server knows when it
    /// handed a segment over, and it knows nothing of the browser fetching it,
    /// taking it apart and painting it; what a viewer counts is the wait
    /// between letting go of the bar and seeing the film again, and that
    /// number lived in neither place. Read from the picture the browser says
    /// it has actually put on screen, not from a picture it has decoded, since
    /// decoding ahead of an empty screen is the fault this is here to catch.
    ThePictureCameBack {
        /// Where the viewer had asked to land.
        asked_for_second: f64,
        /// The moment of the film on the picture that came up, which says
        /// whether the browser landed where it was asked to.
        showed_second: f64,
        /// How long after the jump, in milliseconds.
        after_ms: u32,
        /// Whether the browser held that moment already when the jump was
        /// made. A landing in film it holds ought to cost nothing at all; a
        /// cold one has to be waited for, and the two must never be read as
        /// one number.
        was_held_already: bool,
        /// How many separate stretches the browser held when the jump was
        /// made.
        stretches: u32,
    },
    /// The film stopped for want of something to show.
    ///
    /// What is held on either side of where it stopped is the whole question:
    /// nothing ahead is a film waiting for the server, something ahead is a
    /// film that will not read what it already has.
    PlaybackStalled {
        at_second: f64,
        /// Whether the browser was still trying to move to a new place.
        ///
        /// A move that never finishes is a film with its clock, its picture
        /// and its sound all stopped at once, which is the one shape of this
        /// that nothing here could see before: it was read as a film nobody
        /// was playing.
        was_seeking: bool,
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
    /// One of the real moments on the way to a film playing, and how long the
    /// one before it took.
    ///
    /// The number shown under the ring while a film is being prepared is built
    /// from these, and it climbed to a hundred and stopped moving on a slow
    /// connection, with nothing anywhere saying which of the moments it was
    /// waiting on. The two that matter are the server producing what the
    /// browser needs and that reaching the browser afterwards, and only the
    /// browser can see where the time between them actually went.
    LoadingStage {
        stage: LoadingStage,
        /// How long the stage before this one lasted, in milliseconds.
        after_ms: u32,
    },
}

/// One of the real moments a page passes through on the way to a film
/// playing, in the order it passes through them.
///
/// A closed list like every other fact here: the page cannot invent a stage,
/// and the order below is what a reading of the journal is checked against.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LoadingStage {
    /// A session has been asked for and nothing has answered yet.
    Opening,
    /// The server opened a session to rebuild the film into.
    SessionOpened,
    /// The library read the playlist and knows what to ask for next.
    ManifestParsed,
    /// The server said it is actively writing the piece this browser needs.
    Producing,
    /// That piece exists on the server now. What is left is the network.
    Produced,
    /// The first piece of the film reached the browser and was read.
    FirstFragmentLoaded,
    /// The browser knows it has a film. The film is playing.
    Done,
}

impl LoadingStage {
    /// The word written in the journal.
    fn as_word(self) -> &'static str {
        match self {
            Self::Opening => "opening",
            Self::SessionOpened => "session opened",
            Self::ManifestParsed => "manifest parsed",
            Self::Producing => "producing",
            Self::Produced => "produced",
            Self::FirstFragmentLoaded => "first fragment loaded",
            Self::Done => "done",
        }
    }
}

/// What moved the film, as far as the page can tell.
///
/// A closed list like the facts themselves: the page picks one of these and
/// the server writes the word. The page knows its own controls and nothing
/// else, so anything the browser or the library does to the film on its own
/// falls under the last of them, which is a fact in its own right and the one
/// worth catching.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum HowItMoved {
    /// One press on the bar with nothing in between. The library is held back
    /// and set going again in the same breath, and clicking along the bar does
    /// that once per click.
    AClick,
    /// A press on the bar followed the whole way. The library is held back for
    /// as long as the hand is down and set going once, wherever it landed.
    ADrag,
    /// A fixed step, from a button or an arrow key. There is no in between to
    /// hold anything back for.
    AStep,
    /// The film put back where this viewer left it, the moment the browser
    /// knew how long it was. Nobody moved anything: it is the film starting
    /// where it was meant to.
    PickedUpWhereItWasLeft,
    /// Nothing on the page asked for it. The browser or the library moved the
    /// film by itself, which is a different fault from a viewer moving it.
    NotThePage,
}

impl HowItMoved {
    /// The word written in the journal.
    fn as_word(self) -> &'static str {
        match self {
            Self::AClick => "a click on the bar",
            Self::ADrag => "a drag of the bar",
            Self::AStep => "a step",
            Self::PickedUpWhereItWasLeft => "picked up where it was left",
            Self::NotThePage => "not the page",
        }
    }
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
            moved_by,
        } => tracing::debug!(
            %session,
            from_second,
            to_second,
            was_playing,
            moved_by = moved_by.as_word(),
            "the viewer jumped"
        ),
        Seen::ThePictureArrived {
            across,
            down,
            drawn_across,
            drawn_down,
        } => tracing::debug!(
            %session,
            across,
            down,
            drawn_across,
            drawn_down,
            "the browser says the picture is this shape"
        ),
        Seen::ThePictureCameBack {
            asked_for_second,
            showed_second,
            after_ms,
            was_held_already,
            stretches,
        } => tracing::debug!(
            %session,
            asked_for_second,
            showed_second,
            after_ms,
            was_held_already,
            stretches,
            "a jump put a picture back on the screen"
        ),
        Seen::PlaybackStalled {
            at_second,
            was_seeking,
            held_from_second,
            held_to_second,
            stretches,
            ready_state,
            pictures_shown,
        } => tracing::debug!(
            %session,
            at_second,
            was_seeking,
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
        Seen::LoadingStage { stage, after_ms } => tracing::debug!(
            %session,
            stage = stage.as_word(),
            after_ms,
            "the page reached a loading stage"
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
    fn a_loading_stage_names_itself_and_how_long_the_one_before_it_took() {
        let said: FromThePage = serde_json::from_str(
            r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9","saw":"loading_stage",
                "stage":"produced","after_ms":3421}"#,
        )
        .expect("a stage this module names is read");

        match said.seen {
            Seen::LoadingStage { stage, after_ms } => {
                assert_eq!(stage.as_word(), "produced");
                assert_eq!(after_ms, 3421);
            }
            other => panic!("read as the wrong fact: {other:?}"),
        }

        // A stage this module has no name for is refused outright, like every
        // other thing a page might try to say.
        assert!(serde_json::from_str::<FromThePage>(
            r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9","saw":"loading_stage",
                "stage":"somewhere_the_page_invented","after_ms":0}"#
        )
        .is_err());
    }

    #[test]
    fn a_page_can_say_what_only_it_can_see_of_a_film_playing() {
        // The server knows what it produced and when it handed it over. It
        // does not know whether any of it reached a screen, and a film that
        // stops in the middle of itself is read from here or from nowhere.
        for (tried, what) in [
            (
                r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9","saw":"viewer_jumped",
                    "from_second":612.5,"to_second":600.0,"was_playing":true,
                    "moved_by":"a_click"}"#,
                "a jump",
            ),
            (
                r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9","saw":"playback_stalled",
                    "at_second":612.5,"was_seeking":false,"held_from_second":600.0,"held_to_second":612.6,
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
            (
                r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9",
                    "saw":"the_picture_came_back","asked_for_second":603.7,"showed_second":603.666,
                    "after_ms":424,"was_held_already":false,"stretches":3}"#,
                "a jump that put a picture back on the screen",
            ),
            (
                r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9",
                    "saw":"the_picture_arrived","across":3840,"down":2160,
                    "drawn_across":1280,"drawn_down":720}"#,
                "the shape of the picture the browser got",
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
                "at_second":612.5,"was_seeking":true,"held_from_second":null,"held_to_second":null,
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
    fn a_jump_says_what_moved_the_film_and_a_click_is_not_a_drag() {
        // The maintainer was told a run of segments asked for backwards had
        // followed a drag of the bar. He clicks. Nothing in the line could say
        // which, and the two are not the same gesture at all: a fault chased
        // on the wrong one is a fault fixed in the wrong place.
        for (word, expected) in [
            ("a_click", "a click on the bar"),
            ("a_drag", "a drag of the bar"),
            ("a_step", "a step"),
            ("picked_up_where_it_was_left", "picked up where it was left"),
            ("not_the_page", "not the page"),
        ] {
            let said: FromThePage = serde_json::from_str(&format!(
                r#"{{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9","saw":"viewer_jumped",
                    "from_second":612.5,"to_second":600.0,"was_playing":true,
                    "moved_by":"{word}"}}"#
            ))
            .unwrap_or_else(|error| panic!("{word} is a word this module names: {error}"));
            match said.seen {
                Seen::ViewerJumped { moved_by, .. } => assert_eq!(moved_by.as_word(), expected),
                other => panic!("read as the wrong fact: {other:?}"),
            }
        }

        // A word this module has no name for is refused outright, like every
        // other thing a page might try to say.
        assert!(serde_json::from_str::<FromThePage>(
            r#"{"session":"01a0a143-2ab0-748d-8924-e3208b7930c9","saw":"viewer_jumped",
                "from_second":612.5,"to_second":600.0,"was_playing":true,
                "moved_by":"whatever the browser felt like"}"#
        )
        .is_err());
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
