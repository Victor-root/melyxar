//! A device waiting at the door, let in by an account signed in elsewhere.
//!
//! The device asks for a code and is handed, besides it, a secret it alone
//! holds. It shows the code, as six digits and as a QR code leading to the
//! page that lets it in, and asks with its secret, every little while,
//! whether somebody did. An account signed in on a telephone or a computer
//! types the code, or follows the QR code, and says yes; the device's next
//! question is answered with a session of its own. The code only names the
//! request: whoever saw it on a screen across a room holds no session by it.
//!
//! Held in memory: a request lives ten minutes, and a server restarted
//! meanwhile only makes the device ask for another.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use melyxar_auth::{draw_link_code, fingerprint_of, SessionToken};
use melyxar_core::id::UserId;
use melyxar_core::user::User;

use crate::accounts::{open_a_session, OpenedSession, Remembered};
use crate::activity::{record, Event};
use crate::{AppError, AppState, Result};

/// How long a code lets a device in.
pub const LASTS: Duration = Duration::from_secs(10 * 60);

/// The most requests waiting at once. Asking for one needs no account, so
/// it is bounded rather than left to fill the memory.
const MOST_WAITING: usize = 200;

/// How many wrong codes an account may type before it is held back, for as
/// long as a code lasts: a million codes, of which a few are waiting, are
/// not to be walked through.
const WRONG_CODES: u32 = 5;

/// The longest address a QR code is made to lead to.
const LONGEST_ADDRESS: usize = 300;

/// The requests waiting, and the wrong codes each account typed lately.
#[derive(Default)]
pub struct Links {
    held: Mutex<Held>,
}

#[derive(Default)]
struct Held {
    /// By code.
    waiting: HashMap<String, Waiting>,
    /// How many wrong codes, since when.
    wrong: HashMap<UserId, (u32, Instant)>,
}

struct Waiting {
    secret: String,
    device_name: String,
    client: Option<String>,
    until: Instant,
    /// The account it opens on and who said so, once somebody did.
    let_in: Option<(UserId, String)>,
}

impl Links {
    fn held(&self) -> std::sync::MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

impl Held {
    fn forget_what_is_over(&mut self, now: Instant) {
        self.waiting.retain(|_, waiting| waiting.until > now);
        self.wrong.retain(|_, (_, since)| now.duration_since(*since) < LASTS);
    }

