//! What the page saw of the player of music, written into the journal under
//! a tag for each part of it: `music-decks` for the sound, `music-queue` for
//! what plays and why, `music-controls` for the presses, `music-tabs` for
//! the tab that holds the sound.
//!
//! The server sends the sound and never hears it: which deck held which
//! song, which press moved the queue and which tab played are only known to
//! the page. Like `page`, this is a closed list of facts the server names
//! itself, and every name from outside is read before it is written down.

mod controls;
mod decks;
mod queue;
mod tabs;

use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_core::id::WorkId;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::identifiers::parse_work;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/system/journal/music",
        axum::routing::post(what_the_page_saw),
    )
}

/// One fact, named by the server.
#[derive(Debug, Deserialize)]
#[serde(tag = "saw", rename_all = "snake_case")]
enum Seen {
    Decks(decks::Decks),
    DeckEvent(decks::Event),
    DeckAsked(decks::Asked),
    QueueMoved(queue::Moved),
    Pressed(controls::Pressed),
    Tab(tabs::Tab),
}

#[derive(Debug, Deserialize)]
struct FromThePage {
    /// The song playing, as the page holds it, when there is one.
    #[serde(default)]
    song: Option<String>,
    #[serde(flatten)]
    seen: Seen,
}

#[derive(Debug, Serialize)]
struct Written {
    written: bool,
}

/// A song named by the page, read as one.
fn song_named(value: Option<&str>) -> Result<Option<WorkId>> {
    value.map(parse_work).transpose()
}

/// Whose page saw it, and the song it was playing.
struct Said {
    account: String,
    song: Option<WorkId>,
}

async fn what_the_page_saw(
    crate::account::Viewer(viewer): crate::account::Viewer,
    Json(said): Json<FromThePage>,
) -> Result<Json<Written>> {
    // Every account writes here, so each line says whose page saw it.
    let by = Said {
        account: viewer.name.clone(),
        song: song_named(said.song.as_deref())?,
    };
    match said.seen {
        Seen::Decks(seen) => decks::write_decks(&by, &seen),
        Seen::DeckEvent(seen) => decks::write_event(&by, &seen)?,
        Seen::DeckAsked(seen) => decks::write_asked(&by, &seen)?,
        Seen::QueueMoved(seen) => queue::write(&by, &seen)?,
        Seen::Pressed(seen) => controls::write(&by, &seen),
        Seen::Tab(seen) => tabs::write(&by, &seen),
    }
    Ok(Json(Written { written: true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SONG: &str = "01a0f0c6-c1fa-769d-9f83-3467c8c4586b";

    fn read(json: &str) -> std::result::Result<FromThePage, serde_json::Error> {
        serde_json::from_str(json)
    }

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    #[test]
    fn each_fact_is_read_by_its_name() {
        let facts = [
            format!(
                r#"{{"song":"{SONG}","saw":"decks","moment":"both_sounding","live_deck":1,"fade_seconds":0,
                    "deck0_second":143.2,"deck0_length":null,"deck0_sounding":true,
                    "deck1_second":61.5,"deck1_length":212.4,"deck1_sounding":true,"together_seconds":2.5}}"#
            ),
            format!(
                r#"{{"song":"{SONG}","saw":"deck_event","deck":0,"live_deck":0,"event":"ended","second":249.8,
                    "length":249.8,"paused":true,"deck_song":"{SONG}","error":null}}"#
            ),
            format!(r#"{{"saw":"deck_asked","deck":1,"action":"prepare","deck_song":"{SONG}"}}"#),
            format!(
                r#"{{"song":"{SONG}","saw":"queue_moved","why":"ended","from_song":"{SONG}","at":3,"length":12,
                    "shuffle":true,"repeat":"all"}}"#
            ),
            r#"{"saw":"pressed","command":"next","from":"card","holds":true}"#.to_string(),
            r#"{"saw":"tab","holds":false,"following":true}"#.to_string(),
        ];
        let read: Vec<Seen> = facts
            .iter()
            .map(|json| read(json).unwrap_or_else(|error| panic!("{error}: {json}")).seen)
            .collect();
        assert!(matches!(read[0], Seen::Decks(_)));
        assert!(matches!(read[1], Seen::DeckEvent(_)));
        assert!(matches!(read[2], Seen::DeckAsked(_)));
        assert!(matches!(read[3], Seen::QueueMoved(_)));
        assert!(matches!(read[4], Seen::Pressed(_)));
        assert!(matches!(read[5], Seen::Tab(_)));
    }

    #[test]
    fn a_name_outside_the_list_is_refused() {
        for refused in [
            r#"{"saw":"anything_else","message":"hi"}"#.to_string(),
            r#"{"saw":"pressed","command":"dance","from":"card","holds":true}"#.to_string(),
            r#"{"saw":"deck_asked","deck":1,"action":"explode"}"#.to_string(),
            r#"{"saw":"deck_event","deck":0,"live_deck":0,"event":"whatever","second":0,"paused":true}"#.to_string(),
        ] {
            assert!(read(&refused).is_err(), "{refused}");
        }
    }

    #[test]
    fn a_song_is_read_as_one() {
        assert!(matches!(song_named(Some(SONG)), Ok(Some(_))));
        assert!(matches!(song_named(None), Ok(None)));
        assert!(song_named(Some("a song somebody likes")).is_err());
    }
}
