//! What the page saw, written into the journal.
//!
//! Half of what happens when a film starts happens in the browser, and the
//! journal showed none of it: the server could say it had asked for the
//! seventeenth minute while the page asked for the opening of the film, with
//! nothing anywhere saying which of the two was wrong.
//!
//! So the page may write here. What it may not do is choose its own words. It
//! sends one fact out of a list this module defines, and the server writes the
//! line itself. A page cannot tell the journal anything it has no vocabulary
//! for, whatever else ends up running in the browser, and reading this file is
//! enough to know exactly what can appear under the `page` tag.

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
}

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
        }
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
