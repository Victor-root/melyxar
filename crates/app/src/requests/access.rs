//! Whether requests are on, and who may make them. An administrator always
//! may; anybody else once given the right.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

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

/// The same, for what asks the metadata provider, counted for the account.
///
/// The provider holds back the whole server once it is asked too much,
/// identification included: an account looking titles up as fast as a
/// program can would leave every film added meanwhile unidentified. Sixty a
/// minute is one a second, more than anybody types.
pub(super) async fn require_a_look(state: &AppState, who: &User) -> Result<()> {
    require(state, who).await?;
    match a_look_allowed(who.id, Instant::now()) {
        true => Ok(()),
        false => Err(Trouble::Refused(Refused::TooFast)),
    }
}

/// How many times an account may ask the provider in a minute.
const LOOKS_A_MINUTE: u32 = 60;
const A_MINUTE: Duration = Duration::from_secs(60);

/// Counts one look for this account, and answers whether it is allowed.
fn a_look_allowed(who: UserId, at: Instant) -> bool {
    static LOOKED: OnceLock<Mutex<HashMap<UserId, (Instant, u32)>>> = OnceLock::new();
    let mut looked = LOOKED
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    looked.retain(|_, (since, _)| at.duration_since(*since) < A_MINUTE);
    let (_, looks) = looked.entry(who).or_insert((at, 0));
    *looks += 1;
    *looks <= LOOKS_A_MINUTE
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

    #[test]
    fn an_account_asks_the_provider_sixty_times_a_minute_at_most() {
        let who = UserId::new();
        let at = Instant::now();
        for _ in 0..LOOKS_A_MINUTE {
            assert!(a_look_allowed(who, at));
        }
        assert!(!a_look_allowed(who, at + Duration::from_secs(59)));
        assert!(a_look_allowed(UserId::new(), at), "another account is not held back");
        assert!(a_look_allowed(who, at + A_MINUTE), "and a minute later, nor is this one");
    }

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
