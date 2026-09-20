//! Signing in, signing out, and who is asking.
//!
//! The rules live here rather than in the layer above, so that a second way in
//! behaves exactly like the browser. The terminal already needs that: somebody
//! who has locked themselves out puts a password back from a command, and it
//! has to be the same password rule and the same signing out of every device
//! that the interface applies.
//!
//! Nothing here knows what a cookie is. It is handed a token and gives one
//! back; where that token travels and how long a browser keeps it belongs to
//! the layer that speaks HTTP.

use std::collections::HashMap;
use std::sync::Mutex;

use melyxar_auth::{fingerprint_of, hash_password, password_matches, AuthError};
use melyxar_core::id::UserId;
use melyxar_core::time::{now, Timestamp};
use melyxar_core::user::{Permissions, User};

use crate::{AppError, AppState, Result};

/// The fewest characters a password may hold, for whoever has to say so.
///
/// Sent with the refusal rather than written into the interface, so the rule
/// is stated in one place and the wording still belongs to the client.
pub use melyxar_auth::SHORTEST_PASSWORD;

/// What the layer above needs to speak about sessions, handed on from here.
///
/// Re-exported for the same reason as everything else in this crate: the layer
/// above asks the use cases and never reaches past them to the storage, which
/// is what keeps the dependencies pointing one way.
pub use melyxar_auth::SessionToken;
pub use melyxar_database::sessions::{SignedIn, A_SESSION_LASTS};

/// What somebody filling in the door can be told to put right.
///
/// A word rather than a sentence, like every refusal this server sends: the
/// wording is the client's, in its own language, and the field it belongs
/// beside is the client's to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NameNeeded,
    PasswordTooShort,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NameNeeded => "name_needed",
            Self::PasswordTooShort => "password_too_short",
        }
    }
}

/// What can go wrong here: something somebody typed, or the server itself.
///
/// Kept apart on purpose, as everywhere else a form is filled in. The first is
/// shown beside what was typed; the second is a failure and is shown as one.
#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error("refused: {}", .0.as_str())]
    Refused(Refused),
    #[error(transparent)]
    Failed(#[from] AppError),
}

impl From<melyxar_database::DatabaseError> for Trouble {
    fn from(error: melyxar_database::DatabaseError) -> Self {
        Self::Failed(AppError::from(error))
    }
}

/// Hashes a password, telling a rule it breaks apart from a failure.
fn stored_form_of(password: &str) -> std::result::Result<String, Trouble> {
    match hash_password(password) {
        Ok(hashed) => Ok(hashed),
        Err(AuthError::PasswordTooShort) => Err(Trouble::Refused(Refused::PasswordTooShort)),
        Err(other) => Err(Trouble::Failed(AppError::Auth(other))),
    }
}

/// How many wrong answers in a row an account takes before it is held back.
///
/// Ten, which is more mistakes than anybody makes typing their own password
/// and far fewer than guessing one needs.
const ALLOWED_TRIES: u32 = 10;

/// How long an account is left alone for, once it has been held back.
///
/// A minute, and again for every further round of wrong answers. Guessing a
/// password of eight characters at ten tries a minute takes longer than the
/// sun has left.
const HELD_BACK_FOR: time::Duration = time::Duration::minutes(1);

/// Wrong passwords, counted per account and kept in memory.
///
/// Two things this is for. Guessing a password should cost time rather than a
/// handful of tries; and checking one is *made* expensive on purpose, memory
/// and all, so somebody throwing a thousand guesses a second at this server
/// would be asking it to grind itself to a halt. Held back, none of those
/// guesses is ever checked, so neither happens.
///
/// Only accounts that exist are counted, so nobody can fill this by inventing
/// names: a name nobody has is answered before any password is checked, and
/// there is nothing to hold back.
#[derive(Default)]
pub struct WrongAnswers {
    by_account: Mutex<HashMap<UserId, InARow>>,
}

