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
//! the layer that speaks HTTP. What is kept here is only the wish somebody
//! expressed about it, written down beside the device, so that a token handed
//! to the same browser again is kept for exactly as long as the one before.

use std::collections::HashMap;
use std::sync::Mutex;

use melyxar_auth::{fingerprint_of, hash_password, password_matches, AuthError};
use melyxar_core::id::UserId;
use melyxar_core::time::{now, Timestamp};
use melyxar_core::user::{Permissions, User};

use melyxar_database::users::KeptAnAdministrator;

use crate::activity::{record, Event};
use crate::{AppError, AppState, Result};

/// How much of a name typed at the door is written down when it was refused:
/// anything can be typed there, and a journal is not where it goes whole.
const LONGEST_NAME_WRITTEN: usize = 64;

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
pub use melyxar_database::sessions::{Remembered, SignedIn, AN_UNUSED_SESSION_IS_KEPT_FOR};

/// What somebody filling in the door can be told to put right.
///
/// A word rather than a sentence, like every refusal this server sends: the
/// wording is the client's, in its own language, and the field it belongs
/// beside is the client's to know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NameNeeded,
    NameTaken,
    PasswordTooShort,
    /// Afterwards nobody would be an administrator of this server.
    LastAdministrator,
    /// Not something an administrator does to their own account from the
    /// administration: stepping down, taking it away, signing it out of
    /// everywhere, or putting a password on it without the current one.
    NotYourself,
    /// A library that is not on this server was granted.
    NoSuchLibrary,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NameNeeded => "name_needed",
            Self::NameTaken => "name_taken",
            Self::PasswordTooShort => "password_too_short",
            Self::LastAdministrator => "last_administrator",
            Self::NotYourself => "not_yourself",
            Self::NoSuchLibrary => "no_such_library",
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

