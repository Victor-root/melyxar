//! The passkeys of an account: made from its profile, with its password, and
//! used at the door in place of a name and a password.
//!
//! Each use is a ceremony in two halves: the server hands a challenge to the
//! browser, the device signs it, and the browser brings the answer back. The
//! challenge is held here, in memory, between the two: never written down,
//! good for one answer only, and forgotten after five minutes. How many may
//! wait at once is bounded, the oldest going first, so that asking for
//! challenges without ever answering costs this server nothing but a few
//! of them.
//!
//! A passkey that signs in follows the rules of a password that signs in:
//! the same brake on the address a failed try comes from, the same account
//! kept out while it waits for an administrator, the same session opened.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::Instant;

use melyxar_auth::SessionToken;
use melyxar_auth::passkeys::{
    AuthenticationResult, CEREMONY_LASTS, DiscoverableAuthentication, Passkey, PasskeyRegistration,
    Site, Webauthn, checker_for, to_be_found,
};
pub use melyxar_auth::passkeys::{
    CreationChallengeResponse, PublicKeyCredential, RegisterPublicKeyCredential,
    RequestChallengeResponse,
};
use melyxar_core::id::{PasskeyId, UserId};
use melyxar_core::time::{Timestamp, now};
use melyxar_core::user::User;
use melyxar_database::passkeys::{NewPasskey, StoredPasskey};

use crate::accounts::{
    Confirmed, Remembered, SignedInOrNot, WayIn, open_a_session, password_confirmed, seconds_of,
    turn_from,
};
use crate::activity::{Event, record};
use crate::wrong_answers::{Origin, Turn};
use crate::{AppError, AppState, Result};

/// The most passkeys one account keeps.
pub const MOST_PASSKEYS: usize = 20;

/// The longest name a passkey may be given, in characters.
pub const LONGEST_PASSKEY_NAME: usize = 60;

/// How many ceremonies of each kind may wait for an answer at once.
const MOST_WAITING: usize = 256;

/// How many of them may come from one place: a few browsers of one home.
const MOST_WAITING_FROM_ONE_PLACE: usize = 8;

/// Why a passkey was not made, renamed or used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// The page is not in https under a domain name, where browsers make and
    /// offer passkeys.
    NotASite,
    /// The account already has as many as it may keep.
    TooMany,
    NameNeeded,
    NameTooLong,
    /// The current password given is not the account's.
    WrongPassword,
    /// The device's answer did not pass: too late, already used, made for
    /// another site, or not signed by the key it names.
    NotMade,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotASite => "not_a_site",
            Self::TooMany => "too_many",
            Self::NameNeeded => "name_needed",
            Self::NameTooLong => "name_too_long",
            Self::WrongPassword => "wrong_password",
            Self::NotMade => "not_made",
        }
    }
}

/// What can go wrong here: something to put right, a brake, or the server.
#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error("refused: {}", .0.as_str())]
    Refused(Refused),
    /// Too many wrong tries from where this comes, by the brake on passwords.
    #[error("held back for {seconds} seconds")]
    HeldBack { seconds: i64 },
    #[error(transparent)]
    Failed(#[from] AppError),
}

impl From<melyxar_database::DatabaseError> for Trouble {
    fn from(error: melyxar_database::DatabaseError) -> Self {
        Self::Failed(AppError::from(error))
    }
}

/// A passkey as its account sees it: never its key, only what tells it apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasskeyView {
    pub id: PasskeyId,
    pub name: String,
    /// The site it was made under, which is the only one it serves.
    pub site: String,
    pub created_at: Timestamp,
    pub used_at: Option<Timestamp>,
}

impl From<StoredPasskey> for PasskeyView {
    fn from(stored: StoredPasskey) -> Self {
        Self {
            id: stored.id,
            name: stored.name,
            site: stored.rp_id,
            created_at: stored.created_at,
            used_at: stored.used_at,
        }
    }
}

/// A challenge handed out, and the word it is brought back under.
#[derive(Debug)]
pub struct Challenge<T> {
    pub ceremony: String,
    pub options: T,
}

