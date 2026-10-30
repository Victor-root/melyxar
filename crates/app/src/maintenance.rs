//! Maintenance: the server closed for a while to everybody but its
//! administrators, announced a few minutes before it takes hold.
//!
//! Kept in the database, so a server restarted in the middle of one stays in
//! it, and held in memory, since the gate asks about it for every request a
//! piece of a film included.

use std::sync::Arc;

use melyxar_core::time::{now, Timestamp};
use melyxar_core::user::User;
pub use melyxar_database::settings::Maintenance as Kept;
use serde::Serialize;
use tokio::sync::watch;

use crate::activity::Event;
use crate::{AppState, Result};

/// The delays it may be announced with, in minutes: none, or long enough to
/// finish a scene.
pub const DELAYS: [i64; 5] = [0, 1, 5, 10, 15];

/// The longest message, in characters: a few lines on a screen.
pub const LONGEST_MESSAGE: usize = 500;

/// Maintenance as the server holds it right now, one for the whole server.
#[derive(Clone)]
pub struct Held(Arc<watch::Sender<Kept>>);

impl Default for Held {
    fn default() -> Self {
        Self(Arc::new(watch::channel(Kept::default()).0))
    }
}

/// Where maintenance stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Off,
    /// Switched on, and taking hold soon: everything still works.
    Announced,
    /// Only an administrator is served.
    On,
}

/// Where maintenance stands at this instant: over once its end has come,
/// whether or not it has been switched off yet.
pub fn phase_of(kept: &Kept, at: Timestamp) -> Phase {
    if !kept.enabled || kept.until.is_some_and(|until| at >= until) {
        Phase::Off
    } else if kept.starts_at.is_some_and(|starts| at < starts) {
        Phase::Announced
    } else {
        Phase::On
    }
}

/// What every page and every television is told of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Seen {
    pub state: Phase,
    /// How long before it takes hold, while it is announced. Counted here
    /// rather than handed over as an hour, so a device whose clock is wrong
    /// still counts down right.
    pub starts_in_seconds: Option<i64>,
    /// When it ends by itself, when the administrator gave an end.
    #[serde(with = "time::serde::rfc3339::option")]
    pub until: Option<Timestamp>,
    pub message: Option<String>,
}

/// What is told of it at this instant.
pub fn seen_at(kept: &Kept, at: Timestamp) -> Seen {
    let state = phase_of(kept, at);
    Seen {
        state,
        starts_in_seconds: kept
            .starts_at
            .filter(|_| state == Phase::Announced)
            // Rounded up, so a page never shows nothing left while it is not
            // yet in force.
            .map(|starts| ((starts - at).whole_milliseconds() + 999).div_euclid(1000) as i64),
        until: kept.until.filter(|_| state != Phase::Off),
        message: kept.message.clone().filter(|_| state != Phase::Off),
    }
}

/// What is told of it now.
pub fn seen(state: &AppState) -> Seen {
    seen_at(&state.maintenance().0.borrow(), now())
}

/// Whether this request is turned away: maintenance is in force and whoever
/// asks, if anybody, is not an administrator.
pub fn shuts_out(state: &AppState, who: Option<&User>) -> bool {
    !who.is_some_and(|user| user.permissions.is_administrator)
        && phase_of(&state.maintenance().0.borrow(), now()) == Phase::On
}

/// How many seconds a client turned away is asked to wait before trying
/// again: until the end the administrator expects, when it is still to come.
pub fn retry_after_seconds(state: &AppState) -> Option<i64> {
    let until = state.maintenance().0.borrow().until?;
    let left = (until - now()).whole_seconds();
    (left > 0).then_some(left)
}

/// Moved on every time maintenance is switched, changed or takes hold, for
/// every live line.
pub fn follow(state: &AppState) -> watch::Receiver<Kept> {
    state.maintenance().0.subscribe()
}

