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
use std::time::{Duration, Instant, SystemTime};

use melyxar_core::time::Millis;
use melyxar_ffmpeg::command::{Command, Input, Output, StreamSelection, WhereToCut};
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

/// How many steps of exactly one segment backwards, one after another, are a
/// player walking rather than a viewer jumping.
///
/// A viewer drags the cursor and lands anywhere; a player that asks for the
/// segment immediately before the one the tool was just set going on is
/// reaching behind what it holds, and a player that does it twice running is
/// walking backwards and will keep going. Read in the maintainer's journal:
/// sixty five of those in twenty two seconds, each one stopping the tool and
/// starting it again for a single segment, and nothing to watch throughout.
///
/// The first step is answered where it asks, because it costs one segment and
/// may be all the player wanted. From the second on, the run covers the
/// stretch being walked into, so the steps after it are already on the disk.
const A_WALK_BACKWARDS: u32 = 2;

/// How long a segment is waited for before the answer is that it is too slow.
///
/// Long enough for a slow first segment on a busy machine, short enough that a
/// player is not left hanging on a tool that died without saying so.
const PATIENCE: Duration = Duration::from_secs(30);

/// How often the folder is looked at while waiting.
///
/// Fine on purpose. A wait ends the moment a file appears or the tool says it
/// has passed the end of one, and looking on a slow rhythm does not make the
/// wait shorter, it rounds it up to the next look: read in the maintainer's
/// journal, every single wait came out a multiple of the old hundred and
/// twenty thousandths, a segment already finished still costing one whole look
/// before it was handed over. A look is a lock, two numbers and a glance at the
/// folder, which is nothing beside producing film.
const LOOK_AGAIN_EVERY: Duration = Duration::from_millis(10);

/// How long a request for the header waits for its own segment request before
/// setting the tool going itself.
///
/// A player asks for both in the same breath and the segment is what chooses
/// where the tool starts, so the header gives it a beat to arrive. Kept as a
/// length of time rather than as a number of looks: how often the folder is
/// looked at is about noticing a file quickly and has nothing to say about how
/// long two requests of one player take to arrive.
const A_PLAYER_ASKS_FOR_BOTH_WITHIN: Duration = Duration::from_millis(120);

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
    /// Where the viewer is about to begin.
    ///
    /// Carried because the header every segment needs is written with the
    /// first segment produced, whichever one that is. Without this it was the
    /// first segment of the film, produced in full for nobody: the tool was
    /// then stopped and started again where the viewer really was, and that
    /// whole detour sat in front of the picture.
    pub where_the_viewer_starts: Millis,
    /// Where this film can really be started, in order, when it has been read
    /// for them.
    ///
    /// Only ever given for a picture carried over untouched, because that is
    /// the only case where it changes anything: a picture the server rebuilds
    /// gets a key frame on every boundary, put there by the server itself.
    /// Empty means the usual grid, which is what a film nobody has read for
    /// them gets.
    pub where_it_can_be_started: Vec<Millis>,
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
///
/// No longer compared for exact equality, because it carries a rate now and a
/// rate is not a whole number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Preparation {
    pub step: PreparationStep,
    /// Segments on the disk, counted from where the tool was started.
    pub ready: u32,
    /// How many make a comfortable start.
    pub wanted: u32,
    /// What the tool at work is doing this second, when one is at work.
    pub producing: Option<Producing>,
}

/// How hard the machine is working on this film, as the tool tells it.
///
/// Asked for by the maintainer and shown by every media server there is, for
/// a reason: it is the one number that says whether the machine is coping.
/// The two together, because neither alone answers it. Pictures a second is
/// the work being done and means nothing without knowing the film; the speed
/// is that work against what watching it costs, and one is exactly keeping up.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Producing {
    /// Pictures a second the tool says it is writing just now.
    pub pictures_a_second: f64,
    /// The same work against real time. Below one and the picture will stop.
    pub speed: f64,
}

/// Whether a tool that has written as far as `written_to` will reach `index`
/// soon enough that waiting beats starting it again.
///
/// Measured from where the tool has got to, never from where it was set going.
/// Those are the same thing only for the first few seconds of a run: measured
/// from the start, the window closed after six segments however far the tool
/// had come, so ordinary playback stopped the tool and started it again every
/// six segments, for the segment it was on the point of writing. Seen in a
/// journal: the tool writing the five hundred and nineteenth, the player asking
/// for the five hundred and twentieth, and the whole thing begun again from a
/// standing start for one and a half seconds.
///
/// Behind is never worth waiting for: the tool only moves forward, so a request
/// for something it has already passed is a viewer who went back.
fn already_on_its_way(from: u32, written_to: u32, index: u32) -> bool {
    index >= from && index < written_to.max(from) + WORTH_WAITING_FOR
}

/// How many steps backwards in a row this request makes, given the run it is
/// about to replace.
///
/// One step is the segment immediately before where that run was set going.
/// Anything else, forwards or a real jump, is not a step and starts the count
/// again.
fn steps_back_after(from: u32, steps_back: u32, index: u32) -> u32 {
    match index + 1 == from {
        true => steps_back + 1,
        false => 0,
    }
}

/// Where to set the tool going for a request that has made this many steps
/// backwards in a row.
///
/// Where it asks, until the steps say the player is walking. From there the
/// run begins a handful earlier, so that the segment asked for and the ones
/// the player is about to ask for all come out of one run. It costs the walk
/// one longer wait and saves it every restart after that.
fn where_to_begin(index: u32, steps_back: u32) -> u32 {
    match steps_back >= A_WALK_BACKWARDS {
        true => index.saturating_sub(WORTH_WAITING_FOR),
        false => index,
    }
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

/// Whether a segment file holds whole boxes from its beginning to its end.
///
/// How a finished segment is told from one cut off in the middle, without
/// knowing anything about who wrote it or what they are doing now: a segment
/// is a run of boxes, each naming its own length, and the last of them ends
/// exactly where the file does. Walks the lengths rather than the bytes, so it
/// costs a handful of eight byte reads whatever the segment weighs.
async fn every_box_is_whole(path: &Path) -> bool {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let (Ok(mut file), Ok(length)) = (
        tokio::fs::File::open(path).await,
        tokio::fs::metadata(path).await.map(|it| it.len()),
    ) else {
        return false;
    };

    let mut at = 0u64;
    let mut header = [0u8; 8];
    while at < length {
        if file.seek(std::io::SeekFrom::Start(at)).await.is_err()
            || file.read_exact(&mut header).await.is_err()
        {
            return false;
        }
        let box_length = u64::from(u32::from_be_bytes([
            header[0], header[1], header[2], header[3],
        ]));
        // Nothing a segment holds uses the two lengths that mean anything
        // other than themselves, and a length of nothing would not move.
        if box_length < 8 {
            return false;
        }
        at += box_length;
    }
    at == length && length > 0
}

/// Who last wrote a segment file, told from one run's point of view.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum WhoWrote {
    /// There is no such file.
    Nobody,
    /// The run being asked about, so what it is doing now applies to the file.
    ThisRun,
    /// A run that has since been stopped, so the tool at work now is not
    /// touching the file. Whether that run got to the end of it is a separate
    /// question, which the file's own shape answers.
    AnEarlierRun,
}

/// The tool at work, and where it started.
struct AtWork {
    process: RunningProcess,
    /// The segment number it was started on, so a request can tell whether it
    /// is ahead of the tool or somewhere else entirely.
    from: u32,
    /// How many steps of one segment backwards, one after another, led to this
    /// run being started.
    ///
    /// Carried from run to run because that is the only place the shape of a
    /// walk lives: each request on its own is just a segment somewhere else in
    /// the film.
    steps_back: u32,
    /// The moment it was set going, which is what tells its own files apart
    /// from the ones an earlier run left in the same folder.
    ///
    /// A run writes its segments one after another from where it started, and
    /// so did the run before it, so the numbers alone say nothing about who
    /// wrote what. The only mark a file carries is when it was written.
    began_at: SystemTime,
    /// Whether what this run produced has already been read back and said.
    was_read_back: bool,
    /// How far into the film the tool says it has written, in milliseconds.
    ///
    /// The tool's own account of itself rather than anything read off the
    /// disk. A file that exists may still be growing, and the only way to tell
    /// without asking the tool is to wait for the next one to appear, which
    /// means producing a whole extra segment before handing over the one
    /// somebody is waiting for. On a jump that is the difference between one
    /// wait and two.
    reached: Arc<AtomicI64>,
    /// How fast the tool says it is working, as thousandths of real time.
    ///
    /// A whole number because it lives in the same place as the position, read
    /// from another task while a viewer waits. Below a thousand means the
    /// machine is producing the film more slowly than somebody can watch it,
    /// which is the one thing that turns into stuttering rather than into a
    /// wait at the start.
    speed: Arc<AtomicI64>,
    /// How many pictures a second the tool says it is writing, in thousandths.
    ///
    /// Beside the speed rather than instead of it: the speed says whether the
    /// machine is keeping up with somebody watching, and this says what it is
    /// actually doing to get there. A film at twenty five pictures a second
    /// produced at forty is the same machine as one at fifty produced at
    /// eighty, and only one of the two numbers says which film it is.
    pictures: Arc<AtomicI64>,
    /// How long the tool took to say anything at all, in milliseconds, or a
    /// negative number while it has not.
    ///
    /// It says nothing until it has opened the film, found its way to the
    /// point asked for and begun producing. On a jump into a large file that
    /// is a real part of the wait, and it is a different part from producing
    /// the segment: one is attacked by asking the tool to look at less of the
    /// file, the other only by producing faster.
    opening: Arc<AtomicI64>,
}

