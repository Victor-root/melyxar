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

use std::net::IpAddr;

use melyxar_auth::{fingerprint_of, AuthError};
use melyxar_core::id::UserId;
use melyxar_core::time::{now, Timestamp};
use melyxar_core::user::{Permissions, User};

use melyxar_database::users::KeptAnAdministrator;

use crate::activity::{record, Event};
use crate::wrong_answers::{Origin, Turn};
use crate::{AppError, AppState, Result};

/// The longest name an account may be given, in characters.
///
/// Room for anybody's first and last names. A name is shown to everybody, at
/// the door, in every list and in the journal, and without a bound an account
/// could call itself two megabytes that the door then sent to every visitor.
/// A name typed at the door is written down cut to the same length: no
/// account answers to more.
pub const LONGEST_NAME: usize = 64;

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
    NameTooLong,
    NameTaken,
    /// Reads the same as another account's name, though it is written
    /// otherwise: a letter of another alphabet, or one nobody can see.
    NameLooksTaken,
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
            Self::NameTooLong => "name_too_long",
            Self::NameTaken => "name_taken",
            Self::NameLooksTaken => "name_looks_taken",
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

/// A name as it is kept: without the spaces around it, never empty and
/// never longer than [`LONGEST_NAME`].
fn name_of(asked: &str) -> std::result::Result<&str, Trouble> {
    let name = asked.trim();
    if how_it_reads(name).trim().is_empty() {
        return Err(Trouble::Refused(Refused::NameNeeded));
    }
    if name.chars().count() > LONGEST_NAME {
        return Err(Trouble::Refused(Refused::NameTooLong));
    }
    Ok(name)
}

/// A name as it reads on a screen, for two of them to be told apart: what
/// cannot be seen left out, every letter that is drawn like another written
/// as that other one (the skeleton of the Unicode rules on confusable text),
/// and in small letters. Two names that read the same here are the same name
/// to whoever picks one at the door.
///
/// Drawn like another before the capitals go and again after: a capital I is
/// drawn like a small l, which it no longer is once made small.
fn how_it_reads(name: &str) -> String {
    use unicode_security::confusable_detection::skeleton;
    use unicode_security::general_security_profile::IdentifierType;
    use unicode_security::GeneralSecurityProfile;
    let seen: String = name
        .chars()
        .filter(|letter| letter.identifier_type() != Some(IdentifierType::Default_Ignorable))
        .collect();
    let small: String = skeleton(&seen).flat_map(char::to_lowercase).collect();
    skeleton(&small).collect()
}

/// Refuses a name another account's reads like, though it is written
/// otherwise; the very same name is left to the database to refuse, which
/// it does whoever asks at the same moment.
async fn not_read_like_another(
    state: &AppState,
    name: &str,
    except: Option<UserId>,
) -> std::result::Result<(), Trouble> {
    let reads = how_it_reads(name);
    let others = state.database().list_users().await?;
    let like = others
        .iter()
        .filter(|other| Some(other.id) != except)
        .find(|other| how_it_reads(&other.name) == reads);
    match like {
        Some(other) if other.name.to_lowercase() == name.to_lowercase() => {
            Err(Trouble::Refused(Refused::NameTaken))
        }
        Some(_) => Err(Trouble::Refused(Refused::NameLooksTaken)),
        None => Ok(()),
    }
}

/// Hashes a password, telling a rule it breaks apart from a failure.
async fn stored_form_of(state: &AppState, password: &str) -> std::result::Result<String, Trouble> {
    match state.passwords().hash(password).await {
        Ok(hashed) => Ok(hashed),
        Err(AuthError::PasswordTooShort) => Err(Trouble::Refused(Refused::PasswordTooShort)),
        Err(other) => Err(Trouble::Failed(AppError::Auth(other))),
    }
}