    /// The request this code names, counting a wrong one against the account
    /// typing it.
    fn named(&mut self, who: UserId, code: &str, now: Instant) -> Result<&mut Waiting> {
        self.forget_what_is_over(now);
        if self.wrong.get(&who).is_some_and(|(count, _)| *count >= WRONG_CODES) {
            return Err(held_back(self.wrong[&who].1, now));
        }
        if !self.waiting.contains_key(code) {
            let wrong = self.wrong.entry(who).or_insert((0, now));
            wrong.0 += 1;
            return Err(melyxar_core::Error::not_found("no device waits on that code").into());
        }
        Ok(self.waiting.get_mut(code).expect("just looked at"))
    }
}

fn held_back(since: Instant, now: Instant) -> AppError {
    let left = LASTS.saturating_sub(now.duration_since(since)).as_secs().max(1);
    melyxar_core::Error::new(
        melyxar_core::error::ErrorCode::TooManyAttempts,
        format!("too many wrong codes, {left} seconds to wait"),
    )
    .into()
}

/// What a device at the door is handed.
#[derive(Debug)]
pub struct Asked {
    pub code: String,
    /// What it asks with, and it alone.
    pub secret: SessionToken,
    pub lasts_seconds: u64,
    /// The QR code leading to the page that lets it in, a row of squares at a
    /// time, dark ones true.
    pub squares: Vec<Vec<bool>>,
}

/// A device at the door asks to be let in from elsewhere. The address is
/// the server as the device reaches it, which the QR code leads to.
pub fn ask(state: &AppState, device_name: &str, client: Option<&str>, address: &str) -> Result<Asked> {
    let address = address.trim_end_matches('/');
    let fit = address.len() <= LONGEST_ADDRESS
        && (address.starts_with("http://") || address.starts_with("https://"))
        && !address.chars().any(char::is_whitespace);
    if !fit {
        return Err(melyxar_core::Error::invalid_input("a device is reached at an http or https address").into());
    }
    let now = Instant::now();
    let mut held = state.links().held();
    held.forget_what_is_over(now);
    if held.waiting.len() >= MOST_WAITING {
        return Err(melyxar_core::Error::conflict("too many devices wait to be let in").into());
    }
    let code = loop {
        let code = draw_link_code()?;
        if !held.waiting.contains_key(&code) {
            break code;
        }
    };
    let secret = SessionToken::new()?;
    held.waiting.insert(
        code.clone(),
        Waiting {
            secret: secret.fingerprint(),
            device_name: device_name.to_string(),
            client: client.map(str::to_string),
            until: now + LASTS,
            let_in: None,
        },
    );
    drop(held);
    Ok(Asked {
        squares: squares_of(&format!("{address}/link?code={code}"))?,
        code,
        secret,
        lasts_seconds: LASTS.as_secs(),
    })
}

/// The QR code of these words, a row of squares at a time.
fn squares_of(words: &str) -> Result<Vec<Vec<bool>>> {
    let drawn = qrcode::QrCode::new(words.as_bytes())
        .map_err(|error| melyxar_core::Error::internal(format!("no QR code for the address: {error}")))?;
    let width = drawn.width();
    Ok(drawn
        .to_colors()
        .chunks(width)
        .map(|row| row.iter().map(|square| *square == qrcode::Color::Dark).collect())
        .collect())
}

/// Where a device's request stands.
#[derive(Debug)]
pub enum Standing {
    /// Nobody said yes yet.
    Waiting { seconds_left: u64 },
    /// Somebody did: the device's own session.
    Opened(Box<OpenedSession>),
    /// Over, or never was: the device asks for another.
    Gone,
}

/// Asked by the device, with its secret: whether it was let in.
pub async fn standing(state: &AppState, secret: &str) -> Result<Standing> {
    let now = Instant::now();
    let fingerprint = fingerprint_of(secret);
    let taken = {
        let mut held = state.links().held();
        held.forget_what_is_over(now);
        let Some(code) = held
            .waiting
            .iter()
            .find(|(_, waiting)| waiting.secret == fingerprint)
            .map(|(code, _)| code.clone())
        else {
            return Ok(Standing::Gone);
        };
        let waiting = &held.waiting[&code];
        if waiting.let_in.is_none() {
            return Ok(Standing::Waiting {
                seconds_left: waiting.until.saturating_duration_since(now).as_secs(),
            });
        }
        held.waiting.remove(&code).expect("just found")
    };
    let Some((account, by)) = taken.let_in else {
        return Ok(Standing::Gone);
    };
    let Some(user) = state.database().user(account).await? else {
        return Ok(Standing::Gone);
    };
    let user_name = user.name.clone();
    let opened = open_a_session(state, user, &taken.device_name, Remembered::Yes, taken.client.as_deref()).await?;
    record(
        state,
        Event::DeviceLinked {
            user: account,
            user_name,
            device: taken.device_name,
            by,
        },
    )
    .await;
    Ok(Standing::Opened(Box::new(opened)))
}

/// The device a code names, for the page that lets it in to say which.
pub fn device_named(state: &AppState, who: &User, code: &str) -> Result<String> {
    let mut held = state.links().held();
    Ok(held.named(who.id, code, Instant::now())?.device_name.clone())
}

/// Lets in the device this code names, on the account of whoever says so,
/// or, for an administrator, on the account they chose.
pub async fn let_in(state: &AppState, who: &User, code: &str, on: Option<UserId>) -> Result<()> {
    let account = match on {
        Some(other) if other != who.id => {
            if !who.permissions.is_administrator {
                return Err(melyxar_core::Error::forbidden("only an administrator opens another's profile").into());
            }
            if state.database().user(other).await?.is_none() {
                return Err(melyxar_core::Error::not_found("no such account").into());
            }
            other
        }
        _ => who.id,
    };
    let mut held = state.links().held();
    let waiting = held.named(who.id, code, Instant::now())?;
    waiting.let_in = Some((account, who.name.clone()));
    tracing::info!(by = %who.name, device = %waiting.device_name, "a device is let in by a code");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::user::Permissions;

    async fn a_server() -> (tempfile::TempDir, AppState, User, User) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = melyxar_config::Config {
            directories: melyxar_config::Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
                ..Default::default()
            },
            ..melyxar_config::Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let database = melyxar_database::Database::open_in_memory().await.expect("database opens");
        let keeper = database
            .create_user("keeper", None, &Permissions::administrator())
            .await
            .expect("account created");
        let viewer = database
            .create_user("viewer", None, &Permissions::viewer())
            .await
            .expect("account created");
        (directory, AppState::new(config, database, None, None), keeper, viewer)
    }

