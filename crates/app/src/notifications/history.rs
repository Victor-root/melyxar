//! What an account keeps: its notifications page by page, read or unread,
//! and the ones it removes. Every change is told to all its pages at once.

use melyxar_core::id::{NotificationId, UserId};

use super::live::{tell, Change};
use super::{with_posters, Told};
use crate::{AppState, Result};

/// How many notifications a page of the history holds.
const A_PAGE: usize = 30;

/// One page of an account's history, newest first.
#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub told: Vec<Told>,
    /// Unread over the whole history, not only this page.
    pub unread: i64,
    /// Whether older ones remain.
    pub more: bool,
}

/// The page older than a notification already shown, or the newest.
pub async fn page(state: &AppState, user: UserId, before: Option<NotificationId>) -> Result<Page> {
    let database = state.database();
    let mut found = database
        .notifications_of(user, before, A_PAGE as i64 + 1)
        .await?;
    let more = found.len() > A_PAGE;
    found.truncate(A_PAGE);
    Ok(Page {
        told: with_posters(state, found).await?,
        unread: database.unread_notifications(user).await?,
        more,
    })
}

/// Marks read the notifications named, or every one when none is.
pub async fn mark_read(state: &AppState, user: UserId, which: Option<&[NotificationId]>) -> Result<()> {
    let now = melyxar_core::time::now();
    let changed = state.database().mark_notifications(user, which, Some(now)).await?;
    if !changed.is_empty() {
        tell(state, user, Change::Read(changed));
    }
    Ok(())
}

/// Marks unread the notifications named.
pub async fn mark_unread(state: &AppState, user: UserId, which: &[NotificationId]) -> Result<()> {
    let changed = state.database().mark_notifications(user, Some(which), None).await?;
    if !changed.is_empty() {
        tell(state, user, Change::Unread(changed));
    }
    Ok(())
}

/// Removes one notification from an account's history, whatever it is.
pub async fn remove(state: &AppState, user: UserId, id: NotificationId) -> Result<()> {
    match state.database().remove_notification(user, id).await? {
        true => {
            tell(state, user, Change::Removed(vec![id]));
            Ok(())
        }
        false => Err(melyxar_core::Error::not_found("no such notification").into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::notifications::live::follow;
    use crate::an_empty_server;
    use crate::notifications::{send, Audience, Level, Outgoing, Said};
    use melyxar_core::user::Permissions;

    async fn sent(state: &AppState, user: UserId, priority: bool) -> NotificationId {
        let said = Said::Message {
            title: "Hello".to_string(),
            text: String::new(),
        };
        let outgoing = Outgoing {
            priority,
            ..Outgoing::new(said, Level::Ok, Audience::Accounts(vec![user]))
        };
        send(state, outgoing).await.expect("sent")[0].id
    }

    #[tokio::test]
    async fn read_unread_and_removed_are_told_to_every_page_of_the_account() {
        let (_held, state) = an_empty_server().await;
        let user = state
            .database()
            .create_user("somebody", None, &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let plain = sent(&state, user, false).await;
        let urgent = sent(&state, user, true).await;

        let mut line = follow(&state);
        mark_read(&state, user, None).await.expect("marked");
        mark_read(&state, user, None).await.expect("nothing left to mark");
        mark_unread(&state, user, &[plain]).await.expect("marked");
        remove(&state, user, plain).await.expect("removed");
        assert!(remove(&state, user, plain).await.is_err(), "already gone");
        remove(&state, user, urgent).await.expect("an urgent one goes like the others");

        let mut said = Vec::new();
        while let Ok(news) = line.try_recv() {
            said.push(match news.change {
                Change::Read(mut ids) => {
                    ids.sort();
                    let mut both = vec![plain, urgent];
                    both.sort();
                    assert_eq!(ids, both);
                    "read"
                }
                Change::Unread(ids) => {
                    assert_eq!(ids, vec![plain]);
                    "unread"
                }
                Change::Removed(ids) => {
                    assert!(ids == vec![plain] || ids == vec![urgent]);
                    "removed"
                }
                _ => "other",
            });
        }
        assert_eq!(
            said,
            vec!["read", "unread", "removed", "removed"],
            "a mark that changed nothing says nothing"
        );

        let page = page(&state, user, None).await.expect("read");
        assert!(page.told.is_empty());
        assert_eq!(page.unread, 0);
        assert!(!page.more);
    }

    #[tokio::test]
    async fn the_history_comes_a_page_at_a_time() {
        let (_held, state) = an_empty_server().await;
        let user = state
            .database()
            .create_user("somebody", None, &Permissions::viewer())
            .await
            .expect("account")
            .id;
        for _ in 0..A_PAGE + 2 {
            sent(&state, user, false).await;
        }
        let first = page(&state, user, None).await.expect("read");
        assert_eq!(first.told.len(), A_PAGE);
        assert!(first.more);
        assert_eq!(first.unread, A_PAGE as i64 + 2);
        let last = first.told.last().map(|told| told.notification.id);
        let second = page(&state, user, last).await.expect("read");
        assert_eq!(second.told.len(), 2);
        assert!(!second.more);
    }
}
