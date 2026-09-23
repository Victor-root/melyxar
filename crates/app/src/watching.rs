//! What is being watched right now, device by device.
//!
//! Held in memory and nowhere else: it is true for a minute at most, and a
//! film played as it lies on the disk leaves no other trace on the server
//! than what this hears. See the decisions on what is being watched.
//!
//! A watch is born when a player asks how to play a film, which is the one
//! moment every road through the player shares, plain file included. It is
//! kept up to date by the position the player already sends every ten
//! seconds, said to be paused when that position stops moving, and it ends
//! when the player says it is leaving or has not been heard for a while.
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

use crate::playback::{PlayPlan, Producing};
use crate::{AppState, Result};

/// How long a player may go unheard before its film is taken as stopped. It
/// speaks every ten seconds, so this is several missed in a row.
const SILENT_FOR: Duration = Duration::from_secs(40);

/// How long a position may stand still before the film is said to be paused.
/// Longer than one beat of the player, so one beat is never enough.
const PAUSED_AFTER: Duration = Duration::from_secs(15);

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
}

/// One film playing on one device.
#[derive(Debug, Clone)]
struct Watch {
    viewer: Viewer,
    work: WorkId,
    /// What was decided for it. Absent for a film heard of before its plan
    /// was, which is a player carrying on across a restart of the server.
    plan: Option<Arc<PlayPlan>>,
    /// The session converting it, when one is.
    session: Option<SessionId>,
    position: Millis,
    started_at: Timestamp,
    heard_at: Instant,
    moved_at: Instant,
    /// When an administrator asked for it to stop, when one did.
    stop_asked_at: Option<Instant>,
}

/// What is being watched, held for the whole server.
#[derive(Debug, Default)]
pub struct Watching {
    held: Mutex<Held>,
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

impl Watching {
    fn held(&self) -> std::sync::MutexGuard<'_, Held> {
        self.held.lock().unwrap_or_else(|held| held.into_inner())
    }

    /// A player asked how to play a film: a watch starts, or the one already
    /// on this device carries on with a new plan when it is the same film,
    /// which is a track or a size changed partway through.
    fn starting(&self, viewer: Viewer, plan: Arc<PlayPlan>, now: Instant, at: Timestamp) {
        let mut held = self.held();
        held.left.remove(&viewer.device);
        let carried_on = held
            .playing
            .get(&viewer.device)
            .filter(|watch| watch.work == plan.work_id)
            .map(|watch| (watch.position, watch.started_at, watch.session));
        let (position, started_at, session) = carried_on.unwrap_or((
            plan.resume_from.unwrap_or(Millis::ZERO),
            at,
            None,
        ));
        held.playing.insert(
            viewer.device,
            Watch {
                work: plan.work_id,
                viewer,
                plan: Some(plan),
                session,
                position,
                started_at,
                heard_at: now,
                moved_at: now,
                stop_asked_at: None,
            },
        );
    }

    fn session_opened(&self, device: DeviceId, session: SessionId, now: Instant) {
        if let Some(watch) = self.held().playing.get_mut(&device) {
            watch.session = Some(session);
            watch.heard_at = now;
        }
    }

    /// A player said it still has a film open, and where it is when it has
    /// got anywhere yet. Answers whether it has been asked to stop.
    fn heard(
        &self,
        viewer: Viewer,
        work: WorkId,
        position: Option<Millis>,
        now: Instant,
        at: Timestamp,
    ) -> bool {
        let mut held = self.held();
        if held.just_left(viewer.device, work, now) {
            return false;
        }
        match held.playing.get_mut(&viewer.device) {
            Some(watch) if watch.work == work => {
                if let Some(position) = position.filter(|&moved| moved != watch.position) {
                    watch.position = position;
                    watch.moved_at = now;
                }
                watch.heard_at = now;
                watch.stop_asked_at.is_some()
            }
            _ => {
                held.playing.insert(
                    viewer.device,
                    Watch {
                        viewer,
                        work,
                        plan: None,
                        session: None,
                        position: position.unwrap_or(Millis::ZERO),
                        started_at: at,
                        heard_at: now,
                        moved_at: now,
                        stop_asked_at: None,
                    },
                );
                false
            }
        }
    }

    /// A player said it is leaving this film.
    fn gone(&self, device: DeviceId, work: WorkId, now: Instant) {
        self.held().leave(device, work, now);
    }

    /// Marks the film on this device to be stopped, and says whose it is and
    /// which it is, for closing it if the player never obeys.
    fn ask_to_stop(&self, device: DeviceId, now: Instant) -> Option<(UserId, WorkId)> {
        let mut held = self.held();
        let watch = held.playing.get_mut(&device)?;
        watch.stop_asked_at.get_or_insert(now);
        Some((watch.viewer.user, watch.work))
    }

