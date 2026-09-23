//! What is being watched right now, device by device.
//!
//! Held in memory and nowhere else: it is true for a minute at most, and a
//! film played as it lies on the disk leaves no other trace on the server
//! than what this hears. See the decisions on what is being watched.
//!
//! A watch is born when a player asks how to play a film, which is the one
//! moment every road through the player shares, plain file included. The
//! player says at once when it pauses, starts again or jumps, and where it is
//! every ten seconds; in between, the clock of a film that is not paused runs
//! on by itself. It lives while its player holds its live line open, which no
//! hidden tab slows down, and it ends when the player says it is leaving, a
//! short while after that line drops without a word, or after a long silence
//! from a player that never opened one.
//!
//! A film left is remembered for as long as a silent one would have been
//! kept: a player leaving sends its last word while an earlier one may still
//! be on its way, and that one arriving second must not bring the film back.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use melyxar_core::id::{DeviceId, SessionId, UserId, WorkId};
use melyxar_core::time::{Millis, Timestamp};
use melyxar_core::work::WorkKind;
use tokio::sync::watch;

use crate::playback::{PlayPlan, Producing};
use crate::{AppState, Result};

/// How long a player that never opened its line may go unheard before its
/// film is taken as stopped. It speaks every ten seconds, so this is several
/// missed in a row.
const SILENT_FOR: Duration = Duration::from_secs(40);

/// How long a film is kept once its player's line dropped without a word:
/// long enough for a line cut by the network to be tied again.
const REJOIN_WITHIN: Duration = Duration::from_secs(10);

/// How far a position may differ from where the clock alone would have put
/// it before it counts as a jump worth telling at once.
const JUMPED_BY: Millis = Millis::new(3_000);

/// How long a player is given to obey a stop before the server closes what
/// it can close itself.
const OBEY_WITHIN: Duration = Duration::from_secs(25);

/// Who is watching: the account, and the device it is watching on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Viewer {
    pub user: UserId,
    pub user_name: String,
    pub device: DeviceId,
    /// What the browser said it was when it signed in.
    pub device_name: String,
    /// The browser the page itself found, when it could tell better than
    /// the line above.
    pub browser: Option<String>,
}

/// One film playing on one device.
#[derive(Debug)]
struct Watch {
    viewer: Viewer,
    work: WorkId,
    /// What was decided for it. Absent for a film heard of before its plan
    /// was, which is a player carrying on across a restart of the server.
    plan: Option<Arc<PlayPlan>>,
    /// The session converting it, when one is.
    session: Option<SessionId>,
    /// Where the player last said it was, and when that was heard.
    position: Millis,
    position_at: Instant,
    paused: bool,
    started_at: Timestamp,
    heard_at: Instant,
    /// How many live lines its player holds open. While there is one, the
    /// film is being watched however long the player keeps quiet.
    lines: u32,
    /// How long it may go unheard with no line open.
    quiet_for: Duration,
    /// When an administrator asked for it to stop, when one did.
    stop_asked_at: Option<Instant>,
    /// What the live line waits on to be told to stop.
    stop: watch::Sender<bool>,
}

impl Watch {
    fn new(viewer: Viewer, work: WorkId, position: Millis, now: Instant, at: Timestamp) -> Self {
        Self {
            viewer,
            work,
            plan: None,
            session: None,
            position,
            position_at: now,
            paused: false,
            started_at: at,
            heard_at: now,
            lines: 0,
            quiet_for: SILENT_FOR,
            stop_asked_at: None,
            stop: watch::channel(false).0,
        }
    }

    /// Where the film has got to by now: where it was said to be, plus the
    /// time since when it is playing, never past its end.
    fn position_by(&self, now: Instant) -> Millis {
        if self.paused {
            return self.position;
        }
        let ran = i64::try_from(now.duration_since(self.position_at).as_millis()).unwrap_or(i64::MAX);
        let reached = Millis::new(self.position.get().saturating_add(ran));
        match self.plan.as_ref().and_then(|plan| plan.duration) {
            Some(length) if reached > length => length,
            _ => reached,
        }
    }

    fn alive(&self, now: Instant) -> bool {
        self.lines > 0 || now.duration_since(self.heard_at) < self.quiet_for
    }
}