#[derive(Default)]
struct InARow {
    how_many: u32,
    until: Option<Timestamp>,
}

impl WrongAnswers {
    /// How much longer this account is being left alone, when it is.
    fn held_back(&self, who: UserId, at: Timestamp) -> Option<time::Duration> {
        let kept = self.by_account.lock().ok()?;
        let until = kept.get(&who)?.until?;
        (until > at).then(|| until - at)
    }

    /// One more wrong answer, which past a point holds the account back.
    fn one_more(&self, who: UserId, at: Timestamp) {
        let Ok(mut kept) = self.by_account.lock() else {
            return;
        };
        let counted = kept.entry(who).or_default();
        counted.how_many += 1;
        if counted.how_many >= ALLOWED_TRIES {
            counted.how_many = 0;
            counted.until = Some(at + HELD_BACK_FOR);
        }
    }

    /// Somebody got it right, so nothing is held against them any more.
    fn forget(&self, who: UserId) {
        if let Ok(mut kept) = self.by_account.lock() {
            kept.remove(&who);
        }
    }
}

/// What a client is handed when it signs in.
#[derive(Debug)]
pub struct OpenedSession {
    /// Shown to this client once and never stored anywhere in the clear.
    pub token: SessionToken,
    pub user: User,
}

/// What became of an attempt to sign in.
#[derive(Debug)]
pub enum SignedInOrNot {
    /// Kept behind a pointer: a session carries the whole account with it,
    /// and the two answers beside it carry almost nothing, so every one of
    /// them would otherwise be as big as the largest.
    Opened(Box<OpenedSession>),
    /// No account answers to that pair, whichever half of it was wrong.
    NotAPair,
    /// Too many wrong answers in a row on this account, so nothing is being
    /// checked for a while. Said out loud rather than answered as one more
    /// wrong password: somebody who has mistyped theirs ten times needs to be
    /// told to wait, and it tells whoever is guessing nothing they cannot see
    /// from the time it takes anyway.
    HeldBack {
        seconds: i64,
    },
}

/// What became of an attempt to change a password.
#[derive(Debug)]
pub enum PasswordChange {
    /// Changed, and here is the token that replaces every session there was.
    Changed(SessionToken),
    /// What was given as the current password is not the current password.
    NotTheCurrentOne,
}

/// Signs somebody in, or answers nothing.
///
/// One answer for every way it can fail: no such account, the wrong password,
/// or an account that has never been given one. Told apart, they would say
/// which names exist on this server, and none of the three is anything the
/// person in front of the screen can act on differently.
///
/// Not made to take the same time either way. Somebody who can reach this
/// server can also be shown the list of accounts on the sign in screen, by
/// design and by a setting that is on: hiding which names exist while offering
/// to draw them would be a lock on a door standing open.
pub async fn sign_in(
    state: &AppState,
    name: &str,
    password: &str,
    device_name: &str,
) -> Result<SignedInOrNot> {
    let Some((user, stored)) = state.database().user_by_name(name).await? else {
        return Ok(SignedInOrNot::NotAPair);
    };
    // An account waiting for its first password is not an account anybody can
    // sign into. It is the state a server sits in before the wizard has run.
    let Some(stored) = stored else {
        return Ok(SignedInOrNot::NotAPair);
    };

    let at = now();
    // Asked before anything is checked: the checking is the expensive part,
    // and not doing it is the whole point of holding an account back.
    if let Some(left) = state.wrong_answers().held_back(user.id, at) {
        tracing::warn!(account = %user.name, "held back after too many wrong passwords");
        return Ok(SignedInOrNot::HeldBack {
            seconds: left.whole_seconds().max(1),
        });
    }

    if !password_matches(password, &stored) {
        state.wrong_answers().one_more(user.id, at);
        tracing::info!(account = %user.name, "refused a sign in");
        return Ok(SignedInOrNot::NotAPair);
    }
    state.wrong_answers().forget(user.id);

    let token = SessionToken::new()?;
    state
        .database()
        .open_session(user.id, device_name, &token.fingerprint(), now())
        .await?;
    tracing::info!(account = %user.name, device = device_name, "signed in");

    Ok(SignedInOrNot::Opened(Box::new(OpenedSession { token, user })))
}

