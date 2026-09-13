//! Every session at once: how many may run, and when they are swept away.
//!
//! Two things are being protected. The machine, since converting a film costs
//! real work and a server that accepts every request at once serves everyone
//! badly rather than most people well. And the disk, since a session that ends
//! without being closed leaves a folder behind, and nothing ever comes back
//! for it.
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

/// How long a session nobody has asked anything of is kept.
///
/// Long enough to survive a viewer pausing to answer the door, short enough
/// that a tab closed at midnight is not still holding a tool at dawn. A
/// player asks for a segment every few seconds while playing, and the
/// heartbeat covers a pause.
pub const KEPT_WHILE_IDLE: Duration = Duration::from_secs(120);

/// Every session this server is serving.
pub struct Sessions {
    live: Mutex<HashMap<SessionId, Arc<Session>>>,
    folder: PathBuf,
    tools: ToolPaths,
    /// How many sessions may convert at once. Copying is not counted: it costs
    /// almost nothing, and refusing it would turn a cheap request away for the
    /// benefit of an expensive one.
    most_at_once: usize,
}

impl Sessions {
    pub fn new(folder: PathBuf, tools: ToolPaths, most_at_once: usize) -> Self {
        Self {
            live: Mutex::new(HashMap::new()),
            folder,
            tools,
            most_at_once,
        }
    }

    /// Opens a session, unless the machine is already doing as much as it can.
    ///
    /// `expensive` is what the playback decision said: a stream being rebuilt
    /// counts against the limit, one being copied does not.
    pub async fn open(&self, recipe: Recipe, expensive: bool) -> Result<Arc<Session>> {
        let mut live = self.live.lock().await;

        if expensive && live.len() >= self.most_at_once {
            // Told plainly rather than accepted and served badly: a machine
            // converting five films at once finishes none of them in time.
            return Err(StreamingError::TooManyAtOnce);
        }

        let id = SessionId::new();
        let session = Arc::new(
            Session::open(
                id,
                recipe,
                self.folder.join(id.to_string()),
                self.tools.clone(),
            )
            .await?,
        );
        live.insert(id, Arc::clone(&session));
        tracing::info!(session = %id, live = live.len(), "playback session opened");
        Ok(session)
    }

    /// The session with this name, while it is still live.
    pub async fn get(&self, id: SessionId) -> Result<Arc<Session>> {
        self.live
            .lock()
            .await
            .get(&id)
            .cloned()
            .ok_or(StreamingError::NoSuchSession)
    }

    /// Closes one session and forgets it.
    pub async fn close(&self, id: SessionId) {
        let session = self.live.lock().await.remove(&id);
        if let Some(session) = session {
            session.close().await;
            tracing::info!(session = %id, "playback session closed");
        }
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
            for (id, session) in live.drain() {
                if session.idle_for().await >= idle_for {
                    going.push(session);
                } else {
                    keeping.insert(id, session);
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
        let all: Vec<Arc<Session>> = self.live.lock().await.drain().map(|(_, s)| s).collect();
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
    use super::*;
    use melyxar_core::time::Millis;
    use melyxar_ffmpeg::command::{AudioOutput, StreamSelection, VideoOutput};

    fn recipe(source: PathBuf) -> Recipe {
        Recipe {
            source,
            duration: Millis::new(20_000),
            streams: StreamSelection::default(),
            video: VideoOutput::Copy,
            audio: AudioOutput::Copy,
            if_the_card_refuses: None,
        }
    }

    fn sessions(folder: PathBuf, most_at_once: usize) -> Sessions {
        Sessions::new(
            folder,
            ToolPaths::discover(None, None).expect("the tools are installed here"),
            most_at_once,
        )
    }

    #[tokio::test]
    async fn a_session_is_found_again_by_its_name_and_forgotten_once_closed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"), 2);

        let session = sessions
            .open(recipe(directory.path().join("film.mkv")), false)
            .await
            .expect("a session");
        assert_eq!(sessions.live_count().await, 1);
        assert_eq!(
            sessions.get(session.id).await.expect("found again").id,
            session.id
        );

        sessions.close(session.id).await;
        assert_eq!(sessions.live_count().await, 0);
        assert!(matches!(
            sessions.get(session.id).await,
            Err(StreamingError::NoSuchSession)
        ));
    }

    #[tokio::test]
    async fn a_machine_already_converting_all_it_can_says_so_rather_than_accepting() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"), 1);

        sessions
            .open(recipe(directory.path().join("one.mkv")), true)
            .await
            .expect("the first fits");
        assert!(
            matches!(
                sessions
                    .open(recipe(directory.path().join("two.mkv")), true)
                    .await,
                Err(StreamingError::TooManyAtOnce)
            ),
            "a machine converting more than it can finishes none of them in time"
        );
    }

    #[tokio::test]
    async fn a_film_being_copied_is_never_turned_away() {
        // Copying costs almost nothing, so counting it against the limit would
        // refuse a cheap request for the benefit of an expensive one.
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"), 1);

        sessions
            .open(recipe(directory.path().join("one.mkv")), true)
            .await
            .expect("the expensive one fits");
        sessions
            .open(recipe(directory.path().join("two.mkv")), false)
            .await
            .expect("a copy is welcome all the same");
        assert_eq!(sessions.live_count().await, 2);
    }

    #[tokio::test]
    async fn a_session_nobody_is_watching_any_more_is_swept_away() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"), 4);

        let session = sessions
            .open(recipe(directory.path().join("film.mkv")), false)
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
        let sessions = sessions(directory.path().join("sessions"), 4);
        let session = sessions
            .open(recipe(directory.path().join("film.mkv")), false)
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
    async fn folders_a_previous_run_left_behind_are_removed_at_start_up() {
        // A crash, a power cut, a container killed outright: nothing else ever
        // comes back for these.
        let directory = tempfile::tempdir().expect("temporary directory");
        let folder = directory.path().join("sessions");
        std::fs::create_dir_all(folder.join("01a0-left-behind")).expect("an old folder");
        std::fs::write(folder.join("01a0-left-behind").join("segment-0.m4s"), b"x")
            .expect("an old segment");

        let sessions = sessions(folder.clone(), 4);
        assert_eq!(sessions.sweep_what_a_previous_run_left().await, 1);
        assert!(!folder.join("01a0-left-behind").exists());
    }

    #[tokio::test]
    async fn closing_everything_leaves_nothing_live() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let sessions = sessions(directory.path().join("sessions"), 4);
        sessions
            .open(recipe(directory.path().join("one.mkv")), false)
            .await
            .expect("a session");
        sessions
            .open(recipe(directory.path().join("two.mkv")), false)
            .await
            .expect("another");

        sessions.close_all().await;
        assert_eq!(sessions.live_count().await, 0);
    }
}