/// The ceremonies waiting for their answer.
#[derive(Default)]
pub struct Ceremonies {
    making: Mutex<HashMap<String, Waiting<Making>>>,
    signing_in: Mutex<HashMap<String, Waiting<SigningIn>>>,
}

struct Waiting<T> {
    since: Instant,
    from: Origin,
    what: T,
}

struct Making {
    user: UserId,
    site: Site,
    state: PasskeyRegistration,
}

struct SigningIn {
    site: Site,
    state: DiscoverableAuthentication,
}

/// Keeps a ceremony under a word drawn for it, the stale ones and, past the
/// bound, the oldest put away first.
fn keep<T>(waiting: &Mutex<HashMap<String, Waiting<T>>>, from: Origin, what: T) -> Result<String> {
    let word = SessionToken::new()?.as_text().to_string();
    let mut held = waiting
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    held.retain(|_, one| one.since.elapsed() < CEREMONY_LASTS);
    // One place asking again and again only ever puts its own away, so it
    // cannot push out what everybody else is answering.
    while held.values().filter(|one| one.from == from).count() >= MOST_WAITING_FROM_ONE_PLACE {
        put_the_oldest_away(&mut held, |one| one.from == from);
    }
    while held.len() >= MOST_WAITING {
        put_the_oldest_away(&mut held, |_| true);
    }
    held.insert(
        word.clone(),
        Waiting {
            since: Instant::now(),
            from,
            what,
        },
    );
    Ok(word)
}

fn put_the_oldest_away<T>(
    held: &mut HashMap<String, Waiting<T>>,
    among: impl Fn(&Waiting<T>) -> bool,
) {
    let oldest = held
        .iter()
        .filter(|(_, one)| among(one))
        .min_by_key(|(_, one)| one.since)
        .map(|(word, _)| word.clone());
    if let Some(oldest) = oldest {
        held.remove(&oldest);
    }
}

/// Takes a ceremony away for its one answer, if it is still waiting.
fn take<T>(waiting: &Mutex<HashMap<String, Waiting<T>>>, word: &str) -> Option<T> {
    let mut held = waiting
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    held.remove(word)
        .filter(|one| one.since.elapsed() < CEREMONY_LASTS)
        .map(|one| one.what)
}

async fn checker(state: &AppState, site: &Site) -> std::result::Result<Webauthn, Trouble> {
    let server_name = state.database().server_settings().await?.server_name;
    let applications: Vec<String> = state
        .config()
        .passkeys
        .applications
        .iter()
        .filter_map(|fingerprint| melyxar_auth::passkeys::application_origin(fingerprint))
        .collect();
    checker_for(site, &server_name, &applications).map_err(|error| {
        tracing::warn!(%error, site = %site.rp_id, "could not check passkeys for this site");
        Trouble::Refused(Refused::NotASite)
    })
}

fn site_of(origin: Option<&str>) -> std::result::Result<Site, Trouble> {
    origin
        .and_then(Site::of_origin)
        .ok_or(Trouble::Refused(Refused::NotASite))
}

fn name_of(name: &str) -> std::result::Result<String, Trouble> {
    let name = name.trim();
    if name.is_empty() {
        return Err(Trouble::Refused(Refused::NameNeeded));
    }
    if name.chars().count() > LONGEST_PASSKEY_NAME {
        return Err(Trouble::Refused(Refused::NameTooLong));
    }
    Ok(name.to_string())
}

/// The passkeys of an account, the oldest first.
pub async fn passkeys_of(state: &AppState, who: &User) -> Result<Vec<PasskeyView>> {
    Ok(state
        .database()
        .passkeys_of(who.id)
        .await?
        .into_iter()
        .map(PasskeyView::from)
        .collect())
}

