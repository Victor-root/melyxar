//! Steering what plays on one device of an account from another of its
//! devices: pausing it, moving it, changing its tracks, or starting something
//! there (see the decision in docs/architecture/README.md).
//!
//! A device can be steered while its live line is open, the line every page
//! and the television already hold: an order travels down it, with no other
//! connection. Only the account's own devices, an administrator's included.

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use melyxar_core::id::{DeviceId, MediaSourceId, UserId, WorkId};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, watch};

use crate::watching::{Viewer, Watched};
use crate::{AppError, AppState, Result};

/// How many orders a line holds before it is read. A line that far behind is
/// one nobody reads; the remote pressed again is the answer to a lost order.
const WAITING_ORDERS: usize = 16;

/// The most steps a device's volume is said to go by: a system's are a few
/// dozen at most.
const MOST_VOLUME_STEPS: u32 = 1_000;

/// The most songs one order hands over: a queue, not a library.
const SONGS_HANDED_OVER: usize = 5_000;

/// What a device is asked to do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Order {
    Toggle,
    Pause,
    Resume,
    Stop,
    /// To this moment of what plays.
    Seek { seconds: f64 },
    /// Back or forward by this much.
    Step { seconds: f64 },
    Next,
    Previous,
    /// From nothing to everything.
    Volume { level: f64 },
    Muted { muted: bool },
    Audio { track: String },
    /// None takes the words away.
    Subtitle { track: Option<String> },
    Shuffle,
    Repeat,
    /// To this song of the queue.
    Jump { index: u32 },
    /// A film or an episode, from where it was left, in this copy of it
    /// when one is named.
    PlayWork {
        work: WorkId,
        source: Option<MediaSourceId>,
    },
    /// Songs as the pages hold them, from one of them.
    PlaySongs {
        songs: Vec<serde_json::Value>,
        at: u32,
        shuffle: bool,
        from_seconds: Option<f64>,
    },
}

impl Order {
    /// Whether it starts something rather than steering what plays: one
    /// player on the device takes it rather than every one.
    fn starts_something(&self) -> bool {
        matches!(self, Self::PlayWork { .. } | Self::PlaySongs { .. })
    }

    /// What is wrong with it, when something is.
    fn fault(&self) -> Option<&'static str> {
        let moment = |seconds: f64| seconds.is_finite() && seconds >= 0.0;
        match self {
            Self::Seek { seconds } if !moment(*seconds) => Some("a moment is a number of seconds from the start"),
            Self::Step { seconds } if !seconds.is_finite() => Some("a step is a number of seconds"),
            Self::Volume { level } if !(0.0..=1.0).contains(level) => Some("a volume is between nothing and everything"),
            Self::PlaySongs { songs, at, from_seconds, .. } => {
                if songs.is_empty() || songs.len() > SONGS_HANDED_OVER {
                    Some("songs to play are at least one and at most a queue")
                } else if *at as usize >= songs.len() {
                    Some("the song to start from is one of them")
                } else if from_seconds.is_some_and(|seconds| !moment(seconds)) {
                    Some("a moment is a number of seconds from the start")
                } else {
                    None
                }
            }
            _ => None,
        }
    }
}

/// What a device says of itself that the server cannot know otherwise: how
/// loud it plays, and the queue of its music.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Told {
    pub volume: Option<f64>,
    /// How many steps its volume goes by, when it goes by steps, as a
    /// television's does under its keys: a remote then moves it a step at
    /// a time.
    #[serde(default)]
    pub volume_steps: Option<u32>,
    pub muted: Option<bool>,
    pub queue: Option<ToldQueue>,
}

/// The queue of a device's music, as the pages hold it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToldQueue {
    pub songs: Vec<serde_json::Value>,
    /// The song playing, as a place in `songs`.
    pub at: u32,
    pub shuffle: bool,
    pub repeat: String,
}

impl Told {
    fn fault(&self) -> Option<&'static str> {
        if self.volume.is_some_and(|volume| !(0.0..=1.0).contains(&volume)) {
            return Some("a volume is between nothing and everything");
        }
        if self.volume_steps.is_some_and(|steps| steps == 0 || steps > MOST_VOLUME_STEPS) {
            return Some("a volume goes by a few steps");
        }
        match &self.queue {
            Some(queue) if queue.songs.len() > SONGS_HANDED_OVER => Some("a queue is at most so many songs"),
            Some(queue) if !queue.songs.is_empty() && queue.at as usize >= queue.songs.len() => {
                Some("the song playing is one of the queue")
            }
            _ => None,
        }
    }
}

