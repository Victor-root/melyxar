//! What the application saw of a camera watched live, written into the
//! journal under a tag for each side of it: `camera-open` for how it is
//! reached and how long to the first picture, `camera-delay` for how far
//! behind the camera the picture is, `camera-frames` for the pictures
//! received, shown and lost, `camera-net` for the exchange with the camera,
//! `camera-screen` for the screen around the picture, and `camera-error` for
//! everything that failed.
//!
//! The application opens the camera itself and nothing of it passes through
//! the server, which hears of it only here. Like `page`, a closed list of
//! facts the server names itself; the few words from outside (the camera's
//! name, a codec, a decoder, an error) are cut short.

mod delay;
mod errors;
mod frames;
mod net;
mod open;
mod screen;

use axum::{Json, Router};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use crate::page::cut_to;

/// How much of a camera's name is written down.
const ENOUGH_OF_A_NAME: usize = 80;

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/system/journal/camera",
        axum::routing::post(what_the_application_saw),
    )
}

/// One fact, named by the server.
#[derive(Debug, Deserialize)]
#[serde(tag = "saw", rename_all = "snake_case")]
enum Seen {
    Opened(open::Opened),
    Stream(open::Stream),
    Decoder(open::Decoder),
    FirstPicture(open::FirstPicture),
    LetGo(open::LetGo),
    Second(Second),
    CatchingUp(delay::CatchingUp),
    StoodStill(frames::StoodStill),
    Waiting(frames::Waiting),
    MovingAgain(frames::MovingAgain),
    Dropped(frames::Dropped),
    SoundRanDry(frames::SoundRanDry),
    Exchange(net::Exchange),
    ReopenedWithoutSound(net::ReopenedWithoutSound),
    Screen(screen::Screen),
    Picture(screen::Size),
    Drawn(screen::Size),
    Held(screen::Held),
    Failed(errors::Failed),
}

/// The screen the camera is watched on.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Where {
    Phone,
    Television,
}

impl Where {
    fn as_word(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Television => "television",
        }
    }
}

#[derive(Debug, Deserialize)]
struct FromTheApplication {
    camera: String,
    screen: Where,
    /// Milliseconds since the camera was asked to open.
    at_ms: u64,
    #[serde(flatten)]
    seen: Seen,
}

/// What the player of a camera is doing.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum State {
    Idle,
    Buffering,
    Ready,
    Ended,
}

impl State {
    fn as_word(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Buffering => "waiting for the camera",
            Self::Ready => "ready",
            Self::Ended => "ended",
        }
    }
}

/// Each second of a camera open: written as three lines, the delay, the
/// frames and the memory, each under its own tag.
#[derive(Debug, Deserialize)]
struct Second {
    state: State,
    playing: bool,
    /// What has come from the camera and is not shown yet.
    behind_ms: i64,
    speed: f64,
    /// Time since the first picture that the picture did not move.
    lost_ms: i64,
    received: i64,
    shown: i64,
    dropped: i64,
    memory_mb: u64,
    most_memory_mb: u64,
}

#[derive(Debug, Serialize)]
struct Written {
    written: bool,
}

/// Whose application saw it, which camera, on which screen, and when.
struct Said {
    account: String,
    camera: String,
    screen: &'static str,
    at_ms: u64,
}

