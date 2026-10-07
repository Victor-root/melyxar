//! Passwords hashed and checked away from the threads that answer requests,
//! a few at a time.
//!
//! Turning a password into its stored form, or checking one against it, is
//! made slow and hungry for memory on purpose. Done where requests are
//! answered, a burst of sign ins would freeze every page and every segment of
//! a film for as long as it lasted. So it is done where blocking work goes,
//! and never more than [`AT_ONCE`] of it together: unbounded, the same burst
//! would ask for a stored form's worth of memory hundreds of times over. The
//! ones past that wait their turn without holding any thread.

use std::sync::Arc;

use melyxar_auth::AuthError;
use tokio::sync::{OnceCell, Semaphore};

/// How many passwords are hashed or checked at the same moment.
///
/// Two, which nobody signing in ever waits behind: a check takes a few tens
/// of milliseconds, and people do not sign in twenty times a second.
const AT_ONCE: usize = 2;

/// What the stand-in for an account nobody holds is made from. Its stored
/// form is drawn with a fresh salt like any other, so nothing is learnt from
/// knowing it.
const NOBODY_S: &str = "a password that belongs to nobody at all";

pub(crate) struct Passwords {
    /// Each turn is held by the work itself rather than by the request that
    /// asked for it: somebody who gives up waiting does not free a turn the
    /// work they started is still using.
    turns: Arc<Semaphore>,
    /// The stored form a password is checked against when the name it came
    /// with has no account: made the first time it is needed.
    nobody_s: OnceCell<String>,
}

impl Default for Passwords {
    fn default() -> Self {
        Self {
            turns: Arc::new(Semaphore::new(AT_ONCE)),
            nobody_s: OnceCell::new(),
        }
    }
}

impl Passwords {
    /// The stored form of a password, or the rule it breaks.
    pub(crate) async fn hash(&self, password: &str) -> Result<String, AuthError> {
        let password = password.to_string();
        let turn = self
            .turns
            .clone()
            .acquire_owned()
            .await
            .map_err(|error| AuthError::CouldNotHash(error.to_string()))?;
        tokio::task::spawn_blocking(move || {
            let _turn = turn;
            melyxar_auth::hash_password(&password)
        })
        .await
        .map_err(|error| AuthError::CouldNotHash(error.to_string()))?
    }

    /// Whether a password is the one behind a stored form.
    ///
    /// A check that could not be run answers no, like a stored form that
    /// cannot be read: refusing is the only safe thing to say about a
    /// password nobody looked at.
    pub(crate) async fn matches(&self, password: &str, stored: &str) -> bool {
        let (password, stored) = (password.to_string(), stored.to_string());
        let Ok(turn) = self.turns.clone().acquire_owned().await else {
            return false;
        };
        tokio::task::spawn_blocking(move || {
            let _turn = turn;
            melyxar_auth::password_matches(&password, &stored)
        })
        .await
        .unwrap_or(false)
    }

    /// Checks a password the way it would be checked against an account, for
    /// a name that has none. Answered at once, a name with no account behind
    /// it would say so to whoever times the answer, and the list of names
    /// kept from the door would be read back one guess at a time.
    pub(crate) async fn checked_against_nobody(&self, password: &str) {
        if let Ok(stored) = self.nobody_s.get_or_try_init(|| self.hash(NOBODY_S)).await {
            self.matches(password, stored).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_password_hashed_here_is_recognised_here() {
        let passwords = Passwords::default();
        let stored = passwords.hash("quiet harbour").await.expect("hashed");
        assert!(passwords.matches("quiet harbour", &stored).await);
        assert!(!passwords.matches("quiet harbours", &stored).await);
    }

    #[tokio::test]
    async fn a_name_with_no_account_takes_as_long_as_one_with_an_account() {
        let passwords = Passwords::default();
        let stored = passwords.hash("quiet harbour").await.expect("hashed");
        // The stand-in is made the first time, and only timed afterwards.
        passwords.checked_against_nobody("a guess").await;

        let timed = |checked: std::time::Instant| checked.elapsed().as_secs_f64();
        let started = std::time::Instant::now();
        passwords.matches("a guess", &stored).await;
        let with_an_account = timed(started);
        let started = std::time::Instant::now();
        passwords.checked_against_nobody("a guess").await;
        let with_none = timed(started);

        assert!(
            with_none > with_an_account / 3.0,
            "{with_none} s against {with_an_account} s: the same work, so the same time"
        );
    }

    #[tokio::test]
    async fn the_password_rule_still_speaks_from_here() {
        assert!(matches!(
            Passwords::default().hash("short").await,
            Err(AuthError::PasswordTooShort)
        ));
    }

    #[tokio::test]
    async fn no_more_than_the_allowed_number_run_together() {
        let passwords = std::sync::Arc::new(Passwords::default());
        let stored = passwords.hash("quiet harbour").await.expect("hashed");
        let held: Vec<_> = (0..AT_ONCE)
            .map(|_| passwords.turns.try_acquire().expect("a turn is free"))
            .collect();

        let waiting = tokio::spawn({
            let passwords = passwords.clone();
            async move { passwords.matches("quiet harbour", &stored).await }
        });
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(!waiting.is_finished(), "every turn is taken, so the check waits");

        drop(held);
        assert!(waiting.await.expect("checked"));
    }
}