/// A name as it is kept: without the spaces around it, and never empty.
fn name_of(asked: &str) -> std::result::Result<&str, Trouble> {
    let name = asked.trim();
    if name.is_empty() {
        return Err(Trouble::Refused(Refused::NameNeeded));
    }
    Ok(name)
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
///
/// A browser that says which one it is has the session this account held on
/// it replaced rather than joined by another.
pub async fn sign_in(
    state: &AppState,
    name: &str,
    password: &str,
    device_name: &str,
    remembered: Remembered,
    client: Option<&str>,
) -> Result<SignedInOrNot> {
    let refused = || {
        record(
            state,
            Event::SignInRefused {
                name: name.chars().take(LONGEST_NAME_WRITTEN).collect(),
                device: device_name.to_string(),
            },
        )
    };
    let Some((user, stored)) = state.database().user_by_name(name).await? else {
        refused().await;
        return Ok(SignedInOrNot::NotAPair);
    };
    // An account waiting for its first password is not an account anybody can
    // sign into. It is the state a server sits in before the wizard has run.
    let Some(stored) = stored else {
        refused().await;
        return Ok(SignedInOrNot::NotAPair);
    };

    let at = now();
    // Asked before anything is checked: the checking is the expensive part,
    // and not doing it is the whole point of holding an account back.
    if let Some(left) = state.wrong_answers().held_back(user.id, at) {
        tracing::warn!(account = %user.name, "held back after too many wrong passwords");
        record(
            state,
            Event::SignInHeldBack {
                user: user.id,
                user_name: user.name.clone(),
                device: device_name.to_string(),
            },
        )
        .await;
        return Ok(SignedInOrNot::HeldBack {
            seconds: left.whole_seconds().max(1),
        });
    }

    if !password_matches(password, &stored) {
        state.wrong_answers().one_more(user.id, at);
        tracing::info!(account = %user.name, "refused a sign in");
        refused().await;
        return Ok(SignedInOrNot::NotAPair);
    }
    state.wrong_answers().forget(user.id);

    let token = SessionToken::new()?;
    let device_id = state
        .database()
        .open_session(
            user.id,
            device_name,
            &token.fingerprint(),
            remembered,
            now(),
            a_browser_identifier(client),
        )
        .await?;
    tracing::info!(account = %user.name, device = device_name, "signed in");
    record(
        state,
        Event::SignedIn {
            user: user.id,
            user_name: user.name.clone(),
            device: device_name.to_string(),
            device_id,
        },
    )
    .await;

    Ok(SignedInOrNot::Opened(Box::new(OpenedSession { token, user })))
}

/// The longest identifier a browser may give itself. The ones this
/// interface makes are thirty six characters.
const LONGEST_BROWSER_IDENTIFIER: usize = 64;

/// What a browser said it is called, when it has the shape of an identifier.
///
/// Anything else is ignored rather than refused: it only ever serves to find
/// the session this account held on the same browser, and a browser saying
/// nothing usable simply signs in as a new device.
fn a_browser_identifier(said: Option<&str>) -> Option<&str> {
    said.filter(|said| {
        !said.is_empty()
            && said.len() <= LONGEST_BROWSER_IDENTIFIER
            && said
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
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

/// The longest name a page may give its browser. Every real one is a word
/// or two.
pub const LONGEST_BROWSER_NAME: usize = 40;

/// Writes down which browser a device is, as its own page found, when that
/// is news. A page says it every time it opens, and saying the same again
/// writes nothing.
pub async fn name_the_browser(
    state: &AppState,
    device: melyxar_core::id::DeviceId,
    known: Option<&str>,
    said: Option<&str>,
) -> Result<()> {
    let said = said.map(str::trim).filter(|name| !name.is_empty());
    if said.is_some_and(|name| name.chars().count() > LONGEST_BROWSER_NAME) {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "a browser is not called anything that long",
        )));
    }
    if said == known {
        return Ok(());
    }
    state.database().name_the_browser(device, said).await?;
    // The line of its sign in was written before the page could say, and
    // would otherwise go on naming the browser the line it sends claims.
    if let Some(browser) = said {
        state.journal().browser_found(device, browser).await;
    }
    Ok(())
}

/// Signs one device out, and says whether there was one to sign out.
pub async fn sign_out(state: &AppState, token: &str) -> Result<bool> {
    let holder = who_holds(state, token).await?;
    let closed = state
        .database()
        .close_session(&fingerprint_of(token))
        .await?;
    if let Some(holder) = holder.filter(|_| closed) {
        record(
            state,
            Event::SignedOut {
                user: holder.user.id,
                user_name: holder.user.name,
                device: holder.device_name,
                browser: holder.device_browser,
            },
        )
        .await;
    }
    Ok(closed)
}

/// Changes somebody's password, and signs every device of theirs out.
///
/// Every device including the one asking, which is then given a fresh session
/// straight away: somebody changes their password because they believe another
/// person knows it, and a change that leaves that person signed in has changed
/// nothing. Asking the one who changed it to sign in again would be a way of
/// saying the same thing, more rudely.
///
/// The fresh session is kept exactly as the one it replaces was: somebody who
/// signed in on a machine that is not theirs did not ask to be remembered on
/// it, and changing a password is not a place to quietly decide otherwise.
pub async fn change_password(
    state: &AppState,
    who: &User,
    current: &str,
    wanted: &str,
    device_name: &str,
    remembered: Remembered,
    client: Option<&str>,
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
    record(
        state,
        Event::PasswordChanged {
            user: who.id,
            user_name: who.name.clone(),
        },
    )
    .await;

    let token = SessionToken::new().map_err(|error| Trouble::Failed(AppError::Auth(error)))?;
    state
        .database()
        .open_session(
            who.id,
            device_name,
            &token.fingerprint(),
            remembered,
            now(),
            a_browser_identifier(client),
        )
        .await?;
    Ok(PasswordChange::Changed(token))
}

/// Gives somebody's account the name they asked for.
///
/// Nothing else moves: everything of theirs hangs off the account and not off
/// its name, and every device of theirs stays signed in. The lines already in
/// the journal keep the name they were written under.
pub async fn rename(
    state: &AppState,
    who: &User,
    wanted: &str,
) -> std::result::Result<User, Trouble> {
    let name = name_of(wanted)?;
    if name == who.name {
        return Ok(who.clone());
    }
    if !state.database().rename_user(who.id, name).await? {
        return Err(Trouble::Refused(Refused::NameTaken));
    }
    tracing::info!(from = %who.name, to = %name, "renamed an account");
    record(
        state,
        Event::AccountRenamed {
            user: who.id,
            from: who.name.clone(),
            to: name.to_string(),
        },
    )
    .await;
    Ok(User {
        name: name.to_string(),
        ..who.clone()
    })
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
    let name = name_of(name)?;

    // Hashed before the door is looked at, because hashing is the slow part
    // and a password the rule refuses is refused whatever else is true.
    let hashed = stored_form_of(password)?;

    // The looking and the writing happen together down there, so two people
    // reaching a brand new server in the same breath cannot both come through.
    let Some(user) = state.database().create_the_first_user(name, &hashed).await? else {
        return Err(Trouble::Failed(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Conflict,
            "this server has already been set up",
        ))));
    };
    tracing::info!(account = %user.name, "created the first account");
    record(
        state,
        Event::AccountCreated {
            user: user.id,
            user_name: user.name.clone(),
        },
    )
    .await;
    Ok(user)
}

/// One account as the administration lists it: the account itself, and the
/// devices it is signed in on.
#[derive(Debug, Clone, PartialEq)]
pub struct Listed {
    pub user: User,
    /// Absent for an account signed in nowhere.
    pub devices: Option<DevicesOfAnAccount>,
}

pub use melyxar_database::sessions::DevicesOfAnAccount;