/// How many tries in a row without the right password an address may be set
/// to take before it is held back.
///
/// Ten by default, which is more mistakes than anybody makes typing their
/// own password and far fewer than guessing one needs. Never fewer than three,
/// which would shut out whoever mistypes twice, nor more than a hundred, past
/// which the wait no longer slows any guessing.
pub const SIGN_IN_TRIES: std::ops::RangeInclusive<i64> = 3..=100;

/// How many tries in a row an address takes before it is held back.
pub async fn sign_in_tries(state: &AppState) -> Result<i64> {
    Ok(state.database().sign_in_tries().await?)
}

/// Sets it, refusing what is out of bounds.
pub async fn set_sign_in_tries(state: &AppState, tries: i64) -> Result<i64> {
    if !SIGN_IN_TRIES.contains(&tries) {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "an account takes between three and a hundred wrong passwords",
        )));
    }
    state.database().set_sign_in_tries(tries).await?;
    Ok(tries)
}

/// What a try from here is allowed, before its password is looked at.
///
/// An address already held back is told so without even the setting being
/// read: while it is held back, it costs this server nothing.
async fn turn_from(state: &AppState, from: Origin, at: Timestamp) -> Result<Turn> {
    if let Some(left) = state.wrong_answers().held_back(from, at) {
        return Ok(Turn::HeldBack(left));
    }
    let allowed = u32::try_from(state.database().sign_in_tries().await?).unwrap_or(u32::MAX);
    Ok(state.wrong_answers().take_a_turn(from, allowed, at))
}

/// A wait as somebody is told it: in whole seconds, and never none.
fn seconds_of(left: time::Duration) -> i64 {
    left.whole_seconds().max(1)
}

