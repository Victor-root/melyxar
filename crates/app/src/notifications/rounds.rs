//! What the server does of its own accord, every minute: announce what has
//! settled in the libraries, recall a maintenance whose time is near and
//! take away the ones whose time has passed; and once a day, forget what
//! was read a year ago.

use melyxar_core::time::Timestamp;

use super::live::{tell, Change};
use super::sending::RECALLED_BEFORE;
use super::with_posters;
use crate::{AppState, Result};

/// How often the rounds are made.
const EVERY: std::time::Duration = std::time::Duration::from_secs(60);

/// How often what was read long ago is forgotten.
const FORGETTING_EVERY: time::Duration = time::Duration::days(1);

/// How long a notification that was read is kept.
const READ_KEPT_FOR: time::Duration = time::Duration::days(365);

/// Recalls a maintenance whose time is near, and takes away the ones whose
/// time has passed, telling each account's pages.
async fn mind_the_maintenance(state: &AppState, now: Timestamp) -> Result<()> {
    let database = state.database();
    let recalled = database.recall_maintenance(now + RECALLED_BEFORE, now).await?;
    for told in with_posters(state, recalled).await? {
        tell(state, told.notification.user_id, Change::Recalled(told));
    }
    for (user, id) in database.forget_past_maintenance(now).await? {
        tell(state, user, Change::Removed(vec![id]));
    }
    Ok(())
}

async fn forget_what_was_read_long_ago(state: &AppState, now: Timestamp) {
    match state
        .database()
        .forget_read_notifications_before(now - READ_KEPT_FOR)
        .await
    {
        Ok(0) => {}
        Ok(forgotten) => tracing::info!(forgotten, "notifications read a year ago were forgotten"),
        Err(error) => tracing::warn!(%error, "notifications read long ago could not be forgotten"),
    }
}

/// Makes the rounds for as long as the server runs.
pub fn keep_making_the_rounds(state: &AppState) -> tokio::task::JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move {
        let mut last_forgotten: Option<Timestamp> = None;
        loop {
            tokio::time::sleep(EVERY).await;
            let now = melyxar_core::time::now();
            if let Err(error) = super::news::announce(&state).await {
                tracing::warn!(%error, "what arrived in the libraries could not be announced");
            }
            if let Err(error) = mind_the_maintenance(&state, now).await {
                tracing::warn!(%error, "the announced maintenance could not be minded");
            }
            if last_forgotten.is_none_or(|last| now - last >= FORGETTING_EVERY) {
                forget_what_was_read_long_ago(&state, now).await;
                last_forgotten = Some(now);
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::live::follow;
    use crate::notifications::testing::a_server;
    use crate::notifications::{send, Audience, Level, Outgoing, Said};
    use melyxar_core::user::Permissions;

    #[tokio::test]
    async fn a_maintenance_is_recalled_near_its_time_and_taken_away_after() {
        let (_held, state) = a_server().await;
        let user = state
            .database()
            .create_user("somebody", None, &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let now = melyxar_core::time::now();
        let due = now + time::Duration::hours(3);
        let said = Said::Maintenance {
            title: "Down tonight".to_string(),
            text: String::new(),
        };
        let kept = send(
            &state,
            Outgoing {
                due_at: Some(due),
                ..Outgoing::new(said, Level::Attention, Audience::Everyone)
            },
        )
        .await
        .expect("sent");
        let id = kept[0].id;

        let mut line = follow(&state);
        mind_the_maintenance(&state, now).await.expect("minded");
        assert!(line.try_recv().is_err(), "three hours away: nothing yet");

        mind_the_maintenance(&state, due - time::Duration::minutes(30))
            .await
            .expect("minded");
        let news = line.try_recv().expect("recalled");
        assert!(matches!(news.change, Change::Recalled(ref told) if told.notification.id == id));

        mind_the_maintenance(&state, due).await.expect("minded");
        let news = line.try_recv().expect("taken away");
        assert!(matches!(news.change, Change::Removed(ref ids) if ids == &vec![id]));
        assert!(state
            .database()
            .notifications_of(user, None, 10)
            .await
            .expect("read")
            .is_empty());
    }
}
