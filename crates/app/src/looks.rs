//! Every page told that how it looks moved: an account's own choices, read
//! by its pages only, or the server's name, logo or theme, read by all. Only
//! that something moved is told, never what: each page reads again what it
//! wears.

use std::sync::Arc;

use melyxar_core::id::UserId;
use tokio::sync::broadcast;

use crate::AppState;

/// How many words a line may fall behind before it is told it missed some.
const KEPT: usize = 64;

/// What moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Moved {
    /// What this account chose for itself.
    Account(UserId),
    /// What the server wears for everybody.
    Server,
}

/// Where the word goes out from, one for the whole server.
#[derive(Clone)]
pub struct Outbox(Arc<broadcast::Sender<Moved>>);

impl Default for Outbox {
    fn default() -> Self {
        Self(Arc::new(broadcast::channel(KEPT).0))
    }
}

/// Says this account's choices moved.
pub(crate) fn account_moved(state: &AppState, user: UserId) {
    // Nobody listening is nobody to tell.
    let _ = state.looks().0.send(Moved::Account(user));
}

/// Says what the server wears moved.
pub(crate) fn server_moved(state: &AppState) {
    let _ = state.looks().0.send(Moved::Server);
}

/// Told every time how a page looks moves, for a page's live line.
pub fn follow(state: &AppState) -> broadcast::Receiver<Moved> {
    state.looks().0.subscribe()
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::Config;
    use melyxar_core::user::{Permissions, Preferences};
    use melyxar_database::Database;

    #[tokio::test]
    async fn a_line_is_told_when_an_account_chooses_and_when_the_server_is_renamed() {
        let database = Database::open_in_memory().await.expect("database opens");
        let state = AppState::new(Config::default(), database, None, Default::default());
        let user = state
            .database()
            .create_user("viewer", None, &Permissions::viewer())
            .await
            .expect("account made")
            .id;
        let mut told = follow(&state);

        crate::preferences::save(&state, user, Preferences::default())
            .await
            .expect("kept");
        assert_eq!(told.recv().await.expect("told"), Moved::Account(user));

        crate::server::rename(&state, "Home cinema")
            .await
            .expect("renamed");
        assert_eq!(told.recv().await.expect("told"), Moved::Server);
    }
}