/// Writes down that an address has just been held back, once for the whole
/// hold rather than once for every try it then refuses.
async fn held_back_from_now(state: &AppState, name: &str, device: &str, address: Option<IpAddr>) {
    tracing::warn!(?address, "held back an address after too many wrong passwords");
    record(
        state,
        Event::SignInHeldBack {
            name: name.chars().take(LONGEST_NAME).collect(),
            device: device.to_string(),
            address: address.map(|address| address.to_string()),
        },
    )
    .await;
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
    /// Too many tries in a row without the right password from where this
    /// one comes, so nothing from there is being checked for a while. Said
    /// out loud rather than answered as one more wrong password: somebody
    /// who has mistyped theirs ten times needs to be told to wait. It says
    /// nothing about the name typed, which is not what is held back.
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
    /// Too many wrong current passwords from where this comes, by the same
    /// brake as signing in: a session left open somewhere is not a way to
    /// guess the password behind it at leisure.
    HeldBack { seconds: i64 },
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
/// Every try counts against the address it comes from, whatever name it
/// types: see [`crate::wrong_answers`].
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
    address: Option<IpAddr>,
) -> Result<SignedInOrNot> {
    let from = Origin::of(address);
    // Asked before anything is checked: the checking is the expensive part,
    // and not doing it is the whole point of holding an address back.
    let holds_back = match turn_from(state, from, now()).await? {
        Turn::HeldBack(left) => return Ok(SignedInOrNot::HeldBack { seconds: seconds_of(left) }),
        Turn::Checked { holds_back } => holds_back,
    };

    let signing_in = match state.database().user_by_name(name).await? {
        Some((user, Some(stored))) => {
            let right = state.passwords().matches(password, &stored).await;
            if !right {
                tracing::info!(account = %user.name, "refused a sign in");
            }
            right.then_some(user)
        }
        // An account waiting for its first password is not an account anybody
        // can sign into. It is the state a server sits in before the wizard
        // has run. Checked all the same, for the answer to take as long.
        Some((_, None)) | None => {
            state.passwords().checked_against_nobody(password).await;
            None
        }
    };
    let Some(user) = signing_in else {
        record(
            state,
            Event::SignInRefused {
                name: name.chars().take(LONGEST_NAME).collect(),
                device: device_name.to_string(),
                address: address.map(|address| address.to_string()),
            },
        )
        .await;
        if holds_back {
            held_back_from_now(state, name, device_name, address).await;
        }
        return Ok(SignedInOrNot::NotAPair);
    };
    state.wrong_answers().forget(from);

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
#[allow(clippy::too_many_arguments)]
pub async fn change_password(
    state: &AppState,
    who: &User,
    current: &str,
    wanted: &str,
    device_name: &str,
    remembered: Remembered,
    client: Option<&str>,
    address: Option<IpAddr>,
) -> std::result::Result<PasswordChange, Trouble> {
    let from = Origin::of(address);
    let holds_back = match turn_from(state, from, now()).await? {
        Turn::HeldBack(left) => return Ok(PasswordChange::HeldBack { seconds: seconds_of(left) }),
        Turn::Checked { holds_back } => holds_back,
    };

    let stored = state
        .database()
        .user_by_name(&who.name)
        .await?
        .and_then(|(_, stored)| stored);
    // An account with no password yet is one the wizard has not finished, and
    // it is reached through the wizard rather than through here.
    let right = match stored {
        Some(stored) => state.passwords().matches(current, &stored).await,
        None => false,
    };
    if !right {
        if holds_back {
            held_back_from_now(state, &who.name, device_name, address).await;
        }
        return Ok(PasswordChange::NotTheCurrentOne);
    }
    state.wrong_answers().forget(from);

    // Hashed before anything is taken away, so a password the rule refuses
    // leaves the account exactly as it was rather than signed out of
    // everywhere with its old password still on it.
    let hashed = stored_form_of(state, wanted).await?;
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
    not_read_like_another(state, name, Some(who.id)).await?;
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

/// Whether the first steps of this server, taken in the browser once its
/// first account exists, are still ahead: the libraries it is to hold.
pub async fn first_steps_pending(state: &AppState) -> Result<bool> {
    Ok(state.database().first_steps_pending().await?)
}

/// Puts the first steps behind this server, for good. Whatever was left out
/// of them is done from the administration like anything else.
pub async fn finish_first_steps(state: &AppState) -> Result<()> {
    state.database().finish_first_steps().await?;
    tracing::info!("the first steps of this server are done");
    Ok(())
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
///
/// It reads the interface in the language chosen on its door, the browser's
/// unless another was picked there.
pub async fn create_the_first_account(
    state: &AppState,
    name: &str,
    password: &str,
    language: Option<&str>,
) -> std::result::Result<User, Trouble> {
    let name = name_of(name)?;
    let already_set_up = || {
        Trouble::Failed(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::Conflict,
            "this server has already been set up",
        )))
    };

    // Looked at before anything is hashed: this door answers anybody for as
    // long as the server runs, and hashing first handed whoever reached it
    // the slowest thing this server does, for free, with every request.
    if !still_to_be_set_up(state).await? {
        return Err(already_set_up());
    }
    let hashed = stored_form_of(state, password).await?;

    // Looked at again where the account is written, together with the
    // writing, so two people reaching a brand new server in the same breath
    // cannot both come through.
    let Some(mut user) = state.database().create_the_first_user(name, &hashed).await? else {
        return Err(already_set_up());
    };
    if let Some(language) = language.filter(|language| melyxar_core::user::is_an_interface_language(language)) {
        user.preferences.interface_language = language.to_string();
        state
            .database()
            .save_preferences(user.id, &user.preferences)
            .await?;
    }
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
    not_read_like_another(state, name, None).await?;
    let permissions = checked(state, permissions).await?;
    let hashed = stored_form_of(state, password).await?;
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
    pub may_download: bool,
    pub may_manage_collections: bool,
    pub may_edit_tags: bool,
    pub may_upload: bool,
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
            may_download: self.may_download,
            may_manage_collections: self.may_manage_collections,
            may_edit_tags: self.may_edit_tags,
            may_upload: self.may_upload,
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
    let hashed = stored_form_of(state, password).await?;
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

    /// How many wrong answers a server that was never set takes.
    const ALLOWED_TRIES: u32 = 10;

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
        match sign_in(state, name, password, device, Remembered::Yes, None, None).await.expect("asked") {
            SignedInOrNot::Opened(opened) => Some(*opened),
            SignedInOrNot::NotAPair => None,
            SignedInOrNot::HeldBack { seconds } => panic!("held back for {seconds} seconds"),
        }
    }

    async fn a_server_with_an_account() -> (tempfile::TempDir, AppState) {
        let (directory, state) = a_server().await;
        create_the_first_account(&state, "victor", "quiet harbour", None)
            .await
            .expect("first account created");
        (directory, state)
    }

    #[tokio::test]
    async fn a_brand_new_server_says_it_still_has_to_be_set_up() {
        let (_directory, state) = a_server().await;
        assert!(still_to_be_set_up(&state).await.expect("asked"));

        create_the_first_account(&state, "victor", "quiet harbour", None)
            .await
            .expect("first account created");
        assert!(!still_to_be_set_up(&state).await.expect("asked"));
    }

    #[tokio::test]
    async fn the_first_account_speaks_the_language_its_door_was_read_in() {
        let (_directory, state) = a_server().await;
        let user = create_the_first_account(&state, "someone", "quiet harbour", Some("fr"))
            .await
            .expect("first account created");
        let kept = state.database().user(user.id).await.expect("read").expect("there");
        assert_eq!(kept.preferences.interface_language, "fr");
    }

    #[tokio::test]
    async fn the_first_account_reads_the_language_chosen_on_its_door() {
        let (_directory, state) = a_server().await;
        let user = create_the_first_account(&state, "someone", "quiet harbour", Some("fr"))
            .await
            .expect("first account created");
        let kept = state.database().user(user.id).await.expect("read").expect("there");
        assert_eq!(kept.preferences.interface_language, "fr");
    }

    #[tokio::test]
    async fn the_first_account_is_an_administrator_and_is_named_by_whoever_makes_it() {
        let (_directory, state) = a_server().await;
        let user = create_the_first_account(&state, "victor", "quiet harbour", None)
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
        assert!(create_the_first_account(&state, "someone else", "another password", None)
            .await
            .is_err());
    }

    #[tokio::test]
    async fn a_server_already_set_up_refuses_before_hashing_anything() {
        // This door answers anybody for as long as the server runs, and
        // hashing is the slowest thing the server does. A password too short
        // to be hashed at all shows which came first: it is answered as a
        // server already set up, not as a password the rule refuses.
        let (_directory, state) = a_server_with_an_account().await;
        let refused = create_the_first_account(&state, "someone else", "short", None).await;
        assert!(
            matches!(
                &refused,
                Err(Trouble::Failed(AppError::Domain(error)))
                    if error.code == melyxar_core::error::ErrorCode::Conflict
            ),
            "{refused:?}"
        );
    }

    #[tokio::test]
    async fn an_account_cannot_be_made_without_a_name_or_with_too_short_a_password() {
        let (_directory, state) = a_server().await;
        assert!(create_the_first_account(&state, "   ", "quiet harbour", None)
            .await
            .is_err());
        assert!(create_the_first_account(&state, "victor", "short", None)
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

    /// Where the tries of these tests come from, and where somebody else's do.
    fn here() -> Option<IpAddr> {
        Some("203.0.113.9".parse().expect("an address"))
    }

    fn elsewhere() -> Option<IpAddr> {
        Some("198.51.100.4".parse().expect("an address"))
    }

    async fn a_try(state: &AppState, name: &str, password: &str, from: Option<IpAddr>) -> SignedInOrNot {
        sign_in(state, name, password, "a browser", Remembered::Yes, None, from)
            .await
            .expect("asked")
    }

    async fn the_kinds_in_the_journal(state: &AppState) -> Vec<String> {
        crate::activity::page(state, &[crate::activity::Category::Access], None, 100)
            .await
            .expect("journal read")
            .into_iter()
            .map(|line| line.kind)
            .collect()
    }

    #[tokio::test]
    async fn too_many_wrong_passwords_hold_their_address_back() {
        let (_directory, state) = a_server_with_an_account().await;

        for _ in 0..ALLOWED_TRIES {
            assert!(matches!(
                a_try(&state, "victor", "not the password", here()).await,
                SignedInOrNot::NotAPair
            ));
        }

        // The right password is not even looked at while it is held back,
        // which is the point of holding it back: checking one is made
        // expensive on purpose, so a thousand guesses a second would be asking
        // this server to grind itself to a halt.
        let SignedInOrNot::HeldBack { seconds } = a_try(&state, "victor", "quiet harbour", here()).await else {
            panic!("ten wrong answers in a row have to hold their address back");
        };
        assert!(seconds > 0, "it has to say how long to wait: {seconds}");
    }

    #[tokio::test]
    async fn whoever_guesses_at_a_name_does_not_shut_its_owner_out() {
        // Counted per account, ten wrong passwords a minute at a name offered
        // at the door kept its owner, the administrator included, from ever
        // signing in again.
        let (_directory, state) = a_server_with_an_account().await;
        for _ in 0..ALLOWED_TRIES {
            a_try(&state, "victor", "not the password", here()).await;
        }
        assert!(matches!(
            a_try(&state, "victor", "quiet harbour", elsewhere()).await,
            SignedInOrNot::Opened(_)
        ));
    }

    #[tokio::test]
    async fn names_nobody_has_count_like_any_other_wrong_answer() {
        // Otherwise inventing a new name for each guess would never be held
        // back, and being held back would say which names exist.
        let (_directory, state) = a_server_with_an_account().await;
        for attempt in 0..ALLOWED_TRIES {
            a_try(&state, &format!("nobody {attempt}"), "quiet harbour", here()).await;
        }
        assert!(matches!(
            a_try(&state, "victor", "quiet harbour", here()).await,
            SignedInOrNot::HeldBack { .. }
        ));
    }

    #[tokio::test]
    async fn getting_it_right_clears_what_was_held_against_an_address() {
        let (_directory, state) = a_server_with_an_account().await;

        for _ in 0..ALLOWED_TRIES - 1 {
            a_try(&state, "victor", "not the password", here()).await;
        }
        assert!(matches!(
            a_try(&state, "victor", "quiet harbour", here()).await,
            SignedInOrNot::Opened(_)
        ));

        // Back to nothing, so the next mistake is the first one again and
        // somebody who mistypes their password now and then is never locked
        // out by a week of them.
        for _ in 0..ALLOWED_TRIES - 1 {
            a_try(&state, "victor", "not the password", here()).await;
        }
        assert!(matches!(
            a_try(&state, "victor", "quiet harbour", here()).await,
            SignedInOrNot::Opened(_)
        ));
    }

    #[tokio::test]
    async fn an_address_is_held_back_after_the_tries_the_administrator_set() {
        let (_directory, state) = a_server_with_an_account().await;
        assert!(set_sign_in_tries(&state, 2).await.is_err(), "fewer than three shuts out a typo");
        set_sign_in_tries(&state, 3).await.expect("set");

        for _ in 0..3 {
            a_try(&state, "victor", "not the password", here()).await;
        }
        assert!(matches!(
            a_try(&state, "victor", "quiet harbour", here()).await,
            SignedInOrNot::HeldBack { .. }
        ));
    }

    #[tokio::test]
    async fn a_hold_is_written_down_once_rather_than_for_every_try_it_refuses() {
        // Somebody hammering at the door while held back would otherwise
        // fill the journal, and the one connection that writes to the
        // database, as fast as they can send.
        let (_directory, state) = a_server_with_an_account().await;
        for _ in 0..ALLOWED_TRIES + 20 {
            a_try(&state, "victor", "not the password", here()).await;
        }

        let kinds = the_kinds_in_the_journal(&state).await;
        let counted = |kind: &str| kinds.iter().filter(|one| *one == kind).count();
        assert_eq!(counted("sign_in_refused"), ALLOWED_TRIES as usize);
        assert_eq!(counted("sign_in_held_back"), 1);
    }

    #[tokio::test]
    async fn the_current_password_asked_for_a_change_is_braked_like_signing_in() {
        // A session left open on a borrowed machine is not a way to guess the
        // password behind it at leisure, and then take the account away from
        // its owner with it.
        let (_directory, state) = a_server_with_an_account().await;
        let opened = a_session(&state, "victor", "quiet harbour", "a browser")
            .await
            .expect("signed in");
        let change = |current: &'static str| {
            change_password(
                &state,
                &opened.user,
                current,
                "amber field road",
                "a browser",
                Remembered::Yes,
                None,
                here(),
            )
        };

        for _ in 0..ALLOWED_TRIES {
            assert!(matches!(
                change("not the current one").await.expect("asked"),
                PasswordChange::NotTheCurrentOne
            ));
        }
        assert!(matches!(
            change("quiet harbour").await.expect("asked"),
            PasswordChange::HeldBack { .. }
        ));
        assert!(
            a_session(&state, "victor", "quiet harbour", "a browser").await.is_some(),
            "nothing was changed while it was held back"
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
            rename(&state, &here.user, &"é".repeat(LONGEST_NAME + 1)).await,
            Err(Trouble::Refused(Refused::NameTooLong))
        ));
        assert!(
            rename(&state, &here.user, &"é".repeat(LONGEST_NAME)).await.is_ok(),
            "counted in characters, so a name in any language has the same room"
        );
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
            may_download: permissions.may_download,
            may_manage_collections: permissions.may_manage_collections,
            may_edit_tags: permissions.may_edit_tags,
            may_upload: permissions.may_upload,
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
                None,
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

    #[test]
    fn a_name_reads_the_same_whatever_alphabet_or_invisible_letter_it_is_written_with() {
        let reads_like = |one: &str, other: &str| how_it_reads(one) == how_it_reads(other);
        // A Cyrillic i, a Greek o, a capital I drawn like a small l.
        assert!(reads_like("Zoe", "Z\u{03BF}e"));
        assert!(reads_like("Amberfield", "Amberf\u{0456}eld"));
        assert!(reads_like("Bill", "BiII"));
        // A letter nobody sees, and the same name in capitals.
        assert!(reads_like("Zoe", "Zo\u{200B}e"));
        assert!(reads_like("Zoe", "ZOE"));
        // An accent is something everybody sees, and emoji stay welcome.
        assert!(!reads_like("Zoe", "Zoé"));
        assert!(!reads_like("Zoe", "Zoe 🎬"));
    }

    #[test]
    fn a_name_of_nothing_but_invisible_letters_is_no_name() {
        assert!(matches!(name_of("\u{200B}\u{2060}"), Err(Trouble::Refused(Refused::NameNeeded))));
        assert!(name_of("Zoe 🎬").is_ok());
    }

    #[tokio::test]
    async fn a_name_reading_like_another_account_s_is_refused() {
        let (_directory, state, admin, zoe) = an_administrator_and_somebody().await;
        for looks_like_zoe in ["z\u{03BF}e", "zo\u{200B}e"] {
            let trouble = create_account(&state, looks_like_zoe, "amber field road", &Permissions::viewer())
                .await
                .expect_err("refused");
            assert_eq!(refused(trouble), Refused::NameLooksTaken, "{looks_like_zoe:?}");
            let trouble = rename_account(&state, admin.id, looks_like_zoe)
                .await
                .expect_err("refused");
            assert_eq!(refused(trouble), Refused::NameLooksTaken, "{looks_like_zoe:?}");
        }
        // Its own name, written another way, is still its own.
        assert!(rename(&state, &zoe.user, "Zoe").await.is_ok());
        assert!(rename_account(&state, admin.id, "Zoé").await.is_ok(), "an accent reads otherwise");
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
