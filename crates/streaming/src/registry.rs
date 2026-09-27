//! Every session at once: how many may run, how much of the disk they may
//! fill, and when they are swept away.
//!
//! Two things are being protected, each only as far as the owner asked. The
//! machine, since converting a film costs real work. And the disk, since the
//! segments of every film being watched pile up behind each viewer, and a
//! session that ends without being closed leaves a folder behind that nothing
//! ever comes back for.
//!
//! A viewer never says goodbye: they close a tab, lose a connection, put a
//! telephone in a pocket. So a session is kept alive by being used, and
//! anything nobody has touched for a while is swept away.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use melyxar_ffmpeg::ToolPaths;
use tokio::sync::Mutex;

use crate::session::{Recipe, Session, SessionId};
use crate::{Result, StreamingError};

/// A megabyte, for the journal.
const MEGABYTE: u64 = 1024 * 1024;

/// How much of the disk everything under a folder fills, a level down: the
/// sessions' folders and the segments in them.
async fn size_of(folder: &std::path::Path) -> u64 {
    let mut total = 0;
    let Ok(mut sessions) = tokio::fs::read_dir(folder).await else {
        return 0;
    };
    while let Ok(Some(session)) = sessions.next_entry().await {
        let Ok(mut files) = tokio::fs::read_dir(session.path()).await else {
            continue;
        };
        while let Ok(Some(file)) = files.next_entry().await {
            if let Ok(about) = file.metadata().await {
                total += about.len();
            }
        }
    }
    total
}

/// How long a session nobody has asked anything of is kept.
///
/// Long enough to survive a viewer pausing to answer the door, short enough
/// that a tab closed at midnight is not still holding a tool at dawn. A
/// player asks for a segment every few seconds while playing, and the
/// heartbeat covers a pause.
pub const KEPT_WHILE_IDLE: Duration = Duration::from_secs(120);

/// What the owner allows the sessions, as the settings say at this moment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Limits {
    /// How many sessions may convert at once. None: no ceiling.
    pub most_at_once: Option<u32>,
    /// How much of the disk every session's segments together may fill.
    /// None: no ceiling.
    pub room: Option<Room>,
}

/// A ceiling on the disk the segments fill, and what is never given up to
/// keep under it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room {
    pub most_bytes: u64,
    /// How far behind each viewer the segments stay whatever the ceiling
    /// says, so a step back lands on something already there.
    pub kept_behind: Duration,
}

/// A session being served, and whether it is rebuilding the film rather than
/// copying it.
struct Live {
    session: Arc<Session>,
    expensive: bool,
}

/// Every session this server is serving.
pub struct Sessions {
    live: Mutex<HashMap<SessionId, Live>>,
    folder: PathBuf,
    tools: ToolPaths,
}

impl Sessions {
    pub fn new(folder: PathBuf, tools: ToolPaths) -> Self {
        Self {
            live: Mutex::new(HashMap::new()),
            folder,
            tools,
        }
    }

    /// Opens a session, unless the machine is already converting as many as
    /// its owner allowed, or the disk is full of what nobody may give up.
    ///
    /// `expensive` is what the playback decision said: a stream being rebuilt
    /// counts against the ceiling on conversions, one being copied does not,
    /// since it costs almost nothing and refusing it would turn a cheap
    /// request away for the benefit of an expensive one. Both fill the disk
    /// alike, so both are held to the room. The limits are asked of the
    /// settings on every opening, so a change applies to the next film.
    pub async fn open(
        &self,
        watcher: melyxar_core::id::UserId,
        recipe: Recipe,
        expensive: bool,
        limits: Limits,
    ) -> Result<Arc<Session>> {
        // Room is made first, since it walks every session, and refused only
        // when nothing more may go: the last resort, after everything watched
        // long enough ago has been given up.
        if let Some(room) = limits.room {
            if self.make_room(room).await >= room.most_bytes {
                return Err(StreamingError::NoRoomLeft);
            }
        }

        let mut live = self.live.lock().await;

        let converting = live.values().filter(|kept| kept.expensive).count();
        if expensive
            && limits
                .most_at_once
                .is_some_and(|most| converting >= most as usize)
        {
            // Told plainly rather than accepted and served badly: a machine
            // converting five films at once finishes none of them in time.
            return Err(StreamingError::TooManyAtOnce);
        }

        let id = SessionId::new();
        let session = Arc::new(
            Session::open(
                id,
                watcher,
                recipe,
                self.folder.join(id.to_string()),
                self.tools.clone(),
            )
            .await?,
        );
        live.insert(
            id,
            Live {
                session: Arc::clone(&session),
                expensive,
            },
        );
        tracing::info!(session = %id, live = live.len(), "playback session opened");
        Ok(session)
    }