/// What is being watched, held for the whole server.
#[derive(Debug)]
pub struct Watching {
    held: Mutex<Held>,
    /// Moved on whenever something an administrator sees changes, for the
    /// administration's live line to send the list again at once.
    changed: watch::Sender<u64>,
    /// Set once the server is stopping: every live line ends, or the server
    /// would wait for them for ever before it could stop.
    closing: watch::Sender<bool>,
}

impl Default for Watching {
    fn default() -> Self {
        Self {
            held: Mutex::default(),
            changed: watch::channel(0).0,
            closing: watch::channel(false).0,
        }
    }
}

#[derive(Debug, Default)]
struct Held {
    playing: HashMap<DeviceId, Watch>,
    /// The film each device last left, and when.
    left: HashMap<DeviceId, (WorkId, Instant)>,
}

impl Held {
    /// This device is done with this film: forgotten if it is the one
    /// playing there, and remembered as left either way.
    fn leave(&mut self, device: DeviceId, work: WorkId, now: Instant) -> Option<Watch> {
        self.left.insert(device, (work, now));
        let playing_it = self
            .playing
            .get(&device)
            .is_some_and(|watch| watch.work == work);
        playing_it.then(|| self.playing.remove(&device)).flatten()
    }

    /// Whether this device left this film a moment ago.
    fn just_left(&self, device: DeviceId, work: WorkId, now: Instant) -> bool {
        self.left.get(&device).is_some_and(|&(left, at)| {
            left == work && now.duration_since(at) < SILENT_FOR
        })
    }
}

/// One watch as it stood when asked about.
#[derive(Debug, Clone)]
struct Seen {
    viewer: Viewer,
    work: WorkId,
    plan: Option<Arc<PlayPlan>>,
    session: Option<SessionId>,
    position: Millis,
    started_at: Timestamp,
    paused: bool,
    stopping: bool,
}

/// What a live line waits on: the word to stop. Its end is the end of the
/// line, for whatever reason it ends.
pub struct Line {
    pub stop: watch::Receiver<bool>,
    /// Set when the server is stopping, which ends the line without a word:
    /// the player ties it again once the server is back.
    pub closing: watch::Receiver<bool>,
    _held: Holding,
}

/// Counts a line as open for as long as it lives.
struct Holding {
    state: AppState,
    device: DeviceId,
    work: WorkId,
}

impl Drop for Holding {
    fn drop(&mut self) {
        self.state
            .watching()
            .line_closed(self.device, self.work, Instant::now());
    }
}

