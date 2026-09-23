//! The administration's live line: what is written in the activity journal,
//! told the moment it is written, and what is being watched, sent again the
//! moment it changes when the page shows it.
//!
//! One line per page of the administration, whatever that page shows: each
//! line held open is one of the few connections a browser opens to a server.

use std::time::Duration;

use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::Response;
use axum::Router;
use melyxar_app::AppState;
use serde::Deserialize;
use tokio::sync::watch;

use crate::account::Administrator;
use crate::playback::{live, watched_views};

/// How often what is being watched is sent again when nothing else changed,
/// which is how the speed of a conversion keeps moving.
const RESENT_EVERY: Duration = Duration::from_secs(2);

pub fn router() -> Router<AppState> {
    Router::new().route("/api/v1/system/live", axum::routing::get(line))
}

#[derive(Debug, Default, Deserialize)]
struct Asked {
    /// Whether the page shows what is being watched.
    #[serde(default)]
    playing: bool,
}

/// What the line waits on between two words.
struct Following {
    state: AppState,
    written: watch::Receiver<u64>,
    /// Absent when the page does not show what is being watched.
    playing: Option<watch::Receiver<u64>>,
    closing: watch::Receiver<bool>,
    /// What is being watched is owed at once, before any news.
    owed: bool,
}

/// Whatever moves what is being watched, or the beat that sends it anyway;
/// never, for a page that does not show it.
async fn playing_moved(playing: &mut Option<watch::Receiver<u64>>) -> Option<()> {
    match playing {
        Some(changes) => {
            tokio::select! {
                moved = changes.changed() => moved.ok(),
                () = tokio::time::sleep(RESENT_EVERY) => Some(()),
            }
        }
        None => std::future::pending().await,
    }
}

async fn what_is_watched(state: &AppState) -> Result<Event, axum::Error> {
    match watched_views(state).await {
        Ok(views) => Event::default().event("playing").json_data(views),
        // Said to the page, which keeps what it last showed and says the
        // list could not be read; why is in the journal.
        Err(error) => {
            tracing::warn!(%error, "what is being watched could not be read");
            Ok(Event::default().event("failed").data("failed"))
        }
    }
}

async fn line(
    _: Administrator,
    State(state): State<AppState>,
    Query(asked): Query<Asked>,
) -> Response {
    let following = Following {
        written: melyxar_app::activity::written(&state),
        playing: asked
            .playing
            .then(|| melyxar_app::watching::changes(&state)),
        closing: melyxar_app::watching::closing(&state),
        owed: asked.playing,
        state,
    };
    let told = futures_util::stream::unfold(following, |mut following| async move {
        if following.owed {
            following.owed = false;
            let event = what_is_watched(&following.state).await;
            return Some((event, following));
        }
        let event = tokio::select! {
            written = following.written.changed() => {
                written.ok()?;
                Ok(Event::default().event("activity").data("written"))
            }
            moved = playing_moved(&mut following.playing) => {
                moved?;
                if let Some(changes) = following.playing.as_mut() {
                    changes.borrow_and_update();
                }
                what_is_watched(&following.state).await
            }
            // Only whether it closed is kept: what it hands back may not be
            // held across what the other branches wait on.
            () = async {
                let _ = following.closing.wait_for(|closed| *closed).await;
            } => return None,
        };
        Some((event, following))
    });
    live(Sse::new(told).keep_alive(KeepAlive::default()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }
}