/// Where the task reading the tool's reports puts what it reads.
///
/// Named rather than a handful of counters in a tuple: the task that writes
/// them and the session that reads them are pages apart, and a number nobody
/// can name from where it is read is a number nobody checks.
struct Mirrored {
    reached: Arc<AtomicI64>,
    speed: Arc<AtomicI64>,
    pictures: Arc<AtomicI64>,
    opening: Arc<AtomicI64>,
}

impl AtWork {
    /// How far into the film the tool says it has written.
    fn reached(&self) -> Millis {
        Millis::new(self.reached.load(Ordering::Relaxed))
    }

    /// How fast the tool says it is working, relative to real time.
    fn speed(&self) -> f64 {
        self.speed.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// How many pictures a second the tool says it is writing.
    fn pictures_a_second(&self) -> f64 {
        self.pictures.load(Ordering::Relaxed) as f64 / 1000.0
    }

    /// How long the tool took to say anything, when it has.
    fn opening(&self) -> Option<u128> {
        match self.opening.load(Ordering::Relaxed) {
            silent if silent < 0 => None,
            said => Some(said as u128),
        }
    }
}

/// What had to happen before a segment could be handed over.
///
/// Named slices rather than one total. A total says a jump is slow, which
/// whoever jumped already knows; the slices say which part to attack, and they
/// live in four different places: the tool being stopped, the tool being
/// started, the tool reading its way to the point asked for, and the segment
/// being finished once it exists.
#[derive(Debug, Default, Clone, Copy)]
struct WhatItTook {
    stopping: Duration,
    starting: Duration,
    appearing: Duration,
    settling: Duration,
}

/// One film being watched.
pub struct Session {
    pub id: SessionId,
    /// Whose session this is.
    ///
    /// A session is reached by its name alone, and its name is enough to be
    /// handed every segment of what is being watched. Written down here so
    /// that being signed in is not the same thing as being signed in as the
    /// person who opened it.
    pub watcher: melyxar_core::id::UserId,
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

/// A watcher for the tests here and next door, which are about what a session
/// produces and how the registry holds it rather than about whose it is.
///
/// Drawn once and the same afterwards, so that a session opened for it is
/// found again by it.
#[cfg(test)]
pub(crate) static NOBODY_IN_PARTICULAR: std::sync::LazyLock<melyxar_core::id::UserId> =
    std::sync::LazyLock::new(melyxar_core::id::UserId::new);

impl Session {
    /// Opens a session and prepares its folder. Nothing is produced yet: the
    /// first segment asked for is what starts the tool.
    pub async fn open(
        id: SessionId,
        watcher: melyxar_core::id::UserId,
        recipe: Recipe,
        folder: PathBuf,
        tools: ToolPaths,
    ) -> Result<Self> {
        tokio::fs::create_dir_all(&folder).await?;
        Ok(Self {
            id,
            watcher,
            playlist: match recipe.where_it_can_be_started.is_empty() {
                true => Playlist::on_a_fixed_grid(recipe.duration),
                false => {
                    Playlist::on_these_boundaries(recipe.duration, &recipe.where_it_can_be_started)
                }
            },
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

    /// The playlist a player is handed, which says where to begin.
    pub fn playlist_text(&self) -> String {
        self.playlist.to_text(self.recipe.where_the_viewer_starts)
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
        tracing::debug!(session = %self.id, "the header was asked for");
        self.wait_for_the_header(&path).await?;
        Ok(path)
    }

    /// Waits for the header, produced by whoever the tool ends up working for.
    ///
    /// The header never picks a part of the film of its own. A player asks for
    /// it and for the segment it belongs to at the same moment, and it is that
    /// segment which sets the tool going; the header is the same bytes whatever
    /// is produced. Choosing as well is how the two requests of one player came
    /// to disagree about where the film starts, and each then waited out its
    /// patience for what the other had taken the tool away from.
    ///
    /// A player that asked for the header alone is still answered: a beat
    /// later, with nothing running, the tool is set going where the viewer is.
    async fn wait_for_the_header(&self, path: &Path) -> Result<()> {
        let deadline = Instant::now() + PATIENCE;
        let give_the_segment_until = Instant::now() + A_PLAYER_ASKS_FOR_BOTH_WITHIN;
        loop {
            let at_work = {
                let mut running = self.running.lock().await;
                running
                    .as_mut()
                    .map(|at_work| (at_work.from, at_work.process.has_exited()))
            };

            match at_work {
                Some((from, tool_is_gone)) => {
                    // A header that merely exists may still be being written,
                    // and half a header is not something a browser can say
                    // anything useful about: it refuses the film outright. The
                    // tool closes the header before opening the first segment
                    // of its run, so that segment appearing is what says so.
                    if path.exists() && self.path_of(from).exists() {
                        return Ok(());
                    }
                    // The tool is gone and the header is still not whole.
                    // Waiting out the patience answers the same thing far
                    // later, and with the wrong reason.
                    if tool_is_gone {
                        return Err(self.why_nothing_came(from).await);
                    }
                }
                None if Instant::now() >= give_the_segment_until => {
                    self.make_sure_someone_is_producing(self.where_the_viewer_starts())
                        .await?;
                }
                None => {}
            }

            if Instant::now() >= deadline {
                return Err(StreamingError::TooSlow);
            }
            tokio::time::sleep(LOOK_AGAIN_EVERY).await;
        }
    }

    /// Which segment the tool at work has written as far as.
    ///
    /// From the tool's own account of itself, which counts from where it was
    /// set going rather than from the beginning of the film. A picture carried
    /// over untouched is deliberately let to begin early and nothing here knows
    /// how early, so for those this reads a segment or two further along than
    /// the truth. That way round on purpose: the wait it decides is then a
    /// little longer than it needed to be, never shorter, and the tool is on
    /// its way there either way.
    fn written_to(&self, at_work: &AtWork) -> u32 {
        let written = self.playlist.start_of(at_work.from).get() + at_work.reached().get().max(0);
        self.playlist.segment_holding(Millis::new(written))
    }

    /// Which segment the viewer is about to watch.
    fn where_the_viewer_starts(&self) -> u32 {
        self.playlist
            .segment_holding(self.recipe.where_the_viewer_starts)
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
                    at_work.pictures_a_second(),
                    at_work.speed(),
                    at_work.process.has_exited(),
                )
            })
        };
        let Some((from, pictures_a_second, speed, tool_gone)) = at_work else {
            return Preparation {
                step: PreparationStep::Starting,
                ready: 0,
                wanted: self.enough_from(self.where_the_viewer_starts()),
                producing: None,
            };
        };

        // Counted as a run rather than a total: a segment on its own with a
        // hole before it does not let a film start.
        let mut ready = 0;
        while self.finished_being_written(from + ready).await {
            ready += 1;
        }

        let wanted = self.enough_from(from);
        Preparation {
            step: step_for(ready, wanted),
            ready,
            wanted,
            // Nothing until the tool has spoken, and nothing once it has
            // finished. A rate of nothing is what a tool that has not started
            // looks like, and the last rate a tool gave before it reached the
            // end of the film is not what the machine is doing now: both would
            // be read as work going on this second.
            producing: (!tool_gone && (pictures_a_second > 0.0 || speed > 0.0)).then_some(
                Producing {
                    pictures_a_second,
                    speed,
                },
            ),
        }
    }

