//! One film being watched: a folder, a tool, and segments on demand.
//!
//! A session is asked for a segment by number. Three things can be true. The
//! segment is already on disk, and it is handed over. The tool is running and
//! about to reach it, so waiting a moment is cheaper than starting again.
//! Or it is somewhere else entirely, which is what a viewer dragging the
//! cursor looks like: the tool is stopped and started again at that point.
//!
//! That third case is the whole reason the server owns the playlist. Nothing
//! about jumping is special here; it is the ordinary path with a different
//! number.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use melyxar_core::time::Millis;
use melyxar_ffmpeg::command::{Command, Input, Output, StreamSelection};
use melyxar_ffmpeg::process::RunningProcess;
use melyxar_ffmpeg::ToolPaths;
use tokio::sync::Mutex;

use crate::playlist::Playlist;
use crate::{Result, StreamingError};

/// How far ahead of the segment being produced a request is still worth
/// waiting for rather than starting the tool again.
///
/// Restarting costs a second or two and throws away work already done, so a
/// request for what is nearly ready waits. Beyond this, the viewer has jumped.
const WORTH_WAITING_FOR: u32 = 6;

/// How long a segment is waited for before the answer is that it is too slow.
///
/// Long enough for a slow first segment on a busy machine, short enough that a
/// player is not left hanging on a tool that died without saying so.
const PATIENCE: Duration = Duration::from_secs(30);

/// How often the folder is looked at while waiting.
const LOOK_AGAIN_EVERY: Duration = Duration::from_millis(120);

/// What a session was asked to produce.
///
/// Taken from the playback decision: this crate does not decide anything about
/// codecs, it carries out what was decided.
#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub source: PathBuf,
    /// How long the film runs, which is what the playlist is written from.
    pub duration: Millis,
    pub streams: StreamSelection,
    pub video: melyxar_ffmpeg::command::VideoOutput,
    pub audio: melyxar_ffmpeg::command::AudioOutput,
    /// What to rebuild the picture with if the card will not have it, in
    /// order, each one asking less of the card than the one before.
    ///
    /// A card is proved at start-up on a generated picture, which is the right
    /// way round but does not prove every film: a driver refuses a size, a
    /// colour layout, a film nobody thought of. That refusal must not reach a
    /// viewer as a black screen, so it costs one restart and a line in the log
    /// naming what the tool said.
    ///
    /// A ladder rather than a single step because the rungs are not equal. A
    /// card that will not read one film will still rebuild its picture, and
    /// giving up on the card entirely at the first refusal would throw away
    /// most of what it was doing.
    pub if_the_card_refuses: Vec<melyxar_ffmpeg::command::VideoOutput>,
}

/// Where the preparation of a film has got to.
///
/// Named steps rather than a proportion alone: "three seconds of nineteen" is
/// a number nobody can act on, while "reading the film" and "building the
/// picture" say which part is slow when one of them is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreparationStep {
    /// Nothing has been asked of the tool yet.
    Starting,
    /// The tool is running and has not written anything here yet. On a large
    /// film this is the tool reading its way to the point asked for.
    Reading,
    /// Segments are appearing, and there are not enough of them yet.
    Producing,
    /// Enough is on the disk for the film to start without stopping again.
    Ready,
}

impl PreparationStep {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Reading => "reading",
            Self::Producing => "producing",
            Self::Ready => "ready",
        }
    }
}

/// How far the preparation has got, in a form a page can show.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preparation {
    pub step: PreparationStep,
    /// Segments on the disk, counted from where the tool was started.
    pub ready: u32,
    /// How many make a comfortable start.
    pub wanted: u32,
}

/// Whether a tool started at `from` will reach `index` soon enough that
/// waiting beats starting again.
///
/// Behind is never worth waiting for: the tool only moves forward, so a
/// request for something it has already passed is a viewer who went back.
fn already_on_its_way(from: u32, index: u32) -> bool {
    index >= from && index < from + WORTH_WAITING_FOR
}

/// Which step a running tool is on, from what it has produced so far.
fn step_for(ready: u32, wanted: u32) -> PreparationStep {
    if ready >= wanted {
        PreparationStep::Ready
    } else if ready == 0 {
        // The tool is running and nothing is readable yet. On a large file
        // this is it reading its way to the point asked for.
        PreparationStep::Reading
    } else {
        PreparationStep::Producing
    }
}

/// The tool at work, and where it started.
struct AtWork {
    process: RunningProcess,
    /// The segment number it was started on, so a request can tell whether it
    /// is ahead of the tool or somewhere else entirely.
    from: u32,
    /// How far into the film the tool says it has written, in milliseconds.
    ///
    /// The tool's own account of itself rather than anything read off the
    /// disk. A file that exists may still be growing, and the only way to tell
    /// without asking the tool is to wait for the next one to appear, which
    /// means producing a whole extra segment before handing over the one
    /// somebody is waiting for. On a jump that is the difference between one
    /// wait and two.
    reached: Arc<AtomicI64>,
}