/// The devices whose line is open, held for the whole server.
pub struct Reachable {
    held: Mutex<HashMap<DeviceId, Present>>,
    /// Numbers each line, the newest the highest.
    lines_opened: AtomicU64,
    /// Moved on whenever a device can be reached or no longer can.
    changed: watch::Sender<u64>,
}

impl Default for Reachable {
    fn default() -> Self {
        Self {
            held: Mutex::default(),
            lines_opened: AtomicU64::new(0),
            changed: watch::channel(0).0,
        }
    }
}

/// One device with its line open.
#[derive(Debug)]
struct Present {
    user: UserId,
    name: String,
    browser: Option<String>,
    /// Each line by its number, oldest first.
    lines: Vec<(u64, mpsc::Sender<Order>)>,
    told: Told,
}

/// Where the orders for one line arrive, for as long as it lives.
pub struct Line {
    pub orders: mpsc::Receiver<Order>,
    _held: Holding,
}

/// Counts the line as open until it ends.
struct Holding {
    state: AppState,
    device: DeviceId,
    number: u64,
}

impl Drop for Holding {
    fn drop(&mut self) {
        self.state.reachable().line_closed(self.device, self.number);
    }
}

impl Reachable {
    fn held(&self) -> std::sync::MutexGuard<'_, HashMap<DeviceId, Present>> {
        self.held.lock().unwrap_or_else(|held| held.into_inner())
    }

    fn tell(&self) {
        self.changed.send_modify(|count| *count = count.wrapping_add(1));
    }

    fn line_opened(&self, viewer: &Viewer) -> (u64, mpsc::Receiver<Order>) {
        let number = self.lines_opened.fetch_add(1, Ordering::Relaxed);
        let (sender, orders) = mpsc::channel(WAITING_ORDERS);
        let mut held = self.held();
        let fresh = !held.contains_key(&viewer.device);
        let present = held.entry(viewer.device).or_insert_with(|| Present {
            user: viewer.user,
            name: viewer.device_name.clone(),
            browser: None,
            lines: Vec::new(),
            told: Told::default(),
        });
        // A newer page knows the browser better than an older one did.
        if viewer.browser.is_some() {
            present.browser.clone_from(&viewer.browser);
        }
        present.lines.push((number, sender));
        drop(held);
        if fresh {
            self.tell();
        }
        (number, orders)
    }

    fn line_closed(&self, device: DeviceId, number: u64) {
        let mut held = self.held();
        let Some(present) = held.get_mut(&device) else {
            return;
        };
        present.lines.retain(|(one, _)| *one != number);
        let gone = present.lines.is_empty();
        if gone {
            held.remove(&device);
        }
        drop(held);
        if gone {
            self.tell();
        }
    }

    /// Hands the order to the device's lines: to the newest alone for one
    /// that starts something, to every one otherwise. Answers whether any
    /// took it.
    fn hand(&self, user: UserId, device: DeviceId, order: &Order) -> bool {
        let held = self.held();
        let Some(present) = held.get(&device).filter(|present| present.user == user) else {
            return false;
        };
        let lines = match order.starts_something() {
            true => &present.lines[present.lines.len().saturating_sub(1)..],
            false => &present.lines[..],
        };
        lines
            .iter()
            .fold(false, |taken, (_, line)| line.try_send(order.clone()).is_ok() || taken)
    }

    /// Keeps what a device says of itself. Answers whether it can be reached,
    /// which is the only time it is worth keeping.
    fn told(&self, device: DeviceId, told: Told) -> bool {
        let mut held = self.held();
        let Some(present) = held.get_mut(&device) else {
            return false;
        };
        let moved = present.told != told;
        present.told = told;
        drop(held);
        if moved {
            self.tell();
        }
        true
    }

    /// The devices of this account that can be reached, but this one.
    fn of(&self, user: UserId, except: DeviceId) -> Vec<Reached> {
        let mut found: Vec<_> = self
            .held()
            .iter()
            .filter(|(device, present)| present.user == user && **device != except)
            .map(|(device, present)| Reached {
                device: *device,
                name: present.name.clone(),
                browser: present.browser.clone(),
                told: present.told.clone(),
            })
            .collect();
        found.sort_by(|one, other| one.name.cmp(&other.name).then(one.device.cmp(&other.device)));
        found
    }
}