/// Every account of this server, ordered by name, with where each is signed
/// in. Two reads whatever the number of accounts.
pub async fn every_account(state: &AppState) -> Result<Vec<Listed>> {
    let database = state.database();
    let mut devices = database.devices_of_every_account().await?;
    Ok(database
        .list_users()
        .await?
        .into_iter()
        .map(|user| Listed {
            devices: devices.remove(&user.id),
            user,
        })
        .collect())
}

/// The account behind an identifier, or the refusal a page shows for one
/// that is not there.
async fn account(state: &AppState, id: UserId) -> std::result::Result<User, Trouble> {
    state
        .database()
        .user(id)
        .await?
        .ok_or_else(|| Trouble::Failed(melyxar_core::Error::not_found("account").into()))
}

/// Refuses what an administrator may not do to their own account.
fn not_to_yourself(acting: &User, target: UserId) -> std::result::Result<(), Trouble> {
    match acting.id == target {
        true => Err(Trouble::Refused(Refused::NotYourself)),
        false => Ok(()),
    }
}

/// Rights as they will be kept, every library they grant checked to be one
/// this server has.
///
/// A grant of a library that is not there is refused rather than dropped: it
/// is a screen out of date, and saying so is better than quietly granting
/// less than was asked.
async fn checked(
    state: &AppState,
    wanted: &Permissions,
) -> std::result::Result<Permissions, Trouble> {
    let settled = wanted.clone().settled();
    if !settled.allowed_libraries.is_empty() {
        let there: Vec<_> = state
            .database()
            .list_libraries()
            .await?
            .into_iter()
            .map(|library| library.id)
            .collect();
        if settled
            .allowed_libraries
            .iter()
            .any(|library| !there.contains(library))
        {
            return Err(Trouble::Refused(Refused::NoSuchLibrary));
        }
    }
    Ok(settled)
}

/// Makes an account, with the rights it is to have.
///
/// The one way an account is made once the server is set up, from the
/// administration as from a terminal on the machine itself.
pub async fn create_account(
    state: &AppState,
    name: &str,
    password: &str,
    permissions: &Permissions,
) -> std::result::Result<User, Trouble> {
    let name = name_of(name)?;
    let permissions = checked(state, permissions).await?;
    let hashed = stored_form_of(password)?;
    // Left to the unique index, as a rename is, so two accounts made under
    // one name in the same breath cannot both come through.
    let user = match state
        .database()
        .create_user(name, Some(&hashed), &permissions)
        .await
    {
        Ok(user) => user,
        Err(error) if error.is_a_duplicate() => {
            return Err(Trouble::Refused(Refused::NameTaken));
        }
        Err(error) => return Err(error.into()),
    };
    tracing::info!(
        account = %user.name,
        administrator = permissions.is_administrator,
        "made an account"
    );
    record(
        state,
        Event::AccountCreated {
            user: user.id,
            user_name: user.name.clone(),
        },
    )
    .await;
    Ok(user)
}

/// What the administration decides about an account.
///
/// The right to download and the age limit are not among them yet: neither
/// is applied anywhere, so an account keeps whatever it holds of them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rights {
    pub is_administrator: bool,
    pub sees_every_library: bool,
    pub libraries: Vec<melyxar_core::id::LibraryId>,
    pub may_delete: bool,
    pub may_delete_from_disk: bool,
    /// How many films it may watch at once, when it is limited.
    pub most_streams: Option<i32>,
}

impl Rights {
    /// These rights laid over the ones an account holds.
    pub fn over(&self, held: &Permissions) -> Permissions {
        Permissions {
            is_administrator: self.is_administrator,
            sees_every_library: self.sees_every_library,
            allowed_libraries: self.libraries.clone(),
            may_delete: self.may_delete,
            may_delete_from_disk: self.may_delete_from_disk,
            max_sessions: self.most_streams,
            ..held.clone()
        }
    }
}

/// Gives an account the rights it is to have, all at once.
///
/// Takes effect on the next thing any device of theirs asks: the rights of
/// whoever is asking are read afresh with every request, so nothing already
/// open keeps what was taken away.
pub async fn set_rights(
    state: &AppState,
    acting: &User,
    target: UserId,
    rights: &Rights,
) -> std::result::Result<User, Trouble> {
    let before = account(state, target).await?;
    let settled = checked(state, &rights.over(&before.permissions)).await?;
    // An administrator stepping down from the administration would lose the
    // screen they are standing on with the next thing it asks.
    if settled.is_administrator != before.permissions.is_administrator {
        not_to_yourself(acting, target)?;
    }
    match state.database().set_permissions(target, &settled).await? {
        KeptAnAdministrator::Done => {}
        KeptAnAdministrator::NoSuchAccount => {
            return Err(Trouble::Failed(melyxar_core::Error::not_found("account").into()));
        }
        KeptAnAdministrator::WouldLeaveNone => {
            return Err(Trouble::Refused(Refused::LastAdministrator));
        }
    }
    // A film already playing asks nothing more of the library it is in: one
    // of a library just taken away would otherwise play to its end.
    if settled.sees_less_than(&before.permissions) {
        crate::watching::stop_everything_of(state, target);
    }
    tracing::info!(
        account = %before.name,
        by = %acting.name,
        administrator = settled.is_administrator,
        every_library = settled.sees_every_library,
        granted = settled.allowed_libraries.len(),
        "changed the rights of an account"
    );
    record(
        state,
        Event::RightsChanged {
            user: target,
            user_name: before.name.clone(),
            by: acting.name.clone(),
        },
    )
    .await;
    Ok(User {
        permissions: settled,
        ..before
    })
}