impl Watching {
    fn held(&self) -> std::sync::MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(|held| held.into_inner())
    }

    fn tell(&self) {
        self.changed.send_modify(|count| *count = count.wrapping_add(1));
    }

    /// A player asked how to play a film: a watch starts, or the one already
    /// on this device carries on with a new plan when it is the same film,
    /// which is a track or a size changed partway through.
    fn starting(&self, viewer: Viewer, plan: Arc<PlayPlan>, now: Instant, at: Timestamp) {
        let mut held = self.held();
        held.left.remove(&viewer.device);
        let device = viewer.device;
        let work = plan.work_id;
        match held.playing.get_mut(&device) {
            Some(watch) if watch.work == work => {
                watch.viewer = viewer;
                watch.plan = Some(plan);
                watch.heard_at = now;
            }
            _ => {
                let mut watch = Watch::new(
                    viewer,
                    work,
                    plan.resume_from.unwrap_or(Millis::ZERO),
                    now,
                    at,
                );
                // Nothing plays until the picture does.
                watch.paused = true;
                watch.plan = Some(plan);
                held.playing.insert(device, watch);
            }
        }
        drop(held);
        self.tell();
    }

    fn session_opened(&self, device: DeviceId, session: SessionId, now: Instant) {
        if let Some(watch) = self.held().playing.get_mut(&device) {
            watch.session = Some(session);
            watch.heard_at = now;
        }
        self.tell();
    }

    /// A player said it still has a film open, where it is when it has got
    /// anywhere yet, and whether it is paused when it says. Answers whether
    /// it has been asked to stop.
    fn heard(
        &self,
        viewer: Viewer,
        work: WorkId,
        position: Option<Millis>,
        paused: Option<bool>,
        now: Instant,
        at: Timestamp,
    ) -> bool {
        let mut held = self.held();
        if held.just_left(viewer.device, work, now) {
            return false;
        }
        let (stop, changed) = match held.playing.get_mut(&viewer.device) {
            Some(watch) if watch.work == work => {
                let expected = watch.position_by(now);
                let mut changed = false;
                if let Some(position) = position {
                    changed |= position.get().abs_diff(expected.get()) > JUMPED_BY.get().unsigned_abs();
                    watch.position = position;
                } else {
                    watch.position = expected;
                }
                watch.position_at = now;
                if let Some(paused) = paused {
                    changed |= watch.paused != paused;
                    watch.paused = paused;
                }
                watch.heard_at = now;
                (watch.stop_asked_at.is_some(), changed)
            }
            _ => {
                let mut watch = Watch::new(viewer, work, position.unwrap_or(Millis::ZERO), now, at);
                watch.paused = paused.unwrap_or(false);
                held.playing.insert(watch.viewer.device, watch);
                (false, true)
            }
        };
        drop(held);
        if changed {
            self.tell();
        }
        stop
    }

    /// A player said it is leaving this film.
    fn gone(&self, device: DeviceId, work: WorkId, now: Instant) {
        let left = self.held().leave(device, work, now);
        if left.is_some() {
            self.tell();
        }
    }

    /// A player opened its live line for this film. Refused when its device
    /// is playing another film, which is a player left behind by a newer one.
    fn line_opened(&self, viewer: Viewer, work: WorkId, now: Instant, at: Timestamp) -> Option<watch::Receiver<bool>> {
        let mut held = self.held();
        if held.just_left(viewer.device, work, now) {
            return None;
        }
        let device = viewer.device;
        let fresh = !held.playing.contains_key(&device);
        let watch = held
            .playing
            .entry(device)
            .or_insert_with(|| Watch::new(viewer, work, Millis::ZERO, now, at));
        if watch.work != work {
            return None;
        }
        watch.lines += 1;
        watch.quiet_for = SILENT_FOR;
        watch.heard_at = now;
        let stop = watch.stop.subscribe();
        drop(held);
        if fresh {
            self.tell();
        }
        Some(stop)
    }

    /// A live line ended. The last one gone without a word, the film is kept
    /// only as long as a line cut by the network takes to be tied again.
    fn line_closed(&self, device: DeviceId, work: WorkId, now: Instant) {
        if let Some(watch) = self.held().playing.get_mut(&device).filter(|watch| watch.work == work) {
            watch.lines = watch.lines.saturating_sub(1);
            if watch.lines == 0 {
                watch.heard_at = now;
                watch.quiet_for = REJOIN_WITHIN;
            }
        }
    }

    /// Marks the film on this device to be stopped, tells its live line, and
    /// says whose it is and which it is, for closing it if the player never
    /// obeys.
    fn ask_to_stop(&self, device: DeviceId, now: Instant) -> Option<(UserId, WorkId)> {
        let mut held = self.held();
        let watch = held.playing.get_mut(&device)?;
        watch.stop_asked_at.get_or_insert(now);
        watch.stop.send_replace(true);
        let whose = (watch.viewer.user, watch.work);
        drop(held);
        self.tell();
        Some(whose)
    }

    /// Forgets this film on this device if it was asked to stop and is still
    /// there, answering what converted it.
    fn never_obeyed(&self, device: DeviceId, work: WorkId, now: Instant) -> Option<Option<SessionId>> {
        let mut held = self.held();
        let disobeyed = held
            .playing
            .get(&device)
            .is_some_and(|watch| watch.work == work && watch.stop_asked_at.is_some());
        let session = disobeyed
            .then(|| held.leave(device, work, now))
            .flatten()
            .map(|watch| watch.session);
        drop(held);
        if session.is_some() {
            self.tell();
        }
        session
    }

    /// Everything playing, oldest first, once the players gone are let go.
    fn now_playing(&self, now: Instant) -> Vec<Seen> {
        let mut held = self.held();
        let before = held.playing.len();
        held.playing.retain(|_, watch| watch.alive(now));
        let let_go = held.playing.len() != before;
        held.left
            .retain(|_, &mut (_, at)| now.duration_since(at) < SILENT_FOR);
        let mut seen: Vec<Seen> = held
            .playing
            .values()
            .map(|watch| Seen {
                viewer: watch.viewer.clone(),
                work: watch.work,
                plan: watch.plan.clone(),
                session: watch.session,
                position: watch.position_by(now),
                started_at: watch.started_at,
                paused: watch.paused,
                stopping: watch.stop_asked_at.is_some(),
            })
            .collect();
        drop(held);
        if let_go {
            self.tell();
        }
        seen.sort_by_key(|one| one.started_at);
        seen
    }
}