    /// The session with this name, while it is still live and if it is theirs.
    ///
    /// Somebody else's reads as a session that is not there. A name is all it
    /// takes to be handed every segment of what is being watched, so being
    /// signed in cannot be the same thing as being signed in as whoever
    /// opened it. Asked for here rather than by each caller: a caller can
    /// forget.
    pub async fn get(
        &self,
        id: SessionId,
        watcher: melyxar_core::id::UserId,
    ) -> Result<Arc<Session>> {
        self.live
            .lock()
            .await
            .get(&id)
            .filter(|kept| kept.session.watcher == watcher)
            .map(|kept| Arc::clone(&kept.session))
            .ok_or(StreamingError::NoSuchSession)
    }

    /// Closes one session of theirs and forgets it.
    ///
    /// Somebody else's is left alone: a name is all it would take to stop a
    /// film somebody else is in the middle of.
    pub async fn close(&self, id: SessionId, watcher: melyxar_core::id::UserId) {
        let session = self
            .live
            .lock()
            .await
            .get(&id)
            .filter(|kept| kept.session.watcher == watcher)
            .map(|kept| Arc::clone(&kept.session));
        if let Some(session) = session {
            self.live.lock().await.remove(&id);
            session.close().await;
            tracing::info!(session = %id, "playback session closed");
        }
    }

    /// Gives up the segments furthest behind their viewers, oldest first,
    /// until every session together fits in the room, and answers how much of
    /// the disk they fill once done.
    ///
    /// What lies within `kept_behind` of a viewer, the header and everything
    /// ahead are never given up, so a full cache can stay over its ceiling:
    /// that is what refuses the next film rather than cutting one being
    /// watched.
    pub async fn make_room(&self, room: Room) -> u64 {
        let mut used = size_of(&self.folder).await;
        if used <= room.most_bytes {
            return used;
        }

        let sessions: Vec<Arc<Session>> = self
            .live
            .lock()
            .await
            .values()
            .map(|kept| Arc::clone(&kept.session))
            .collect();
        let mut behind = Vec::new();
        for session in &sessions {
            for path in session.segments_left_behind(room.kept_behind).await {
                if let Ok(about) = tokio::fs::metadata(&path).await {
                    let written = about.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH);
                    behind.push((written, about.len(), path));
                }
            }
        }
        behind.sort_by_key(|(written, _, _)| *written);