/// Whether this account is still an administrator, asked again by what stays
/// open for one: a live line outlives the request that opened it.
pub async fn still_an_administrator(state: &AppState, id: UserId) -> Result<bool> {
    Ok(state
        .database()
        .user(id)
        .await?
        .is_some_and(|user| user.permissions.is_administrator))
}

/// Gives another account the name the administration typed for it.
pub async fn rename_account(
    state: &AppState,
    target: UserId,
    wanted: &str,
) -> std::result::Result<User, Trouble> {
    let before = account(state, target).await?;
    rename(state, &before, wanted).await
}

/// Puts a password on another account from the administration, and signs
/// every device of theirs out.
///
/// Never on one's own account: from here no current password is asked for,
/// so a session left open on a borrowed machine would otherwise be enough to
/// take an administrator's account from them. Their own is changed from
/// their profile, current password first.
pub async fn put_a_password(
    state: &AppState,
    acting: &User,
    target: UserId,
    password: &str,
) -> std::result::Result<(), Trouble> {
    not_to_yourself(acting, target)?;
    let user = account(state, target).await?;
    password_put_on(state, &user, password).await
}

/// Signs every device of another account out.
pub async fn sign_out_everywhere(
    state: &AppState,
    acting: &User,
    target: UserId,
) -> std::result::Result<u64, Trouble> {
    not_to_yourself(acting, target)?;
    let user = account(state, target).await?;
    let closed = state.database().close_every_session_of(user.id).await?;
    crate::watching::stop_everything_of(state, user.id);
    tracing::warn!(account = %user.name, by = %acting.name, closed, "signed an account out of every device");
    record(
        state,
        Event::SignedOutEverywhere {
            user: user.id,
            user_name: user.name,
            by: acting.name.clone(),
        },
    )
    .await;
    Ok(closed)
}

pub use melyxar_database::sessions::SignedInDevice;

/// Every device signed in to this server, the most recently used first.
pub async fn every_device(state: &AppState) -> Result<Vec<SignedInDevice>> {
    Ok(state.database().signed_in_devices(None).await?)
}

/// The devices this account is signed in on, the most recently used first.
pub async fn devices_of(state: &AppState, who: &User) -> Result<Vec<SignedInDevice>> {
    Ok(state.database().signed_in_devices(Some(who.id)).await?)
}

/// Signs one device out from the administration, whoever's it is.
pub async fn sign_out_a_device(
    state: &AppState,
    acting: &User,
    device: melyxar_core::id::DeviceId,
) -> Result<()> {
    let found = every_device(state).await?.into_iter().find(|one| one.id == device);
    signed_out(state, acting, found).await
}

/// Signs one of one's own devices out: a phone lost, a computer lent.
///
/// A device of another account is one that is not there.
pub async fn sign_out_my_device(
    state: &AppState,
    who: &User,
    device: melyxar_core::id::DeviceId,
) -> Result<()> {
    let found = devices_of(state, who).await?.into_iter().find(|one| one.id == device);
    signed_out(state, who, found).await
}

/// Signs every one of one's own devices out but the one asking: the tidy
/// after a lost phone, or after sessions nobody holds any more.
///
/// Answers how many were signed out; nothing is written in the journal when
/// there were none.
pub async fn sign_out_my_other_devices(
    state: &AppState,
    who: &User,
    kept: melyxar_core::id::DeviceId,
) -> Result<usize> {
    let closed = state.database().close_other_devices(who.id, kept).await?;
    if closed.is_empty() {
        return Ok(0);
    }
    for device in &closed {
        crate::watching::stop(state, *device);
    }
    tracing::warn!(account = %who.name, closed = closed.len(), "signed out every other device");
    record(
        state,
        Event::OtherDevicesSignedOut {
            user: who.id,
            user_name: who.name.clone(),
            count: closed.len(),
        },
    )
    .await;
    Ok(closed.len())
}