impl AtWork {
    /// How far into the film the tool says it has written.
    fn reached(&self) -> Millis {
        Millis::new(self.reached.load(Ordering::Relaxed))
    }
}

/// One film being watched.
pub struct Session {
    pub id: SessionId,
    recipe: Recipe,
    playlist: Playlist,
    folder: PathBuf,
    tools: ToolPaths,
    running: Mutex<Option<AtWork>>,
    /// How many rungs down the ladder this session has already gone.
    ///
    /// One way only: a card that refused this film refuses it the same way
    /// again, and trying a rung twice would cost a viewer two waits to reach
    /// the same place.
    stepped_down: std::sync::atomic::AtomicUsize,
    /// When this session was last asked for anything. A session nobody is
    /// watching any more is swept away, tool and folder together.
    touched: Mutex<Instant>,
}

/// What a session is called, which also names its folder.
pub type SessionId = melyxar_core::id::SessionId;

impl Session {
    /// Opens a session and prepares its folder. Nothing is produced yet: the
    /// first segment asked for is what starts the tool.
    pub async fn open(
        id: SessionId,
        recipe: Recipe,
        folder: PathBuf,
        tools: ToolPaths,
    ) -> Result<Self> {
        tokio::fs::create_dir_all(&folder).await?;
        Ok(Self {
            id,
            playlist: Playlist::new(recipe.duration),
            recipe,
            folder,
            tools,
            running: Mutex::new(None),
            stepped_down: std::sync::atomic::AtomicUsize::new(0),
            touched: Mutex::new(Instant::now()),
        })
    }

    /// What the picture is being rebuilt with right now.
    fn video_now(&self) -> melyxar_ffmpeg::command::VideoOutput {
        let rungs_down = self.stepped_down.load(Ordering::SeqCst);
        match rungs_down.checked_sub(1) {
            None => self.recipe.video.clone(),
            Some(index) => self
                .recipe
                .if_the_card_refuses
                .get(index)
                .cloned()
                .unwrap_or_else(|| self.recipe.video.clone()),
        }
    }

    /// Whether this failure is worth one more try, asking less of the card.
    ///
    /// Only a tool that refused: a machine that was merely slow would be just
    /// as slow the second time, and a segment outside the film is not there
    /// whoever rebuilds it.
    fn step_down_from_the_card(&self, error: &StreamingError) -> bool {
        if !matches!(error, StreamingError::MediaTool(_)) {
            return false;
        }
        let rungs_down = self.stepped_down.fetch_add(1, Ordering::SeqCst);
        if rungs_down >= self.recipe.if_the_card_refuses.len() {
            return false;
        }
        tracing::error!(
            session = %self.id,
            reason = %error,
            rung = rungs_down + 1,
            of = self.recipe.if_the_card_refuses.len(),
            "the card would not rebuild this film that way; asking it for less"
        );
        true
    }

    pub fn playlist(&self) -> &Playlist {
        &self.playlist
    }

    pub fn folder(&self) -> &Path {
        &self.folder
    }

    /// The header every segment needs.
    pub async fn initialisation(&self) -> Result<PathBuf> {
        let path = self.folder.join("init.mp4");
        if path.exists() {
            return Ok(path);
        }
        // The header is written with the first segment, so asking for it
        // before anything has been produced starts the film from its
        // beginning, which is what the player is about to ask for anyway.
        self.segment(0).await?;
        Ok(path)
    }

    /// How far the preparation has got.
    ///
    /// Read from the folder rather than remembered, because the folder is what
    /// is actually true: a count kept alongside would drift the first time a
    /// tool died between two segments.
    pub async fn preparation(&self) -> Preparation {
        let at_work = {
            let mut running = self.running.lock().await;
            running.as_mut().map(|at_work| {
                (
                    at_work.from,
                    at_work.reached(),
                    at_work.process.has_exited(),
                )
            })
        };
        let Some((from, reached, tool_gone)) = at_work else {
            return Preparation {
                step: PreparationStep::Starting,
                ready: 0,
                wanted: self.enough_from(0),
            };
        };

        // Counted as a run rather than a total: a segment on its own with a
        // hole before it does not let a film start.
        let mut on_disk = 0;
        while self.path_of(from + on_disk).exists() {
            on_disk += 1;
        }

        let mut ready = 0;
        while ready < on_disk && self.finished_being_written(from + ready, reached, tool_gone) {
            ready += 1;
        }

        let wanted = self.enough_from(from);
        Preparation {
            step: step_for(ready, wanted),
            ready,
            wanted,
        }
    }