    /// Removes what a tool left half written when it was stopped where it
    /// stood.
    ///
    /// A segment whose end the tool had not reached was still being written,
    /// and a file that is merely there is handed over on sight. Without this,
    /// jumping back to that second of the film later serves a truncated
    /// segment, which breaks playback in a way nobody can read. It is exactly
    /// what asking the tool politely used to buy, at a fraction of the price.
    ///
    /// Exactly one file is ever at risk, and it is the newest one this run
    /// wrote: the tool closes a segment before it opens the next, so a file
    /// with a successor of its own is finished by definition. Which file that
    /// is comes from the folder and not from the tool's account of itself,
    /// because the account arrives every tenth of a second while the tool
    /// works at thirty times real time, and taking it at its word threw away
    /// whole minutes of finished film on every jump.
    ///
    /// Being the newest is not on its own a reason to remove it: a run that
    /// reached the end of the film, or that closed the segment it was on an
    /// instant before it was stopped, left a whole file there. The file's own
    /// shape is what says which it is, as it does everywhere else here, and
    /// the tool is already stopped by the time it is asked, so nothing is
    /// growing under the question. Read in the maintainer's journal: a run
    /// that had produced the film to its last segment, stopped, and the last
    /// segment of the film taken away as if it had been caught mid write.
    async fn remove_what_was_half_written(&self, from: u32, began_at: SystemTime) {
        let mut newest = None;
        let mut index = from;
        while self.who_wrote(index, began_at).await == WhoWrote::ThisRun {
            newest = Some(index);
            index += 1;
        }
        let Some(newest) = newest else {
            return;
        };
        if self.finished_being_written(newest).await {
            return;
        }

        if let Err(error) = tokio::fs::remove_file(self.path_of(newest)).await {
            tracing::warn!(
                session = %self.id,
                index = newest,
                error = %error,
                "a half written segment could not be removed, so it is left to be produced again"
            );
            return;
        }
        // Said out loud, because it is a removal nobody asked for: a player
        // that asks for it again finds it gone. Silently, that is a film
        // stopping in the middle of itself with nothing anywhere to say why.
        tracing::debug!(
            session = %self.id,
            index = newest,
            the_tool_was_set_going_at = from,
            "the segment the stopped tool was inside was taken away"
        );
    }

    /// Who last wrote a segment, as far as one run is concerned.
    ///
    /// A run set going in the middle of the film lands in the middle of what an
    /// earlier run finished there and writes straight over the front of it, so
    /// the numbers alone say nothing about who wrote what. A file this run
    /// never touched still carries the moment the other one wrote it, which is
    /// before this one was set going.
    async fn who_wrote(&self, index: u32, began_at: SystemTime) -> WhoWrote {
        match tokio::fs::metadata(self.path_of(index)).await {
            Err(_) => WhoWrote::Nobody,
            Ok(there) => match there.modified() {
                Ok(written) if written >= began_at => WhoWrote::ThisRun,
                // A moment nobody can read is treated as this run's, which
                // costs a wait rather than a segment handed over half written.
                Err(_) => WhoWrote::ThisRun,
                Ok(_) => WhoWrote::AnEarlierRun,
            },
        }
    }