/// Starts making a passkey for an account, once its current password says it
/// is its owner asking rather than whoever found a session left open.
pub async fn start_making(
    state: &AppState,
    who: &User,
    password: &str,
    origin: Option<&str>,
    device_name: &str,
    address: Option<IpAddr>,
) -> std::result::Result<Challenge<CreationChallengeResponse>, Trouble> {
    let site = site_of(origin)?;
    match password_confirmed(state, who, password, device_name, address).await? {
        Confirmed::Yes => {}
        Confirmed::No => return Err(Trouble::Refused(Refused::WrongPassword)),
        Confirmed::HeldBack { seconds } => return Err(Trouble::HeldBack { seconds }),
    }
    let kept = state.database().passkeys_of(who.id).await?;
    if kept.len() >= MOST_PASSKEYS {
        return Err(Trouble::Refused(Refused::TooMany));
    }
    // A device already holding one of this account's keys is not asked to
    // make a second one it would keep beside the first.
    let already = kept
        .iter()
        .filter(|key| key.rp_id == site.rp_id)
        .map(|key| key.credential_id.clone())
        .collect::<Vec<_>>();
    let checker = checker(state, &site).await?;
    let (options, ceremony) = checker
        .start_passkey_registration(*who.id.as_uuid(), &who.name, &who.name, Some(already))
        .map_err(|error| {
            tracing::warn!(%error, "could not start making a passkey");
            Trouble::Refused(Refused::NotMade)
        })?;
    let word = keep(
        &state.passkey_ceremonies().making,
        Origin::of(address),
        Making {
            user: who.id,
            site,
            state: ceremony,
        },
    )?;
    Ok(Challenge {
        ceremony: word,
        options: to_be_found(options),
    })
}

/// Finishes making a passkey from what the device answered, and keeps it
/// under the name the account gave it.
pub async fn finish_making(
    state: &AppState,
    who: &User,
    ceremony: &str,
    name: &str,
    answer: &RegisterPublicKeyCredential,
    origin: Option<&str>,
) -> std::result::Result<PasskeyView, Trouble> {
    let name = name_of(name)?;
    let site = site_of(origin)?;
    let making = take(&state.passkey_ceremonies().making, ceremony)
        .filter(|making| making.user == who.id && making.site == site)
        .ok_or(Trouble::Refused(Refused::NotMade))?;
    let checker = checker(state, &site).await?;
    let made = checker
        .finish_passkey_registration(answer, &making.state)
        .map_err(|error| {
            tracing::info!(%error, account = %who.name, "refused a passkey being made");
            Trouble::Refused(Refused::NotMade)
        })?;
    if state.database().passkeys_of(who.id).await?.len() >= MOST_PASSKEYS {
        return Err(Trouble::Refused(Refused::TooMany));
    }
    let credential = written(&made)?;
    let kept = state
        .database()
        .add_passkey(NewPasskey {
            user: who.id,
            credential_id: made.cred_id().as_ref(),
            rp_id: &site.rp_id,
            name: &name,
            credential: &credential,
            at: now(),
        })
        .await
        // The same key twice: the device already holds one, kept before.
        .map_err(|_| Trouble::Refused(Refused::NotMade))?;
    tracing::info!(account = %who.name, site = %site.rp_id, "made a passkey");
    record(
        state,
        Event::PasskeyAdded {
            user: who.id,
            user_name: who.name.clone(),
            name: name.clone(),
        },
    )
    .await;
    Ok(kept.into())
}

/// Renames one of an account's passkeys.
pub async fn rename(
    state: &AppState,
    who: &User,
    id: PasskeyId,
    name: &str,
) -> std::result::Result<bool, Trouble> {
    let name = name_of(name)?;
    Ok(state.database().rename_passkey(who.id, id, &name).await?)
}

/// Takes one of an account's passkeys away. Whether it was one of its own.
pub async fn remove(state: &AppState, who: &User, id: PasskeyId) -> Result<bool> {
    let Some(name) = state.database().remove_passkey(who.id, id).await? else {
        return Ok(false);
    };
    tracing::info!(account = %who.name, "took a passkey away");
    record(
        state,
        Event::PasskeyRemoved {
            user: who.id,
            user_name: who.name.clone(),
            name,
        },
    )
    .await;
    Ok(true)
}