    /// Whether the tool has finished writing one segment.
    ///
    /// Three ways of knowing the same thing, cheapest and soonest first. The
    /// tool's own account is the one that matters: a file that is merely there
    /// may still be growing, and a truncated segment breaks playback in a way
    /// nobody can read. Waiting for the next file to appear says the same
    /// thing, and it is what this used to rely on alone, at the price of
    /// producing a whole extra segment before handing over the one somebody is
    /// waiting for.
    fn finished_being_written(&self, index: u32, reached: Millis, tool_gone: bool) -> bool {
        reached >= self.playlist.start_of(index + 1)
            || self.path_of(index + 1).exists()
            || tool_gone
    }

    /// How many segments make a comfortable start from here.
    ///
    /// The usual handful, or whatever is left of the film when the viewer
    /// landed near the end: waiting for six segments of a film with three left
    /// would be waiting for ever.
    fn enough_from(&self, index: u32) -> u32 {
        WORTH_WAITING_FOR.min(self.playlist.segment_count().saturating_sub(index))
    }

    /// Hands over one segment, producing it if it is not there yet.
    pub async fn segment(&self, index: u32) -> Result<PathBuf> {
        self.touch().await;

        if index >= self.playlist.segment_count() {
            return Err(StreamingError::NoSuchSegment);
        }

        let path = self.path_of(index);
        if path.exists() {
            return Ok(path);
        }

        self.make_sure_someone_is_producing(index).await?;
        match self.wait_for(&path, index).await {
            Ok(()) => Ok(path),
            Err(error) if self.step_down_from_the_card(&error) => {
                self.make_sure_someone_is_producing(index).await?;
                self.wait_for(&path, index).await?;
                Ok(path)
            }
            Err(error) => Err(error),
        }
    }

    /// Starts the tool at this segment, unless one is already on its way here.
    async fn make_sure_someone_is_producing(&self, index: u32) -> Result<()> {
        let mut running = self.running.lock().await;

        if let Some(at_work) = running.as_mut() {
            // A tool that has finished is started again wherever the request
            // is: there is nothing on its way any more.
            if already_on_its_way(at_work.from, index) && !at_work.process.has_exited() {
                return Ok(());
            }
        }

        // Either nothing is running, or what is running is working on another
        // part of the film. A viewer who jumped is not waiting for the piece
        // in between to be produced first.
        if let Some(at_work) = running.take() {
            at_work.process.stop().await?;
        }

        let command = self.command_from(index);

        // The tool is asked to say where it has got to, and one task keeps the
        // latest answer. That answer is what lets a finished segment be handed
        // over the moment it is finished, rather than once the next one has
        // been produced on top of it.
        let reached = Arc::new(AtomicI64::new(0));
        let (reports, mut incoming) =
            tokio::sync::mpsc::channel::<melyxar_ffmpeg::process::Progress>(4);
        let mirror = reached.clone();
        tokio::spawn(async move {
            while let Some(report) = incoming.recv().await {
                mirror.store(report.position.get(), Ordering::Relaxed);
            }
        });

        let process = RunningProcess::start(&self.tools.ffmpeg, &command, Some(reports))?;
        tracing::debug!(session = %self.id, index, "producing from here");
        *running = Some(AtWork {
            process,
            from: index,
            reached,
        });
        Ok(())
    }

    fn command_from(&self, index: u32) -> Command {
        let start = self.playlist.start_of(index);
        let mut input = Input::new(&self.recipe.source);
        if index > 0 {
            input = input.starting_at(start);
        }

        Command::new(
            input,
            Output::Segments {
                pattern: self.folder.join("segment-%d.m4s"),
                initialisation: self.folder.join("init.mp4"),
                tool_playlist: self.folder.join("tool.m3u8"),
                duration: self.playlist.segment,
                start_number: index,
            },
        )
        .with_streams(self.recipe.streams)
        .with_video(self.video_now())
        .with_audio(self.recipe.audio.clone())
        // Asked for so that a segment can be handed over as soon as the tool
        // says it has passed the end of it.
        .reporting_progress()
    }

    /// Waits for a file to appear, giving up rather than hanging for ever.
    async fn wait_for(&self, path: &Path, index: u32) -> Result<()> {
        let deadline = Instant::now() + PATIENCE;
        loop {
            let (reached, tool_is_gone) = self.where_the_tool_has_got_to().await;
            if path.exists() && self.finished_being_written(index, reached, tool_is_gone) {
                return Ok(());
            }

            // The tool is gone and what was asked for is not there. Waiting
            // out the deadline only delays the same answer, and answers it
            // with the wrong reason: a tool that died on its first frame
            // looked exactly like a machine that was merely slow.
            if tool_is_gone {
                return Err(self.why_nothing_came(index).await);
            }

            if Instant::now() >= deadline {
                return Err(StreamingError::TooSlow);
            }
            tokio::time::sleep(LOOK_AGAIN_EVERY).await;
        }
    }

