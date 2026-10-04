//! Whether requests are on, and who may make them. An administrator always
//! may; anybody else once given the right.

use melyxar_core::id::UserId;
use melyxar_core::user::User;

use super::live::moved;
use super::{Refused, Result, Trouble};
use crate::AppState;

/// What one account may do with requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Access {
    pub enabled: bool,
    pub may_ask: bool,
}

/// One account, as the page that gives the right lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asker {
    pub id: UserId,
    pub name: String,
    pub administrator: bool,
    pub may_ask: bool,
}

/// Whether requests are on.
pub async fn is_on(state: &AppState) -> Result<bool> {
    Ok(state.database().requests_enabled().await?)
}

pub async fn of(state: &AppState, who: &User) -> Result<Access> {
    let enabled = is_on(state).await?;
    let may_ask = enabled
        && (who.permissions.is_administrator
            || state.database().request_rights().await?.contains(&who.id));
    Ok(Access { enabled, may_ask })
}

/// Refuses an account that may not ask, or anybody while requests are off.
pub(super) async fn require(state: &AppState, who: &User) -> Result<()> {
    match of(state, who).await?.may_ask {
        true => Ok(()),
        false => Err(Trouble::Refused(Refused::NotAllowed)),
    }
}

pub async fn switch(state: &AppState, enabled: bool) -> Result<()> {
    state.database().set_requests_enabled(enabled).await?;
    moved(state);
    Ok(())
}

/// Every account, and whether it may ask.
pub async fn askers(state: &AppState) -> Result<Vec<Asker>> {
    let database = state.database();
    let rights = database.request_rights().await?;
    Ok(database
        .list_users()
        .await?
        .into_iter()
        .map(|user| Asker {
            may_ask: user.permissions.is_administrator || rights.contains(&user.id),
            administrator: user.permissions.is_administrator,
            id: user.id,
            name: user.name,
        })
        .collect())
}

/// Gives an account the right to ask, or takes it back.
pub async fn allow(state: &AppState, user: UserId, may_ask: bool) -> Result<()> {
    let database = state.database();
    if database.user(user).await?.is_none() {
        return Err(Trouble::Failed(melyxar_core::Error::not_found("account").into()));
    }
    database.set_request_right(user, may_ask).await?;
    moved(state);
    Ok(())
}

#[cfg(test)]
mod tests {
    use melyxar_core::user::Permissions;

    use super::*;
    use crate::an_empty_server;

    #[tokio::test]
    async fn nobody_may_ask_while_off_and_then_administrators_and_who_was_given_the_right() {
        let (_held, state) = an_empty_server().await;
        let database = state.database();
        let admin = database
            .create_user("admin", None, &Permissions::administrator())
            .await
            .expect("account");
        let viewer = database
            .create_user("viewer", None, &Permissions::viewer())
            .await
            .expect("account");

        let off = of(&state, &admin).await.expect("read");
        assert_eq!(off, Access { enabled: false, may_ask: false });
        assert!(matches!(
            require(&state, &admin).await,
            Err(Trouble::Refused(Refused::NotAllowed))
        ));

        let line = super::super::live::follow(&state);
        switch(&state, true).await.expect("switched");
        assert!(line.has_changed().expect("open"), "every page is told");
        assert!(of(&state, &admin).await.expect("read").may_ask);
        assert!(!of(&state, &viewer).await.expect("read").may_ask);

        allow(&state, viewer.id, true).await.expect("allowed");
        assert!(of(&state, &viewer).await.expect("read").may_ask);
        let listed = askers(&state).await.expect("read");
        assert!(listed.iter().all(|asker| asker.may_ask));
        assert!(matches!(
            allow(&state, UserId::new(), true).await,
            Err(Trouble::Failed(_))
        ));
    }
}