/// Signs this device out, stops what it was playing, and says so in the
/// journal.
async fn signed_out(state: &AppState, acting: &User, found: Option<SignedInDevice>) -> Result<()> {
    let found = found.ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("device")))?;
    // Gone between the two reads is gone all the same.
    if state.database().close_device(found.id).await?.is_none() {
        return Ok(());
    }
    crate::watching::stop(state, found.id);
    tracing::warn!(account = %found.user_name, device = %found.name, by = %acting.name, "signed a device out");
    record(
        state,
        Event::DeviceSignedOut {
            user: found.user_id,
            user_name: found.user_name,
            device: found.name,
            browser: found.browser,
            by: acting.name.clone(),
        },
    )
    .await;
    Ok(())
}

/// Takes another account away from the administration.
pub async fn remove_account_by_id(
    state: &AppState,
    acting: &User,
    target: UserId,
) -> std::result::Result<(), Trouble> {
    not_to_yourself(acting, target)?;
    let user = account(state, target).await?;
    taken_away(state, user).await
}

/// Takes an account away, with everything of theirs, its picture included.
///
/// Refuses the last administrator. A server with nobody who may manage it
/// cannot be put right from any screen it serves, and nothing in it would
/// ever say why.
async fn taken_away(state: &AppState, user: User) -> std::result::Result<(), Trouble> {
    match state.database().delete_user(user.id).await? {
        KeptAnAdministrator::Done => {}
        KeptAnAdministrator::NoSuchAccount => {
            return Err(Trouble::Failed(melyxar_core::Error::not_found("account").into()));
        }
        KeptAnAdministrator::WouldLeaveNone => {
            return Err(Trouble::Refused(Refused::LastAdministrator));
        }
    }
    crate::watching::stop_everything_of(state, user.id);
    crate::avatars::forget_every_one_of(state, user.id).await;
    tracing::warn!(account = %user.name, "took an account away");
    record(state, Event::AccountRemoved { user_name: user.name }).await;
    Ok(())
}

/// Takes an account away from a terminal, by its name, and says whether
/// there was one by that name.
pub async fn remove_account(state: &AppState, name: &str) -> std::result::Result<bool, Trouble> {
    let Some((user, _)) = state.database().user_by_name(name).await? else {
        return Ok(false);
    };
    taken_away(state, user).await?;
    Ok(true)
}

/// Says which libraries an account may see, from a terminal, and whether
/// there was an account by that name. Every other right stays as it was.
pub async fn set_what_an_account_may_see(
    state: &AppState,
    name: &str,
    sees_every_library: bool,
    granted: &[melyxar_core::id::LibraryId],
) -> std::result::Result<bool, Trouble> {
    let Some((user, _)) = state.database().user_by_name(name).await? else {
        return Ok(false);
    };
    // An administrator sees every library whatever is written: said here
    // rather than done in silence, since a command that claims to have
    // limited an account and has not is the worst answer it could give.
    if user.permissions.is_administrator && !sees_every_library {
        return Err(Trouble::Failed(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Conflict,
            "an administrator sees every library",
        ))));
    }
    let wanted = Permissions {
        sees_every_library,
        allowed_libraries: granted.to_vec(),
        ..user.permissions.clone()
    };
    let settled = checked(state, &wanted).await?;
    state.database().set_permissions(user.id, &settled).await?;
    tracing::info!(
        account = %user.name,
        every = sees_every_library,
        granted = granted.len(),
        "said which libraries an account may see"
    );
    Ok(true)
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
    password_put_on(state, &user, password).await?;
    Ok(true)
}