/// A player asked how to play a film, and was answered with this plan.
pub fn starting(state: &AppState, viewer: Viewer, plan: &PlayPlan) {
    state.watching().starting(
        viewer,
        Arc::new(plan.clone()),
        Instant::now(),
        melyxar_core::time::now(),
    );
}

/// The film on this device is now converted by this session.
pub fn session_opened(state: &AppState, device: DeviceId, session: SessionId) {
    state
        .watching()
        .session_opened(device, session, Instant::now());
}

/// A player said it still has a film open, where it is when it has got
/// anywhere yet, and whether it is paused when it says. Answers whether it
/// has been asked to stop.
pub fn heard(
    state: &AppState,
    viewer: Viewer,
    work: WorkId,
    position: Option<Millis>,
    paused: Option<bool>,
) -> bool {
    state.watching().heard(
        viewer,
        work,
        position,
        paused,
        Instant::now(),
        melyxar_core::time::now(),
    )
}

/// A player said it is leaving this film.
pub fn gone(state: &AppState, device: DeviceId, work: WorkId) {
    state.watching().gone(device, work, Instant::now());
}

/// A player opened its live line for this film. Absent when its device is
/// playing another film or has just left this one.
pub fn line(state: &AppState, viewer: Viewer, work: WorkId) -> Option<Line> {
    let device = viewer.device;
    let stop = state
        .watching()
        .line_opened(viewer, work, Instant::now(), melyxar_core::time::now())?;
    Some(Line {
        stop,
        closing: closing(state),
        _held: Holding {
            state: state.clone(),
            device,
            work,
        },
    })
}

/// What the administration's live line waits on: moved on whenever what it
/// shows changes.
pub fn changes(state: &AppState) -> watch::Receiver<u64> {
    state.watching().changed.subscribe()
}

/// Set when the server is stopping, for every live line to end.
pub fn closing(state: &AppState) -> watch::Receiver<bool> {
    state.watching().closing.subscribe()
}

/// Ends every live line, for the server to be able to stop.
pub fn close_every_line(state: &AppState) {
    state.watching().closing.send_replace(true);
}

/// Asks the player on this device to stop, and closes what the server can
/// close itself if it has not obeyed in time. Answers whether anything was
/// playing there.
pub fn stop(state: &AppState, device: DeviceId) -> bool {
    let Some((watcher, work)) = state.watching().ask_to_stop(device, Instant::now()) else {
        return false;
    };
    tracing::info!(device = %device, "an administrator asked a film to stop");
    let state = state.clone();
    tokio::spawn(async move {
        tokio::time::sleep(OBEY_WITHIN).await;
        let Some(session) = state.watching().never_obeyed(device, work, Instant::now()) else {
            return;
        };
        // A player that never answered: an old one, or one cut off from the
        // network. What converts for it is closed; a plain file is only ever
        // read piece by piece, and there is nothing to close.
        if let (Some(session), Some(sessions)) = (session, state.sessions()) {
            sessions.close(session, watcher).await;
        }
        tracing::info!(device = %device, "a film asked to stop never did, and was closed");
    });
    true
}

/// One film being watched, as the administration shows it.
#[derive(Debug, Clone)]
pub struct Watched {
    pub device: DeviceId,
    pub user_name: String,
    pub device_name: String,
    /// The browser the page found, when it could tell.
    pub browser: Option<String>,
    pub work: WorkId,
    pub title: String,
    pub kind: WorkKind,
    pub year: Option<i32>,
    /// For an episode: its series, and where it sits in it.
    pub series: Option<String>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    /// A wide picture of it, as a path in the picture cache, when it has one.
    pub picture: Option<String>,
    /// What was decided for it, when the plan was heard.
    pub plan: Option<Arc<PlayPlan>>,
    /// How hard the machine is working on it, when a session converts it and
    /// its tool is running.
    pub producing: Option<Producing>,
    pub position: Millis,
    pub started_at: Timestamp,
    pub paused: bool,
    /// Asked to stop, and not stopped yet.
    pub stopping: bool,
}