        let mut removed = 0;
        for (_, size, path) in behind {
            if used <= room.most_bytes {
                break;
            }
            if tokio::fs::remove_file(&path).await.is_ok() {
                used = used.saturating_sub(size);
                removed += 1;
            }
        }
        // Said once something went, and quietly while nothing could: a cache
        // held over its ceiling by what may not go is asked again every sweep.
        if removed > 0 {
            tracing::info!(
                removed,
                used_megabytes = used / MEGABYTE,
                ceiling_megabytes = room.most_bytes / MEGABYTE,
                "the transcode cache was over its ceiling, and what was watched long enough ago was given up"
            );
        } else {
            tracing::debug!(
                used_megabytes = used / MEGABYTE,
                ceiling_megabytes = room.most_bytes / MEGABYTE,
                "the transcode cache is over its ceiling with nothing far enough behind a viewer to give up"
            );
        }
        used
    }

    pub async fn live_count(&self) -> usize {
        self.live.lock().await.len()
    }

    /// Sweeps away what nobody is watching any more.
    ///
    /// Answers how many were swept, which is what a log line says.
    pub async fn sweep(&self, idle_for: Duration) -> usize {
        let mut going = Vec::new();
        {
            let mut live = self.live.lock().await;
            let mut keeping = HashMap::new();
            for (id, kept) in live.drain() {
                if kept.session.idle_for().await >= idle_for {
                    going.push(kept.session);
                } else {
                    keeping.insert(id, kept);
                }
            }
            *live = keeping;
        }

        let swept = going.len();
        for session in going {
            tracing::info!(session = %session.id, "a session nobody was watching was swept away");
            session.close().await;
        }
        swept
    }

    /// Closes everything, which is what a server does on its way out.
    pub async fn close_all(&self) {
        let all: Vec<Arc<Session>> = self
            .live
            .lock()
            .await
            .drain()
            .map(|(_, kept)| kept.session)
            .collect();
        for session in all {
            session.close().await;
        }
    }

    /// Removes folders left behind by a server that did not get to close them.
    ///
    /// A crash, a power cut, a container killed outright. Nothing else ever
    /// comes back for these, so they are swept at start-up, before any session
    /// of this run exists.
    pub async fn sweep_what_a_previous_run_left(&self) -> usize {
        let Ok(mut entries) = tokio::fs::read_dir(&self.folder).await else {
            return 0;
        };

        let mut removed = 0;
        while let Ok(Some(entry)) = entries.next_entry().await {
            if entry.path().is_dir() && tokio::fs::remove_dir_all(entry.path()).await.is_ok() {
                removed += 1;
            }
        }
        if removed > 0 {
            tracing::info!(removed, "folders left by a previous run were removed");
        }
        removed
    }
}

#[cfg(test)]
mod tests {
    use crate::session::NOBODY_IN_PARTICULAR;

    use super::*;
    use melyxar_core::time::Millis;
    use melyxar_ffmpeg::command::{AudioOutput, StreamSelection, VideoOutput};

    /// Whoever is watching, for the tests that are not about who.
    ///
    /// The same one every time, since what they are about is the registry
    /// rather than whose session it is.
    fn a_watcher() -> melyxar_core::id::UserId {
        *NOBODY_IN_PARTICULAR
    }

    /// A ceiling on conversions and nothing else.
    fn at_most(most: u32) -> Limits {
        Limits {
            most_at_once: Some(most),
            room: None,
        }
    }

    /// Writes a whole segment of `bytes` into a session's folder, one box the
    /// way the tool writes them, so the session hands it over as it is.
    async fn a_segment_written(session: &Session, index: u32, bytes: usize) {
        let mut data = (bytes as u32).to_be_bytes().to_vec();
        data.extend_from_slice(b"free");
        data.resize(bytes, 0);
        tokio::fs::write(session.folder().join(format!("segment-{index}.m4s")), data)
            .await
            .expect("written");
    }

    /// A session of five segments of four seconds, all on the disk, a
    /// thousand bytes each, with its viewer at the last one.
    async fn watched_to_the_end(sessions: &Sessions, directory: &std::path::Path) -> Arc<Session> {
        let session = sessions
            .open(
                a_watcher(),
                recipe(directory.join("film.mkv")),
                false,
                Limits::default(),
            )
            .await
            .expect("a session");
        for index in 0..5 {
            a_segment_written(&session, index, 1000).await;
        }
        session.segment(4).await.expect("handed over");
        session
    }

    fn room(most_bytes: u64) -> Room {
        Room {
            most_bytes,
            kept_behind: Duration::from_secs(4),
        }
    }