/// Whoever holds this token, with their rights and their preferences.
///
/// The one question every request asks before it answers anything.
pub async fn who_holds(state: &AppState, token: &str) -> Result<Option<SignedIn>> {
    Ok(state
        .database()
        .session_holder(&fingerprint_of(token), now())
        .await?)
}

/// Signs one device out, and says whether there was one to sign out.
pub async fn sign_out(state: &AppState, token: &str) -> Result<bool> {
    Ok(state
        .database()
        .close_session(&fingerprint_of(token))
        .await?)
}

/// Changes somebody's password, and signs every device of theirs out.
///
/// Every device including the one asking, which is then given a fresh session
/// straight away: somebody changes their password because they believe another
/// person knows it, and a change that leaves that person signed in has changed
/// nothing. Asking the one who changed it to sign in again would be a way of
/// saying the same thing, more rudely.
pub async fn change_password(
    state: &AppState,
    who: &User,
    current: &str,
    wanted: &str,
    device_name: &str,
) -> std::result::Result<PasswordChange, Trouble> {
    let stored = state
        .database()
        .user_by_name(&who.name)
        .await?
        .and_then(|(_, stored)| stored);
    // An account with no password yet is one the wizard has not finished, and
    // it is reached through the wizard rather than through here.
    let Some(stored) = stored else {
        return Ok(PasswordChange::NotTheCurrentOne);
    };
    if !password_matches(current, &stored) {
        return Ok(PasswordChange::NotTheCurrentOne);
    }

    // Hashed before anything is taken away, so a password the rule refuses
    // leaves the account exactly as it was rather than signed out of
    // everywhere with its old password still on it.
    let hashed = stored_form_of(wanted)?;
    state.database().set_password(who.id, Some(&hashed)).await?;
    let closed = state.database().close_every_session_of(who.id).await?;
    tracing::info!(account = %who.name, closed, "changed a password and signed every device out");

    let token = SessionToken::new().map_err(|error| Trouble::Failed(AppError::Auth(error)))?;
    state
        .database()
        .open_session(who.id, device_name, &token.fingerprint(), now())
        .await?;
    Ok(PasswordChange::Changed(token))
}

/// Whether this server still has to be set up.
///
/// One account is what tells a server that has been set up from one that has
/// not: there is nothing else a brand new server is missing that it cannot go
/// on without.
pub async fn still_to_be_set_up(state: &AppState) -> Result<bool> {
    Ok(state.database().user_count().await? == 0)
}

/// Creates the very first account, which is an administrator.
///
/// The one thing on this server that answers without anybody signed in, so it
/// refuses the moment there is an account to sign in as. Whoever reaches a
/// brand new server first is whoever installed it; a second person reaching it
/// later finds a door that is shut.
///
/// The name is theirs to choose. A name decided here would be one more thing
/// to explain, and it would be the same on every installation of this server
/// in the world.
pub async fn create_the_first_account(
    state: &AppState,
    name: &str,
    password: &str,
) -> std::result::Result<User, Trouble> {
    if !still_to_be_set_up(state).await.map_err(Trouble::Failed)? {
        return Err(Trouble::Failed(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Conflict,
            "this server has already been set up",
        ))));
    }
    let name = name.trim();
    if name.is_empty() {
        return Err(Trouble::Refused(Refused::NameNeeded));
    }

    let hashed = stored_form_of(password)?;
    let user = state
        .database()
        .create_user(name, Some(&hashed), &Permissions::administrator())
        .await?;
    tracing::info!(account = %user.name, "created the first account");
    Ok(user)
}