    const HERE: &str = "http://192.0.2.10:2100";

    #[tokio::test]
    async fn a_device_let_in_by_a_code_gets_its_own_session_once() {
        let (_directory, state, _, viewer) = a_server().await;
        let asked = ask(&state, "Melyxar", None, HERE).expect("asked");
        assert_eq!(asked.code.len(), 6);
        assert!(!asked.squares.is_empty() && asked.squares.iter().all(|row| row.len() == asked.squares.len()));

        let secret = asked.secret.as_text().to_string();
        assert!(matches!(standing(&state, &secret).await.expect("read"), Standing::Waiting { .. }));

        assert_eq!(device_named(&state, &viewer, &asked.code).expect("named"), "Melyxar");
        let_in(&state, &viewer, &asked.code, None).await.expect("let in");
        let Standing::Opened(opened) = standing(&state, &secret).await.expect("read") else {
            panic!("the device is let in");
        };
        assert_eq!(opened.user.id, viewer.id);
        let held = crate::accounts::who_holds(&state, opened.token.as_text()).await.expect("read");
        assert!(held.is_some_and(|holder| holder.user.id == viewer.id), "the session is a real one");

        assert!(matches!(standing(&state, &secret).await.expect("read"), Standing::Gone), "once only");
        assert!(standing(&state, "somebody-else").await.is_ok_and(|s| matches!(s, Standing::Gone)));
    }

    #[tokio::test]
    async fn only_an_administrator_opens_somebody_else_and_wrong_codes_are_held_back() {
        let (_directory, state, keeper, viewer) = a_server().await;
        let asked = ask(&state, "Melyxar", None, HERE).expect("asked");
        assert!(let_in(&state, &viewer, &asked.code, Some(keeper.id)).await.is_err());
        let_in(&state, &keeper, &asked.code, Some(viewer.id)).await.expect("let in");
        let Standing::Opened(opened) = standing(&state, asked.secret.as_text()).await.expect("read") else {
            panic!("the device is let in");
        };
        assert_eq!(opened.user.id, viewer.id);

        let other = ask(&state, "Melyxar", None, HERE).expect("asked");
        let wrong = if other.code == "000000" { "000001" } else { "000000" };
        for _ in 0..WRONG_CODES {
            assert!(device_named(&state, &viewer, wrong).is_err());
        }
        // Held back now, the right code included.
        let refused = device_named(&state, &viewer, &other.code).expect_err("held back");
        assert!(refused.to_string().contains("too many"), "{refused}");
    }

    #[tokio::test]
    async fn a_qr_code_only_ever_leads_to_a_web_address() {
        let (_directory, state, _, _) = a_server().await;
        for wrong in ["javascript:alert(1)", "192.0.2.10", "http://a b"] {
            assert!(ask(&state, "Melyxar", None, wrong).is_err(), "{wrong}");
        }
        assert!(ask(&state, "Melyxar", None, "https://media.example/").is_ok());
    }
}