/// Puts a password on an account without asking for the one before, and
/// signs every device of it out: whoever held it before is out.
async fn password_put_on(
    state: &AppState,
    user: &User,
    password: &str,
) -> std::result::Result<(), Trouble> {
    let hashed = stored_form_of(password)?;
    state.database().set_password(user.id, Some(&hashed)).await?;
    let closed = state.database().close_every_session_of(user.id).await?;
    crate::watching::stop_everything_of(state, user.id);
    tracing::warn!(
        account = %user.name,
        closed,
        "a password was put on an account and every device signed out"
    );
    record(
        state,
        Event::PasswordChanged {
            user: user.id,
            user_name: user.name.clone(),
        },
    )
    .await;
    Ok(())
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
                ..Default::default()
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
        match sign_in(state, name, password, device, Remembered::Yes, None).await.expect("asked") {
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
                sign_in(&state, "victor", "not the password", "a browser", Remembered::Yes, None)
                    .await
                    .expect("asked"),
                SignedInOrNot::NotAPair
            ));
        }

        // The right password is not even looked at while it is held back,
        // which is the point of holding it back: checking one is made
        // expensive on purpose, so a thousand guesses a second would be asking
        // this server to grind itself to a halt.
        let held = sign_in(&state, "victor", "quiet harbour", "a browser", Remembered::Yes, None)
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
            sign_in(&state, "victor", "not the password", "a browser", Remembered::Yes, None)
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
            sign_in(&state, "victor", "not the password", "a browser", Remembered::Yes, None)
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
            sign_in(&state, "victor", "not the password", "a browser", Remembered::Yes, None)
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
    async fn the_line_of_a_sign_in_names_the_browser_its_page_found_afterwards() {
        let (_directory, state) = a_server_with_an_account().await;
        let here = a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .expect("signed in");
        let holder = who_holds(&state, here.token.as_text())
            .await
            .expect("asked")
            .expect("signed in");
        name_the_browser(&state, holder.device, None, Some("Brave"))
            .await
            .expect("named");

        let lines = crate::activity::page(&state, &[crate::activity::Category::Access], None, 10)
            .await
            .expect("journal read");
        let line = lines
            .iter()
            .find(|line| line.kind == "signed_in")
            .expect("the sign in has its line");
        assert_eq!(line.details["browser"], "Brave");
    }

    #[tokio::test]
    async fn a_renamed_account_signs_in_under_its_new_name_and_stays_signed_in() {
        let (_directory, state) = a_server_with_an_account().await;
        let here = a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .expect("signed in");
        create_account(&state, "marc", "amber field road", &Permissions::viewer())
            .await
            .expect("second account made");

        assert!(matches!(
            rename(&state, &here.user, "   ").await,
            Err(Trouble::Refused(Refused::NameNeeded))
        ));
        assert!(matches!(
            rename(&state, &here.user, "Marc").await,
            Err(Trouble::Refused(Refused::NameTaken))
        ));

        let renamed = rename(&state, &here.user, "  Victor B  ").await.expect("renamed");
        assert_eq!(renamed.name, "Victor B", "kept without the spaces around it");
        assert_eq!(renamed.id, here.user.id);
        assert_eq!(
            who_holds(&state, here.token.as_text())
                .await
                .expect("asked")
                .expect("still signed in")
                .user
                .name,
            "Victor B"
        );
        assert!(a_session(&state, "victor b", "quiet harbour", "a browser").await.is_some());
        assert!(a_session(&state, "victor", "quiet harbour", "a browser").await.is_none());

        let lines = crate::activity::page(&state, &[crate::activity::Category::Access], None, 10)
            .await
            .expect("journal read");
        let line = lines
            .iter()
            .find(|line| line.kind == "account_renamed")
            .expect("the rename has its line");
        assert_eq!(line.details["previous_name"], "victor");
        assert_eq!(line.details["user_name"], "Victor B");
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
            Remembered::Yes,
            None,
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
            Remembered::Yes,
            None,
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
            change_password(
                &state,
                &here.user,
                "quiet harbour",
                "short",
                "a browser",
                Remembered::Yes,
                None,
            )
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

    /// The administrator of a fresh server and one ordinary account, each
    /// signed in on a device.
    async fn an_administrator_and_somebody() -> (tempfile::TempDir, AppState, User, OpenedSession) {
        let (directory, state) = a_server_with_an_account().await;
        create_account(&state, "zoe", "amber field road", &Permissions::viewer())
            .await
            .expect("account made");
        let admin = a_session(&state, "victor", "quiet harbour", "a laptop")
            .await
            .expect("signed in")
            .user;
        let zoe = a_session(&state, "zoe", "amber field road", "a phone")
            .await
            .expect("signed in");
        (directory, state, admin, zoe)
    }

    /// The rights the administration would send for these permissions.
    fn rights_of(permissions: &Permissions) -> Rights {
        Rights {
            is_administrator: permissions.is_administrator,
            sees_every_library: permissions.sees_every_library,
            libraries: permissions.allowed_libraries.clone(),
            may_delete: permissions.may_delete,
            may_delete_from_disk: permissions.may_delete_from_disk,
            most_streams: permissions.max_sessions,
        }
    }

    fn refused(trouble: Trouble) -> Refused {
        match trouble {
            Trouble::Refused(refused) => refused,
            Trouble::Failed(error) => panic!("failed rather than refused: {error}"),
        }
    }

    #[tokio::test]
    async fn rights_changed_by_an_administrator_hold_from_the_very_next_request() {
        // Read afresh with every request: a device already signed in keeps
        // nothing of what was taken away.
        let (directory, state, admin, zoe) = an_administrator_and_somebody().await;
        let films = state
            .database()
            .create_library(
                "Films",
                melyxar_core::library::LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), directory.path().join("films"))],
            )
            .await
            .expect("library created")
            .id;

        let wanted = Permissions {
            sees_every_library: false,
            allowed_libraries: vec![films],
            may_delete: true,
            max_sessions: Some(1),
            ..Permissions::viewer()
        };
        let changed = set_rights(&state, &admin, zoe.user.id, &rights_of(&wanted))
            .await
            .expect("changed");
        assert_eq!(changed.permissions, wanted);

        let holder = who_holds(&state, zoe.token.as_text())
            .await
            .expect("asked")
            .expect("still signed in");
        assert_eq!(holder.user.permissions, wanted);

        let written = state
            .database()
            .activity_page(&[], None, 5)
            .await
            .expect("read");
        assert_eq!(written[0].kind, "rights_changed");
        assert_eq!(written[0].user_id, Some(zoe.user.id));
    }

    #[tokio::test]
    async fn a_library_that_is_not_there_is_never_granted() {
        let (_directory, state, admin, zoe) = an_administrator_and_somebody().await;
        let wanted = Permissions {
            sees_every_library: false,
            allowed_libraries: vec![melyxar_core::id::LibraryId::new()],
            ..Permissions::viewer()
        };
        let trouble = set_rights(&state, &admin, zoe.user.id, &rights_of(&wanted))
            .await
            .expect_err("refused");
        assert_eq!(refused(trouble), Refused::NoSuchLibrary);
        let trouble = create_account(&state, "max", "amber field road", &wanted)
            .await
            .expect_err("refused");
        assert_eq!(refused(trouble), Refused::NoSuchLibrary);
    }

    #[tokio::test]
    async fn an_administrator_cannot_undo_their_own_account_from_the_administration() {
        let (_directory, state, admin, _) = an_administrator_and_somebody().await;
        // Another administrator, so that none of this is the last one.
        create_account(&state, "max", "amber field road", &Permissions::administrator())
            .await
            .expect("account made");

        let stepping_down = set_rights(&state, &admin, admin.id, &rights_of(&Permissions::viewer()))
            .await
            .expect_err("refused");
        assert_eq!(refused(stepping_down), Refused::NotYourself);
        let removing = remove_account_by_id(&state, &admin, admin.id)
            .await
            .expect_err("refused");
        assert_eq!(refused(removing), Refused::NotYourself);
        let signing_out = sign_out_everywhere(&state, &admin, admin.id)
            .await
            .expect_err("refused");
        assert_eq!(refused(signing_out), Refused::NotYourself);
        let password = put_a_password(&state, &admin, admin.id, "a whole new one")
            .await
            .expect_err("refused");
        assert_eq!(refused(password), Refused::NotYourself);

        assert!(
            state
                .database()
                .user(admin.id)
                .await
                .expect("read")
                .expect("still there")
                .permissions
                .is_administrator
        );
    }

    #[tokio::test]
    async fn the_last_administrator_stays_whoever_asks() {
        let (_directory, state, admin, zoe) = an_administrator_and_somebody().await;
        // Made an administrator, then asked to take the first one away: the
        // first is not the last any more, so that goes through.
        set_rights(
            &state,
            &admin,
            zoe.user.id,
            &rights_of(&Permissions::administrator()),
        )
            .await
            .expect("changed");
        let zoe_now = state
            .database()
            .user(zoe.user.id)
            .await
            .expect("read")
            .expect("there");
        remove_account_by_id(&state, &zoe_now, admin.id)
            .await
            .expect("taken away");

        // And from the terminal, where nobody is "yourself", the last one
        // is still kept.
        let trouble = remove_account(&state, "zoe").await.expect_err("refused");
        assert_eq!(refused(trouble), Refused::LastAdministrator);
    }

    #[tokio::test]
    async fn a_password_put_on_by_an_administrator_signs_the_account_out() {
        let (_directory, state, admin, zoe) = an_administrator_and_somebody().await;
        put_a_password(&state, &admin, zoe.user.id, "a whole new one")
            .await
            .expect("put");
        assert!(
            who_holds(&state, zoe.token.as_text())
                .await
                .expect("asked")
                .is_none(),
            "whoever held the account before is out"
        );
        assert!(a_session(&state, "zoe", "a whole new one", "a phone")
            .await
            .is_some());
    }

    #[tokio::test]
    async fn signing_an_account_out_of_everywhere_leaves_the_administrator_signed_in() {
        let (_directory, state, admin, zoe) = an_administrator_and_somebody().await;
        let admin_session = a_session(&state, "victor", "quiet harbour", "a desktop")
            .await
            .expect("signed in");
        assert_eq!(
            sign_out_everywhere(&state, &admin, zoe.user.id)
                .await
                .expect("done"),
            1
        );
        assert!(who_holds(&state, zoe.token.as_text())
            .await
            .expect("asked")
            .is_none());
        assert!(who_holds(&state, admin_session.token.as_text())
            .await
            .expect("asked")
            .is_some());
    }

    #[tokio::test]
    async fn signing_in_again_from_the_same_browser_leaves_one_device() {
        let (_directory, state) = a_server_with_an_account().await;
        for _ in 0..3 {
            let opened = sign_in(
                &state,
                "victor",
                "quiet harbour",
                "a browser",
                Remembered::Yes,
                Some("0f6c2c1e-3b7a-4c55-9e0a-5d1f7a9b2c44"),
            )
            .await
            .expect("asked");
            assert!(matches!(opened, SignedInOrNot::Opened(_)));
        }
        assert_eq!(every_device(&state).await.expect("read").len(), 1);
    }

    #[test]
    fn only_what_looks_like_an_identifier_names_a_browser() {
        assert_eq!(a_browser_identifier(Some("0f6c2c1e-3b7a")), Some("0f6c2c1e-3b7a"));
        for nonsense in ["", "a b", "x;DROP", &"a".repeat(65)] {
            assert_eq!(a_browser_identifier(Some(nonsense)), None, "{nonsense}");
        }
        assert_eq!(a_browser_identifier(None), None);
    }

    #[tokio::test]
    async fn one_device_is_signed_out_alone_by_its_owner_or_an_administrator() {
        let (_directory, state, admin, zoe) = an_administrator_and_somebody().await;
        let zoe_again = a_session(&state, "zoe", "amber field road", "a laptop")
            .await
            .expect("signed in");
        let mine = devices_of(&state, &zoe.user).await.expect("read");
        assert_eq!(mine.len(), 2);
        let phone = mine
            .iter()
            .find(|device| device.name == "a phone")
            .expect("the phone")
            .id;
        let laptop = mine
            .iter()
            .find(|device| device.name == "a laptop")
            .expect("the laptop")
            .id;

        // Somebody else's device is one that is not there.
        let admins = devices_of(&state, &admin).await.expect("read")[0].id;
        assert!(sign_out_my_device(&state, &zoe.user, admins).await.is_err());

        sign_out_my_device(&state, &zoe.user, phone)
            .await
            .expect("signed out");
        assert!(who_holds(&state, zoe.token.as_text())
            .await
            .expect("asked")
            .is_none());
        assert!(who_holds(&state, zoe_again.token.as_text())
            .await
            .expect("asked")
            .is_some());

        sign_out_a_device(&state, &admin, laptop)
            .await
            .expect("signed out");
        assert!(devices_of(&state, &zoe.user).await.expect("read").is_empty());

        let written = state
            .database()
            .activity_page(&[], None, 1)
            .await
            .expect("read");
        assert_eq!(written[0].kind, "device_signed_out");
        assert_eq!(written[0].device_name.as_deref(), Some("a laptop"));
    }

    /// The kind of the line written last in the journal.
    async fn last_line(state: &AppState) -> String {
        state
            .database()
            .activity_page(&[], None, 1)
            .await
            .expect("read")[0]
            .kind
            .clone()
    }

    #[tokio::test]
    async fn every_other_device_is_signed_out_and_the_one_asking_stays() {
        let (_directory, state, admin, zoe) = an_administrator_and_somebody().await;
        let laptop = a_session(&state, "zoe", "amber field road", "a laptop")
            .await
            .expect("signed in");
        let asking = devices_of(&state, &zoe.user)
            .await
            .expect("read")
            .into_iter()
            .find(|device| device.name == "a laptop")
            .expect("the laptop")
            .id;

        assert_eq!(
            sign_out_my_other_devices(&state, &zoe.user, asking)
                .await
                .expect("signed out"),
            1
        );
        assert!(who_holds(&state, zoe.token.as_text())
            .await
            .expect("asked")
            .is_none());
        assert!(who_holds(&state, laptop.token.as_text())
            .await
            .expect("asked")
            .is_some());
        assert_eq!(devices_of(&state, &admin).await.expect("read").len(), 1);
        assert_eq!(last_line(&state).await, "other_devices_signed_out");

        // Nothing left to sign out, and nothing written about it.
        assert_eq!(
            sign_out_my_other_devices(&state, &zoe.user, asking)
                .await
                .expect("asked"),
            0
        );
        assert_eq!(last_line(&state).await, "other_devices_signed_out");
    }

    #[tokio::test]
    async fn a_name_already_taken_is_refused_as_such() {
        let (_directory, state, admin, zoe) = an_administrator_and_somebody().await;
        let trouble = create_account(&state, "ZOE", "amber field road", &Permissions::viewer())
            .await
            .expect_err("refused");
        assert_eq!(refused(trouble), Refused::NameTaken);
        let trouble = rename_account(&state, admin.id, "zoe")
            .await
            .expect_err("refused");
        assert_eq!(refused(trouble), Refused::NameTaken);
        let listed = every_account(&state).await.expect("listed");
        assert_eq!(
            listed
                .iter()
                .map(|one| (one.user.name.as_str(), one.devices.map(|d| d.signed_in)))
                .collect::<Vec<_>>(),
            vec![("victor", Some(1)), ("zoe", Some(1))],
            "each with the devices it is signed in on"
        );
        let _ = zoe;
    }
}