/// Puts back what the database holds as the server comes up, and waits again
/// for the start of a maintenance that was only announced and for its end.
/// One whose end came while the server was down is switched off at once.
pub async fn bring_back(state: &AppState) -> Result<()> {
    let kept = state.database().maintenance().await?;
    if kept.enabled {
        tracing::info!(starts_at = ?kept.starts_at, until = ?kept.until, "the server comes up in maintenance");
    }
    let starts_at = kept.starts_at.filter(|_| kept.enabled);
    let until = kept.until.filter(|_| kept.enabled);
    state.maintenance().0.send_replace(kept);
    if let Some(starts_at) = starts_at {
        wait_for_the_start(state, starts_at);
    }
    if let Some(until) = until {
        wait_for_the_end(state, until);
    }
    Ok(())
}

/// What an administrator asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wanted {
    pub on: bool,
    /// Minutes before it takes hold, one of [`DELAYS`]. Only read as it is
    /// switched on: a change of message or end leaves the start where it is.
    pub delay_minutes: i64,
    /// When it switches itself off; never, without one.
    pub until: Option<Timestamp>,
    pub message: Option<String>,
}

fn refused(why: &str) -> crate::AppError {
    melyxar_core::Error::invalid_input(why).into()
}

/// What is kept for what was asked, or what is wrong with it.
fn kept_for(wanted: &Wanted, before: &Kept, at: Timestamp) -> Result<Kept> {
    if !wanted.on {
        return Ok(Kept::default());
    }
    if !DELAYS.contains(&wanted.delay_minutes) {
        return Err(refused("a maintenance is announced with one of the delays offered"));
    }
    let message = wanted
        .message
        .as_deref()
        .map(str::trim)
        .filter(|message| !message.is_empty());
    if message.is_some_and(|message| message.chars().count() > LONGEST_MESSAGE) {
        return Err(refused("a maintenance message this long would not fit on the screen"));
    }
    let starts_at = match before.starts_at.filter(|_| before.enabled) {
        Some(starts_at) => starts_at,
        None => at + time::Duration::minutes(wanted.delay_minutes),
    };
    if wanted.until.is_some_and(|until| until <= starts_at.max(at)) {
        return Err(refused("a maintenance ends after it starts"));
    }
    Ok(Kept {
        enabled: true,
        starts_at: Some(starts_at),
        until: wanted.until,
        message: message.map(str::to_string),
    })
}

/// Switches maintenance as an administrator asked, and tells every page.
pub async fn set(state: &AppState, by: &User, wanted: Wanted) -> Result<Seen> {
    let before = state.maintenance().0.borrow().clone();
    let kept = kept_for(&wanted, &before, now())?;
    state.database().set_maintenance(&kept).await?;
    if kept.enabled != before.enabled {
        let by = by.name.clone();
        state
            .journal()
            .write(if kept.enabled {
                Event::MaintenanceOn { by }
            } else {
                Event::MaintenanceOff { by }
            })
            .await;
    }
    let starts_at = kept.starts_at.filter(|_| kept.enabled && !before.enabled);
    let until = kept.until.filter(|_| kept.enabled && kept.until != before.until.filter(|_| before.enabled));
    state.maintenance().0.send_replace(kept);
    if let Some(starts_at) = starts_at {
        wait_for_the_start(state, starts_at);
    }
    if let Some(until) = until {
        wait_for_the_end(state, until);
    }
    Ok(seen(state))
}

/// Waits for the end the administrator gave, then switches maintenance off
/// and tells every page. Does nothing if it was switched off, or given
/// another end, meanwhile.
fn wait_for_the_end(state: &AppState, until: Timestamp) {
    let state = state.clone();
    tokio::spawn(async move {
        let left = (until - now()).max(time::Duration::ZERO);
        tokio::time::sleep(left.unsigned_abs()).await;
        let still = {
            let kept = state.maintenance().0.borrow();
            kept.enabled && kept.until == Some(until)
        };
        if !still {
            return;
        }
        if let Err(error) = state.database().set_maintenance(&Kept::default()).await {
            // Over all the same: the gate reads the end, and the next start
            // of the server switches it off.
            tracing::warn!(%error, "a maintenance over could not be switched off in the database");
        }
        tracing::info!("maintenance ended at the time it was given");
        state.journal().write(Event::MaintenanceEnded).await;
        state.maintenance().0.send_replace(Kept::default());
    });
}

