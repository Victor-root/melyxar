//! Everything the server tells somebody: notifications kept for every
//! account, told live to each of its pages, and what deserves an
//! administrator's look today. Nothing outside this module knows how a
//! notification is made, kept or shown: the rest of the server only calls
//! [`send`].
//!
//! See the decisions on notifications in `docs/architecture/README.md`.

pub mod attention;
pub mod choices;
pub mod history;
pub mod live;
pub mod messages;
pub mod news;
pub mod rounds;
mod sending;
#[cfg(test)]
mod testing;

pub use sending::{send, Audience, Kind, Level, Outgoing, Said, SeriesArrived};

use melyxar_core::id::WorkId;
use melyxar_database::images::StoredImage;
use melyxar_database::notifications::Notification;

use crate::{AppState, Result};

/// A notification as it is shown: what is kept of it, and the poster of the
/// work it shows, every size of it, largest first.
#[derive(Debug, Clone, PartialEq)]
pub struct Told {
    pub notification: Notification,
    pub poster: Vec<StoredImage>,
}

/// Gives each notification the poster of its work, in one query.
async fn with_posters(state: &AppState, notifications: Vec<Notification>) -> Result<Vec<Told>> {
    let works: Vec<WorkId> = notifications.iter().filter_map(|one| one.work_id).collect();
    let posters = state.database().posters_of(&works).await?;
    Ok(notifications
        .into_iter()
        .map(|notification| Told {
            poster: notification
                .work_id
                .and_then(|work| posters.get(&work).cloned())
                .unwrap_or_default(),
            notification,
        })
        .collect())
}