/// One device that can be reached, and what it said of itself.
struct Reached {
    device: DeviceId,
    name: String,
    browser: Option<String>,
    told: Told,
}

/// Opens the way for orders to this line of this device.
pub fn line(state: &AppState, viewer: &Viewer) -> Line {
    let (number, orders) = state.reachable().line_opened(viewer);
    Line {
        orders,
        _held: Holding {
            state: state.clone(),
            device: viewer.device,
            number,
        },
    }
}

/// What a remote waits on: moved on whenever one device can be reached or no
/// longer can.
pub fn changes(state: &AppState) -> watch::Receiver<u64> {
    state.reachable().changed.subscribe()
}

/// Asks one of this account's devices to do something. Refused for a device
/// that is not this account's or cannot be reached, and for an order that
/// makes no sense.
pub fn send(state: &AppState, user: UserId, device: DeviceId, order: &Order) -> Result<()> {
    if let Some(fault) = order.fault() {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(fault)));
    }
    if !state.reachable().hand(user, device, order) {
        return Err(AppError::Domain(melyxar_core::Error::not_found(
            "that device cannot be reached",
        )));
    }
    Ok(())
}

/// Keeps what this device says of itself, for the remotes steering it.
/// Refused for one that makes no sense, and for a device that cannot be
/// reached, which no remote shows.
pub fn tell(state: &AppState, device: DeviceId, told: Told) -> Result<()> {
    if let Some(fault) = told.fault() {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(fault)));
    }
    if !state.reachable().told(device, told) {
        return Err(AppError::Domain(melyxar_core::Error::not_found(
            "this device cannot be reached",
        )));
    }
    Ok(())
}

/// One device a remote can steer, and what plays there.
#[derive(Debug, Clone)]
pub struct Steerable {
    pub device: DeviceId,
    /// What the browser said it was when it signed in.
    pub name: String,
    /// The browser the page found, when it could tell better.
    pub browser: Option<String>,
    pub playing: Option<Watched>,
    pub told: Told,
}

