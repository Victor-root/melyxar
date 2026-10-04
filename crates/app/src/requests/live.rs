//! Every page told that the requests moved: one was made, decided, taken
//! back or arrived, or who may ask changed. Only that something moved is
//! told, never what: each page reads again what it shows, as far as its
//! account may see it.

use std::sync::Arc;

use tokio::sync::watch;

use crate::AppState;

/// Where the word goes out from, one for the whole server.
#[derive(Clone)]
pub struct Moved(Arc<watch::Sender<u64>>);

impl Default for Moved {
    fn default() -> Self {
        Self(Arc::new(watch::channel(0).0))
    }
}

/// Says the requests moved.
pub(super) fn moved(state: &AppState) {
    state
        .requests()
        .0
        .send_modify(|count| *count = count.wrapping_add(1));
}

/// Moved on every time the requests do, for a page's live line.
pub fn follow(state: &AppState) -> watch::Receiver<u64> {
    state.requests().0.subscribe()
}
