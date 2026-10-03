//! What changes in an account's notifications, told to every page it has
//! open, the moment it changes: one arrived, was read on another device,
//! was removed. Held in memory and lost with a restart, which loses nothing:
//! a page that opens its line again reads its history afresh.

use melyxar_core::id::{NotificationId, UserId};
use tokio::sync::broadcast;

use super::Told;
use crate::AppState;

/// How many changes may wait for a slow page before it is told it missed
/// some and must read its history again.
const WAITING_AT_MOST: usize = 256;

/// One change, for one account.
#[derive(Debug, Clone)]
pub struct News {
    pub user: UserId,
    pub change: Change,
}

#[derive(Debug, Clone)]
pub enum Change {
    /// A notification for this account.
    Arrived {
        told: Told,
        /// Kept in its history; otherwise only said on the screen.
        kept: bool,
        /// Shown on the screen as it arrives.
        screen: bool,
    },
    /// A maintenance whose time is near, unread again and shown once more.
    Recalled(Told),
    Read(Vec<NotificationId>),
    Unread(Vec<NotificationId>),
    Removed(Vec<NotificationId>),
    /// What the account chose changed on one of its devices.
    ChoicesChanged,
}

/// Where the changes go out from, one for the whole server.
#[derive(Clone)]
pub struct Outbox(broadcast::Sender<News>);

impl Default for Outbox {
    fn default() -> Self {
        Self(broadcast::channel(WAITING_AT_MOST).0)
    }
}

impl Outbox {
    /// Nobody listening is not a failure: the history holds what matters.
    pub(super) fn tell(&self, user: UserId, change: Change) {
        let _ = self.0.send(News { user, change });
    }
}

/// Every change from now on, for whoever opened a line; each page keeps
/// only its own account's.
pub fn follow(state: &AppState) -> broadcast::Receiver<News> {
    state.notifications().0.subscribe()
}

/// Says a change to every page of an account.
pub(super) fn tell(state: &AppState, user: UserId, change: Change) {
    state.notifications().tell(user, change);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_change_reaches_every_line_opened_before_it() {
        let outbox = Outbox::default();
        let mut first = outbox.0.subscribe();
        let mut second = outbox.0.subscribe();
        let user = UserId::new();
        outbox.tell(user, Change::ChoicesChanged);
        for line in [&mut first, &mut second] {
            let news = line.recv().await.expect("told");
            assert_eq!(news.user, user);
            assert!(matches!(news.change, Change::ChoicesChanged));
        }
    }
}