    /// Whether a segment is finished and can be handed over.
    ///
    /// The file's own shape answers it, and nothing else does. Who wrote it,
    /// and whether a tool is at work this second, change nothing: a segment
    /// either ends where its last box ends or it is still being written.
    ///
    /// The tool's own account of itself used to answer this, and it turns out
    /// it cannot. Measured: the tool holds a whole segment in memory and writes
    /// it out in one burst once it has passed the end of it, half a megabyte at
    /// a time. So the file only ever exists at all while the tool is already
    /// saying it is past the end, and for as long as that burst lasts the file
    /// stops in the middle of its own picture data. On a busy machine the burst
    /// is long enough to be caught: what went to the browser was a fifth of a
    /// segment, and the film stopped dead and began again from the beginning.
    ///
    /// The other old answer, waiting for the next file to appear, is sound and
    /// slow for the same reason: the tool opens that file only once it has a
    /// whole segment of its own to write into it, which is a segment of work
    /// later. Walking the boxes answers at the first moment the answer is yes.
    async fn finished_being_written(&self, index: u32) -> bool {
        every_box_is_whole(&self.path_of(index)).await
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
    ///
    /// A file that is merely there may be one the tool is writing this very
    /// second, and a segment cut off in the middle is handed to a browser that
    /// shows the beginning of it and then nothing at all until the next one.
    /// Reported from a real library: a jump forward landed exactly where the
    /// tool had got to, and the picture stood still for five seconds with the
    /// sound running on, ending on the very second the next segment began.
    /// It is the same fault the subtitles had and the same answer: what is
    /// handed over is whole or it is waited for.
    pub async fn segment(&self, index: u32) -> Result<PathBuf> {
        self.touch().await;

        if index >= self.playlist.segment_count() {
            return Err(StreamingError::NoSuchSegment);
        }

        let path = self.path_of(index);
        if self.finished_being_written(index).await {
            // The ordinary case, and the one the journal used to say nothing
            // about: a film playing on writes nothing at all, so the order a
            // player asked for its segments in could not be read anywhere. It
            // is the first thing wanted when a film stops in the middle of
            // itself, and one line per segment is a handful a minute.
            tracing::debug!(
                session = %self.id,
                index,
                at_second = self.playlist.start_of(index).as_seconds_f64(),
                "a segment was handed over from what is already there"
            );
            self.say_how_the_streams_came_out(index).await;
            return Ok(path);
        }

        let started = self.make_sure_someone_is_producing(index).await?;
        let waited = match self.wait_for(&path, index).await {
            Ok(waited) => waited,
            Err(error) if self.step_down_from_the_card(&error) => {
                self.make_sure_someone_is_producing(index).await?;
                self.wait_for(&path, index).await?
            }
            Err(error) => return Err(error),
        };

        self.say_what_it_took(index, started, waited).await;
        self.say_how_the_streams_came_out(index).await;
        Ok(path)
    }

    /// Reads back where the picture and the sound of a run really begin.
    ///
    /// The file says where each of its streams starts, and so does what comes
    /// out of the tool, and the two are not obliged to agree: a film whose
    /// sound begins seconds after its picture can be served with the gap
    /// preserved, with the gap closed, or with the sound moved, and each of
    /// the three looks completely different to somebody watching. Nothing
    /// anywhere said which of them happened.
    ///
    /// Read off the first piece the run produced, once per run, and only for
    /// somebody reading the journal closely. A piece of a fragmented film says
    /// nothing on its own: it carries the sound and the picture but not what
    /// they are, so it is read behind the header that does, which is the same
    /// pair a player is given.
    async fn say_how_the_streams_came_out(&self, index: u32) {
        if !tracing::enabled!(tracing::Level::DEBUG) || !self.is_the_first_of_its_run(index).await {
            return;
        }

        let read = match self.read_back(index).await {
            Ok(report) => report,
            Err(error) => {
                tracing::debug!(
                    session = %self.id,
                    index,
                    error = %error,
                    "what the run produced could not be read back"
                );
                return;
            }
        };

        let lining_up = melyxar_media_probe::HowTheStreamsLineUp::of(&read);
        tracing::debug!(
            session = %self.id,
            index,
            at_second = self.playlist.start_of(index).as_seconds_f64(),
            video_starts_at_ms = lining_up.video_starts_at,
            audio_starts_at_ms = lining_up.audio_starts_at,
            sound_after_picture_ms = lining_up.offset(),
            "where the picture and the sound of what the tool produced begin"
        );
    }

    /// Whether this is the piece the run at work was set going on, asked once.
    ///
    /// Marked as it answers, so a piece handed over again says nothing a
    /// second time.
    async fn is_the_first_of_its_run(&self, index: u32) -> bool {
        let mut running = self.running.lock().await;
        match running.as_mut() {
            Some(at_work) if at_work.from == index && !at_work.was_read_back => {
                at_work.was_read_back = true;
                true
            }
            _ => false,
        }
    }

    /// Reads one piece of the film back, behind the header that describes it.
    async fn read_back(&self, index: u32) -> Result<melyxar_ffmpeg::probe::ProbeReport> {
        let whole = self.folder.join(format!("read-back-{index}.mp4"));
        let header = tokio::fs::read(self.folder.join("init.mp4")).await?;
        let piece = tokio::fs::read(self.path_of(index)).await?;
        tokio::fs::write(&whole, [header, piece].concat()).await?;

        let read = melyxar_ffmpeg::probe::probe(&self.tools.ffprobe, &whole).await;
        // Removed whatever the reading said: it exists for one line of the
        // journal, and a folder of these would be a cache filling up for
        // nobody.
        tokio::fs::remove_file(&whole).await.ok();
        Ok(read?)
    }

    /// Writes down where the wait for one segment actually went.
    ///
    /// Only when the tool had to be set going, which is a film starting or a
    /// viewer jumping: those are the waits anybody notices, and one line every
    /// four seconds of ordinary playback would bury them. The ordinary case is
    /// still written down, quietly, for a session being looked at closely.
    async fn say_what_it_took(&self, index: u32, started: Option<WhatItTook>, waited: WhatItTook) {
        let (speed, opening) = {
            let running = self.running.lock().await;
            match running.as_ref() {
                Some(at_work) => (Some(at_work.speed()), at_work.opening()),
                None => (None, None),
            }
        };
        let Some(getting_going) = started else {
            tracing::debug!(
                session = %self.id,
                index,
                appearing_ms = waited.appearing.as_millis(),
                settling_ms = waited.settling.as_millis(),
                speed,
                "waited for a segment already on its way"
            );
            return;
        };

        let total =
            getting_going.stopping + getting_going.starting + waited.appearing + waited.settling;
        tracing::info!(
            session = %self.id,
            index,
            at_second = self.playlist.start_of(index).as_seconds_f64(),
            stopping_ms = getting_going.stopping.as_millis(),
            starting_ms = getting_going.starting.as_millis(),
            opening_ms = opening,
            appearing_ms = waited.appearing.as_millis(),
            settling_ms = waited.settling.as_millis(),
            total_ms = total.as_millis(),
            speed,
            "a segment was produced from a standing start"
        );
    }

    /// Starts the tool at this segment, unless one is already on its way here.
    ///
    /// Answers nothing when one already was, and how long each part of getting
    /// one going took when it was not.
    async fn make_sure_someone_is_producing(&self, index: u32) -> Result<Option<WhatItTook>> {
        let mut running = self.running.lock().await;

        if let Some(at_work) = running.as_mut() {
            let from = at_work.from;
            let written_to = self.written_to(at_work);
            let gone = at_work.process.has_exited();
            let waiting = already_on_its_way(from, written_to, index) && !gone;
            // Every decision to wait or to begin again, with the three numbers
            // it was taken on. This is the choice that costs a viewer one wait
            // or two, and it used to leave nothing behind at all: a tool
            // started again showed up in the journal as a slow segment, which
            // is not the same thing and sends anybody reading it elsewhere.
            tracing::debug!(
                session = %self.id,
                index,
                set_going_at = from,
                written_to,
                tool_gone = gone,
                decision = if waiting { "wait" } else { "begin again" },
                "a segment was asked for while the tool was at work"
            );
            // A tool that has finished is started again wherever the request
            // is: there is nothing on its way any more.
            if waiting {
                return Ok(None);
            }
        }

        // Either nothing is running, or what is running is working on another
        // part of the film. A viewer who jumped is not waiting for the piece
        // in between to be produced first.
        let steps_back = match running.as_ref() {
            Some(at_work) => steps_back_after(at_work.from, at_work.steps_back, index),
            None => 0,
        };
        let begin_at = where_to_begin(index, steps_back);

        let stopping = Instant::now();
        if let Some(at_work) = running.take() {
            // Stopped where it stands rather than asked to finish. Everything
            // it was doing is worthless the moment somebody jumps elsewhere,
            // and measured on a real film, asking politely cost the best part
            // of a second of the viewer's wait, every time. What it leaves
            // half written is cleared away instead, which costs a file
            // removal.
            let (was_at, began_at) = (at_work.from, at_work.began_at);
            at_work.process.stop_now().await?;
            self.remove_what_was_half_written(was_at, began_at).await;
        }
        let stopping = stopping.elapsed();

        let starting = Instant::now();
        let command = self.command_from(begin_at);

        // The tool is asked to say where it has got to, and one task keeps the
        // latest answer. That answer is what lets a finished segment be handed
        // over the moment it is finished, rather than once the next one has
        // been produced on top of it. How fast it says it is working rides
        // along, because a jump that is slow because the machine cannot keep
        // up and a jump that is slow because the tool spent the time opening a
        // large file are two different problems.
        let reached = Arc::new(AtomicI64::new(0));
        let speed = Arc::new(AtomicI64::new(0));
        let pictures = Arc::new(AtomicI64::new(0));
        let opening = Arc::new(AtomicI64::new(-1));
        let (reports, mut incoming) =
            tokio::sync::mpsc::channel::<melyxar_ffmpeg::process::Progress>(4);
        let mirrored = Mirrored {
            reached: reached.clone(),
            speed: speed.clone(),
            pictures: pictures.clone(),
            opening: opening.clone(),
        };
        let began = Instant::now();
        tokio::spawn(async move {
            while let Some(report) = incoming.recv().await {
                // The first word out of the tool is the moment it stopped
                // opening the film and started producing it.
                let _ = mirrored.opening.compare_exchange(
                    -1,
                    began.elapsed().as_millis() as i64,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
                mirrored
                    .reached
                    .store(report.position.get(), Ordering::Relaxed);
                if let Some(rate) = report.speed {
                    mirrored
                        .speed
                        .store((rate * 1000.0) as i64, Ordering::Relaxed);
                }
                if let Some(rate) = report.pictures_a_second {
                    mirrored
                        .pictures
                        .store((rate * 1000.0) as i64, Ordering::Relaxed);
                }
            }
        });

        // Read before the tool exists, so that every file it goes on to write
        // is at or after it and none of the ones already there can be.
        let began_at = SystemTime::now();
        // Every argument, said once per run. What a film is served as is
        // decided in a dozen places, and a film that plays wrong is a question
        // about the one command that produced it: without it, the answer has
        // to be guessed from the settings that were in force an hour ago.
        tracing::debug!(
            session = %self.id,
            index,
            arguments = ?command.to_arguments(),
            "the tool is being set going with these arguments"
        );
        let process = RunningProcess::start(&self.tools.ffmpeg, &command, Some(reports))?;
        tracing::debug!(
            session = %self.id,
            index,
            // Where the run begins, which is the segment asked for unless the
            // player is walking backwards and is being given the stretch it is
            // walking into.
            set_going_at = begin_at,
            steps_back,
            at_second = self.playlist.start_of(begin_at).as_seconds_f64(),
            // Not the same number: the tool is aimed at the middle of the
            // segment so that it lands on the one picture wanted. The two
            // being far apart would be a segment far longer than the rest.
            tool_aimed_at_second = self.set_going_at(begin_at).as_seconds_f64(),
            "producing from here"
        );
        *running = Some(AtWork {
            process,
            from: begin_at,
            steps_back,
            began_at,
            was_read_back: false,
            reached,
            speed,
            pictures,
            opening,
        });
        Ok(Some(WhatItTook {
            stopping,
            starting: starting.elapsed(),
            ..WhatItTook::default()
        }))
    }

    fn command_from(&self, index: u32) -> Command {
        let mut input = Input::new(&self.recipe.source);
        if index > 0 {
            input = input.starting_at(self.set_going_at(index));
        }

        Command::new(
            input,
            Output::Segments {
                pattern: self.folder.join("segment-%d.m4s"),
                initialisation: self.folder.join("init.mp4"),
                tool_playlist: self.folder.join("tool.m3u8"),
                cut: self.where_to_cut(),
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

    /// Where the tool is to cut, which follows from who chose the boundaries.
    ///
    /// A picture the server rebuilds is given a starting point on every
    /// boundary of the grid the playlist was written on, so the tool is asked
    /// for that grid. A picture carried over untouched has its starting points
    /// wherever its encoder left them, the playlist is cut at all of them, and
    /// the tool is asked for all of them too.
    fn where_to_cut(&self) -> WhereToCut {
        match self.playlist.cut_where_the_film_allows() {
            true => WhereToCut::AtEveryKeyFrame,
            false => WhereToCut::Every(crate::playlist::SEGMENT_DURATION),
        }
    }

    /// Where the tool is set going so that it begins on one exact segment.
    ///
    /// The middle of it rather than its beginning, for a film cut where its
    /// own pictures allow. Asked for the very moment a picture stands on its
    /// own, the tool serves the one before it: measured, and it left every
    /// segment of the reading one place behind where the playlist said it was.
    /// The middle of a segment is past the picture wanted and short of the
    /// next one, so it names one and only one.
    ///
    /// A rebuilt picture starts exactly where it is asked to, and asking for
    /// the middle of a segment would begin it in the middle.
    fn set_going_at(&self, index: u32) -> Millis {
        let start = self.playlist.start_of(index);
        if !self.playlist.cut_where_the_film_allows() {
            return start;
        }
        let ends_at = self.playlist.start_of(index + 1).get();
        Millis::new(start.get() + (ends_at - start.get()) / 2)
    }

    /// Waits for a file to appear, giving up rather than hanging for ever.
    ///
    /// Answers how the wait divided: how long until the segment was there at
    /// all, and how long from there until it was finished. The first is the
    /// tool opening the film and reading its way to the point asked for, the
    /// second is the segment itself, and they are attacked in different ways.
    async fn wait_for(&self, path: &Path, index: u32) -> Result<WhatItTook> {
        let began = Instant::now();
        let deadline = began + PATIENCE;
        let mut appeared: Option<Instant> = None;
        loop {
            let tool_is_gone = self.the_tool_is_gone().await;
            if path.exists() && appeared.is_none() {
                appeared = Some(Instant::now());
            }
            if self.finished_being_written(index).await {
                let appeared = appeared.unwrap_or(began);
                return Ok(WhatItTook {
                    appearing: appeared.saturating_duration_since(began),
                    settling: appeared.elapsed(),
                    ..WhatItTook::default()
                });
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

    /// Whether nothing is producing for this session any more.
    ///
    /// Read before the segment it is about, never after: a tool that is still
    /// there when asked and finishes the segment a moment later is waited for
    /// one more beat, where the other way round a segment that never came would
    /// be reported as a tool still working on it.
    async fn the_tool_is_gone(&self) -> bool {
        let mut running = self.running.lock().await;
        match running.as_mut() {
            Some(at_work) => at_work.process.has_exited(),
            None => true,
        }
    }

    fn path_of(&self, index: u32) -> PathBuf {
        self.folder.join(format!("segment-{index}.m4s"))
    }

    async fn touch(&self) {
        *self.touched.lock().await = Instant::now();
    }

    /// Says that somebody still has this film open, though they are asking
    /// for nothing.
    ///
    /// A session is kept alive by being used, which is what a film playing
    /// does every few seconds. A film paused asks for nothing at all, and
    /// without this it is swept away with the viewer sitting in front of it:
    /// they press play, every segment answers that the session is over, and
    /// the film is lost where they left it.
    pub async fn still_watching(&self) {
        self.touch().await;
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

    /// A clip whose pictures stand on their own at the moments given.
    ///
    /// The shape a real film has and the made up ones above do not: places
    /// close together, because a cut in a scene puts one there, and long
    /// stretches with none. It is the only shape that tells a rule the tool
    /// shares from one it does not.
    async fn clip_with_key_frames_at(path: &Path, seconds: u32, at: &[f64]) {
        let forced = at
            .iter()
            .map(|moment| format!("{moment:.3}"))
            .collect::<Vec<_>>()
            .join(",");
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
                // Far enough apart that nothing but the list below puts one in.
                "-g",
                "10000",
                "-sc_threshold",
                "0",
                "-force_key_frames",
                &forced,
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
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(i64::from(seconds) * 1000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Copy,
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: Vec::new(),
                if_the_card_refuses: Vec::new(),
            },
            directory.join("session"),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens")
    }

    /// How many pictures a second every made up clip here holds.
    ///
    /// The one place it is written down, so a clip made at another rate cannot
    /// quietly turn a count of pictures into the wrong number of seconds.
    const PICTURES_A_SECOND: f64 = 24.0;

    /// A moment comfortably before anything a test has just written, so that
    /// every file in the folder counts as the run's own.
    fn a_moment_ago() -> SystemTime {
        SystemTime::now() - Duration::from_secs(60)
    }

    /// The shape of a segment a tool closed: one whole box, ending where the
    /// file does. Enough for anything that reads a segment's shape rather than
    /// its film.
    fn a_finished_segment() -> [u8; 8] {
        [0, 0, 0, 8, b'm', b'd', b'a', b't']
    }

    /// The shape of a segment caught halfway onto the disk: a box that names a
    /// length the file does not reach, which is what the tool pouring out a
    /// segment leaves behind for as long as the pouring lasts.
    fn a_segment_still_being_written() -> [u8; 12] {
        [0, 0, 0x40, 0, b'm', b'd', b'a', b't', 1, 2, 3, 4]
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
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(60_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Encode(melyxar_ffmpeg::command::AudioEncode::browser_stereo(
                    "aac",
                )),
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: Vec::new(),
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
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(40_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Copy,
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: Vec::new(),
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
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(12_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Encode(refused),
                audio: AudioOutput::Copy,
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: Vec::new(),
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
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(12_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Encode(melyxar_ffmpeg::command::VideoEncode::software_h264()),
                audio: AudioOutput::Copy,
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: Vec::new(),
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
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(60_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Encode(encode),
                audio: AudioOutput::Copy,
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: Vec::new(),
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
            !next_one.exists() || session.the_tool_is_gone().await,
            "a segment was handed over only once the one after it had been \
             produced on top of it, which is twice the wait for nothing"
        );
        session.close().await;
    }

    /// The commonest film in a personal collection: a picture every browser
    /// reads, carried over untouched, and a soundtrack none of them do.
    async fn film_with_a_soundtrack_no_browser_reads(path: &Path, seconds: u32) {
        let made = tokio::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                &format!("testsrc2=size=1280x720:rate=24:duration={seconds}"),
                "-f",
                "lavfi",
                "-i",
                &format!("sine=frequency=440:duration={seconds}"),
                "-c:v",
                "libx264",
                "-preset",
                "ultrafast",
                "-g",
                "96",
                "-c:a",
                "eac3",
                "-ac",
                "6",
                "-shortest",
            ])
            .arg(path)
            .output()
            .await
            .expect("the tool runs");
        assert!(made.status.success(), "the film was made");
    }

    #[tokio::test]
    async fn the_commonest_film_of_all_hands_over_whole_segments() {
        // A picture carried over untouched is produced faster than anything
        // can be watched: the tool reports having passed the end of a segment
        // within a few milliseconds of starting. If a segment is handed over
        // on that word alone while the file is still being written, the viewer
        // gets a black screen and a browser that says only that it could not
        // play the film.
        //
        // Run several times over, because what is being caught is a race and
        // one pass proves nothing about the next.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("film.mkv");
        film_with_a_soundtrack_no_browser_reads(&source, 60).await;

        for attempt in 0..3 {
            let session = Session::open(
                SessionId::new(),
                *NOBODY_IN_PARTICULAR,
                Recipe {
                    source: source.clone(),
                    duration: Millis::new(60_000),
                    streams: StreamSelection::default(),
                    video: VideoOutput::Copy,
                    audio: AudioOutput::Encode(
                        melyxar_ffmpeg::command::AudioEncode::browser_stereo("aac"),
                    ),
                    where_the_viewer_starts: Millis::ZERO,
                    where_it_can_be_started: Vec::new(),
                    if_the_card_refuses: Vec::new(),
                },
                directory.path().join(format!("session-{attempt}")),
                ToolPaths::discover(None, None).expect("the tools are installed here"),
            )
            .await
            .expect("the session opens");

            for index in 0..3 {
                let segment = session.segment(index).await.expect("a segment");
                let whole = session.folder().join(format!("readable-{index}.mp4"));
                let mut bytes =
                    std::fs::read(session.folder().join("init.mp4")).expect("the header");
                bytes.extend(std::fs::read(&segment).expect("the segment"));
                std::fs::write(&whole, bytes).expect("written");

                // Counted rather than read off the header. A fragment of a
                // film carries no length of its own, and the analyser answers
                // that it does not know: on a whole segment as readily as on a
                // cut one, so the answer proved nothing either way. What is
                // actually in it is the pictures, and those can be counted.
                let read = tokio::process::Command::new("ffprobe")
                    .args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-select_streams",
                        "v:0",
                        "-count_frames",
                        "-show_entries",
                        "stream=nb_read_frames",
                        "-of",
                        "csv=p=0",
                    ])
                    .arg(&whole)
                    .output()
                    .await
                    .expect("the analyser runs");
                // Read strictly, because the answer here is an accusation. An
                // analyser that would not answer at all used to come back as a
                // segment holding no film whatsoever, which reads in the
                // failure as the very fault this is here to catch.
                let said = String::from_utf8_lossy(&read.stdout);
                let counted: f64 = said.trim().parse().unwrap_or_else(|error| {
                    panic!(
                        "attempt {attempt}, segment {index}: the analyser said nothing \
                         readable about it, so this proves nothing either way: {error}, \
                         it said {said:?} and complained {:?}",
                        String::from_utf8_lossy(&read.stderr)
                    )
                });
                let lasted = counted / PICTURES_A_SECOND;

                assert!(
                    lasted > 3.5,
                    "attempt {attempt}, segment {index} was handed over holding {lasted} \
                     seconds of a four second segment: a browser refuses that outright and \
                     says only that it could not play the film"
                );
            }
            session.close().await;
        }
    }

    #[tokio::test]
    async fn stopping_a_reading_leaves_alone_what_an_earlier_one_finished() {
        // Two readings of the same film, as a viewer jumping about produces.
        // The first runs ahead and finishes a block of segments. The second is
        // set going a long way behind it and is stopped part way, and what it
        // had not finished is cleared away. What the first one finished is not
        // its to clear away, and a viewer who goes back there finds the film
        // made again from nothing.
        //
        // Read in the maintainer's journal: twelve stops took a hundred and
        // sixty six segments away, of which at most twelve could have been
        // unfinished. One stop alone took eighty four, which is seven minutes
        // of film thrown out.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 120).await;
        let last = session.playlist().segment_count() - 1;

        // The first reading, set going part way in and left alone until it has
        // run out of film. Everything from there to the end is finished. The
        // wait is generous because it is a real tool producing a minute of
        // real film on a machine that may be busy with the rest of the suite.
        session.segment(10).await.expect("a segment part way in");
        for _ in 0..6_000 {
            if session.path_of(last).exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            session.path_of(last).exists(),
            "the first reading never finished, so there is nothing here to protect"
        );

        // A viewer goes back a few seconds, which lands just before the block
        // and reads over the front of it. Its own first segment is finished,
        // and the ones after it were already there.
        session.segment(9).await.expect("a segment just before");
        let still_there_before_the_stop = session.path_of(last - 1).exists();

        // Then anything at all elsewhere, which stops that reading where it
        // stands. What it had not finished goes; what it never wrote is not
        // its to take.
        session.segment(0).await.expect("back to the beginning");

        assert!(
            still_there_before_the_stop,
            "the block was already gone before the stop, so this proves nothing"
        );
        for kept in [15, last - 1] {
            assert!(
                session.path_of(kept).exists(),
                "segment {kept} was finished by the first reading and the stopping of the second took it away"
            );
        }
        session.close().await;
    }

    #[tokio::test]
    async fn a_tool_stopped_where_it_stood_leaves_nothing_half_written_behind() {
        // This is what asking the tool politely used to buy, and it cost the
        // best part of a second of the viewer's wait on every jump. A file
        // that is merely there is handed over on sight, so a segment the tool
        // had not finished would be served truncated to whoever jumped back
        // to that second of the film later.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 60).await;
        for index in 0..6 {
            std::fs::write(session.folder().join(format!("segment-{index}.m4s")), b"x")
                .expect("a segment");
        }

        // Set going at the beginning: the five it had opened and closed are
        // finished, and the sixth is the one it was inside.
        session
            .remove_what_was_half_written(0, a_moment_ago())
            .await;

        for finished in 0..5 {
            assert!(
                session
                    .folder()
                    .join(format!("segment-{finished}.m4s"))
                    .exists(),
                "segment {finished} has one after it, so the tool had closed it"
            );
        }
        assert!(
            !session.folder().join("segment-5.m4s").exists(),
            "the newest one is the one it was inside, and would be served truncated"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_run_set_going_far_in_leaves_the_beginning_of_the_film_alone() {
        // The beginning of the film is where a viewer goes back to, and the
        // removal starts where the run started rather than at the front of the
        // folder: nothing before that is any of its business.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 600).await;
        for index in [0, 1, 2, 100, 101, 102] {
            std::fs::write(session.folder().join(format!("segment-{index}.m4s")), b"x")
                .expect("a segment");
        }

        // Set going at the hundredth segment: it closed the hundredth and the
        // hundred and first, and was inside the next.
        session
            .remove_what_was_half_written(100, a_moment_ago())
            .await;

        for kept in [0, 1, 2, 100, 101] {
            assert!(
                session
                    .folder()
                    .join(format!("segment-{kept}.m4s"))
                    .exists(),
                "segment {kept} was finished, by this run or another, and must be kept"
            );
        }
        assert!(
            !session.folder().join("segment-102.m4s").exists(),
            "the one it was inside is the one that was never finished"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn the_removal_stops_at_the_first_file_this_run_never_wrote() {
        // Removing everything that follows in one unbroken line is removing
        // whatever an earlier run left further along, which it finished and
        // which a viewer going back there is about to ask for again.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 60).await;
        for index in 0..6 {
            std::fs::write(session.folder().join(format!("segment-{index}.m4s")), b"x")
                .expect("a segment an earlier run finished");
        }

        tokio::time::sleep(Duration::from_millis(20)).await;
        let began_at = SystemTime::now();
        tokio::time::sleep(Duration::from_millis(20)).await;

        // Set going at the beginning again, it got as far as the fifth segment
        // and was stopped inside it. The sixth is not its business.
        for index in 0..5 {
            std::fs::write(session.folder().join(format!("segment-{index}.m4s")), b"x")
                .expect("a segment this run wrote");
        }
        session.remove_what_was_half_written(0, began_at).await;

        assert!(
            !session.folder().join("segment-4.m4s").exists(),
            "the segment this run was inside would be served truncated"
        );
        assert!(
            session.folder().join("segment-5.m4s").exists(),
            "the earlier run finished this one and nothing here wrote it"
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
    async fn a_segment_is_finished_when_its_last_box_ends_where_the_file_does() {
        // Both answers this replaced were wrong. The tool's own account of
        // itself says it is past the end of a segment while it is still
        // pouring that segment onto the disk, so a fifth of a segment was
        // handed over and the browser refused the film outright. Reading the
        // file after it instead is true but a segment of work late, and it
        // broke a film of the maintainer's in its own way: a run set going
        // behind an earlier one rewrote the front of its block, and the
        // untouched file just past the one being rewritten was read as proof
        // that the rewriting had moved on.
        //
        // Told here rather than with the real tool, because the moment a
        // segment is on disk but not yet whole lasts as long as it takes to
        // write it: measured, under a millisecond for the hundred and forty
        // kilobyte segments of a made up clip, against tens of megabytes for a
        // film of a viewer's. A clip heavy enough to catch it would be heavier
        // than the rest of these tests put together.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 60).await;
        let write = |index: u32, bytes: &[u8]| {
            std::fs::write(session.folder().join(format!("segment-{index}.m4s")), bytes)
                .expect("a segment on disk")
        };

        assert!(
            !session.finished_being_written(3).await,
            "there is no such file, so nothing is finished"
        );

        write(3, &a_segment_still_being_written());
        assert!(
            !session.finished_being_written(3).await,
            "it stops in the middle of its own picture data, so it is still being written"
        );

        write(3, &a_finished_segment());
        assert!(
            session.finished_being_written(3).await,
            "its last box ends where the file does, so whoever wrote it got to the end"
        );
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
    async fn the_header_alone_sets_the_tool_going_where_the_viewer_is() {
        // A player that asks for the header alone, which no ordinary one does:
        // the header comes with the segment it belongs to. Nothing else is
        // going to set the tool going, so the header does it, and where the
        // viewer is rather than at the opening of a film nobody is at.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        clip(&source, 40).await;
        let session = Session::open(
            SessionId::new(),
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(40_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Copy,
                where_the_viewer_starts: Millis::new(20_000),
                where_it_can_be_started: Vec::new(),
                if_the_card_refuses: Vec::new(),
            },
            directory.path().join("session"),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens");

        let header = session.initialisation().await.expect("the header");
        assert!(header.exists());

        let wanted = session.playlist().segment_holding(Millis::new(20_000));
        assert!(wanted > 0, "twenty seconds in is not the opening of a film");
        assert!(
            session.path_of(wanted).exists(),
            "the header comes with the segment the viewer is about to watch"
        );
        assert!(
            !session.path_of(0).exists(),
            "nothing is produced at the opening of a film nobody is at"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn the_header_follows_the_segment_a_player_asks_for_beside_it() {
        // What a player really does: the header and the segment it belongs to,
        // at the same moment. The header picking a part of the film of its own
        // made the two requests of one player fight over the tool, and a
        // viewer changing a subtitle then waited out the patience for a
        // segment nobody was producing any more.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        clip(&source, 60).await;
        let session = Arc::new(
            Session::open(
                SessionId::new(),
                *NOBODY_IN_PARTICULAR,
                Recipe {
                    source,
                    duration: Millis::new(60_000),
                    streams: StreamSelection::default(),
                    video: VideoOutput::Copy,
                    audio: AudioOutput::Copy,
                    where_the_viewer_starts: Millis::new(48_000),
                    where_it_can_be_started: Vec::new(),
                    if_the_card_refuses: Vec::new(),
                },
                directory.path().join("session"),
                ToolPaths::discover(None, None).expect("the tools are installed here"),
            )
            .await
            .expect("the session opens"),
        );

        let asked_beside_it = {
            let session = session.clone();
            tokio::spawn(async move { session.segment(0).await })
        };
        // In the order a player really asks: the segment sets the tool going,
        // and the header arrives beside it.
        for _ in 0..1_000 {
            if session.running.lock().await.is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        let header = session.initialisation().await;
        let beside_it = asked_beside_it.await.expect("the request finished");

        assert!(header.is_ok(), "the header: {header:?}");
        assert!(beside_it.is_ok(), "the segment beside it: {beside_it:?}");
        // Where the tool was set going, which is the whole question. Looking
        // for the header's own segment on the disk would prove nothing: the
        // tool set going at the opening runs on through the film and reaches
        // it on its own, sooner or later depending on how busy the machine is.
        let set_going_at = session
            .running
            .lock()
            .await
            .as_ref()
            .map(|at_work| at_work.from);
        assert_eq!(
            set_going_at,
            Some(0),
            "the header takes what the player asked for and sets nothing going of its own"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn the_header_arrives_even_when_another_request_moves_the_tool() {
        // A player asks for the header and for a segment at the same moment,
        // and the two need not be about the same part of the film. Waiting for
        // the segment the header was fetched with meant waiting for something
        // the other request had just taken the tool away from: half a minute,
        // then a film that would not start at all.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        clip(&source, 60).await;
        let session = Arc::new(
            Session::open(
                SessionId::new(),
                *NOBODY_IN_PARTICULAR,
                Recipe {
                    source,
                    duration: Millis::new(60_000),
                    streams: StreamSelection::default(),
                    video: VideoOutput::Copy,
                    audio: AudioOutput::Copy,
                    where_the_viewer_starts: Millis::new(8_000),
                    where_it_can_be_started: Vec::new(),
                    if_the_card_refuses: Vec::new(),
                },
                directory.path().join("session"),
                ToolPaths::discover(None, None).expect("the tools are installed here"),
            )
            .await
            .expect("the session opens"),
        );

        // Far enough ahead that the tool is started again rather than waited
        // for, and never comes back past what the header was fetched with.
        let asking_elsewhere = {
            let session = session.clone();
            tokio::spawn(async move { session.segment(12).await })
        };
        let header = session.initialisation().await;
        let elsewhere = asking_elsewhere.await.expect("the request finished");

        assert!(header.is_ok(), "the header: {header:?}");
        assert!(elsewhere.is_ok(), "the other segment: {elsewhere:?}");
        assert!(
            every_box_is_whole(&header.expect("the header")).await,
            "half a header is not something a browser can say anything about: \
             it refuses the film outright"
        );
        session.close().await;
    }

    /// Whether a file of boxes ends exactly where its last box does.
    ///
    /// What "finished being written" means for a header: each box says its own
    /// length, so walking them lands on the end of the file when nothing was
    /// cut off and past it or short of it when something was.
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

    #[tokio::test]
    async fn a_segment_the_tool_has_not_finished_is_never_handed_over_half_made() {
        // A file that is merely there may be one the tool is writing this very
        // second. Handed over then, a browser shows the beginning of it and
        // then nothing at all until the next segment begins.
        //
        // Reported from a real library and read in the journal: a jump forward
        // landed where the tool had got to, the picture stood still for five
        // seconds with the sound running on, and it came back on the very
        // second the next segment started. It is the same fault the subtitles
        // had, and the same answer: what is handed over is whole or it is
        // waited for.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 120).await;

        // The tool set going at the beginning, so that something is at work.
        session.segment(0).await.expect("the first segment");

        // What a segment the tool is in the middle of writing looks like from
        // the outside: a file that is there, ahead of where the tool has got
        // to, with nothing after it. Far enough ahead that the tool cannot
        // have reached it already, which the next line makes sure of rather
        // than assumes.
        let waiting_for_it = 25;
        let half_made = session.path_of(waiting_for_it);
        std::fs::write(&half_made, b"not a whole segment").expect("a file that is merely there");
        assert!(
            !session.path_of(waiting_for_it + 1).exists(),
            "the tool had already gone past it, so there was nothing half made to catch"
        );

        let handed_over = session
            .segment(waiting_for_it)
            .await
            .expect("the segment is produced");
        assert_eq!(handed_over, half_made);
        assert!(
            every_box_is_whole(&handed_over).await,
            "what was handed over is the piece of film, not the beginning of it"
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

    /// Which segment the tool at work says it has written as far as.
    async fn written_to(session: &Session) -> u32 {
        let running = session.running.lock().await;
        running
            .as_ref()
            .map(|at_work| session.written_to(at_work))
            .unwrap_or(0)
    }

    /// A session the tool is still working on when a test asks it anything.
    ///
    /// Long, and rebuilt rather than carried over. Both matter: these tests ask
    /// what a tool at work does, and a short film copied over is finished
    /// before the question can be put. Answered then, the answer is the right
    /// one for a different question, which is a test that passes for the wrong
    /// reason on a slow machine and fails on a fast one.
    async fn a_session_still_at_work(directory: &Path) -> Session {
        let source = directory.join("source.mp4");
        clip(&source, 240).await;
        Session::open(
            SessionId::new(),
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(240_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Encode(melyxar_ffmpeg::command::VideoEncode::software_h264()),
                audio: AudioOutput::Copy,
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: Vec::new(),
                if_the_card_refuses: Vec::new(),
            },
            directory.join("session"),
            ToolPaths::discover(None, None).expect("the tools are installed here"),
        )
        .await
        .expect("the session opens")
    }

    /// Waits until the tool is past the window it was set going with, and says
    /// how far it has written. Fails the test if it finished first.
    async fn once_the_tool_is_under_way(session: &Session) -> u32 {
        for _ in 0..600 {
            if written_to(session).await > WORTH_WAITING_FOR {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let written = written_to(session).await;
        assert!(
            written > WORTH_WAITING_FOR,
            "the tool has to be past the window it was set going with for these \
             to be asking anything: it reached {written}"
        );

        let still_at_work = {
            let mut running = session.running.lock().await;
            running
                .as_mut()
                .is_some_and(|at_work| !at_work.process.has_exited())
        };
        assert!(
            still_at_work,
            "and it has to still be at work: a tool that has finished is \
             started again wherever the request is, which is another rule"
        );
        written
    }

    #[tokio::test]
    async fn watching_a_film_through_never_stops_the_tool_to_start_it_again() {
        // A player reads ahead of what it is showing, so it asks for segments
        // the tool has not written yet. Those are the cheapest wait there is,
        // the very next thing the tool will do. Measured from where the tool
        // was set going rather than from where it had got to, they fell outside
        // the window once the run was six segments old: the tool was stopped
        // and begun again from a standing start, every six segments, for the
        // whole film.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = a_session_still_at_work(directory.path()).await;

        session.segment(0).await.expect("the first segment");
        let written = once_the_tool_is_under_way(&session).await;

        // The very next segment, which is what a player asks for next.
        let started_again = session
            .make_sure_someone_is_producing(written + 1)
            .await
            .expect("the request is answered");

        assert!(
            started_again.is_none(),
            "the next segment the tool is about to write is waited for"
        );
        assert_eq!(
            producing_from(&session).await,
            Some(0),
            "and the tool is the one that was set going at the beginning"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn a_player_walking_backwards_stops_the_tool_once_and_not_once_a_segment() {
        // The freeze this comes from: a player asking for the segment before
        // the one the tool had just been set going on, over and over, sixty
        // five times running. Answered each one where it asked, that is sixty
        // five tools started and stopped for one segment each, and a film
        // frozen on the screen the whole time.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = a_session_still_at_work(directory.path()).await;

        // A viewer well into the film, which is where going back means
        // anything at all.
        session.segment(40).await.expect("the segment jumped to");
        assert_eq!(producing_from(&session).await, Some(40));

        // The rule is asked directly rather than through requests for the
        // segments: a request produces film, and how long this machine takes
        // to do that has nothing to say about where the tool is set going.
        session
            .make_sure_someone_is_producing(39)
            .await
            .expect("the first step back is answered");
        assert_eq!(
            producing_from(&session).await,
            Some(39),
            "one step back may be all the player wanted, so it is answered where it asks"
        );

        session
            .make_sure_someone_is_producing(38)
            .await
            .expect("the second step back is answered");
        assert_eq!(
            producing_from(&session).await,
            Some(38 - WORTH_WAITING_FOR),
            "a second step running is a walk, and the run covers the stretch it walks into"
        );

        for step in (38 - WORTH_WAITING_FOR..38).rev() {
            let started_again = session
                .make_sure_someone_is_producing(step)
                .await
                .expect("the request is answered");
            assert!(
                started_again.is_none(),
                "segment {step} is inside the run already going, so nothing is started again"
            );
        }
        assert_eq!(
            producing_from(&session).await,
            Some(38 - WORTH_WAITING_FOR),
            "and the walk was served by the one tool throughout"
        );
        session.close().await;
    }

    #[tokio::test]
    async fn jumping_ahead_starts_the_tool_where_the_viewer_landed() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = a_session_still_at_work(directory.path()).await;

        session.segment(0).await.expect("the first segment");
        assert_eq!(producing_from(&session).await, Some(0));
        let written = once_the_tool_is_under_way(&session).await;

        // The rule is asked directly rather than through a request for the
        // segment: a request finds a segment already written and rightly hands
        // it over without starting anything, and going through it would test
        // how fast this machine is rather than what the rule says.
        let landed_on = written + WORTH_WAITING_FOR + 4;
        session
            .make_sure_someone_is_producing(landed_on)
            .await
            .expect("the tool starts again");
        assert_eq!(
            producing_from(&session).await,
            Some(landed_on),
            "a viewer dragging the cursor towards the end must not wait for \
             everything in between to be produced first"
        );

        let landed = session
            .segment(landed_on)
            .await
            .expect("the segment landed on");
        assert!(landed.exists());
        session.close().await;
    }

    #[test]
    fn what_is_nearly_ready_is_waited_for_and_the_rest_is_a_jump() {
        // The other half of the same rule, asked as the question it is.
        // Restarting costs a second or two and throws away work already done,
        // so a request for what is nearly ready waits.
        assert!(already_on_its_way(0, 0, 0), "the one being produced");
        assert!(
            already_on_its_way(0, 0, WORTH_WAITING_FOR - 1),
            "nearly there"
        );
        assert!(
            !already_on_its_way(0, 0, WORTH_WAITING_FOR),
            "beyond that, the viewer has jumped"
        );
        assert!(!already_on_its_way(0, 0, 400), "the end of a long film");

        // The tool only moves forward, so anything behind it is a viewer who
        // went back and will never be reached by waiting.
        assert!(!already_on_its_way(10, 10, 9));
        assert!(!already_on_its_way(10, 10, 0));
        assert!(already_on_its_way(10, 10, 12), "the window travels with it");
    }

    #[test]
    fn the_window_is_measured_from_where_the_tool_has_got_to() {
        // What this cost, seen in a journal: a tool set going at the five
        // hundred and fourteenth segment, writing the five hundred and
        // nineteenth, and a player asking for the five hundred and twentieth,
        // which is the very next one. Measured from where the tool was set
        // going, that fell outside the window, so the tool was stopped and
        // begun again from a standing start for the segment it was on the
        // point of writing. Every six segments, for the whole film.
        assert!(
            already_on_its_way(514, 519, 520),
            "the very next segment the tool is about to write"
        );
        assert!(
            already_on_its_way(514, 600, 605),
            "an hour into a run, the next few are still the next few"
        );
        assert!(
            !already_on_its_way(514, 519, 519 + WORTH_WAITING_FOR),
            "beyond the window from where it has got to, the viewer has jumped"
        );
        assert!(
            !already_on_its_way(514, 600, 513),
            "behind where the tool was set going is a viewer who went back"
        );
    }

    #[test]
    fn stepping_back_once_is_answered_where_it_asks_and_walking_is_answered_ahead_of_itself() {
        // What this cost, read in the maintainer's journal: a player asking
        // for the segment before the one the tool had just been set going on,
        // sixty five times in a row, twenty two seconds of a film frozen on
        // the screen with the tool stopped and started again for every single
        // one of them.
        assert_eq!(steps_back_after(579, 0, 578), 1, "the first step back");
        assert_eq!(steps_back_after(578, 1, 577), 2, "and the one after it");
        assert_eq!(steps_back_after(577, 2, 576), 3, "and it keeps going");
        assert_eq!(
            steps_back_after(579, 3, 590),
            0,
            "forwards is not a step back, however many came before"
        );
        assert_eq!(
            steps_back_after(579, 3, 100),
            0,
            "and neither is a viewer dragging the cursor somewhere else"
        );

        assert_eq!(
            where_to_begin(578, 1),
            578,
            "one step may be all the player wanted, and it is the cheapest answer"
        );
        assert_eq!(
            where_to_begin(577, 2),
            577 - WORTH_WAITING_FOR,
            "a second step running is a walk, and the stretch it walks into comes out of one run"
        );
        assert_eq!(
            where_to_begin(2, 4),
            0,
            "a walk that reaches the beginning of the film stops there"
        );
        assert_eq!(where_to_begin(1107, 0), 1107, "a jump asks where it asks");
    }

    #[tokio::test]
    async fn a_segment_the_stopped_tool_had_finished_is_left_where_it_is() {
        // Read in the maintainer's journal: a run that had produced the film
        // all the way to its last segment, stopped so that a jump could be
        // served, and the last segment of the film taken away with it. Nothing
        // was half written, and a viewer going to the end paid for it.
        let directory = tempfile::tempdir().expect("temporary directory");
        let session = session_of(directory.path(), 600).await;
        let write = |index: u32, bytes: &[u8]| {
            std::fs::write(session.folder().join(format!("segment-{index}.m4s")), bytes)
                .expect("a segment on disk")
        };

        for index in 100..103 {
            write(index, &a_finished_segment());
        }
        session
            .remove_what_was_half_written(100, a_moment_ago())
            .await;
        for kept in 100..103 {
            assert!(
                session
                    .folder()
                    .join(format!("segment-{kept}.m4s"))
                    .exists(),
                "segment {kept} ends where its last box does, so nobody was inside it"
            );
        }

        // And the segment that really was caught halfway is still taken away.
        write(103, &a_segment_still_being_written());
        session
            .remove_what_was_half_written(100, a_moment_ago())
            .await;
        assert!(
            !session.folder().join("segment-103.m4s").exists(),
            "it stops in the middle of its own picture data, so it would be served truncated"
        );
        session.close().await;
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
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(60_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Encode(encode),
                audio: AudioOutput::Encode(melyxar_ffmpeg::command::AudioEncode::browser_stereo(
                    "aac",
                )),
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: Vec::new(),
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
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(60_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Copy,
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: Vec::new(),
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
    async fn a_film_cut_where_it_allows_lands_a_jump_where_it_was_aimed() {
        // The other half of the test above, and the reason this exists. Cut on
        // a grid, a copied picture lands on the key frame before the point
        // asked for, measured at six seconds early on a film with one every
        // ten. Cut where the film allows, the point asked for is itself a key
        // frame, so there is nothing earlier to fall back to.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        clip_with_key_frames_every(&source, 60, 240).await;

        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let found = melyxar_ffmpeg::probe::key_frames(
            &tools.ffprobe,
            &source,
            melyxar_ffmpeg::AskedToStop::never(),
        )
        .await
        .expect("the film says where it can be started");
        let boundaries = melyxar_ffmpeg::probe::where_the_film_can_be_cut(&found);

        let session = Session::open(
            SessionId::new(),
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(60_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Copy,
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: boundaries,
                if_the_card_refuses: Vec::new(),
            },
            directory.path().join("session"),
            tools,
        )
        .await
        .expect("the session opens");
        assert!(
            session.playlist().cut_where_the_film_allows(),
            "the film was read for where it can be started"
        );

        // Three segments in, which on this film is thirty seconds: a place a
        // grid would have put in the middle of two key frames.
        session.segment(3).await.expect("the segment landed on");
        let announced = clock_of(&session, 3).await;
        let expected = session.playlist().start_of(3).as_seconds_f64();
        assert!(
            (announced - expected).abs() < 0.5,
            "the playlist says this segment covers second {expected}, and it \
             announces itself at {announced}"
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
    async fn a_film_with_pictures_close_together_stays_where_the_playlist_put_it() {
        // The defect this guards against was reported from a real library: one
        // film out of three hundred where the opening was right, the picture
        // fell further and further behind the bar as it played, and the ending
        // stopped with twenty minutes still on the clock.
        //
        // The cause is one rule believed by two parties. The playlist used to
        // skip a place the film can be started when it lay less than a segment
        // past the last cut, on the understanding that the tool did the same.
        // It does not: the length it aims for advances by a fixed step at
        // every cut instead of being measured from the cut it just made, so
        // once a segment runs longer than that step the aim stays behind and
        // the tool cuts at every place there is. Every skipped place is then
        // one segment of drift, for the rest of the reading.
        //
        // Only a film carrying two of them close together shows it, which is
        // why a whole library can be fine and one film cannot.
        let directory = tempfile::tempdir().expect("temporary directory");
        let source = directory.path().join("source.mp4");
        let close_together = [
            0.0, 2.2, 10.9, 14.0, 23.5, 33.7, 41.5, 51.7, 53.9, 66.4, 75.9, 83.7,
        ];
        clip_with_key_frames_at(&source, 90, &close_together).await;

        let tools = ToolPaths::discover(None, None).expect("the tools are installed here");
        let found = melyxar_ffmpeg::probe::key_frames(
            &tools.ffprobe,
            &source,
            melyxar_ffmpeg::AskedToStop::never(),
        )
        .await
        .expect("the film says where it can be started");
        let session = Session::open(
            SessionId::new(),
            *NOBODY_IN_PARTICULAR,
            Recipe {
                source,
                duration: Millis::new(90_000),
                streams: StreamSelection::default(),
                video: VideoOutput::Copy,
                audio: AudioOutput::Encode(melyxar_ffmpeg::command::AudioEncode::browser_stereo(
                    "aac",
                )),
                where_the_viewer_starts: Millis::ZERO,
                where_it_can_be_started: melyxar_ffmpeg::probe::where_the_film_can_be_cut(&found),
                if_the_card_refuses: Vec::new(),
            },
            directory.path().join("session"),
            tools,
        )
        .await
        .expect("the session opens");
        assert!(
            session.playlist().cut_where_the_film_allows(),
            "the film was read for where it can be started"
        );
        assert!(
            session.playlist().segment_count() >= 8,
            "a film with too few segments would not show a drift that builds up"
        );

        // A jump, and then the film played on from there: one reading of the
        // film, which is where the drift used to build up. Every segment of it
        // has to hold the part of the film the playlist says it holds.
        let jumped_to = 2;
        for index in jumped_to..session.playlist().segment_count() {
            session
                .segment(index)
                .await
                .unwrap_or_else(|error| panic!("segment {index} was produced: {error}"));
            let announced = clock_of(&session, index).await;
            let expected = session.playlist().start_of(index).as_seconds_f64();
            assert!(
                (announced - expected).abs() < 0.5,
                "segment {index} holds second {announced} of the film and the \
                 playlist says it holds second {expected}"
            );
        }
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