    #[tokio::test]
    async fn a_full_cache_gives_up_what_lies_far_behind_the_viewer_and_nothing_else() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));
        let session = watched_to_the_end(&sessions, directory.path()).await;

        let used = sessions.make_room(room(2500)).await;

        assert_eq!(used, 2000);
        let on_the_disk = |index: u32| session.folder().join(format!("segment-{index}.m4s")).exists();
        assert!(
            (0..3).all(|index| !on_the_disk(index)),
            "further behind the viewer than kept"
        );
        assert!(
            on_the_disk(3) && on_the_disk(4),
            "within what is kept behind the viewer, and where the viewer is"
        );
    }

    #[tokio::test]
    async fn a_cache_under_its_ceiling_gives_up_nothing() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));
        let session = watched_to_the_end(&sessions, directory.path()).await;

        assert_eq!(sessions.make_room(room(10_000)).await, 5000);
        assert!(session.folder().join("segment-0.m4s").exists());
    }

    #[tokio::test]
    async fn a_cache_full_of_what_may_not_go_refuses_the_next_film() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));
        watched_to_the_end(&sessions, directory.path()).await;

        let refused = sessions
            .open(
                a_watcher(),
                recipe(directory.path().join("another.mkv")),
                false,
                Limits {
                    most_at_once: None,
                    room: Some(room(1500)),
                },
            )
            .await;

        assert!(
            matches!(refused, Err(StreamingError::NoRoomLeft)),
            "the two segments around the viewer stay, and alone fill more than allowed"
        );
    }

    fn recipe(source: PathBuf) -> Recipe {
        Recipe {
            source,
            duration: Millis::new(20_000),
            streams: StreamSelection::default(),
            video: VideoOutput::Copy,
            audio: AudioOutput::Copy,
            where_the_viewer_starts: Millis::ZERO,
            where_it_can_be_started: Vec::new(),
            if_the_card_refuses: Vec::new(),
        }
    }

    #[tokio::test]
    async fn a_session_of_somebody_else_reads_as_one_that_is_not_there() {
        // Its name is all it takes to be handed every segment of what is
        // being watched, so being signed in cannot be the same thing as being
        // signed in as whoever opened it.
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().to_path_buf());
        let mine = sessions
            .open(a_watcher(), recipe(directory.path().join("film.mkv")), false, Limits::default())
            .await
            .expect("session opened");

        let somebody_else = melyxar_core::id::UserId::new();
        assert!(
            matches!(
                sessions.get(mine.id, somebody_else).await,
                Err(StreamingError::NoSuchSession)
            ),
            "a name is not a right to what it names"
        );

        // Nor is it a right to stop a film somebody else is in the middle of.
        sessions.close(mine.id, somebody_else).await;
        assert!(
            sessions.get(mine.id, a_watcher()).await.is_ok(),
            "it is still theirs and still live"
        );

        sessions.close(mine.id, a_watcher()).await;
        assert!(sessions.get(mine.id, a_watcher()).await.is_err());
    }

    fn sessions(folder: PathBuf) -> Sessions {
        Sessions::new(
            folder,
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
    }

    #[tokio::test]
    async fn a_session_is_found_again_by_its_name_and_forgotten_once_closed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));

        let session = sessions
            .open(a_watcher(), recipe(directory.path().join("film.mkv")), false, Limits::default())
            .await
            .expect("a session");
        assert_eq!(sessions.live_count().await, 1);
        assert_eq!(
            sessions.get(session.id, a_watcher()).await.expect("found again").id,
            session.id
        );

        sessions.close(session.id, a_watcher()).await;
        assert_eq!(sessions.live_count().await, 0);
        assert!(matches!(
            sessions.get(session.id, a_watcher()).await,
            Err(StreamingError::NoSuchSession)
        ));
    }

    #[tokio::test]
    async fn a_machine_already_converting_all_it_can_says_so_rather_than_accepting() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));

        sessions
            .open(a_watcher(), recipe(directory.path().join("one.mkv")), true, at_most(1))
            .await
            .expect("the first fits");
        assert!(
            matches!(
                sessions
                    .open(a_watcher(), recipe(directory.path().join("two.mkv")), true, at_most(1))
                    .await,
                Err(StreamingError::TooManyAtOnce)
            ),
            "a machine converting more than it can finishes none of them in time"
        );
    }

    #[tokio::test]
    async fn with_no_ceiling_every_conversion_is_welcome() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));

        for name in ["one.mkv", "two.mkv", "three.mkv"] {
            sessions
                .open(a_watcher(), recipe(directory.path().join(name)), true, Limits::default())
                .await
                .expect("nobody set a ceiling");
        }
        assert_eq!(sessions.live_count().await, 3);
    }

    #[tokio::test]
    async fn a_film_being_copied_is_never_turned_away_nor_counted() {
        // Copying costs almost nothing, so counting it against the ceiling
        // would refuse a cheap request for the benefit of an expensive one.
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));

        sessions
            .open(a_watcher(), recipe(directory.path().join("one.mkv")), false, at_most(1))
            .await
            .expect("a copy is welcome");
        sessions
            .open(a_watcher(), recipe(directory.path().join("two.mkv")), true, at_most(1))
            .await
            .expect("the copy holds no place a conversion needs");
        sessions
            .open(a_watcher(), recipe(directory.path().join("three.mkv")), false, at_most(1))
            .await
            .expect("a copy is welcome all the same");
        assert_eq!(sessions.live_count().await, 3);
    }

    #[tokio::test]
    async fn a_session_nobody_is_watching_any_more_is_swept_away() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));

        let session = sessions
            .open(a_watcher(), recipe(directory.path().join("film.mkv")), false, Limits::default())
            .await
            .expect("a session");
        let folder = session.folder().to_path_buf();

        assert_eq!(
            sessions.sweep(Duration::from_secs(60)).await,
            0,
            "it was opened a moment ago: nobody has had time to walk away"
        );
        assert_eq!(sessions.live_count().await, 1);

        // A viewer never says goodbye: they close a tab and nothing arrives.
        assert_eq!(sessions.sweep(Duration::ZERO).await, 1);
        assert_eq!(sessions.live_count().await, 0);
        assert!(!folder.exists(), "and the folder goes with it");
    }

    #[tokio::test]
    async fn a_session_being_used_is_not_swept_from_under_a_viewer() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));
        let session = sessions
            .open(a_watcher(), recipe(directory.path().join("film.mkv")), false, Limits::default())
            .await
            .expect("a session");

        tokio::time::sleep(Duration::from_millis(50)).await;
        // Asking for anything is what says someone is still there.
        let _ = session.segment(999).await;

        assert_eq!(
            sessions.sweep(Duration::from_millis(40)).await,
            0,
            "a viewer who asked for something forty milliseconds ago is watching"
        );
    }

    #[tokio::test]
    async fn a_session_somebody_has_paused_is_not_swept_from_under_them() {
        // A film playing asks for a segment every few seconds, which is what
        // says a viewer is there. A film paused asks for nothing at all, and
        // it used to be swept away with the viewer sitting in front of it:
        // pressing play then answered that the session was over for every
        // segment of the film, which reaches a viewer as a browser that
        // cannot read it. Reported from a real library after a pause.
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));
        let session = sessions
            .open(a_watcher(), recipe(directory.path().join("film.mkv")), false, Limits::default())
            .await
            .expect("a session");

        tokio::time::sleep(Duration::from_millis(50)).await;
        session.still_watching().await;

        assert_eq!(
            sessions.sweep(Duration::from_millis(40)).await,
            0,
            "somebody said forty milliseconds ago that they were still there"
        );
        assert_eq!(sessions.live_count().await, 1);
    }

    #[tokio::test]
    async fn folders_a_previous_run_left_behind_are_removed_at_start_up() {
        // A crash, a power cut, a container killed outright: nothing else ever
        // comes back for these.
        let directory = tempfile::tempdir().expect("temporary directory");
        let folder = directory.path().join("sessions");
        std::fs::create_dir_all(folder.join("01a0-left-behind")).expect("an old folder");
        std::fs::write(folder.join("01a0-left-behind").join("segment-0.m4s"), b"x")
            .expect("an old segment");

        let sessions = sessions(folder.clone());
        assert_eq!(sessions.sweep_what_a_previous_run_left().await, 1);
        assert!(!folder.join("01a0-left-behind").exists());
    }

    #[tokio::test]
    async fn closing_everything_leaves_nothing_live() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"));
        sessions
            .open(a_watcher(), recipe(directory.path().join("one.mkv")), false, Limits::default())
            .await
            .expect("a session");
        sessions
            .open(a_watcher(), recipe(directory.path().join("two.mkv")), false, Limits::default())
            .await
            .expect("another");

        sessions.close_all().await;
        assert_eq!(sessions.live_count().await, 0);
    }
}