    /// Forgets this film on this device if it was asked to stop and is still
    /// there, answering what converted it.
    fn never_obeyed(&self, device: DeviceId, work: WorkId, now: Instant) -> Option<Option<SessionId>> {
        let mut held = self.held();
        let disobeyed = held
            .playing
            .get(&device)
            .is_some_and(|watch| watch.work == work && watch.stop_asked_at.is_some());
        disobeyed
            .then(|| held.leave(device, work, now))
            .flatten()
            .map(|watch| watch.session)
    }

    /// Everything playing, oldest first, once the players gone quiet are let
    /// go.
    fn now_playing(&self, now: Instant) -> Vec<Seen> {
        let mut held = self.held();
        held.playing
            .retain(|_, watch| now.duration_since(watch.heard_at) < SILENT_FOR);
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
                position: watch.position,
                started_at: watch.started_at,
                paused: now.duration_since(watch.moved_at) >= PAUSED_AFTER,
                stopping: watch.stop_asked_at.is_some(),
            })
            .collect();
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

/// A player said it still has a film open, and where it is when it has got
/// anywhere yet. Answers whether it has been asked to stop.
pub fn heard(state: &AppState, viewer: Viewer, work: WorkId, position: Option<Millis>) -> bool {
    state.watching().heard(
        viewer,
        work,
        position,
        Instant::now(),
        melyxar_core::time::now(),
    )
}

/// A player said it is leaving this film.
pub fn gone(state: &AppState, device: DeviceId, work: WorkId) {
    state.watching().gone(device, work, Instant::now());
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

    #[test]
    fn a_film_starts_where_it_resumes_and_follows_its_player() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, seconds(60.0)), start, at());

        let seen = watching.now_playing(start);
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].position, Millis::from_seconds_f64(60.0));
        assert!(seen[0].plan.is_some());

        let later = start + Duration::from_secs(10);
        assert!(!watching.heard(viewer(device), work, seconds(70.0), later, at()));
        assert_eq!(watching.now_playing(later)[0].position, Millis::from_seconds_f64(70.0));
    }

    #[test]
    fn a_player_that_has_not_got_anywhere_yet_keeps_its_film_where_it_resumes() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, seconds(600.0)), start, at());
        let later = start + Duration::from_secs(30);
        watching.heard(viewer(device), work, None, later, at());

        let seen = watching.now_playing(later + Duration::from_secs(30));
        assert_eq!(seen.len(), 1, "still preparing is still there");
        assert_eq!(seen[0].position, Millis::from_seconds_f64(600.0));
    }

    #[test]
    fn a_position_that_stands_still_is_a_pause() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        watching.starting(viewer(device), plan_for(work, None), start, at());
        let position = seconds(30.0);
        watching.heard(viewer(device), work, position, start + Duration::from_secs(10), at());
        assert!(!watching.now_playing(start + Duration::from_secs(20))[0].paused, "one beat is not a pause");
        watching.heard(viewer(device), work, position, start + Duration::from_secs(20), at());
        watching.heard(viewer(device), work, position, start + Duration::from_secs(30), at());
        assert!(watching.now_playing(start + Duration::from_secs(30))[0].paused);
    }

    #[test]
    fn a_player_gone_quiet_or_gone_is_let_go() {
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
        assert!(!watching.heard(viewer(device), work, seconds(40.0), start, at()));
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
        watching.heard(viewer(device), work, seconds(500.0), start, at());
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
        watching.heard(viewer(device), work, seconds(12.0), start, at());
        let seen = watching.now_playing(start);
        assert_eq!(seen.len(), 1);
        assert!(seen[0].plan.is_none());
    }

    #[test]
    fn a_stop_is_told_to_the_player_and_kept_for_one_that_never_obeys() {
        let watching = Watching::default();
        let device = DeviceId::new();
        let work = WorkId::new();
        let start = Instant::now();
        let who = viewer(device);
        let session = SessionId::new();
        watching.starting(who.clone(), plan_for(work, None), start, at());
        watching.session_opened(device, session, start);

        assert_eq!(watching.ask_to_stop(device, start), Some((who.user, work)));
        assert!(watching.now_playing(start)[0].stopping);
        assert!(watching.heard(who.clone(), work, None, start, at()), "told on its next beat");

        assert_eq!(watching.never_obeyed(device, work, start), Some(Some(session)));
        assert!(watching.now_playing(start).is_empty());
        assert!(!watching.heard(who, work, seconds(90.0), start, at()));
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
}