async fn what_the_application_saw(
    crate::account::Viewer(viewer): crate::account::Viewer,
    Json(said): Json<FromTheApplication>,
) -> Result<Json<Written>> {
    let by = Said {
        account: viewer.name.clone(),
        camera: cut_to(&said.camera, ENOUGH_OF_A_NAME).to_string(),
        screen: said.screen.as_word(),
        at_ms: said.at_ms,
    };
    match said.seen {
        Seen::Opened(seen) => open::write_opened(&by, &seen),
        Seen::Stream(seen) => open::write_stream(&by, &seen),
        Seen::Decoder(seen) => open::write_decoder(&by, &seen),
        Seen::FirstPicture(seen) => open::write_first_picture(&by, &seen),
        Seen::LetGo(seen) => open::write_let_go(&by, &seen),
        Seen::Second(seen) => {
            delay::write_second(&by, &seen);
            frames::write_second(&by, &seen);
            screen::write_memory(&by, &seen);
        }
        Seen::CatchingUp(seen) => delay::write_catching_up(&by, &seen),
        Seen::StoodStill(seen) => frames::write_stood_still(&by, &seen),
        Seen::Waiting(seen) => frames::write_waiting(&by, &seen),
        Seen::MovingAgain(seen) => frames::write_moving_again(&by, &seen),
        Seen::Dropped(seen) => frames::write_dropped(&by, &seen),
        Seen::SoundRanDry(seen) => frames::write_sound_ran_dry(&by, &seen),
        Seen::Exchange(seen) => net::write_exchange(&by, &seen),
        Seen::ReopenedWithoutSound(seen) => net::write_reopened(&by, &seen),
        Seen::Screen(seen) => screen::write_screen(&by, &seen),
        Seen::Picture(seen) => screen::write_size(&by, "the camera's picture", &seen),
        Seen::Drawn(seen) => screen::write_size(&by, "drawn into", &seen),
        Seen::Held(seen) => screen::write_held(&by, &seen),
        Seen::Failed(seen) => errors::write(&by, &seen),
    }
    Ok(Json(Written { written: true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(json: &str) -> std::result::Result<FromTheApplication, serde_json::Error> {
        serde_json::from_str(json)
    }

    fn fact(rest: &str) -> String {
        format!(r#"{{"camera":"Front door","screen":"television","at_ms":1520,{rest}}}"#)
    }

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    #[test]
    fn each_fact_is_read_by_its_name() {
        let facts = [
            r#""saw":"opened","server_answer_ms":42"#,
            r#""saw":"stream","kind":"video","codec":"avc1.640028","width":1920,"height":1080,"fps":25.0,"supported":true,"selected":true"#,
            r#""saw":"decoder","kind":"video","name":"c2.android.avc.decoder","ready_ms":31"#,
            r#""saw":"first_picture","first_data_ms":300,"streams_ms":410,"decoder_ms":480"#,
            r#""saw":"let_go","frames":1500"#,
            r#""saw":"second","state":"ready","playing":true,"behind_ms":340,"speed":1.05,"lost_ms":120,"received":25,"shown":24,"dropped":1,"memory_mb":61,"most_memory_mb":256"#,
            r#""saw":"catching_up","speed":1.2,"behind_ms":900"#,
            r#""saw":"stood_still","seconds":3,"received":0"#,
            r#""saw":"waiting","in_hand_ms":20"#,
            r#""saw":"moving_again","waited_ms":2400"#,
            r#""saw":"dropped","frames":4,"within_ms":1000"#,
            r#""saw":"sound_ran_dry","last_fed_ms":300"#,
            r#""saw":"exchange","step":"setup","status":200,"answer_ms":18,"transport":"tcp""#,
            r#""saw":"reopened_without_sound","received_ms":1600"#,
            r#""saw":"screen","event":"memory_asked","level":15"#,
            r#""saw":"picture","width":1920,"height":1080"#,
            r#""saw":"drawn","width":1280,"height":720"#,
            r#""saw":"held","ms":240"#,
            r#""saw":"failed","during":"player","code":"ERROR_CODE_IO_NETWORK_CONNECTION_TIMEOUT","causes":"SocketTimeoutException: timeout""#,
        ];
        for json in facts.map(fact) {
            read(&json).unwrap_or_else(|error| panic!("{error}: {json}"));
        }
    }

    #[test]
    fn a_name_outside_the_list_is_refused() {
        for refused in [
            fact(r#""saw":"anything_else","message":"hi""#),
            fact(r#""saw":"screen","event":"dance""#),
            fact(r#""saw":"exchange","step":"steal""#),
            fact(r#""saw":"failed","during":"everywhere","causes":"""#),
            r#"{"camera":"Front door","screen":"fridge","at_ms":0,"saw":"held","ms":1}"#.to_string(),
        ] {
            assert!(read(&refused).is_err(), "{refused}");
        }
    }
}