/// Puts a password on an account from outside, and signs every device out.
///
/// The way back in for somebody who has locked themselves out of their own
/// server. It asks for no current password, so it is reachable only from a
/// terminal on the machine itself, never over HTTP: whoever has a shell there
/// can read the database anyway.
///
/// Says whether there was an account by that name, so the command can tell
/// somebody they have the name wrong rather than claiming to have done
/// something.
pub async fn set_a_password(
    state: &AppState,
    name: &str,
    password: &str,
) -> std::result::Result<bool, Trouble> {
    let Some((user, _)) = state.database().user_by_name(name).await? else {
        return Ok(false);
    };
    let hashed = stored_form_of(password)?;
    state.database().set_password(user.id, Some(&hashed)).await?;
    let closed = state.database().close_every_session_of(user.id).await?;
    tracing::warn!(
        account = %user.name,
        closed,
        "a password was set from the terminal and every device signed out"
    );
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories};
    use melyxar_database::Database;
    use time::macros::datetime;

    async fn a_server() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = Config {
            directories: Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
            },
            ..Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let database = Database::open_in_memory().await.expect("database opens");
        (directory, AppState::new(config, database, None, None))
    }

    /// Signing in as the tests that are about something else want it: the
    /// session when there is one, and nothing when the pair is not a pair.
    async fn a_session(
        state: &AppState,
        name: &str,
        password: &str,
        device: &str,
    ) -> Option<OpenedSession> {
        match sign_in(state, name, password, device).await.expect("asked") {
            SignedInOrNot::Opened(opened) => Some(*opened),
            SignedInOrNot::NotAPair => None,
            SignedInOrNot::HeldBack { seconds } => panic!("held back for {seconds} seconds"),
        }
    }

    async fn a_server_with_an_account() -> (tempfile::TempDir, AppState) {
        let (directory, state) = a_server().await;
        create_the_first_account(&state, "victor", "quiet harbour")
            .await
            .expect("first account created");
        (directory, state)
    }

    #[tokio::test]
    async fn a_brand_new_server_says_it_still_has_to_be_set_up() {
        let (_directory, state) = a_server().await;
        assert!(still_to_be_set_up(&state).await.expect("asked"));

        create_the_first_account(&state, "victor", "quiet harbour")
            .await
            .expect("first account created");
        assert!(!still_to_be_set_up(&state).await.expect("asked"));
    }

    #[tokio::test]
    async fn the_first_account_is_an_administrator_and_is_named_by_whoever_makes_it() {
        let (_directory, state) = a_server().await;
        let user = create_the_first_account(&state, "victor", "quiet harbour")
            .await
            .expect("first account created");
        assert_eq!(user.name, "victor");
        assert!(user.permissions.is_administrator);
    }

    #[tokio::test]
    async fn the_door_that_makes_the_first_account_shuts_behind_it() {
        // It answers without anybody signed in, so a second person reaching
        // this server later must not be able to make themselves an
        // administrator on it.
        let (_directory, state) = a_server_with_an_account().await;
        assert!(create_the_first_account(&state, "someone else", "another password")
            .await
            .is_err());
    }

    #[tokio::test]
    async fn an_account_cannot_be_made_without_a_name_or_with_too_short_a_password() {
        let (_directory, state) = a_server().await;
        assert!(create_the_first_account(&state, "   ", "quiet harbour")
            .await
            .is_err());
        assert!(create_the_first_account(&state, "victor", "short")
            .await
            .is_err());
        assert!(
            still_to_be_set_up(&state).await.expect("asked"),
            "a refused attempt leaves the server waiting rather than half set up"
        );
    }

    #[tokio::test]
    async fn signing_in_with_the_right_password_gives_a_token_that_names_the_account() {
        let (_directory, state) = a_server_with_an_account().await;
        let opened = a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .expect("signed in");
        assert_eq!(opened.user.name, "victor");

        let holder = who_holds(&state, opened.token.as_text())
            .await
            .expect("asked")
            .expect("somebody holds it");
        assert_eq!(holder.user.id, opened.user.id);
    }

    #[tokio::test]
    async fn the_name_is_read_however_it_was_capitalised() {
        let (_directory, state) = a_server_with_an_account().await;
        assert!(a_session(&state, "VICTOR", "quiet harbour", "a browser")
            .await
            .is_some());
    }

    #[tokio::test]
    async fn every_way_of_failing_to_sign_in_gives_the_same_answer() {
        let (_directory, state) = a_server_with_an_account().await;

        // The wrong password.
        assert!(a_session(&state, "victor", "another password", "a browser")
            .await
            .is_none());
        // A name nobody has.
        assert!(a_session(&state, "nobody", "quiet harbour", "a browser")
            .await
            .is_none());
        // An account that has never been given a password: without this, one
        // could be signed into by typing nothing at all.
        state
            .database()
            .create_user("waiting", None, &Permissions::viewer())
            .await
            .expect("account created");
        assert!(a_session(&state, "waiting", "", "a browser")
            .await
            .is_none());
    }

    #[tokio::test]
    async fn too_many_wrong_passwords_hold_an_account_back() {
        let (_directory, state) = a_server_with_an_account().await;

        for _ in 0..ALLOWED_TRIES {
            assert!(matches!(
                sign_in(&state, "victor", "not the password", "a browser")
                    .await
                    .expect("asked"),
                SignedInOrNot::NotAPair
            ));
        }

        // The right password is not even looked at while it is held back,
        // which is the point of holding it back: checking one is made
        // expensive on purpose, so a thousand guesses a second would be asking
        // this server to grind itself to a halt.
        let held = sign_in(&state, "victor", "quiet harbour", "a browser")
            .await
            .expect("asked");
        let SignedInOrNot::HeldBack { seconds } = held else {
            panic!("ten wrong answers in a row have to hold an account back");
        };
        assert!(seconds > 0, "it has to say how long to wait: {seconds}");
    }

    #[tokio::test]
    async fn getting_it_right_clears_what_was_held_against_an_account() {
        let (_directory, state) = a_server_with_an_account().await;

        for _ in 0..ALLOWED_TRIES - 1 {
            sign_in(&state, "victor", "not the password", "a browser")
                .await
                .expect("asked");
        }
        assert!(a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .is_some());

        // Back to nothing, so the next mistake is the first one again and
        // somebody who mistypes their password now and then is never locked
        // out by a week of them.
        for _ in 0..ALLOWED_TRIES - 1 {
            sign_in(&state, "victor", "not the password", "a browser")
                .await
                .expect("asked");
        }
        assert!(a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .is_some());
    }

    #[tokio::test]
    async fn holding_one_account_back_leaves_every_other_alone() {
        let (_directory, state) = a_server_with_an_account().await;
        state
            .database()
            .create_user("someone", None, &Permissions::viewer())
            .await
            .expect("account created");
        set_a_password(&state, "someone", "amber field road")
            .await
            .expect("password set");

        for _ in 0..ALLOWED_TRIES {
            sign_in(&state, "victor", "not the password", "a browser")
                .await
                .expect("asked");
        }

        // Otherwise anybody could shut everyone else out of their own server
        // by typing nonsense at one account.
        assert!(a_session(&state, "someone", "amber field road", "a browser")
            .await
            .is_some());
    }

    #[test]
    fn an_account_is_left_alone_again_once_the_wait_is_over() {
        let counted = WrongAnswers::default();
        let who = UserId::new();
        let at = datetime!(2026-01-01 12:00 UTC);

        for _ in 0..ALLOWED_TRIES {
            counted.one_more(who, at);
        }
        assert!(counted.held_back(who, at).is_some());
        assert!(
            counted
                .held_back(who, at + HELD_BACK_FOR + time::Duration::seconds(1))
                .is_none(),
            "being held back is a wait, not a lock somebody has to come and undo"
        );
    }

    #[tokio::test]
    async fn signing_out_ends_that_session_and_leaves_the_others() {
        let (_directory, state) = a_server_with_an_account().await;
        let here = a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .expect("signed in");
        let elsewhere = a_session(&state, "victor", "quiet harbour", "a television")
            .await
            .expect("signed in");

        assert!(sign_out(&state, here.token.as_text()).await.expect("asked"));
        assert!(who_holds(&state, here.token.as_text())
            .await
            .expect("asked")
            .is_none());
        assert!(
            who_holds(&state, elsewhere.token.as_text())
                .await
                .expect("asked")
                .is_some(),
            "leaving one machine must not sign the television out"
        );
    }

    #[tokio::test]
    async fn changing_a_password_ends_every_other_session_and_keeps_this_one_going() {
        let (_directory, state) = a_server_with_an_account().await;
        let here = a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .expect("signed in");
        let somebody_else = a_session(&state, "victor", "quiet harbour", "another browser")
            .await
            .expect("signed in");

        let changed = change_password(
            &state,
            &here.user,
            "quiet harbour",
            "amber field road",
            "a browser",
        )
        .await
        .expect("asked");
        let PasswordChange::Changed(fresh) = changed else {
            panic!("the current password was the current one");
        };

        assert!(
            who_holds(&state, somebody_else.token.as_text())
                .await
                .expect("asked")
                .is_none(),
            "whoever the password was changed because of is signed out"
        );
        assert!(
            who_holds(&state, here.token.as_text())
                .await
                .expect("asked")
                .is_none(),
            "the old session of the one who changed it goes too"
        );
        assert!(
            who_holds(&state, fresh.as_text())
                .await
                .expect("asked")
                .is_some(),
            "and they carry on with a fresh one rather than being sent back to the door"
        );

        assert!(a_session(&state, "victor", "amber field road", "a browser")
            .await
            .is_some());
        assert!(a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .is_none());
    }

    #[tokio::test]
    async fn the_wrong_current_password_changes_nothing() {
        let (_directory, state) = a_server_with_an_account().await;
        let here = a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .expect("signed in");

        let outcome = change_password(
            &state,
            &here.user,
            "not the current one",
            "amber field road",
            "a browser",
        )
        .await
        .expect("asked");
        assert!(matches!(outcome, PasswordChange::NotTheCurrentOne));

        assert!(
            who_holds(&state, here.token.as_text())
                .await
                .expect("asked")
                .is_some(),
            "a refused change signs nobody out"
        );
        assert!(a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .is_some());
    }

    #[tokio::test]
    async fn a_password_the_rule_refuses_leaves_the_account_exactly_as_it_was() {
        let (_directory, state) = a_server_with_an_account().await;
        let here = a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .expect("signed in");

        assert!(
            change_password(&state, &here.user, "quiet harbour", "short", "a browser")
                .await
                .is_err()
        );
        assert!(
            who_holds(&state, here.token.as_text())
                .await
                .expect("asked")
                .is_some(),
            "signed out of everywhere with the old password still on it would be the worst of both"
        );
        assert!(a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .is_some());
    }

    #[tokio::test]
    async fn a_password_put_back_from_the_terminal_works_and_ends_every_session() {
        let (_directory, state) = a_server_with_an_account().await;
        let locked_out = a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .expect("signed in");

        assert!(set_a_password(&state, "victor", "amber field road")
            .await
            .expect("asked"));
        assert!(
            who_holds(&state, locked_out.token.as_text())
                .await
                .expect("asked")
                .is_none()
        );
        assert!(a_session(&state, "victor", "amber field road", "a browser")
            .await
            .is_some());

        assert!(
            !set_a_password(&state, "nobody", "amber field road")
                .await
                .expect("asked"),
            "a name nobody has is said so rather than claimed to have been done"
        );
    }
}