/// The devices of this account a remote on `from` can steer.
pub async fn steerable(state: &AppState, user: UserId, from: DeviceId) -> Result<Vec<Steerable>> {
    let devices = state.reachable().of(user, from);
    let ids: Vec<DeviceId> = devices.iter().map(|reached| reached.device).collect();
    let mut playing = crate::watching::playing_on(state, &ids).await?;
    Ok(devices
        .into_iter()
        .map(|reached| {
            // The newest when there are two, which is a film left behind.
            let found = playing.iter().rposition(|watched| watched.device == reached.device);
            Steerable {
                device: reached.device,
                name: reached.name,
                browser: reached.browser,
                playing: found.map(|at| playing.swap_remove(at)),
                told: reached.told,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer(user: UserId, device: DeviceId) -> Viewer {
        Viewer {
            user,
            user_name: "Ada".into(),
            device,
            device_name: "Living room".into(),
            browser: None,
        }
    }

    fn read(orders: &mut mpsc::Receiver<Order>) -> Vec<Order> {
        let mut read = Vec::new();
        while let Ok(order) = orders.try_recv() {
            read.push(order);
        }
        read
    }

    fn a_film() -> Order {
        Order::PlayWork {
            work: WorkId::new(),
            source: None,
        }
    }

    #[test]
    fn an_order_reaches_every_line_of_the_device_and_a_start_only_the_newest() {
        let reachable = Reachable::default();
        let (user, device) = (UserId::new(), DeviceId::new());
        let (_, mut older) = reachable.line_opened(&viewer(user, device));
        let (_, mut newer) = reachable.line_opened(&viewer(user, device));

        assert!(reachable.hand(user, device, &Order::Pause));
        assert_eq!(read(&mut older), vec![Order::Pause]);
        assert_eq!(read(&mut newer), vec![Order::Pause]);

        let film = a_film();
        assert!(reachable.hand(user, device, &film));
        assert!(read(&mut older).is_empty());
        assert_eq!(read(&mut newer), vec![film]);
    }

    #[test]
    fn only_the_account_s_own_devices_are_reached_and_listed() {
        let reachable = Reachable::default();
        let (ada, bea) = (UserId::new(), UserId::new());
        let (hers, other) = (DeviceId::new(), DeviceId::new());
        let (_, mut orders) = reachable.line_opened(&viewer(bea, hers));
        let _here = reachable.line_opened(&viewer(ada, other));

        assert!(!reachable.hand(ada, hers, &Order::Pause), "another account's device");
        assert!(read(&mut orders).is_empty());
        assert!(reachable.of(ada, other).is_empty(), "the remote's own device is not listed");
        assert_eq!(reachable.of(bea, other).len(), 1);
    }

    #[test]
    fn a_device_whose_last_line_closed_can_no_longer_be_reached_and_says_so() {
        let reachable = Reachable::default();
        let (user, device) = (UserId::new(), DeviceId::new());
        let mut moved = reachable.changed.subscribe();
        let (first, _orders) = reachable.line_opened(&viewer(user, device));
        let (second, _more) = reachable.line_opened(&viewer(user, device));
        assert!(moved.has_changed().expect("open"), "a device just reached is told");
        moved.borrow_and_update();

        reachable.line_closed(device, first);
        assert!(!moved.has_changed().expect("open"), "still reached by its other line");
        reachable.line_closed(device, second);
        assert!(moved.has_changed().expect("open"));
        assert!(!reachable.hand(user, device, &Order::Pause));
        assert!(reachable.of(user, DeviceId::new()).is_empty());
    }

    #[test]
    fn what_a_device_says_of_itself_is_kept_while_it_can_be_reached_and_said_when_it_moves() {
        let reachable = Reachable::default();
        let (user, device) = (UserId::new(), DeviceId::new());
        let told = Told { volume: Some(0.4), volume_steps: Some(15), muted: Some(false), queue: None };
        assert!(!reachable.told(device, told.clone()), "a device with no line open is no one's");

        let (number, _orders) = reachable.line_opened(&viewer(user, device));
        let mut moved = reachable.changed.subscribe();
        moved.borrow_and_update();
        assert!(reachable.told(device, told.clone()));
        assert!(moved.has_changed().expect("open"));
        moved.borrow_and_update();
        assert!(reachable.told(device, told.clone()));
        assert!(!moved.has_changed().expect("open"), "the same again moves nothing");
        assert_eq!(reachable.of(user, DeviceId::new())[0].told, told);

        reachable.line_closed(device, number);
        assert!(!reachable.told(device, told), "gone with its last line");
    }

    #[test]
    fn what_a_device_says_of_itself_is_refused_when_it_makes_no_sense() {
        assert!(Told { volume: Some(2.0), ..Told::default() }.fault().is_some());
        assert!(Told { volume_steps: Some(0), ..Told::default() }.fault().is_some());
        assert_eq!(Told { volume: Some(0.4), volume_steps: Some(15), ..Told::default() }.fault(), None);
        let queue = |at| ToldQueue { songs: vec![serde_json::json!({})], at, shuffle: false, repeat: "off".into() };
        assert!(Told { queue: Some(queue(1)), ..Told::default() }.fault().is_some());
        assert_eq!(Told { queue: Some(queue(0)), ..Told::default() }.fault(), None);
    }

    #[test]
    fn an_order_that_makes_no_sense_is_refused_before_it_travels() {
        assert!(Order::Seek { seconds: -1.0 }.fault().is_some());
        assert!(Order::Seek { seconds: f64::NAN }.fault().is_some());
        assert!(Order::Volume { level: 1.5 }.fault().is_some());
        assert!(Order::PlaySongs { songs: Vec::new(), at: 0, shuffle: false, from_seconds: None }.fault().is_some());
        assert!(Order::PlaySongs { songs: vec![serde_json::json!({})], at: 1, shuffle: false, from_seconds: None }
            .fault()
            .is_some());
        assert_eq!(Order::Step { seconds: -10.0 }.fault(), None);
        assert_eq!(a_film().fault(), None);
    }

    #[test]
    fn an_order_is_written_the_way_pages_read_it() {
        let order: Order = serde_json::from_str(r#"{"do":"subtitle","track":null}"#).expect("read");
        assert_eq!(order, Order::Subtitle { track: None });
        assert_eq!(
            serde_json::to_value(Order::Seek { seconds: 12.5 }).expect("written"),
            serde_json::json!({"do": "seek", "seconds": 12.5})
        );
    }
}