/// Every film being watched right now, oldest first.
pub async fn now_playing(state: &AppState) -> Result<Vec<Watched>> {
    let database = state.database();
    let mut watched = Vec::new();
    for seen in state.watching().now_playing(Instant::now()) {
        // A work deleted while it played has nothing left to show.
        let Some(work) = database.work(seen.work).await? else {
            continue;
        };
        let ancestry = match work.kind {
            WorkKind::Episode => database.ancestry_of(work.id).await?,
            _ => Vec::new(),
        };
        let season = ancestry.iter().find(|up| up.kind == WorkKind::Season);
        let series = ancestry.iter().find(|up| up.kind == WorkKind::Series);

        // The episode's own still, then its series' wide picture; a film's
        // wide picture, then its poster.
        let mut picture = wide_picture_of(state, work.id).await?;
        if picture.is_none() {
            if let Some(series) = series {
                picture = wide_picture_of(state, series.id).await?;
            }
        }

        let producing = match (seen.session, state.sessions()) {
            (Some(id), Some(sessions)) => match sessions.get(id, seen.viewer.user).await {
                Ok(session) => session.preparation().await.producing,
                Err(_) => None,
            },
            _ => None,
        };

        watched.push(Watched {
            device: seen.viewer.device,
            user_name: seen.viewer.user_name,
            device_name: seen.viewer.device_name,
            browser: seen.viewer.browser,
            work: work.id,
            year: work.release_year.or(series.and_then(|up| up.release_year)),
            episode: (work.kind == WorkKind::Episode).then_some(work.ordinal).flatten(),
            season: season.and_then(|up| up.ordinal),
            series: series.map(|up| up.title.clone()),
            title: work.title,
            kind: work.kind,
            picture,
            plan: seen.plan,
            producing,
            position: seen.position,
            started_at: seen.started_at,
            paused: seen.paused,
            stopping: seen.stopping,
        });
    }
    Ok(watched)
}

/// How wide a picture is enough for a card of the administration.
const WIDE_ENOUGH: i32 = 480;