/// Waits for an announced maintenance to take hold, then tells every page
/// and stops the films of those it shuts out. Does nothing if it was
/// switched off or switched again meanwhile.
fn wait_for_the_start(state: &AppState, starts_at: Timestamp) {
    let state = state.clone();
    tokio::spawn(async move {
        let left = (starts_at - now()).max(time::Duration::ZERO);
        tokio::time::sleep(left.unsigned_abs()).await;
        let still = {
            let kept = state.maintenance().0.borrow();
            kept.enabled && kept.starts_at == Some(starts_at)
        };
        if !still {
            return;
        }
        tracing::info!("maintenance takes hold");
        // Nothing changed but the hour: every line is told to look again.
        state.maintenance().0.send_modify(|_| {});
        stop_what_is_shut_out(&state).await;
    });
}

/// Stops every film watched by an account that is not an administrator, as
/// an administrator's stop does: the player is asked first, and what it
/// converted is closed if it does not obey.
async fn stop_what_is_shut_out(state: &AppState) {
    for watcher in crate::watching::watchers(state) {
        let administrator = match state.database().user(watcher).await {
            Ok(user) => user.is_some_and(|user| user.permissions.is_administrator),
            Err(error) => {
                tracing::warn!(%error, "an account watching could not be read, its films are stopped");
                false
            }
        };
        if !administrator {
            crate::watching::stop_everything_of(state, watcher);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::datetime;

    const NOON: Timestamp = datetime!(2026-10-03 12:00 UTC);

    fn asked(on: bool, delay_minutes: i64) -> Wanted {
        Wanted {
            on,
            delay_minutes,
            until: None,
            message: None,
        }
    }

    #[test]
    fn announced_until_it_starts_then_in_force() {
        let kept = kept_for(&asked(true, 5), &Kept::default(), NOON).expect("kept");
        assert_eq!(phase_of(&kept, NOON), Phase::Announced);
        assert_eq!(phase_of(&kept, NOON + time::Duration::minutes(5)), Phase::On);
        assert_eq!(phase_of(&Kept::default(), NOON), Phase::Off);

        let seen = seen_at(&kept, NOON + time::Duration::seconds(30));
        assert_eq!(seen.state, Phase::Announced);
        assert_eq!(seen.starts_in_seconds, Some(270));
        assert_eq!(seen_at(&kept, NOON + time::Duration::minutes(6)).starts_in_seconds, None);
    }

    #[test]
    fn without_a_delay_it_is_in_force_at_once() {
        let kept = kept_for(&asked(true, 0), &Kept::default(), NOON).expect("kept");
        assert_eq!(phase_of(&kept, NOON), Phase::On);
    }

    #[test]
    fn a_change_of_message_keeps_the_start_where_it_was() {
        let first = kept_for(&asked(true, 10), &Kept::default(), NOON).expect("kept");
        let later = NOON + time::Duration::minutes(3);
        let changed = Wanted {
            message: Some("  New disks  ".into()),
            ..asked(true, 0)
        };
        let kept = kept_for(&changed, &first, later).expect("kept");
        assert_eq!(kept.starts_at, first.starts_at);
        assert_eq!(kept.message.as_deref(), Some("New disks"));
    }

    #[test]
    fn switched_off_it_forgets_everything() {
        let on = Kept {
            enabled: true,
            starts_at: Some(NOON),
            until: Some(NOON + time::Duration::hours(1)),
            message: Some("Back soon".into()),
        };
        assert_eq!(kept_for(&asked(false, 0), &on, NOON).expect("kept"), Kept::default());
        let seen = seen_at(&Kept::default(), NOON);
        assert_eq!(seen.until, None);
        assert_eq!(seen.message, None);
    }

    #[test]
    fn over_once_its_end_has_come() {
        let ending = Wanted {
            until: Some(NOON + time::Duration::hours(1)),
            ..asked(true, 5)
        };
        let kept = kept_for(&ending, &Kept::default(), NOON).expect("kept");
        assert_eq!(phase_of(&kept, NOON + time::Duration::minutes(59)), Phase::On);
        assert_eq!(phase_of(&kept, NOON + time::Duration::hours(1)), Phase::Off);
        assert_eq!(seen_at(&kept, NOON + time::Duration::hours(2)).until, None);
    }

    #[test]
    fn what_cannot_be_kept_is_refused() {
        assert!(kept_for(&asked(true, 7), &Kept::default(), NOON).is_err());
        let past = Wanted {
            until: Some(NOON - time::Duration::minutes(1)),
            ..asked(true, 0)
        };
        assert!(kept_for(&past, &Kept::default(), NOON).is_err());
        let before_it_starts = Wanted {
            until: Some(NOON + time::Duration::minutes(3)),
            ..asked(true, 5)
        };
        assert!(kept_for(&before_it_starts, &Kept::default(), NOON).is_err());
        let long = Wanted {
            message: Some("x".repeat(LONGEST_MESSAGE + 1)),
            ..asked(true, 0)
        };
        assert!(kept_for(&long, &Kept::default(), NOON).is_err());
        // An empty message is no message rather than a refusal.
        let empty = Wanted {
            message: Some("   ".into()),
            ..asked(true, 0)
        };
        assert_eq!(kept_for(&empty, &Kept::default(), NOON).expect("kept").message, None);
    }

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
        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        let administrator = database
            .create_user("keeper", None, &melyxar_core::user::Permissions::administrator())
            .await
            .expect("account created");
        let viewer = database
            .create_user("viewer", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created");
        let state = AppState::new(config, database, None, None);
        (directory, state, administrator, viewer)
    }

    #[tokio::test]
    async fn switched_on_it_turns_away_all_but_an_administrator_and_survives_a_restart() {
        let (_directory, state, administrator, viewer) = a_server().await;
        assert!(!shuts_out(&state, Some(&viewer)));

        let announced = set(&state, &administrator, Wanted {
            message: Some("New disks".into()),
            ..asked(true, 5)
        })
        .await
        .expect("switched on");
        assert_eq!(announced.state, Phase::Announced);
        assert!(announced.starts_in_seconds.is_some_and(|left| left > 290 && left <= 300));
        assert!(!shuts_out(&state, Some(&viewer)), "announced, everything still works");

        let mut told = follow(&state);
        told.borrow_and_update();
        let in_force = set(&state, &administrator, asked(false, 0)).await.expect("off");
        assert_eq!(in_force.state, Phase::Off);
        assert!(told.has_changed().expect("still followed"), "every line is told");

        set(&state, &administrator, asked(true, 0)).await.expect("switched on");
        assert!(shuts_out(&state, Some(&viewer)));
        assert!(shuts_out(&state, None));
        assert!(!shuts_out(&state, Some(&administrator)));

        // A server restarted in the middle of it is still in it.
        let again = AppState::new(
            state.config().clone(),
            state.database().clone(),
            None,
            None,
        );
        assert!(!shuts_out(&again, Some(&viewer)));
        bring_back(&again).await.expect("brought back");
        assert!(shuts_out(&again, Some(&viewer)));

        let lines = state
            .database()
            .activity_page(&[], None, 10)
            .await
            .expect("journal read");
        assert!(lines.iter().any(|line| line.kind == "server_maintenance_on"));
        assert!(lines.iter().any(|line| line.kind == "server_maintenance_off"));
    }

    #[tokio::test]
    async fn it_switches_itself_off_at_the_end_it_was_given() {
        let (_directory, state, administrator, viewer) = a_server().await;
        let mut told = follow(&state);
        set(&state, &administrator, Wanted {
            until: Some(now() + time::Duration::milliseconds(300)),
            ..asked(true, 0)
        })
        .await
        .expect("switched on");
        assert!(shuts_out(&state, Some(&viewer)));
        told.borrow_and_update();

        tokio::time::sleep(std::time::Duration::from_millis(600)).await;
        assert!(!shuts_out(&state, Some(&viewer)));
        assert!(told.has_changed().expect("still followed"), "every line is told");
        assert!(!state.database().maintenance().await.expect("read").enabled);
        let lines = state.database().activity_page(&[], None, 10).await.expect("journal read");
        assert!(lines.iter().any(|line| line.kind == "server_maintenance_ended"));
    }
}