    /// What the tool said before it stopped without producing a segment.
    ///
    /// Its error output is the only place the answer lives, and nothing was
    /// reading it: the process was left where it was and the wait ran to its
    /// deadline. Reading it is the difference between a fix and an evening.
    async fn why_nothing_came(&self, index: u32) -> StreamingError {
        let mut running = self.running.lock().await;
        let stopped = running
            .as_mut()
            .is_some_and(|at_work| at_work.process.has_exited());
        let at_work = stopped.then(|| running.take()).flatten();
        drop(running);

        let Some(at_work) = at_work else {
            tracing::error!(
                session = %self.id,
                index,
                "a segment was asked for, nothing is producing it, and nothing said why"
            );
            return StreamingError::TooSlow;
        };

        match at_work.process.wait().await {
            // It ran to its end and this segment is still not there, which
            // means it was never going to produce it.
            Ok(()) => {
                tracing::error!(
                    session = %self.id,
                    index,
                    "the media tool finished without producing this segment"
                );
                StreamingError::TooSlow
            }
            Err(error) => {
                tracing::error!(
                    session = %self.id,
                    index,
                    reason = %error,
                    "the media tool stopped without producing this segment"
                );
                StreamingError::MediaTool(error)
            }
        }
    }

    /// How far the tool says it has written, and whether it is still there.
    ///
    /// Both under one lock: asking twice would let the tool exit between the
    /// two answers, which is exactly the moment a segment finishes.
    async fn where_the_tool_has_got_to(&self) -> (Millis, bool) {
        let mut running = self.running.lock().await;
        match running.as_mut() {
            Some(at_work) => (at_work.reached(), at_work.process.has_exited()),
            None => (Millis::ZERO, true),
        }
    }

    fn path_of(&self, index: u32) -> PathBuf {
        self.folder.join(format!("segment-{index}.m4s"))
    }

    async fn touch(&self) {
        *self.touched.lock().await = Instant::now();
    }

    /// How long nobody has asked this session for anything.
    pub async fn idle_for(&self) -> Duration {
        self.touched.lock().await.elapsed()
    }