/// The smallest wide picture of a work that fills a card, or its largest.
async fn wide_picture_of(state: &AppState, work: WorkId) -> Result<Option<String>> {
    let images = state
        .database()
        .images_of("work", &work.to_db_string())
        .await?;
    for kind in ["thumb", "backdrop", "poster"] {
        let mut of_kind: Vec<_> = images
            .iter()
            .filter(|image| image.image_kind == kind)
            .collect();
        if of_kind.is_empty() {
            continue;
        }
        of_kind.sort_by_key(|image| image.width.unwrap_or(0));
        let chosen = of_kind
            .iter()
            .find(|image| image.width.unwrap_or(0) >= WIDE_ENOUGH)
            .or(of_kind.last())
            .map(|image| image.relative_path.clone());
        return Ok(chosen);
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn viewer(device: DeviceId) -> Viewer {
        Viewer {
            user: UserId::new(),
            user_name: "somebody".to_string(),
            device,
            device_name: "a browser".to_string(),
            browser: None,
        }
    }

    fn plan_for(work: WorkId, resume_from: Option<Millis>) -> Arc<PlayPlan> {
        Arc::new(PlayPlan {
            source_id: melyxar_core::id::MediaSourceId::new(),
            work_id: work,
            path: std::path::PathBuf::from("/media/Quiet Harbour (2019).mkv"),
            size_bytes: 1,
            container: Some("matroska".to_string()),
            overall_bitrate: None,
            duration: Some(Millis::from_seconds_f64(6000.0)),
            decision: melyxar_playback::decision::PlaybackDecision {
                method: melyxar_playback::decision::PlaybackMethod::DirectPlay,
                video: melyxar_playback::decision::StreamAction::Copy,
                audio: melyxar_playback::decision::StreamAction::Copy,
                subtitles: melyxar_playback::decision::SubtitleDelivery::None,
                audio_stream_index: None,
                subtitle_stream_index: None,
                video_stream_index: None,
                scale_to_height: None,
                bitrate_ceiling: None,
                tone_map: false,
                reasons: Vec::new(),
            },
            resume_from,
            tracks: Vec::new(),
            downmix: melyxar_core::user::DownmixMethod::default(),
            downmix_gain: 1.0,
            rebuild: None,
            thumbnails: None,
            chapters: Vec::new(),
            segments: Vec::new(),
            favourite: false,
        })
    }

    fn at() -> Timestamp {
        melyxar_core::time::now()
    }

    fn seconds(value: f64) -> Option<Millis> {
        Some(Millis::from_seconds_f64(value))
    }

    fn secs(value: u64) -> Duration {
        Duration::from_secs(value)
    }

    #[test]
    fn a_film_starts_paused_where_it_resumes_and_its_clock_runs_once_it_plays() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, seconds(60.0)), start, at());

        let seen = watching.now_playing(start + secs(5));
        assert_eq!(seen[0].position, Millis::from_seconds_f64(60.0));
        assert!(seen[0].paused, "nothing plays until the picture does");

        watching.heard(viewer(device), work, seconds(60.0), Some(false), start + secs(5), at());
        let seen = watching.now_playing(start + secs(12));
        assert!(!seen[0].paused);
        assert_eq!(seen[0].position, Millis::from_seconds_f64(67.0), "the clock ran on its own");
    }

    #[test]
    fn a_pause_is_what_the_player_says_and_stops_the_clock() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, None), start, at());
        let _line = watching.line_opened(viewer(device), work, start, at());
        watching.heard(viewer(device), work, seconds(0.0), Some(false), start, at());
        watching.heard(viewer(device), work, seconds(30.0), Some(true), start + secs(30), at());

        let seen = watching.now_playing(start + secs(90));
        assert!(seen[0].paused);
        assert_eq!(seen[0].position, Millis::from_seconds_f64(30.0));

        watching.heard(viewer(device), work, None, None, start + secs(100), at());
        assert!(watching.now_playing(start + secs(100))[0].paused, "a word without the flag keeps it");
    }

    #[test]
    fn the_clock_never_runs_past_the_end() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, None), start, at());
        watching.heard(viewer(device), work, seconds(5990.0), Some(false), start, at());
        let _line = watching.line_opened(viewer(device), work, start, at());
        assert_eq!(
            watching.now_playing(start + secs(60))[0].position,
            Millis::from_seconds_f64(6000.0)
        );
    }

    #[test]
    fn a_film_whose_line_is_open_is_kept_however_quiet_its_player() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, None), start, at());
        let line = watching.line_opened(viewer(device), work, start, at());
        assert!(line.is_some());

        let much_later = start + SILENT_FOR * 10;
        let seen = watching.now_playing(much_later);
        assert_eq!(seen.len(), 1, "a hidden tab is still watching");
        assert!(seen[0].plan.is_some(), "and keeps what was decided for it");

        watching.line_closed(device, work, much_later);
        assert_eq!(watching.now_playing(much_later + REJOIN_WITHIN / 2).len(), 1, "time to tie it again");
        assert!(watching.now_playing(much_later + REJOIN_WITHIN).is_empty());
    }

    #[test]
    fn a_line_tied_again_in_time_keeps_the_film() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, None), start, at());
        watching.line_opened(viewer(device), work, start, at());
        watching.line_closed(device, work, start);
        watching.line_opened(viewer(device), work, start + secs(3), at());
        assert_eq!(watching.now_playing(start + SILENT_FOR * 3).len(), 1);
    }

    #[test]
    fn a_line_for_another_film_than_the_one_playing_is_refused() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let start = Instant::now();
        let (old, new) = (WorkId::new(), WorkId::new());
        watching.starting(viewer(device), plan_for(new, None), start, at());
        assert!(watching.line_opened(viewer(device), old, start, at()).is_none());

        watching.gone(device, new, start);
        assert!(watching.line_opened(viewer(device), new, start, at()).is_none(), "nor one just left");
    }

    #[test]
    fn a_player_that_never_opened_a_line_is_let_go_after_a_long_silence() {
        let watching = Watching::default();
        let start = Instant::now();
        let quiet = DeviceId::new();
        let leaving = DeviceId::new();
        let (one, other) = (WorkId::new(), WorkId::new());
        watching.starting(viewer(quiet), plan_for(one, None), start, at());
        watching.starting(viewer(leaving), plan_for(other, None), start, at());

        watching.gone(leaving, one, start);
        assert_eq!(watching.now_playing(start).len(), 2, "leaving another film leaves this one");
        watching.gone(leaving, other, start);
        assert_eq!(watching.now_playing(start).len(), 1);
        assert!(watching.now_playing(start + SILENT_FOR).is_empty());
    }

    #[test]
    fn a_word_arriving_after_the_player_left_does_not_bring_the_film_back() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, None), start, at());
        watching.gone(device, work, start);
        assert!(!watching.heard(viewer(device), work, seconds(40.0), None, start, at()));
        assert!(watching.now_playing(start).is_empty());

        watching.starting(viewer(device), plan_for(work, None), start, at());
        assert_eq!(watching.now_playing(start).len(), 1, "opened again, it plays again");
    }

    #[test]
    fn a_new_plan_for_the_same_film_keeps_where_it_was_and_another_film_starts_afresh() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, None), start, at());
        watching.heard(viewer(device), work, seconds(500.0), Some(true), start, at());
        watching.starting(viewer(device), plan_for(work, None), start, at());
        assert_eq!(watching.now_playing(start)[0].position, Millis::from_seconds_f64(500.0));

        let next = WorkId::new();
        watching.starting(viewer(device), plan_for(next, None), start, at());
        let seen = watching.now_playing(start);
        assert_eq!(seen.len(), 1, "one device plays one film");
        assert_eq!(seen[0].work, next);
        assert_eq!(seen[0].position, Millis::ZERO);
    }

    #[test]
    fn a_player_heard_before_its_plan_is_still_shown() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.heard(viewer(device), work, seconds(12.0), Some(false), start, at());
        let seen = watching.now_playing(start);
        assert_eq!(seen.len(), 1);
        assert!(seen[0].plan.is_none());
    }

    #[test]
    fn a_stop_reaches_the_line_at_once_and_the_next_word_too() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        let who = viewer(device);
        let session = SessionId::new();
        watching.starting(who.clone(), plan_for(work, None), start, at());
        watching.session_opened(device, session, start);
        let line = watching.line_opened(who.clone(), work, start, at()).expect("a line");
        assert!(!*line.borrow());

        assert_eq!(watching.ask_to_stop(device, start), Some((who.user, work)));
        assert!(*line.borrow(), "told on the line");
        assert!(watching.now_playing(start)[0].stopping);
        assert!(watching.heard(who.clone(), work, None, None, start, at()), "and on its next word");

        assert_eq!(watching.never_obeyed(device, work, start), Some(Some(session)));
        assert!(watching.now_playing(start).is_empty());
        assert!(!watching.heard(who, work, seconds(90.0), None, start, at()));
        assert!(watching.now_playing(start).is_empty(), "closed, and not brought back by its player");
        assert!(watching.ask_to_stop(DeviceId::new(), start).is_none(), "nothing plays there");
    }

    #[test]
    fn a_player_that_obeyed_is_not_closed_behind_its_back() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, None), start, at());
        watching.ask_to_stop(device, start);
        watching.gone(device, work, start);
        assert!(watching.never_obeyed(device, work, start).is_none());

        watching.starting(viewer(device), plan_for(work, None), start, at());
        assert!(
            watching.never_obeyed(device, work, start).is_none(),
            "started again since, and not asked to stop"
        );
    }

    #[test]
    fn the_administration_is_told_of_a_pause_a_jump_or_a_departure_and_not_of_a_steady_clock() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        let mut told = watching.changed.subscribe();
        watching.starting(viewer(device), plan_for(work, None), start, at());
        watching.heard(viewer(device), work, seconds(0.0), Some(false), start, at());
        told.mark_unchanged();

        watching.heard(viewer(device), work, seconds(10.0), Some(false), start + secs(10), at());
        assert!(!told.has_changed().unwrap(), "where the clock said it would be");

        watching.heard(viewer(device), work, seconds(10.0), Some(true), start + secs(10), at());
        assert!(told.has_changed().unwrap(), "a pause");
        told.mark_unchanged();

        watching.heard(viewer(device), work, seconds(900.0), Some(true), start + secs(11), at());
        assert!(told.has_changed().unwrap(), "a jump");
        told.mark_unchanged();

        watching.gone(device, work, start + secs(12));
        assert!(told.has_changed().unwrap(), "a departure");
    }
}