/// Starts signing somebody in by a passkey, before anybody is named: the
/// browser offers the keys it holds for this site.
pub async fn start_signing_in(
    state: &AppState,
    origin: Option<&str>,
    address: Option<IpAddr>,
) -> std::result::Result<Challenge<RequestChallengeResponse>, Trouble> {
    let site = site_of(origin)?;
    // An address held back is not handed a challenge it could not use.
    if let Some(left) = state.wrong_answers().held_back(Origin::of(address), now()) {
        return Err(Trouble::HeldBack {
            seconds: seconds_of(left),
        });
    }
    let checker = checker(state, &site).await?;
    let (options, ceremony) = checker
        .start_discoverable_authentication()
        .map_err(|error| {
            tracing::warn!(%error, "could not start a passkey sign in");
            Trouble::Refused(Refused::NotMade)
        })?;
    let word = keep(
        &state.passkey_ceremonies().signing_in,
        Origin::of(address),
        SigningIn {
            site,
            state: ceremony,
        },
    )?;
    Ok(Challenge {
        ceremony: word,
        options,
    })
}

/// Finishes signing somebody in by the passkey their device answered with.
#[allow(clippy::too_many_arguments)]
pub async fn finish_signing_in(
    state: &AppState,
    ceremony: &str,
    answer: &PublicKeyCredential,
    origin: Option<&str>,
    device_name: &str,
    remembered: Remembered,
    client: Option<&str>,
    address: Option<IpAddr>,
) -> Result<SignedInOrNot> {
    let from = Origin::of(address);
    let holds_back = match turn_from(state, from, now()).await? {
        Turn::HeldBack(left) => {
            return Ok(SignedInOrNot::HeldBack {
                seconds: seconds_of(left),
            });
        }
        Turn::Checked { holds_back } => holds_back,
    };
    match checked_answer(state, ceremony, answer, origin).await? {
        Some(Answered {
            user,
            key,
            mut credential,
            used,
        }) => {
            // The counter is written whatever comes next: the device has
            // moved it on, and a copy of the key would be caught by it.
            credential.update_credential(&used);
            state
                .database()
                .passkey_used(key, &written(&credential)?, now())
                .await?;
            if user.awaiting_approval {
                return Ok(SignedInOrNot::AwaitingApproval);
            }
            state.wrong_answers().forget(from);
            let opened =
                open_a_session(state, user, device_name, remembered, client, WayIn::Passkey)
                    .await?;
            Ok(SignedInOrNot::Opened(Box::new(opened)))
        }
        None => {
            record(
                state,
                Event::SignInRefused {
                    name: String::new(),
                    device: device_name.to_string(),
                    address: address.map(|address| address.to_string()),
                    by_passkey: true,
                },
            )
            .await;
            if holds_back {
                crate::accounts::held_back_from_now(state, "", device_name, address).await;
            }
            Ok(SignedInOrNot::NotAPair)
        }
    }
}

/// An answer that passed: the account, its key and what the use came to.
struct Answered {
    user: User,
    key: PasskeyId,
    credential: Passkey,
    used: AuthenticationResult,
}

/// The answer, when it passes: its ceremony still waiting, for this very
/// site, naming a key this server keeps for the account the device names,
/// and signed by it.
async fn checked_answer(
    state: &AppState,
    ceremony: &str,
    answer: &PublicKeyCredential,
    origin: Option<&str>,
) -> Result<Option<Answered>> {
    let Some(site) = origin.and_then(Site::of_origin) else {
        return Ok(None);
    };
    let Some(signing_in) =
        take(&state.passkey_ceremonies().signing_in, ceremony).filter(|one| one.site == site)
    else {
        return Ok(None);
    };
    let Ok(checker) = checker(state, &site).await else {
        return Ok(None);
    };
    let Ok((account, credential_id)) = checker.identify_discoverable_authentication(answer) else {
        return Ok(None);
    };
    let Some(key) = state
        .database()
        .passkey_by_credential(credential_id)
        .await?
    else {
        return Ok(None);
    };
    if key.user_id.as_uuid() != &account || key.rp_id != site.rp_id {
        return Ok(None);
    }
    let credential = read_back(&key.credential)?;
    let used = match checker.finish_discoverable_authentication_v2(
        answer,
        signing_in.state,
        &credential,
    ) {
        Ok(used) => used,
        Err(error) => {
            tracing::info!(%error, "refused a passkey at the door");
            return Ok(None);
        }
    };
    let Some(user) = state.database().user(key.user_id).await? else {
        return Ok(None);
    };
    Ok(Some(Answered {
        user,
        key: key.id,
        credential,
        used,
    }))
}