    /// Stops the tool and removes the folder.
    ///
    /// Both, always: a tool left running after a viewer closed the tab is a
    /// core of the machine spent on nobody, and a folder left behind fills a
    /// disk one film at a time.
    pub async fn close(&self) {
        if let Some(at_work) = self.running.lock().await.take() {
            if let Err(error) = at_work.process.stop().await {
                tracing::warn!(session = %self.id, error = %error, "a media tool would not stop");
            }
        }
        if let Err(error) = tokio::fs::remove_dir_all(&self.folder).await {
            if error.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!(
                    session = %self.id,
                    error = %error,
                    "the working folder of a session could not be removed"
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_ffmpeg::command::{AudioOutput, VideoOutput};

    /// A real clip, since what is being tested is what a real tool produces.
    async fn clip(path: &Path, seconds: u32) {
        clip_with_key_frames_every(path, seconds, 24).await
    }

    /// The same, with a say in how far apart its key frames are.
    ///
    /// How far apart they are decides where a tool can start reading, so a
    /// clip with one every second hides every fault that only shows on a real
    /// film, where they are seconds or tens of seconds apart.
    async fn clip_with_key_frames_every(path: &Path, seconds: u32, frames: u32) {
        let made = tokio::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc2=size=320x180:rate=24:duration={seconds}"),
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:duration={seconds}"),
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-g",
                &frames.to_string(),
                "-sc_threshold",
                "0",
                "-c:a",
                "aac",
                "-shortest",
            ])
            .arg(path)
            .output()
            .await
            .expect("the tool runs");
        assert!(made.status.success(), "the clip was made");
    }

    async fn session_of(directory: &Path, seconds: u32) -> Session {
        let source = directory.join("source.mp4");
        clip(&source, seconds).await;

        Session::open(
            SessionId::new(),
            Recipe {
                source,
                duration: Millis::new(i64::from(seconds) * 1000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Copy,
                if_the_card_refuses: Vec::new(),
            },
            directory.join("session"),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens")
    }

    #[tokio::test]
    async fn a_session_nobody_has_asked_anything_of_has_not_started() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 40).await;

        let waiting = session.preparation().await;
        assert_eq!(waiting.step, PreparationStep::Starting);
        assert_eq!(waiting.ready, 0);
        assert_eq!(waiting.wanted, WORTH_WAITING_FOR);
        session.close().await;
    }

    #[tokio::test]
    async fn a_film_produced_to_its_end_is_ready() {
        // Twelve seconds is three segments, fewer than makes a comfortable
        // start anywhere else: waiting for six of them would be waiting for
        // ever.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 12).await;
        assert_eq!(session.playlist().segment_count(), 3);

        session.segment(0).await.expect("the first segment");
        for _ in 0..100 {
            if session.preparation().await.step == PreparationStep::Ready {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        let ready = session.preparation().await;
        assert_eq!(ready.step, PreparationStep::Ready, "{ready:?}");
        assert_eq!(ready.wanted, 3, "the whole of what is left: {ready:?}");
        assert_eq!(ready.ready, 3, "{ready:?}");
        session.close().await;
    }

    /// When the first picture and the first sound of a segment happen, in
    /// milliseconds on the clock of the film.
    async fn when_it_starts(folder: &Path, segment: &Path) -> (f64, f64) {
        let whole = folder.join("readable.mp4");
        let mut bytes = std::fs::read(folder.join("init.mp4")).expect("the header is there");
        bytes.extend(std::fs::read(segment).expect("the segment is there"));
        std::fs::write(&whole, bytes).expect("written");

        let first_of = async |stream: &str| {
            let output = tokio::process::Command::new("ffprobe")
                .args([
                    "-v",
                    "error",
                    "-select_streams",
                    stream,
                    "-show_entries",
                    "packet=pts_time",
                    "-of",
                    "csv=p=0",
                ])
                .arg(&whole)
                .output()
                .await
                .expect("the analyser runs");
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .next()
                .and_then(|line| line.trim().parse::<f64>().ok())
                .expect("a first packet with a time on it")
        };
        (first_of("v:0").await, first_of("a:0").await)
    }

    #[tokio::test]
    async fn a_jump_hands_out_a_segment_whose_sound_belongs_to_its_picture() {
        // The defect this exists for: a copied picture can only begin at a key
        // frame, so the tool rewound to the one before the jump, and then
        // trimmed the sound to the jump itself because the sound was being
        // rebuilt and could start anywhere. One segment, a picture from one
        // moment and a sound from another, ten seconds apart on a film with
        // key frames ten seconds apart.
        //
        // The commonest film of all takes this path: a picture every browser
        // reads and a soundtrack none of them do.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("film.mkv");
        // Key frames ten seconds apart, as a real film has them. One a second
        // hides the whole thing.
        clip_with_key_frames_every(&source, 60, 240).await;

        let folder = directory.path().join("session");
        let session = Session::open(
            SessionId::new(),
            Recipe {
                source,
                duration: Millis::new(60_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Encode(melyxar_ffmpeg::command::AudioEncode::browser_stereo(
                    "aac",
                )),
                if_the_card_refuses: Vec::new(),
            },
            folder.clone(),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens");

        // Five segments of four seconds in: twenty seconds, which is halfway
        // between two key frames and therefore the worst case.
        let segment = session.segment(5).await.expect("the segment is produced");
        let (picture, sound) = when_it_starts(&folder, &segment).await;

        assert!(
            (picture - sound).abs() < 0.2,
            "the sound of a segment has to belong to its picture: \
             picture at {picture}s, sound at {sound}s"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_tool_that_cannot_read_the_film_says_so_instead_of_being_waited_out() {
        // What a viewer meets when a film cannot be converted at all. The tool
        // dies on its first frame, and the wait used to run to its deadline
        // and then blame the machine for being slow, throwing away the one
        // thing worth reading.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("not-a-film.mkv");
        std::fs::write(&source, b"this is not a film at all").expect("file written");

        let session = Session::open(
            SessionId::new(),
            Recipe {
                source,
                duration: Millis::new(40_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Copy,
                if_the_card_refuses: Vec::new(),
            },
            directory.path().join("session"),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens");

        let started = Instant::now();
        let outcome = session.segment(0).await;
        let waited = started.elapsed();

        match outcome {
            Err(StreamingError::MediaTool(error)) => {
                let said = error.to_string();
                assert!(
                    said.len() > "the media tool failed: ".len(),
                    "what the tool said has to travel with the failure: {said}"
                );
            }
            other => panic!("the tool cannot read this and the failure must say so: {other:?}"),
        }
        assert!(
            waited < PATIENCE,
            "a tool that is already gone is not worth waiting {PATIENCE:?} for, waited {waited:?}"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_card_that_will_not_have_this_film_hands_it_to_the_processor_instead() {
        // The card was proved at start-up on a generated picture, which is the
        // right way round and still does not prove every film: a driver
        // refuses a size, a colour layout, a film nobody thought of. Left
        // alone that refusal reaches a viewer as a black screen.
        //
        // Asked for here by naming an encoder the tool does not have, which is
        // the same refusal from the tool's point of view and needs no card to
        // provoke.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        clip(&source, 12).await;

        let refused = melyxar_ffmpeg::command::VideoEncode {
            encoder: "h264_a_card_this_machine_does_not_have".to_string(),
            ..melyxar_ffmpeg::command::VideoEncode::software_h264()
        };
        let session = Session::open(
            SessionId::new(),
            Recipe {
                source,
                duration: Millis::new(12_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Encode(refused),
                audio: AudioOutput::Copy,
                if_the_card_refuses: vec![VideoOutput::Encode(
                    melyxar_ffmpeg::command::VideoEncode::software_h264(),
                )],
            },
            directory.path().join("session"),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens");

        let segment = session
            .segment(0)
            .await
            .expect("a refused card costs a restart, never the film");
        assert!(segment.exists());
        assert_eq!(
            session.stepped_down.load(Ordering::SeqCst),
            1,
            "one rung down, and it stays there rather than asking again every segment"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_film_nothing_can_rebuild_is_still_refused_rather_than_retried_for_ever() {
        // The other half of the same rule: stepping down is one restart, not a
        // loop. Nothing here can read the file, so the processor fails exactly
        // as the card did.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("not-a-film.mkv");
        std::fs::write(&source, b"this is not a film at all").expect("file written");

        let session = Session::open(
            SessionId::new(),
            Recipe {
                source,
                duration: Millis::new(12_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Encode(melyxar_ffmpeg::command::VideoEncode::software_h264()),
                audio: AudioOutput::Copy,
                if_the_card_refuses: vec![VideoOutput::Encode(
                    melyxar_ffmpeg::command::VideoEncode::software_h264(),
                )],
            },
            directory.path().join("session"),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens");

        let started = Instant::now();
        assert!(session.segment(0).await.is_err());
        assert!(
            started.elapsed() < PATIENCE,
            "two refusals are two refusals, not two waits"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_segment_is_handed_over_without_producing_the_one_after_it_first() {
        // The whole point of asking the tool where it has got to. Waiting for
        // the next file to appear says the same thing and costs a whole extra
        // segment, which on a jump is the difference between one wait and two.
        //
        // Made slow on purpose: a machine that produces four seconds of film
        // in a tenth of a second would have written the next one either way,
        // and the test would pass without testing anything.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        clip(&source, 60).await;

        let mut encode = melyxar_ffmpeg::command::VideoEncode::software_h264();
        encode.how = melyxar_ffmpeg::command::Rebuilding::InSoftware {
            quality: 18,
            preset: "veryslow".to_string(),
        };
        encode.scale_to_height = Some(720);
        encode.keyframe_interval = Some(crate::playlist::SEGMENT_DURATION);

        let session = Session::open(
            SessionId::new(),
            Recipe {
                source,
                duration: Millis::new(60_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Encode(encode),
                audio: AudioOutput::Copy,
                if_the_card_refuses: Vec::new(),
            },
            directory.path().join("session"),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens");

        session.segment(0).await.expect("the first segment");
        let next_one = session.folder().join("segment-1.m4s");

        assert!(
            !next_one.exists() || session.where_the_tool_has_got_to().await.1,
            "a segment was handed over only once the one after it had been \
             produced on top of it, which is twice the wait for nothing"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_viewer_landing_near_the_end_is_not_made_to_wait_for_what_is_not_there() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 40).await;
        assert_eq!(session.playlist().segment_count(), 10);

        assert_eq!(session.enough_from(0), WORTH_WAITING_FOR);
        assert_eq!(
            session.enough_from(8),
            2,
            "two segments left, so two is all there is to wait for"
        );
        assert_eq!(session.enough_from(10), 0);
        session.close().await;
    }

    #[test]
    fn every_step_is_one_a_viewer_can_actually_land_on() {
        // A machine that copies faster than it can be watched goes from
        // nothing to ready in under a second, so the two steps in between are
        // only ever seen on a slow one. They still have to be right.
        assert_eq!(step_for(0, 6), PreparationStep::Reading);
        assert_eq!(step_for(1, 6), PreparationStep::Producing);
        assert_eq!(step_for(5, 6), PreparationStep::Producing);
        assert_eq!(step_for(6, 6), PreparationStep::Ready);
        assert_eq!(step_for(80, 6), PreparationStep::Ready);

        // A viewer who landed on the last segment of the film waits for that
        // one and nothing else.
        assert_eq!(step_for(0, 1), PreparationStep::Reading);
        assert_eq!(step_for(1, 1), PreparationStep::Ready);
        assert_eq!(
            step_for(0, 0),
            PreparationStep::Ready,
            "nothing left to wait for is not a wait"
        );
    }

    #[tokio::test]
    async fn a_segment_is_finished_the_moment_the_tool_says_it_passed_the_end_of_it() {
        // What this replaced waited for the next file to appear, which says
        // the same thing and costs a whole extra segment before the one
        // somebody is waiting for can be handed over. On a jump that is the
        // difference between one wait and two.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 60).await;
        let start_of = |index: u32| session.playlist().start_of(index);

        assert!(
            !session.finished_being_written(3, start_of(3), false),
            "the tool is inside this segment, so it is still writing it"
        );
        assert!(
            session.finished_being_written(3, start_of(4), false),
            "it has passed the end of it, so it has closed it"
        );
        assert!(
            session.finished_being_written(3, Millis::ZERO, true),
            "nothing is writing any more, so nothing is growing"
        );

        // The old signal still counts, for a tool that has not spoken yet.
        std::fs::write(session.folder().join("segment-4.m4s"), b"a segment")
            .expect("the next one is written");
        assert!(session.finished_being_written(3, Millis::ZERO, false));
        session.close().await;
    }

    #[tokio::test]
    async fn the_first_segment_asked_for_is_what_starts_the_tool() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 12).await;

        assert!(
            std::fs::read_dir(session.folder())
                .expect("the folder is there")
                .next()
                .is_none(),
            "opening a session produces nothing: someone may never press play"
        );

        let segment = session.segment(0).await.expect("the first segment");
        assert!(segment.exists());
        assert!(
            session.folder().join("init.mp4").exists(),
            "the header every segment needs comes with the first one"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_segment_already_produced_is_handed_over_without_producing_it_again() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 12).await;

        let first = session.segment(0).await.expect("the first segment");
        let written_at = std::fs::metadata(&first)
            .expect("the segment is there")
            .modified()
            .expect("a modification time");

        let again = session.segment(0).await.expect("the same segment");
        assert_eq!(first, again);
        assert_eq!(
            std::fs::metadata(&again)
                .expect("the segment is there")
                .modified()
                .expect("a modification time"),
            written_at,
            "a segment on disk is handed over, not made a second time"
        );
        session.close().await;
    }

    /// Where the tool running right now was started.
    async fn producing_from(session: &Session) -> Option<u32> {
        session
            .running
            .lock()
            .await
            .as_ref()
            .map(|at_work| at_work.from)
    }

    #[tokio::test]
    async fn jumping_ahead_starts_the_tool_where_the_viewer_landed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 60).await;

        session.segment(0).await.expect("the first segment");
        assert_eq!(producing_from(&session).await, Some(0));

        // The rule is asked directly rather than through a request for the
        // segment. Copying a short clip finishes in well under a second, so a
        // request that far ahead usually finds the file already on the disk
        // and rightly hands it over without restarting anything: going through
        // it would test how fast this machine is, not what the rule says.
        session
            .make_sure_someone_is_producing(12)
            .await
            .expect("the tool starts again");
        assert_eq!(
            producing_from(&session).await,
            Some(12),
            "a viewer dragging the cursor towards the end must not wait for \
             everything in between to be produced first"
        );

        let landed = session.segment(12).await.expect("the segment landed on");
        assert!(landed.exists());
        session.close().await;
    }

    #[test]
    fn what_is_nearly_ready_is_waited_for_and_the_rest_is_a_jump() {
        // The other half of the same rule, asked as the question it is.
        // Restarting costs a second or two and throws away work already done,
        // so a request for what is nearly ready waits.
        assert!(already_on_its_way(0, 0), "the one being produced");
        assert!(already_on_its_way(0, WORTH_WAITING_FOR - 1), "nearly there");
        assert!(
            !already_on_its_way(0, WORTH_WAITING_FOR),
            "beyond that, the viewer has jumped"
        );
        assert!(!already_on_its_way(0, 400), "the end of a long film");

        // The tool only moves forward, so anything behind it is a viewer who
        // went back and will never be reached by waiting.
        assert!(!already_on_its_way(10, 9));
        assert!(!already_on_its_way(10, 0));
        assert!(already_on_its_way(10, 12), "the window travels with it");
    }

    /// Where a produced segment says it belongs in the film, read back from
    /// the file itself with its header.
    async fn clock_of(session: &Session, index: u32) -> f64 {
        let assembled = session.folder().join("assembled.mp4");
        let mut bytes = std::fs::read(session.folder().join("init.mp4")).expect("the header");
        bytes.extend(
            std::fs::read(session.folder().join(format!("segment-{index}.m4s")))
                .expect("the segment"),
        );
        std::fs::write(&assembled, bytes).expect("written");

        let read = tokio::process::Command::new("ffprobe")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-show_entries",
                "format=start_time",
                "-of",
                "csv=p=0",
            ])
            .arg(&assembled)
            .output()
            .await
            .expect("the tool runs");
        String::from_utf8_lossy(&read.stdout)
            .trim()
            .parse()
            .expect("a start time")
    }

    #[tokio::test]
    async fn a_film_rebuilt_whole_lands_on_the_second_the_playlist_says() {
        // How far apart the key frames are decides where the tool can start
        // reading, and a real film has one every several seconds rather than
        // every second. A stream being rebuilt is decoded from the key frame
        // before the point asked for and the frames before it are dropped, so
        // the segment lands where it says however far apart they are. A stream
        // being copied cannot do that, which the test below measures.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        clip_with_key_frames_every(&source, 60, 240).await;

        let mut encode = melyxar_ffmpeg::command::VideoEncode::software_h264();
        encode.keyframe_interval = Some(crate::playlist::SEGMENT_DURATION);
        let session = Session::open(
            SessionId::new(),
            Recipe {
                source,
                duration: Millis::new(60_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Encode(encode),
                audio: AudioOutput::Encode(melyxar_ffmpeg::command::AudioEncode::browser_stereo(
                    "aac",
                )),
                if_the_card_refuses: Vec::new(),
            },
            directory.path().join("session"),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens");

        // Twelve segments in is forty eight seconds, which sits between two
        // key frames of this clip rather than on one.
        session.segment(12).await.expect("the segment landed on");
        let announced = clock_of(&session, 12).await;
        let expected = session.playlist().start_of(12).as_seconds_f64();
        assert!(
            (announced - expected).abs() < 0.5,
            "the playlist says this segment covers second {expected}, and it \
             announces itself at {announced}"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_stream_being_copied_can_only_start_where_its_key_frames_are() {
        // Not a fault to fix here, a limit to know, and the one that decides
        // what a jump feels like on most films: a stream carried over
        // untouched has to start on one of its own key frames, so a jump lands
        // on the one before the point asked for. The viewer sees the film from
        // slightly earlier and it plays on correctly from there.
        //
        // What it costs is exactly how far apart those key frames are: six
        // seconds, measured on a film with one every ten. It applies to any
        // stream that is copied, so it covers repackaging and rebuilding the
        // sound alone as well. Writing the playlist on the real key frames
        // instead of on a fixed grid is the way out, and it needs the analysis
        // to record where they are, which is a decision of its own.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        clip_with_key_frames_every(&source, 60, 240).await;

        let session = Session::open(
            SessionId::new(),
            Recipe {
                source,
                duration: Millis::new(60_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Copy,
                if_the_card_refuses: Vec::new(),
            },
            directory.path().join("session"),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens");

        session.segment(12).await.expect("the segment landed on");
        let announced = clock_of(&session, 12).await;
        let expected = session.playlist().start_of(12).as_seconds_f64();
        let early = expected - announced;
        assert!(
            (0.0..=10.5).contains(&early),
            "a copy starts on a key frame at or before the point asked for, \
             and this clip has one every ten seconds: asked for {expected}, \
             got {announced}"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_segment_produced_after_a_jump_says_where_in_the_film_it_belongs() {
        // The defect this guards against is silent: the segment plays, and a
        // player places it at the beginning of the film because that is what
        // its own clock says. Seeking then lands somewhere else entirely.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 60).await;

        session.segment(4).await.expect("a segment part way in");
        let announced = clock_of(&session, 4).await;
        let expected = session.playlist().start_of(4).as_seconds_f64();
        assert!(
            (announced - expected).abs() < 1.0,
            "the playlist says this segment covers second {expected}, and it \
             announces itself at {announced}"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn the_next_segment_is_waited_for_rather_than_started_again() {
        // The ordinary case, and the one that must not restart anything: a
        // player asks for the segment after the one it is playing. Starting
        // the tool again there would throw away work already under way and
        // cost a stutter every few seconds.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 60).await;

        session.segment(0).await.expect("the first segment");
        session.segment(1).await.expect("the next one");
        assert_eq!(
            producing_from(&session).await,
            Some(0),
            "the tool that was already on its way here kept going"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_segment_that_is_not_part_of_the_film_is_refused_rather_than_waited_for() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 12).await;

        assert!(matches!(
            session.segment(999).await,
            Err(StreamingError::NoSuchSegment)
        ));
        session.close().await;
    }

    #[tokio::test]
    async fn closing_a_session_leaves_no_tool_and_no_folder_behind() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 40).await;
        session.segment(0).await.expect("the first segment");

        let pid = session
            .running
            .lock()
            .await
            .as_ref()
            .and_then(|at_work| at_work.process.id());

        session.close().await;

        assert!(
            !session.folder().exists(),
            "a folder left behind fills a disk one film at a time"
        );
        if let Some(pid) = pid {
            let alive = std::process::Command::new("kill")
                .args(["-0", &pid.to_string()])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .expect("the check runs")
                .success();
            assert!(
                !alive,
                "a tool left running after a viewer closed the tab is a core spent on nobody"
            );
        }
    }
}
