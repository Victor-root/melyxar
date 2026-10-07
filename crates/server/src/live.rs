//! The live line of every page: what changes in the account's
//! notifications, told the moment it changes; that the title requests or what
//! the libraries hold moved; and for an administrator, what is written in the
//! activity journal and what is being watched, sent again the moment it
//! changes when the page shows it.
//!
//! One line per page, whatever that page shows: each line held open is one
//! of the few connections a browser opens to a server.

use std::time::Duration;

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Router;
use melyxar_app::notifications::live::News;
use melyxar_app::AppState;
use serde::Deserialize;
use tokio::sync::{broadcast, watch};

use crate::account::Viewer;
use crate::error::ServerError;
use crate::notifications::change_word;
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

/// What only an administrator's line waits on.
struct Administration {
    written: watch::Receiver<u64>,
    /// Absent when the page does not show what is being watched.
    playing: Option<watch::Receiver<u64>>,
    /// What is being watched is owed at once, before any news.
    owed: bool,
}

/// What the line waits on between two words.
struct Following {
    state: AppState,
    /// Who opened it.
    who: melyxar_core::id::UserId,
    /// The session it was opened with. The line outlives the request that
    /// opened it: the session is asked about again before every word, so a
    /// browser signed out, or an account removed, is told nothing more.
    token: String,
    notified: broadcast::Receiver<News>,
    requests: watch::Receiver<u64>,
    libraries: watch::Receiver<u64>,
    /// Absent for anybody not an administrator, and from the moment an
    /// administrator stops being one.
    administration: Option<Administration>,
    closing: watch::Receiver<bool>,
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

/// A line of the journal written, or what is being watched moved; never,
/// for a line that is not an administrator's.
async fn administration_moved(administration: &mut Option<Administration>) -> Option<Word> {
    let Some(watching) = administration else {
        return std::future::pending().await;
    };
    tokio::select! {
        written = watching.written.changed() => written.ok().map(|()| Word::Written),
        moved = playing_moved(&mut watching.playing) => {
            moved?;
            if let Some(changes) = watching.playing.as_mut() {
                changes.borrow_and_update();
            }
            Some(Word::Playing)
        }
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
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Query(asked): Query<Asked>,
    headers: HeaderMap,
) -> Response {
    // Always there: the gate let this through for the session it carries.
    let Some(token) = crate::account::token_in(&headers) else {
        return ServerError::unauthenticated("nobody is signed in").into_response();
    };
    let administration = who.permissions.is_administrator.then(|| Administration {
        written: melyxar_app::activity::written(&state),
        playing: asked
            .playing
            .then(|| melyxar_app::watching::changes(&state)),
        owed: asked.playing,
    });
    let following = Following {
        who: who.id,
        token,
        notified: melyxar_app::notifications::live::follow(&state),
        requests: melyxar_app::requests::live::follow(&state),
        libraries: state.database().library_moves(),
        administration,
        closing: melyxar_app::watching::closing(&state),
        state,
    };
    let told = futures_util::stream::unfold(following, |mut following| async move {
        let event = next_word(&mut following).await?;
        Some((event, following))
    });
    live(Sse::new(told).keep_alive(KeepAlive::default()))
}

/// What woke the line.
enum Word {
    Written,
    Playing,
    Notified(Box<News>),
    /// The title requests moved: the page reads again what it shows of them.
    Requests,
    /// Something a library holds changed: a page showing it reads it again.
    Libraries,
    /// The page fell behind and some changes were lost: it reads its
    /// notifications again.
    Missed,
}

/// The next thing the line has to say, once there is one; nothing once the
/// server is stopping or the account is gone.
async fn next_word(following: &mut Following) -> Option<Result<Event, axum::Error>> {
    loop {
        let word = if following
            .administration
            .as_mut()
            .is_some_and(|watching| std::mem::take(&mut watching.owed))
        {
            Word::Playing
        } else {
            tokio::select! {
                word = administration_moved(&mut following.administration) => word?,
                notified = following.notified.recv() => match notified {
                    Ok(news) if news.user == following.who => Word::Notified(Box::new(news)),
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => Word::Missed,
                    Err(broadcast::error::RecvError::Closed) => return None,
                },
                moved = following.requests.changed() => {
                    moved.ok()?;
                    Word::Requests
                }
                moved = following.libraries.changed() => {
                    moved.ok()?;
                    Word::Libraries
                }
                // Only whether it closed is kept: what it hands back may not
                // be held across what the other branches wait on.
                () = async {
                    let _ = following.closing.wait_for(|closed| *closed).await;
                } => return None,
            }
        };

        // Asked once the word is ready and before it leaves, so nothing is
        // told to a browser signed out or an account removed meanwhile, and
        // nothing of the administration to somebody who stopped being an
        // administrator.
        let account = melyxar_app::accounts::who_holds(&following.state, &following.token)
            .await
            .ok()??
            .user;
        return Some(match word {
            Word::Written | Word::Playing if !account.permissions.is_administrator => {
                following.administration = None;
                continue;
            }
            Word::Written => Ok(Event::default().event("activity").data("written")),
            Word::Playing => what_is_watched(&following.state).await,
            Word::Notified(news) => {
                let (name, data) = change_word(&news.change);
                Event::default().event(name).json_data(data)
            }
            Word::Requests => Ok(Event::default().event("requests").data("moved")),
            Word::Libraries => Ok(Event::default().event("libraries").data("moved")),
            Word::Missed => Event::default()
                .event("notifications_missed")
                .json_data(serde_json::Value::Null),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }
}