/// A passkey as it is kept: its public key and counter, written by the
/// library.
fn written(credential: &Passkey) -> Result<String> {
    serde_json::to_string(credential).map_err(|error| {
        AppError::Domain(melyxar_core::Error::internal(format!(
            "a passkey could not be written: {error}"
        )))
    })
}

fn read_back(credential: &str) -> Result<Passkey> {
    serde_json::from_str(credential).map_err(|error| {
        AppError::Domain(melyxar_core::Error::internal(format!(
            "a passkey kept here could not be read back: {error}"
        )))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts::create_the_first_account;
    use crate::speech::tests::a_server;

    const SITE: Option<&str> = Some("https://melyxar.example.org");
    const FROM_AFAR: Option<IpAddr> = Some(IpAddr::V4(std::net::Ipv4Addr::new(203, 0, 113, 9)));

    async fn an_owner(state: &AppState) -> User {
        create_the_first_account(state, "victor", "quiet harbour", None)
            .await
            .expect("first account created")
    }

    /// An answer no device made: well formed, signed by nothing.
    fn a_forged_answer() -> PublicKeyCredential {
        serde_json::from_value(serde_json::json!({
            "id": "AAAA",
            "rawId": "AAAA",
            "response": {
                "authenticatorData": "AAAA",
                "clientDataJSON": "AAAA",
                "signature": "AAAA",
                "userHandle": "AAAA",
            },
            "type": "public-key",
        }))
        .expect("an answer")
    }

    #[test]
    fn a_ceremony_answers_once() {
        let waiting = Mutex::new(HashMap::new());
        let word = keep(&waiting, Origin::of(FROM_AFAR), 7).expect("kept");
        assert_eq!(word.len(), 64);
        assert_eq!(take(&waiting, &word), Some(7));
        assert_eq!(take(&waiting, &word), None);
        assert_eq!(take(&waiting, "anything else"), None);
    }

    #[test]
    fn the_oldest_ceremonies_make_room_past_the_bound() {
        let waiting = Mutex::new(HashMap::new());
        let first = keep(&waiting, Origin::of(FROM_AFAR), 0).expect("kept");
        for at in 1..=MOST_WAITING {
            keep(
                &waiting,
                Origin::of(Some(IpAddr::from([198, 51, 100, (at % 200) as u8]))),
                at,
            )
            .expect("kept");
        }
        assert_eq!(waiting.lock().expect("held").len(), MOST_WAITING);
        assert_eq!(take(&waiting, &first), None);
    }

    #[test]
    fn one_place_asking_again_and_again_only_puts_its_own_away() {
        let waiting = Mutex::new(HashMap::new());
        let elsewhere = Origin::of(Some(IpAddr::from([198, 51, 100, 7])));
        let theirs = keep(&waiting, elsewhere, 0).expect("kept");
        for at in 1..=MOST_WAITING {
            keep(&waiting, Origin::of(FROM_AFAR), at).expect("kept");
        }
        assert_eq!(
            waiting.lock().expect("held").len(),
            MOST_WAITING_FROM_ONE_PLACE + 1
        );
        assert_eq!(take(&waiting, &theirs), Some(0));
    }

    #[test]
    fn a_passkey_name_is_trimmed_and_bounded() {
        assert_eq!(name_of("  Phone ").ok().as_deref(), Some("Phone"));
        assert!(matches!(
            name_of("   "),
            Err(Trouble::Refused(Refused::NameNeeded))
        ));
        let long = "é".repeat(LONGEST_PASSKEY_NAME + 1);
        assert!(matches!(
            name_of(&long),
            Err(Trouble::Refused(Refused::NameTooLong))
        ));
        assert!(name_of(&"é".repeat(LONGEST_PASSKEY_NAME)).is_ok());
    }

    #[tokio::test]
    async fn a_passkey_is_only_made_under_a_site_and_with_the_password() {
        let (_directory, state) = a_server().await;
        let owner = an_owner(&state).await;
        for origin in [
            None,
            Some("http://melyxar.example.org"),
            Some("https://192.168.1.21:2100"),
        ] {
            let refused =
                start_making(&state, &owner, "quiet harbour", origin, "Brave", FROM_AFAR).await;
            assert!(
                matches!(refused, Err(Trouble::Refused(Refused::NotASite))),
                "{origin:?}"
            );
        }
        let refused = start_making(&state, &owner, "loud harbour", SITE, "Brave", FROM_AFAR).await;
        assert!(matches!(
            refused,
            Err(Trouble::Refused(Refused::WrongPassword))
        ));

        let started = start_making(&state, &owner, "quiet harbour", SITE, "Brave", FROM_AFAR)
            .await
            .expect("started");
        assert_eq!(started.options.public_key.rp.id, "melyxar.example.org");
        assert_eq!(started.options.public_key.user.name, "victor");
    }

    #[tokio::test]
    async fn a_ceremony_is_finished_by_its_account_on_its_site_only() {
        let (_directory, state) = a_server().await;
        let owner = an_owner(&state).await;
        let started = start_making(&state, &owner, "quiet harbour", SITE, "Brave", FROM_AFAR)
            .await
            .expect("started");
        let answer: RegisterPublicKeyCredential = serde_json::from_value(serde_json::json!({
            "id": "AAAA",
            "rawId": "AAAA",
            "response": { "attestationObject": "AAAA", "clientDataJSON": "AAAA" },
            "type": "public-key",
        }))
        .expect("an answer");
        let elsewhere = finish_making(
            &state,
            &owner,
            &started.ceremony,
            "Phone",
            &answer,
            Some("https://other.example.org"),
        )
        .await;
        assert!(matches!(elsewhere, Err(Trouble::Refused(Refused::NotMade))));
        // Taken by the try above: an answer brought twice is never checked twice.
        let again = finish_making(&state, &owner, &started.ceremony, "Phone", &answer, SITE).await;
        assert!(matches!(again, Err(Trouble::Refused(Refused::NotMade))));
        assert!(
            passkeys_of(&state, &owner)
                .await
                .expect("listed")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_forged_answer_at_the_door_counts_as_a_wrong_try() {
        let (_directory, state) = a_server().await;
        an_owner(&state).await;
        assert!(matches!(
            start_signing_in(&state, Some("http://melyxar.example.org"), FROM_AFAR).await,
            Err(Trouble::Refused(Refused::NotASite))
        ));
        let mut held_back = false;
        for _ in 0..20 {
            let started = match start_signing_in(&state, SITE, FROM_AFAR).await {
                Ok(started) => started,
                Err(Trouble::HeldBack { .. }) => {
                    held_back = true;
                    break;
                }
                Err(other) => panic!("{other}"),
            };
            let answered = finish_signing_in(
                &state,
                &started.ceremony,
                &a_forged_answer(),
                SITE,
                "Brave",
                Remembered::Yes,
                None,
                FROM_AFAR,
            )
            .await
            .expect("answered");
            match answered {
                SignedInOrNot::NotAPair => {}
                SignedInOrNot::HeldBack { .. } => {
                    held_back = true;
                    break;
                }
                other => panic!("{other:?}"),
            }
        }
        assert!(
            held_back,
            "the brake on passwords holds forged passkeys back too"
        );
    }
}
