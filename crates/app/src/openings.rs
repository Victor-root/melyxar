//! Finding the opening and closing titles of a season by listening to it.
//!
//! Most files say nothing about where their opening titles are. Measured on a
//! real collection before any of this was written: of fifty episodes taken at
//! random, forty five carried no chapter at all, and a chapter still has to
//! name itself to say anything. So the reading of chapters, which is free and
//! already done, covers well under a tenth of the files it is meant for.
//!
//! What is left is the one thing that is true of every series: the opening is
//! the same in every episode of its season, and nothing else is. The reminder
//! of last week is new footage every week, and so is the episode. So the
//! episodes of one season are listened to and asked what they share.
//!
//! This is the odd one out among the readings of the upkeep. The other three
//! ask a question of one film and get an answer from that film; this one can
//! only ask a season, and a season is read whole or not at all. What is
//! written down is still per file, and a file nobody has listened to holds its
//! whole season waiting, which is what puts a season back in the queue the day
//! an episode is added to it.
//!
//! What it will never find is the reminder of last week. What sits before the
//! opening is sometimes that reminder and sometimes a real scene of the
//! episode, often both, and nothing in the sound tells them apart because both
//! are different every week. A button that sometimes offered to skip the first
//! scene of the episode would be pressed once and never again, which is worse
//! than no button.
//!
//! Everything here writes under its own word in the journal, `openings`. It is
//! the one reading whose answer is a judgement about the films rather than a
//! number, so when a skip button turns up in the wrong place, what decided it
//! has to be one word to ask for and one tick to send.

use std::collections::BTreeMap;
use std::path::Path;

use melyxar_core::id::{MediaSourceId, WorkId};
use melyxar_core::job::JobStep;
use melyxar_core::library::Library;
use melyxar_core::media::{Track, TrackKind};
use melyxar_core::media_log::file_name_of as name_of_file;
use melyxar_core::segments::{MediaSegment, SegmentKind, SegmentOrigin};
use melyxar_core::time::Millis;
use melyxar_database::catalogue::{EpisodeToListenTo, SeasonToListenTo};
use melyxar_ffmpeg::AskedToStop;
use melyxar_jobs::JobHandle;
use melyxar_sound::{what_they_have_in_common, Listened, Stretch};

use crate::{AppState, Result};

/// How much of the beginning of an episode is listened to.
///
/// An opening does not hide in the middle of an episode. Four minutes covers
/// the longest reminder of last week followed by the longest opening anybody
/// writes, and reading the whole episode instead would make this the longest
/// of the four readings for nothing.
const THE_BEGINNING: Millis = Millis::new(4 * 60 * 1_000);

/// And how much of the end, for the closing titles.
const THE_END: Millis = Millis::new(4 * 60 * 1_000);

/// How far in a second look at the beginning reaches.
///
/// Four minutes is right for almost every episode ever made, and wrong for the
/// ones that open on a long cold scene before their titles. Those are mostly
/// the first episode of a season, which is given room to set its year up: a
/// real collection had some thirty files whose opening stopped dead at three
/// minutes fifty nine, which is not where any of those openings really ended
/// but exactly where the listening stopped.
///
/// Ten minutes at the most, and never more than a quarter of the episode. A
/// sitcom of twenty two minutes does not hide its titles at its ninth minute,
/// and reading half of one to prove it would cost more than every opening it
/// could ever find.
const THE_BEGINNING_AGAIN: Millis = Millis::new(10 * 60 * 1_000);

/// How close to the edge of what was read an opening has to end before the
/// edge, rather than the opening itself, is the likelier reason it stopped.
const AGAINST_THE_EDGE: Millis = Millis::new(1_000);

/// How short a shared stretch may be and still deserve a button.
///
/// Under ten seconds, pressing a button is more work than waiting, and a
/// stretch that short is as likely to be two episodes happening to agree as
/// anything anybody wrote.
const SHORTEST_WORTH_A_BUTTON: Millis = Millis::new(10_000);

/// How much two episodes must share before their agreement is worth a word.
///
/// Two episodes with nothing whatever to do with each other still agree about
/// something: a breath, a door closing, a beat of silence. Measured on made up
/// seasons built to share nothing, that came to eight tenths of a second. Say
/// that in the journal and every season that really shares nothing reads as
/// though a button had been taken from it, which buries the one line that
/// matters under a hundred that do not. A title card runs longer than this. A
/// coincidence does not.
const TOO_BRIEF_TO_MENTION: Millis = Millis::new(2_000);

/// And how long. Above this, something has gone wrong rather than right: two
/// copies of one episode filed as two episodes share everything, and a button
/// offering to skip five minutes of the film would do real damage.
const LONGEST_WORTH_A_BUTTON: Millis = Millis::new(3 * 60 * 1_000);

/// How many later episodes each episode is compared against.
///
/// Not all of them. Every pair would be two hundred and thirty comparisons on
/// a season of twenty two, for an answer three give just as well: what is
/// wanted from the extra pairs is not more evidence of the same thing but a
/// second opinion, and three is a second opinion.
const PARTNERS: usize = 3;

/// How many seasons one pass asks for at a time.
///
/// A bound on what is held in memory at once rather than on the work: like the
/// three readings beside it, the end of a batch is not the end of the run.
const IN_ONE_BATCH: i64 = 200;

/// Listens to every season of one library that is still waiting.
///
/// Answers how many seasons were listened to right through.
pub(crate) async fn listen_to_the_seasons_of(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    let Some(tools) = state.tools() else {
        tracing::debug!(
            library = library.name,
            "no media tool here, so no season is listened to for its titles"
        );
        return Ok(0);
    };
    let database = state.database();
    let waiting = database.count_seasons_to_listen_to(library.id).await?;
    if waiting == 0 {
        return Ok(0);
    }

    handle.at_step(JobStep::ListeningForOpenings).await;
    let already_done = database.count_seasons_listened_to(library.id).await?;
    if already_done > 0 {
        handle.advance(already_done).await;
    }
    handle.set_total(already_done + waiting).await;
    tracing::debug!(
        library = library.name,
        waiting,
        already_done,
        "listening to the seasons of this library for the titles their episodes share"
    );

    let tool = tools.ffmpeg.clone();
    let mut listened = 0;
    let mut still_waiting = waiting;
    loop {
        if handle.is_cancelled() {
            break;
        }
        let batch = database
            .seasons_to_listen_to(library.id, IN_ONE_BATCH)
            .await?;
        if batch.is_empty() {
            break;
        }

        let mut stopped = false;
        for season in &batch {
            if handle.is_cancelled() {
                stopped = true;
                break;
            }
            handle.now_working_on(Some(&naming(season))).await;
            if listen_to_one_season(state, &tool, season, handle)
                .await?
                .is_some()
            {
                listened += 1;
            } else {
                stopped = true;
                break;
            }
            handle.advance(1).await;
        }
        if stopped {
            break;
        }

        // What is left is asked for again rather than worked out from what the
        // batch answered, exactly as the three readings beside it do: a batch
        // that left the number where it was would come back for ever.
        let left = database.count_seasons_to_listen_to(library.id).await?;
        if left >= still_waiting {
            if !handle.is_cancelled() {
                tracing::warn!(
                    library = library.name,
                    waiting = left,
                    "nothing of this batch of seasons could be written down, so the \
                     listening stops here"
                );
            }
            break;
        }
        still_waiting = left;
    }

    if listened > 0 {
        tracing::info!(
            library = library.name,
            seasons = listened,
            "the episodes of these seasons were listened to for the titles they share"
        );
    }
    Ok(listened)
}

/// A listening started from a terminal, which somebody is waiting on.
pub struct ListeningJob {
    started: melyxar_jobs::StartedJob,
    outcome: std::sync::Arc<std::sync::Mutex<Vec<HowASeasonWent>>>,
}

impl ListeningJob {
    /// Waits for the listening to end, and says what each season came to.
    ///
    /// A run somebody stopped says what it had got through before it was
    /// stopped, which is what was really written down.
    pub async fn wait(self) -> (melyxar_core::job::JobState, Vec<HowASeasonWent>) {
        let state = self.started.completion.await.unwrap_or_else(|error| {
            tracing::error!(error = %error, "the listening task ended unexpectedly");
            melyxar_core::job::JobState::Failed
        });
        let went = self
            .outcome
            .lock()
            .expect("the report lock is never held across an await")
            .clone();
        (state, went)
    }
}

/// Starts a listening of named seasons as a job of its own.
///
/// A job rather than a bare call, for the same reasons a scan asked for from a
/// terminal is one: it shows up in the list of what is running, it can be
/// stopped, and a second one on the same series is refused rather than run
/// alongside the first.
pub async fn start_listening_again(
    state: &AppState,
    series: Option<&str>,
    seasons: Vec<SeasonToListenTo>,
) -> std::result::Result<ListeningJob, melyxar_jobs::JobError> {
    let outcome = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = std::sync::Arc::clone(&outcome);
    let owned = state.clone();
    // What the second run of the same thing is refused against. Every series
    // at once is its own subject, and not one whose name anybody typed.
    let target = series.map_or_else(|| "every series".to_string(), str::to_lowercase);

    let started = state
        .jobs()
        .clone()
        .start(
            melyxar_core::job::JobKind::ListenForOpenings,
            melyxar_core::job::JobPriority::REQUESTED,
            Some(target),
            move |handle| async move {
                match listen_again_to(&owned, &seasons, &handle).await {
                    Ok(went) => {
                        *recorded
                            .lock()
                            .expect("the report lock is never held across an await") = went;
                        Ok(())
                    }
                    Err(error) => Err(error.to_string()),
                }
            },
        )
        .await?;

    Ok(ListeningJob { started, outcome })
}

/// Listens again to seasons that have already had their answer, forgetting it
/// first.
///
/// What the upkeep does every night, pointed at a handful of seasons instead
/// of at a whole library. A season written down as done is a season never read
/// again, so the old answer has to go before the new rules can reach it.
///
/// This exists for the terminal. A rule about what an opening is takes hours
/// to try on a whole collection and a minute to try on one series, and a rule
/// nobody can try is a rule nobody changes.
pub async fn listen_again_to(
    state: &AppState,
    seasons: &[SeasonToListenTo],
    handle: &JobHandle,
) -> Result<Vec<HowASeasonWent>> {
    let Some(tools) = state.tools() else {
        tracing::warn!("no media tool here, so nothing can be listened to");
        return Ok(Vec::new());
    };
    let database = state.database();

    handle.at_step(JobStep::ListeningForOpenings).await;
    handle.set_total(seasons.len() as i64).await;

    let tool = tools.ffmpeg.clone();
    let mut went = Vec::with_capacity(seasons.len());
    for season in seasons {
        if handle.is_cancelled() {
            break;
        }
        handle.now_working_on(Some(&naming(season))).await;
        let forgotten = database.forget_the_listening_of(season.id).await?;
        tracing::debug!(
            series = season.series,
            season = season.number,
            files = forgotten,
            "what was known about this season is forgotten, so it is read again"
        );

        let Some(how) = listen_to_one_season(state, &tool, season, handle).await? else {
            break;
        };
        went.push(how);
        handle.advance(1).await;
    }
    Ok(went)
}

/// What listening to one season came to.
///
/// The journal says all of this and more as it goes, one line per file. This
/// is the same thing at the size of a season, for a terminal where somebody is
/// waiting on the answer for one series and wants it in a line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HowASeasonWent {
    pub series: String,
    pub number: Option<i32>,
    /// Episodes, not files: two copies of one episode are one episode.
    pub episodes: usize,
    pub with_an_opening: usize,
    pub with_a_closing: usize,
}

/// Listens to one season right through. Answers nothing if somebody stopped it.
async fn listen_to_one_season(
    state: &AppState,
    tool: &std::path::Path,
    season: &SeasonToListenTo,
    handle: &JobHandle,
) -> Result<Option<HowASeasonWent>> {
    let database = state.database();
    let episodes = database.episodes_to_listen_to(season.id).await?;

    // Two copies of one episode hold the same sound from end to end, so what
    // counts here is how many episodes there are and not how many files.
    let distinct = how_many_episodes(&episodes);
    if distinct < 2 {
        // Written down all the same, and this is the point of writing it down:
        // a season of one episode will never have anything to be compared
        // against, and without an answer it would be read again every night
        // for the rest of the server's life.
        tracing::debug!(
            series = season.series,
            season = season.number,
            files = episodes.len(),
            "a season of one episode has nothing to be compared against, so \
             nothing is looked for in it"
        );
        for episode in &episodes {
            database.store_openings(episode.source_id, &[]).await?;
        }
        return Ok(Some(HowASeasonWent {
            series: season.series.clone(),
            number: season.number,
            episodes: distinct,
            with_an_opening: 0,
            with_a_closing: 0,
        }));
    }

    let mut tracks = Vec::with_capacity(episodes.len());
    for episode in &episodes {
        tracks.push(database.tracks_of_source(episode.source_id).await?);
    }
    let chosen = the_track_to_listen_to(&tracks);
    tracing::debug!(
        series = season.series,
        season = season.number,
        episodes = distinct,
        files = episodes.len(),
        language = which_language(&tracks, &chosen).unwrap_or("none stated"),
        "listening to this season"
    );

    let asked_to_stop = AskedToStop::when(handle.cancelled_when());
    let wanted: Vec<(EpisodeToListenTo, Option<i32>)> = episodes
        .iter()
        .cloned()
        .zip(chosen.iter().copied())
        .collect();
    let owned_tool = tool.to_path_buf();
    let read = melyxar_jobs::for_each_bounded(
        wanted,
        state.config().limits.concurrent_probes,
        move |(episode, track)| {
            let tool = owned_tool.clone();
            let asked_to_stop = asked_to_stop.clone();
            async move { listen_to_one_file(&tool, &episode, track, asked_to_stop).await }
        },
    )
    .await;

    // A run somebody stopped writes nothing at all. Every file it did not
    // reach is left waiting, which is what makes the next run pick the season
    // up exactly where this one left it.
    if handle.is_cancelled() || read.iter().any(|how| matches!(how, HowItWent::Stopped)) {
        tracing::debug!(
            series = season.series,
            season = season.number,
            "this season was left where it was, its listening having been stopped"
        );
        return Ok(None);
    }

    let mut heard = Vec::with_capacity(read.len());
    for how in read {
        match how {
            HowItWent::Listened(listened) => heard.push(listened),
            HowItWent::NothingToHear(source_id) => {
                database.store_openings(source_id, &[]).await?;
            }
            HowItWent::Stopped => unreachable!("answered above"),
        }
    }

    // The comparison is seconds of arithmetic on a whole season, which is far
    // too much to do on a thread that is meant to be waiting on something.
    let (mut heard, found) = tokio::task::spawn_blocking(move || {
        let found = what_a_season_shares(&heard);
        (heard, found)
    })
    .await
    .unwrap_or_default();

    // A file with nothing to hear never reached the comparison, so what was
    // heard is a shorter list than the season's files and the two cannot be
    // walked side by side.
    let which_file: std::collections::HashMap<_, _> = episodes
        .iter()
        .cloned()
        .zip(chosen.iter().copied())
        .map(|(episode, track)| (episode.source_id, (episode, track)))
        .collect();

    // Only the episodes whose opening was not found, or was found running
    // hard against the edge of what was read, are listened to further in. On
    // a season that gave up its opening at once this is nobody, and the pass
    // costs exactly what it did before.
    let again: Vec<(usize, Millis, EpisodeToListenTo, Option<i32>)> = found
        .iter()
        .enumerate()
        .filter(|(_, one)| one.how_the_opening_went().a_longer_look_might_help())
        .filter_map(|(at, one)| {
            let (episode, track) = which_file.get(&one.source_id)?;
            let how_far = how_far_in_to_look_again(episode.duration)?;
            Some((at, how_far, episode.clone(), *track))
        })
        .collect();

    let found = if again.is_empty() {
        found
    } else {
        tracing::debug!(
            series = season.series,
            season = season.number,
            episodes = again.len(),
            "these episodes are listened to further in, their openings having been \
             either missing or cut off by the edge of what was read"
        );
        let asked_to_stop = AskedToStop::when(handle.cancelled_when());
        let owned_tool = tool.to_path_buf();
        let deeper = melyxar_jobs::for_each_bounded(
            again,
            state.config().limits.concurrent_probes,
            move |(at, how_far, episode, track)| {
                let tool = owned_tool.clone();
                let asked_to_stop = asked_to_stop.clone();
                async move {
                    let heard = listen_further_into_the_beginning_of(
                        &tool,
                        &episode,
                        track,
                        how_far,
                        asked_to_stop,
                    )
                    .await;
                    (at, heard)
                }
            },
        )
        .await;

        if handle.is_cancelled() {
            tracing::debug!(
                series = season.series,
                season = season.number,
                "this season was left where it was, its listening having been stopped"
            );
            return Ok(None);
        }

        for (at, heard_again) in deeper {
            // A beginning that would not come back a second time leaves the
            // first answer standing, which is the worst this can do.
            if let Some(heard_again) = heard_again {
                heard[at].beginning = heard_again;
            }
        }
        tokio::task::spawn_blocking(move || what_a_season_shares(&heard))
            .await
            .unwrap_or_default()
    };

    let mut with_an_opening = 0;
    let mut with_a_closing = 0;
    for one in &found {
        let kept = only_what_nobody_has_already_said(state, one).await?;
        with_an_opening += usize::from(kept.iter().any(|it| it.kind == SegmentKind::Intro));
        with_a_closing += usize::from(kept.iter().any(|it| it.kind == SegmentKind::Outro));
        say_what_was_found(one, &kept);
        database.store_openings(one.source_id, &kept).await?;
    }

    tracing::info!(
        series = season.series,
        season = season.number,
        episodes = distinct,
        with_an_opening,
        with_a_closing,
        "this season was listened to right through"
    );
    Ok(Some(HowASeasonWent {
        series: season.series.clone(),
        number: season.number,
        episodes: distinct,
        with_an_opening,
        with_a_closing,
    }))
}

/// Writes down, file by file, where the buttons will be.
///
/// The line somebody sends over when a button turns up in the wrong place: it
/// names the file and the two moments, which between them are the whole of
/// what was decided about it.
fn say_what_was_found(one: &Found, kept: &[MediaSegment]) {
    for segment in kept {
        tracing::debug!(
            file = one.file_name,
            what = segment.kind.as_str(),
            from = segment.start.as_seconds_f64(),
            to = segment.end.as_seconds_f64(),
            "a button will offer to skip this"
        );
    }

    for (kind, what) in &one.ends {
        // A button is there, so there is nothing to explain.
        if kept.iter().any(|segment| segment.kind == *kind) {
            continue;
        }
        // Something was heard and the file or a person overruled it, which the
        // line above has already said. Saying it a second way would be wrong.
        if what.stretch().is_some() {
            continue;
        }
        let Some(why) = what.why_not() else { continue };
        match what.how_long() {
            Some(length) => tracing::debug!(
                file = one.file_name,
                what = kind.as_str(),
                shared_seconds = length.as_seconds_f64(),
                "{why}"
            ),
            None => tracing::debug!(file = one.file_name, what = kind.as_str(), "{why}"),
        }
    }
}

/// Leaves out anything the file or a person has already settled.
///
/// A chapter the file names itself is more exact than a comparison can be, and
/// somebody who set a stretch by hand has said the last word on it. Listening
/// only fills in what neither of them covered.
async fn only_what_nobody_has_already_said(
    state: &AppState,
    found: &Found,
) -> Result<Vec<MediaSegment>> {
    if found.segments.is_empty() {
        return Ok(Vec::new());
    }
    let already = state.database().segments_of_source(found.source_id).await?;

    Ok(found
        .segments
        .iter()
        .filter(|segment| {
            let spoken_for = already
                .iter()
                .any(|held| held.kind == segment.kind && held.origin != SegmentOrigin::Detected);
            if spoken_for {
                tracing::debug!(
                    file = found.file_name,
                    what = segment.kind.as_str(),
                    "the file or a person already says where this is, so what was \
                     heard is left aside"
                );
            }
            !spoken_for
        })
        .copied()
        .collect())
}

/// How many episodes a list of files is a list of copies of.
fn how_many_episodes(files: &[EpisodeToListenTo]) -> usize {
    files
        .iter()
        .enumerate()
        .filter(|(at, file)| {
            !files[..*at]
                .iter()
                .any(|before| before.work_id == file.work_id)
        })
        .count()
}

/// How reading one file turned out.
enum HowItWent {
    Listened(ListenedTo),
    /// Nothing usable came back, and reading it again would say the same. The
    /// file is written down as listened to with nothing found, so that it
    /// stops holding its season in the queue.
    NothingToHear(MediaSourceId),
    /// Somebody stopped the run. Nothing about this season is written.
    Stopped,
}

/// One episode file, listened to at both of its ends.
struct ListenedTo {
    work_id: WorkId,
    source_id: MediaSourceId,
    file_name: String,
    /// What its first minutes sound like, from its very first moment.
    beginning: Listened,
    /// What its last minutes sound like, and where they begin in the episode.
    ending: Option<(Millis, Listened)>,
}

/// Reads the two ends of one episode and writes them down.
async fn listen_to_one_file(
    tool: &std::path::Path,
    episode: &EpisodeToListenTo,
    track: Option<i32>,
    asked_to_stop: AskedToStop,
) -> HowItWent {
    let file = name_of_file(&episode.path).to_string();
    let Some(track) = track else {
        tracing::debug!(
            file,
            "this file carries no sound at all, so there is nothing to hear"
        );
        return HowItWent::NothingToHear(episode.source_id);
    };

    let (beginning, ending) = the_two_ends_of(episode.duration);
    let beginning = match samples(
        tool,
        &episode.path,
        track,
        Millis::ZERO,
        beginning,
        &asked_to_stop,
    )
    .await
    {
        Read::Some(samples) => samples,
        Read::Stopped => return HowItWent::Stopped,
        Read::Nothing => {
            tracing::warn!(
                file,
                "the sound of this episode could not be read, so it is written down \
                 as having nothing to share"
            );
            return HowItWent::NothingToHear(episode.source_id);
        }
    };

    let mut heard_at_the_end = None;
    if let Some((at, how_long)) = ending {
        match samples(tool, &episode.path, track, at, how_long, &asked_to_stop).await {
            Read::Some(samples) => heard_at_the_end = Some((at, samples)),
            Read::Stopped => return HowItWent::Stopped,
            // The beginning is what the opening needs, and it is already read.
            // An end that would not come back costs the closing titles of this
            // one episode and nothing else.
            Read::Nothing => tracing::debug!(
                file,
                "the last minutes of this episode could not be read, so only its \
                 beginning is compared"
            ),
        }
    }

    let describing = tokio::task::spawn_blocking(move || {
        (
            Listened::of(&beginning),
            heard_at_the_end.map(|(at, samples)| (at, Listened::of(&samples))),
        )
    })
    .await;
    let Ok((beginning, ending)) = describing else {
        tracing::warn!(file, "the sound of this episode could not be written down");
        return HowItWent::NothingToHear(episode.source_id);
    };

    if beginning.is_empty() {
        tracing::debug!(
            file,
            "too little sound came back from this episode to say anything"
        );
        return HowItWent::NothingToHear(episode.source_id);
    }

    HowItWent::Listened(ListenedTo {
        work_id: episode.work_id,
        source_id: episode.source_id,
        file_name: file,
        beginning,
        ending,
    })
}

/// Listens to the beginning of one episode again, further in than the first
/// time, and writes down what it heard.
///
/// Only the beginning: the closing titles were found at the other end of the
/// episode and are not in question, and reading them twice would double the
/// cost of a pass for nothing.
async fn listen_further_into_the_beginning_of(
    tool: &std::path::Path,
    episode: &EpisodeToListenTo,
    track: Option<i32>,
    how_far: Millis,
    asked_to_stop: AskedToStop,
) -> Option<Listened> {
    let track = track?;
    let Read::Some(samples) = samples(
        tool,
        &episode.path,
        track,
        Millis::ZERO,
        how_far,
        &asked_to_stop,
    )
    .await
    else {
        return None;
    };
    tokio::task::spawn_blocking(move || Listened::of(&samples))
        .await
        .ok()
        .filter(|heard| !heard.is_empty())
}

/// What came back from asking the tool for a stretch of sound.
enum Read {
    Some(Vec<i16>),
    Nothing,
    Stopped,
}

async fn samples(
    tool: &std::path::Path,
    path: &Path,
    track: i32,
    from: Millis,
    how_long: Millis,
    asked_to_stop: &AskedToStop,
) -> Read {
    match melyxar_ffmpeg::sound::samples_of(
        tool,
        path,
        track,
        from,
        how_long,
        asked_to_stop.clone(),
    )
    .await
    {
        Ok(samples) if !samples.is_empty() => Read::Some(samples),
        Ok(_) => Read::Nothing,
        Err(melyxar_ffmpeg::FfmpegError::GivenUp) => Read::Stopped,
        Err(error) => {
            tracing::debug!(
                file = %name_of_file(path),
                %error,
                "the tool would not give up this stretch of sound"
            );
            Read::Nothing
        }
    }
}

/// Which stretch of an episode is its beginning, and which its end.
///
/// Never more than half the episode each, so that the two never meet in the
/// middle: an episode of seven minutes would otherwise be read twice over and
/// its opening found in its own closing titles.
fn the_two_ends_of(duration: Option<Millis>) -> (Millis, Option<(Millis, Millis)>) {
    let Some(duration) = duration.filter(|duration| duration.get() > 0) else {
        // A file whose length nobody knows still has a beginning.
        return (THE_BEGINNING, None);
    };
    let half = Millis::new(duration.get() / 2);
    let beginning = Millis::new(THE_BEGINNING.get().min(half.get()));
    let ending = Millis::new(THE_END.get().min(half.get()));
    if ending < SHORTEST_WORTH_A_BUTTON {
        return (beginning, None);
    }
    (
        beginning,
        Some((Millis::new(duration.get() - ending.get()), ending)),
    )
}

/// Which sound track of each file is the one to listen to.
///
/// The same language across the whole season wherever the season allows it.
/// Two episodes of one season do not always carry their languages in the same
/// order, and comparing one episode's French against another's English is
/// comparing two different pieces of sound: they would be found to share
/// nothing, correctly, and the season would get no button at all.
///
/// A file carrying nothing in that language falls back to the track it calls
/// its own, and then to its first: a season where one episode is missing a
/// language is better served by one comparison that may fail than by one file
/// silently dropped.
///
/// Audio descriptions are set aside before any of that, including from the
/// count deciding the language. A narrator speaking over the picture says
/// something different in every episode, so the one moment the episodes are
/// most alike becomes the one moment these tracks share nothing: a season
/// whose English is an audio description is listened to in whatever other
/// language it carries rather than in one that cannot answer.
fn the_track_to_listen_to(of_each_file: &[Vec<Track>]) -> Vec<Option<i32>> {
    let sound: Vec<Vec<&Track>> = of_each_file
        .iter()
        .map(|tracks| {
            tracks
                .iter()
                .filter(|track| matches!(track.kind, TrackKind::Audio(_)))
                .filter(|track| !track.is_audio_description())
                .collect()
        })
        .collect();

    let mut how_many_carry: BTreeMap<&str, usize> = BTreeMap::new();
    for file in &sound {
        let mut seen: Vec<&str> = Vec::new();
        for language in file.iter().filter_map(|track| track.language.as_deref()) {
            if !seen.contains(&language) {
                seen.push(language);
                *how_many_carry.entry(language).or_insert(0) += 1;
            }
        }
    }
    // The most widely carried, and the first by name where two are level, so
    // that the same season always answers the same way.
    let shared = how_many_carry
        .iter()
        .max_by_key(|(language, carried)| (**carried, std::cmp::Reverse(*language)))
        .map(|(language, _)| *language);

    sound
        .iter()
        .map(|file| {
            shared
                .and_then(|language| {
                    file.iter()
                        .find(|track| track.language.as_deref() == Some(language))
                })
                .or_else(|| file.iter().find(|track| track.is_default))
                .or_else(|| file.first())
                .map(|track| track.stream_index)
        })
        .collect()
}

/// The language the season is being listened to in, for the journal to say so.
fn which_language<'a>(of_each_file: &'a [Vec<Track>], chosen: &[Option<i32>]) -> Option<&'a str> {
    of_each_file
        .iter()
        .zip(chosen)
        .find_map(|(tracks, chosen)| {
            let chosen = (*chosen)?;
            tracks
                .iter()
                .find(|track| track.stream_index == chosen)?
                .language
                .as_deref()
        })
}

/// A name for a season, for the line saying what is being worked on.
fn naming(season: &SeasonToListenTo) -> String {
    match season.number {
        Some(number) => format!("{} S{number:02}", season.series),
        None => season.series.clone(),
    }
}

/// What comparing one end of one episode with its neighbours came to.
///
/// Not a bare `Option`, because the four ways of coming away with nothing call
/// for four different answers and the person reading the journal is entitled
/// to know which one it was. The evening this was written, a season of a
/// cartoon came away empty and the only way to find out whether its opening
/// was three seconds long or simply absent was to ask the person who owns it.
#[derive(Debug, Clone, Copy)]
enum WhatWasShared {
    /// A stretch the neighbours agree on, long enough and short enough.
    AStretch(Stretch),
    /// They agree on something, but it is too short to be worth a button. A
    /// title card of three seconds is not an opening.
    TooShort(Millis),
    /// They agree on something so long it cannot be an opening.
    TooLong(Millis),
    /// One pair saw something and no other pair backed it up, which is what
    /// the second opinion exists to refuse.
    OnlyOnePairSawIt,
    /// The neighbours have nothing whatever in common with this episode here.
    Nothing,
}

impl WhatWasShared {
    /// Whether looking further into the episode might turn up more than this.
    ///
    /// Nothing found is the plain case. A stretch that ends hard against the
    /// edge of what was read is the other one, and the less obvious: an
    /// opening does not usually end at the exact moment the listening did, so
    /// what stopped it was almost certainly the edge.
    fn a_longer_look_might_help(self) -> bool {
        match self.stretch() {
            Some(stretch) => stretch.end.get() + AGAINST_THE_EDGE.get() >= THE_BEGINNING.get(),
            None => true,
        }
    }

    /// The stretch, when there is one.
    fn stretch(self) -> Option<Stretch> {
        match self {
            Self::AStretch(stretch) => Some(stretch),
            _ => None,
        }
    }

    /// Why there is no button, in the words the journal uses.
    ///
    /// Nothing when there is a button, since a line saying why there is none
    /// would then be a lie.
    fn why_not(self) -> Option<&'static str> {
        match self {
            Self::AStretch(_) => None,
            Self::TooShort(_) => Some("what the neighbours share here is too short for a button"),
            Self::TooLong(_) => Some("what the neighbours share here is too long to be an opening"),
            Self::OnlyOnePairSawIt => {
                Some("one pair heard something here and no other pair heard it too")
            }
            Self::Nothing => Some("nothing of this end is shared with its neighbours"),
        }
    }

    /// How long the refused stretch was, for the line that says so.
    fn how_long(self) -> Option<Millis> {
        match self {
            Self::TooShort(length) | Self::TooLong(length) => Some(length),
            _ => None,
        }
    }
}

/// How far in a second look at this episode would reach, or nothing when it
/// would see no more than the first look already did.
///
/// A quarter of the episode never runs into the four minutes read at its other
/// end, whatever its length, so the two never meet however long it is.
fn how_far_in_to_look_again(duration: Option<Millis>) -> Option<Millis> {
    let duration = duration.filter(|duration| duration.get() > 0)?;
    let how_far = Millis::new(THE_BEGINNING_AGAIN.get().min(duration.get() / 4));
    (how_far.get() > THE_BEGINNING.get()).then_some(how_far)
}

/// What listening to one file decided about it.
#[derive(Debug)]
struct Found {
    source_id: MediaSourceId,
    file_name: String,
    segments: Vec<MediaSegment>,
    /// What came of each end, so that an episode with no button can say which
    /// of the four kinds of nothing it came away with.
    ends: Vec<(SegmentKind, WhatWasShared)>,
}

impl Found {
    /// What the comparison came to about this episode's opening.
    fn how_the_opening_went(&self) -> WhatWasShared {
        self.ends
            .iter()
            .find(|(kind, _)| *kind == SegmentKind::Intro)
            .map_or(WhatWasShared::Nothing, |(_, what)| *what)
    }
}

/// What the episodes of one season turned out to share, file by file.
///
/// The whole of the judgement, and pure: descriptors in, stretches out. It
/// reads nothing, writes nothing and asks nothing, which is what lets the
/// awkward seasons be built in a test rather than found on somebody's disk.
fn what_a_season_shares(episodes: &[ListenedTo]) -> Vec<Found> {
    let works: Vec<WorkId> = episodes.iter().map(|episode| episode.work_id).collect();
    // With two episodes there is one opinion to be had, so one is all there
    // can be. With three or more there are several, and a stretch only one
    // pair saw is as likely to be a coincidence as an opening.
    let distinct = works
        .iter()
        .enumerate()
        .filter(|(at, work)| !works[..*at].contains(work))
        .count();
    let enough = if distinct > 2 { 2 } else { 1 };

    let beginnings: Vec<Option<&Listened>> = episodes
        .iter()
        .map(|episode| Some(&episode.beginning))
        .collect();
    let opening = what_they_share(
        &beginnings,
        &vec![Millis::ZERO; episodes.len()],
        &works,
        enough,
    );

    let endings: Vec<Option<&Listened>> = episodes
        .iter()
        .map(|episode| episode.ending.as_ref().map(|(_, heard)| heard))
        .collect();
    let where_endings_begin: Vec<Millis> = episodes
        .iter()
        .map(|episode| episode.ending.as_ref().map_or(Millis::ZERO, |(at, _)| *at))
        .collect();
    let closing = what_they_share(&endings, &where_endings_begin, &works, enough);

    episodes
        .iter()
        .enumerate()
        .map(|(at, episode)| {
            let ends = vec![
                (SegmentKind::Intro, opening[at]),
                (SegmentKind::Outro, closing[at]),
            ];
            let segments = ends
                .iter()
                .filter_map(|(kind, what)| {
                    what.stretch().map(|stretch| MediaSegment {
                        kind: *kind,
                        start: stretch.start,
                        end: stretch.end,
                        origin: SegmentOrigin::Detected,
                    })
                })
                .collect();
            Found {
                source_id: episode.source_id,
                file_name: episode.file_name.clone(),
                segments,
                ends,
            }
        })
        .collect()
}

/// What each of a season's files shares with its neighbours, at one end.
fn what_they_share(
    heard: &[Option<&Listened>],
    where_each_begins: &[Millis],
    works: &[WorkId],
    enough: usize,
) -> Vec<WhatWasShared> {
    let mut proposed: Vec<Vec<Stretch>> = vec![Vec::new(); heard.len()];
    let mut refused: Vec<Vec<Millis>> = vec![Vec::new(); heard.len()];

    for one in 0..heard.len() {
        for step in 1..=PARTNERS {
            let other = one + step;
            if other >= heard.len() {
                break;
            }
            // Two copies of one episode share everything they have, which is
            // the whole episode and not an opening.
            if works[one] == works[other] {
                continue;
            }
            let (Some(first), Some(second)) = (heard[one], heard[other]) else {
                continue;
            };
            let Some(shared) = what_they_have_in_common(first, second) else {
                continue;
            };
            // A stretch no button could sit on is still worth remembering:
            // it is the difference between a three second title card and a
            // season that shares nothing at all, and only the journal can
            // tell those two apart afterwards.
            if !worth_a_button(shared.length()) {
                refused[one].push(shared.length());
                refused[other].push(shared.length());
                continue;
            }
            proposed[one].push(moved(shared.in_the_first(), where_each_begins[one]));
            proposed[other].push(moved(shared.in_the_second(), where_each_begins[other]));
        }
    }

    proposed
        .iter()
        .zip(&refused)
        .map(
            |(proposed, refused)| match what_they_agree_on(proposed, enough) {
                Some(stretch) if worth_a_button(stretch.length()) => {
                    WhatWasShared::AStretch(stretch)
                }
                Some(stretch) => why_not_that_one(stretch.length()),
                // Proposals that nobody else backed up are the second opinion
                // doing its work, and are worth saying so rather than passing for
                // silence.
                None if !proposed.is_empty() => WhatWasShared::OnlyOnePairSawIt,
                None => match refused.iter().copied().max_by_key(|length| length.get()) {
                    Some(length) => why_not_that_one(length),
                    None => WhatWasShared::Nothing,
                },
            },
        )
        .collect()
}

/// Which side of the two limits a refused stretch fell on.
///
/// What is too brief even to be a title card is not a refusal at all, it is
/// two episodes happening to agree, and it is reported as the nothing it is.
fn why_not_that_one(length: Millis) -> WhatWasShared {
    if length < TOO_BRIEF_TO_MENTION {
        WhatWasShared::Nothing
    } else if length < SHORTEST_WORTH_A_BUTTON {
        WhatWasShared::TooShort(length)
    } else {
        WhatWasShared::TooLong(length)
    }
}

/// Whether a shared stretch is worth putting a button over.
fn worth_a_button(length: Millis) -> bool {
    length >= SHORTEST_WORTH_A_BUTTON && length <= LONGEST_WORTH_A_BUTTON
}

/// Moves a stretch from where it sits in what was read to where it sits in the
/// episode.
fn moved(stretch: Stretch, by: Millis) -> Stretch {
    Stretch {
        start: Millis::new(stretch.start.get() + by.get()),
        end: Millis::new(stretch.end.get() + by.get()),
    }
}

/// Whether two proposals are two accounts of the same stretch.
///
/// Half of the longer of the two, which is loose on purpose: two pairs agree
/// on where an opening is and disagree by a second or two on where it ends,
/// because one of them held on through a title card and the other did not.
fn much_the_same(one: Stretch, other: Stretch) -> bool {
    let from = one.start.get().max(other.start.get());
    let to = one.end.get().min(other.end.get());
    let shared = (to - from).max(0);
    let longest = one.length().get().max(other.length().get());
    longest > 0 && shared * 2 >= longest
}

/// The stretch the most pairs agree on, if enough of them do.
///
/// A proposal nobody else saw is left alone, which is the whole reason several
/// pairs are compared: one pair agreeing about a piece of music that happens
/// to run under two consecutive episodes is a coincidence, three pairs
/// agreeing is an opening.
fn what_they_agree_on(proposed: &[Stretch], enough: usize) -> Option<Stretch> {
    let mut best: Option<(usize, Stretch)> = None;
    for candidate in proposed {
        let agreeing: Vec<Stretch> = proposed
            .iter()
            .copied()
            .filter(|other| much_the_same(*candidate, *other))
            .collect();
        if agreeing.len() < enough {
            continue;
        }
        let agreed = the_middle_of(&agreeing);
        let better = match best {
            None => true,
            Some((how_many, held)) => {
                agreeing.len() > how_many
                    || (agreeing.len() == how_many && agreed.length() > held.length())
            }
        };
        if better {
            best = Some((agreeing.len(), agreed));
        }
    }
    best.map(|(_, stretch)| stretch)
}

/// The middle of several accounts of one stretch.
///
/// The middle rather than the average, so that one pair that held on far too
/// long moves the answer by nothing at all.
fn the_middle_of(agreeing: &[Stretch]) -> Stretch {
    let mut starts: Vec<i64> = agreeing.iter().map(|one| one.start.get()).collect();
    let mut ends: Vec<i64> = agreeing.iter().map(|one| one.end.get()).collect();
    starts.sort_unstable();
    ends.sort_unstable();
    Stretch {
        start: Millis::new(starts[starts.len() / 2]),
        end: Millis::new(ends[ends.len() / 2]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::TrackId;
    use melyxar_core::media::{AudioDetails, ColorInfo, Loudness, VideoDetails};

    /// The two crates either side of this one have to agree on how fast sound
    /// is read, and neither has any business knowing about the other: one
    /// launches tools and the other does arithmetic. This is where both are in
    /// view, so this is where they are held together.
    #[test]
    fn the_tool_is_asked_for_sound_at_the_rate_the_comparison_reads() {
        assert_eq!(
            melyxar_ffmpeg::sound::SAMPLES_A_SECOND,
            melyxar_sound::SAMPLES_A_SECOND
        );
    }

    fn a_sound_track(stream_index: i32, language: Option<&str>, is_default: bool) -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index,
            language: language.map(str::to_string),
            title: None,
            is_default,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "aac".into(),
                profile: None,
                channels: 2,
                channel_layout: None,
                sample_rate: None,
                bit_depth: None,
                bitrate: None,
                loudness: Loudness::default(),
            }),
        }
    }

    /// The little of a picture that matters here, which is nothing at all:
    /// what is being told apart is a picture from a sound.
    fn a_picture_of_some_kind() -> VideoDetails {
        VideoDetails {
            codec: "h264".into(),
            profile: None,
            level: None,
            width: 1_920,
            height: 1_080,
            aspect_ratio: None,
            is_interlaced: false,
            frame_rate: None,
            pixel_format: None,
            reference_frames: None,
            color: ColorInfo::default(),
            hdr: None,
            bitrate: None,
            margins: None,
        }
    }

    fn a_picture(stream_index: i32) -> Track {
        Track {
            id: TrackId::new(),
            source_id: MediaSourceId::new(),
            stream_index,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Video(a_picture_of_some_kind()),
        }
    }

    #[test]
    fn a_season_is_listened_to_in_one_language_however_its_files_order_them() {
        // The defect this guards: one episode's French compared against
        // another's English is two different pieces of sound, found to share
        // nothing, correctly, and the season gets no button at all. Which of
        // the two languages is picked does not matter in the least, as long as
        // it is the same one for the whole season.
        let files = vec![
            vec![
                a_picture(0),
                a_sound_track(1, Some("fra"), true),
                a_sound_track(2, Some("eng"), false),
            ],
            vec![
                a_picture(0),
                a_sound_track(1, Some("eng"), true),
                a_sound_track(2, Some("fra"), false),
            ],
        ];
        let chosen = the_track_to_listen_to(&files);
        let spoken = which_language(&files, &chosen).expect("a language was settled on");
        assert!(["fra", "eng"].contains(&spoken), "{spoken}");
        for (tracks, chosen) in files.iter().zip(&chosen) {
            let chosen = chosen.expect("every file carries sound");
            assert_eq!(
                tracks
                    .iter()
                    .find(|track| track.stream_index == chosen)
                    .and_then(|track| track.language.as_deref()),
                Some(spoken),
                "every file of the season is listened to in the same language"
            );
        }
    }

    #[test]
    fn a_file_missing_the_shared_language_falls_back_to_the_track_it_calls_its_own() {
        let files = vec![
            vec![a_sound_track(1, Some("fra"), true)],
            vec![a_sound_track(1, Some("fra"), true)],
            vec![
                a_sound_track(1, Some("eng"), false),
                a_sound_track(2, Some("deu"), true),
            ],
        ];
        assert_eq!(
            the_track_to_listen_to(&files),
            vec![Some(1), Some(1), Some(2)],
            "one file short of a language is better compared than dropped"
        );
    }

    #[test]
    fn a_file_stating_no_language_at_all_is_still_listened_to() {
        let files = vec![
            vec![a_picture(0), a_sound_track(1, None, false)],
            vec![a_picture(0), a_sound_track(1, None, false)],
        ];
        assert_eq!(the_track_to_listen_to(&files), vec![Some(1), Some(1)]);
    }

    #[test]
    fn a_season_whose_english_is_an_audio_description_is_listened_to_elsewhere() {
        // The defect this guards: a series carrying French and an English
        // audio description came away with no button on any of its seasons.
        // English is carried by every file and wins the count, but a narrator
        // saying something different over each episode makes the opening the
        // one moment the season shares nothing. French answers.
        let described = |stream_index: i32| {
            let mut track = a_sound_track(stream_index, Some("eng"), true);
            track.title = Some("Audio Description".into());
            track
        };
        let files = vec![
            vec![
                a_picture(0),
                described(1),
                a_sound_track(2, Some("fra"), false),
            ],
            vec![
                a_picture(0),
                described(1),
                a_sound_track(2, Some("fra"), false),
            ],
        ];
        let chosen = the_track_to_listen_to(&files);
        assert_eq!(chosen, vec![Some(2), Some(2)]);
        assert_eq!(which_language(&files, &chosen), Some("fra"));
    }

    #[test]
    fn a_file_carrying_no_sound_has_no_track_to_listen_to() {
        let files = vec![vec![a_picture(0)], vec![a_sound_track(1, None, true)]];
        assert_eq!(the_track_to_listen_to(&files), vec![None, Some(1)]);
    }

    #[test]
    fn the_two_ends_of_a_short_episode_never_meet_in_the_middle() {
        // An episode of seven minutes read four minutes from each end would be
        // read twice over, and its opening found inside its own closing.
        let (beginning, ending) = the_two_ends_of(Some(Millis::new(7 * 60 * 1_000)));
        let (at, how_long) = ending.expect("a seven minute episode has an end");
        assert_eq!(beginning, Millis::new(3 * 60 * 1_000 + 30_000));
        assert_eq!(at, Millis::new(3 * 60 * 1_000 + 30_000));
        assert_eq!(how_long, Millis::new(3 * 60 * 1_000 + 30_000));
        assert_eq!(
            beginning.get(),
            at.get(),
            "the two meet exactly once, at the middle, and never overlap"
        );
    }

    #[test]
    fn a_long_episode_is_read_four_minutes_from_each_end_and_no_more() {
        let (beginning, ending) = the_two_ends_of(Some(Millis::new(45 * 60 * 1_000)));
        assert_eq!(beginning, THE_BEGINNING);
        assert_eq!(
            ending,
            Some((Millis::new(41 * 60 * 1_000), THE_END)),
            "and the end is read from where it really ends"
        );
    }

    #[test]
    fn an_episode_whose_length_nobody_knows_still_has_a_beginning() {
        assert_eq!(the_two_ends_of(None), (THE_BEGINNING, None));
        assert_eq!(the_two_ends_of(Some(Millis::ZERO)), (THE_BEGINNING, None));
    }

    #[test]
    fn an_episode_too_short_to_have_two_ends_is_only_read_from_the_front() {
        let (beginning, ending) = the_two_ends_of(Some(Millis::new(15_000)));
        assert_eq!(beginning, Millis::new(7_500));
        assert_eq!(
            ending, None,
            "half of it is shorter than anything worth a button"
        );
    }

    fn stretch(from: f64, to: f64) -> Stretch {
        Stretch {
            start: Millis::from_seconds_f64(from),
            end: Millis::from_seconds_f64(to),
        }
    }

    #[test]
    fn a_stretch_only_one_pair_saw_is_left_alone_when_more_is_wanted() {
        // One pair agreeing about a piece of music running under two
        // consecutive episodes is a coincidence. It is also exactly what a
        // button over the first scene of an episode looks like.
        let alone = [stretch(30.0, 90.0)];
        assert_eq!(what_they_agree_on(&alone, 2), None);
        assert_eq!(
            what_they_agree_on(&alone, 1),
            Some(stretch(30.0, 90.0)),
            "and with two episodes there is only ever one opinion to be had"
        );
    }

    #[test]
    fn the_stretch_the_most_pairs_agree_on_is_the_one_that_wins() {
        let proposed = [
            stretch(30.0, 90.0),
            stretch(31.0, 92.0),
            stretch(29.0, 88.0),
            // One pair held on far longer than the others, and is outvoted.
            stretch(200.0, 260.0),
        ];
        let agreed = what_they_agree_on(&proposed, 2).expect("three of four agree");
        assert_eq!(
            agreed,
            stretch(30.0, 90.0),
            "the middle of what they agree on, so one odd account moves it by nothing"
        );
    }

    #[test]
    fn two_accounts_of_one_stretch_are_told_apart_from_two_different_stretches() {
        assert!(much_the_same(stretch(30.0, 90.0), stretch(32.0, 94.0)));
        assert!(!much_the_same(stretch(30.0, 90.0), stretch(80.0, 140.0)));
        assert!(!much_the_same(stretch(30.0, 90.0), stretch(90.0, 150.0)));
    }

    #[test]
    fn a_stretch_too_short_or_too_long_is_never_offered_a_button() {
        assert!(!worth_a_button(Millis::new(9_999)));
        assert!(worth_a_button(SHORTEST_WORTH_A_BUTTON));
        assert!(worth_a_button(LONGEST_WORTH_A_BUTTON));
        assert!(
            !worth_a_button(Millis::new(LONGEST_WORTH_A_BUTTON.get() + 1)),
            "two copies of one episode share everything, which is not an opening"
        );
    }

    /// A run of made up sound that moves the way music moves, with its own
    /// rhythm so that two of them never march in step.
    fn a_tune(seconds: f32, seed: u32) -> Vec<i16> {
        let mut next = seed | 1;
        let rate = melyxar_sound::SAMPLES_A_SECOND as usize;
        let count = (seconds * rate as f32) as usize;
        let note = rate * (13 + (seed % 11) as usize) / 100;
        let mut samples = Vec::with_capacity(count);
        let mut pitch = 440.0f32;
        for at in 0..count {
            if at % note == 0 {
                next = next.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                pitch = 400.0 + (next as f32 / u32::MAX as f32) * 2_000.0;
            }
            let moment = at as f32 / rate as f32;
            let turns = 2.0 * std::f32::consts::PI * pitch * moment;
            let value = 0.7 * turns.sin() + 0.3 * (2.5 * turns).sin();
            samples.push((value * 0.4 * f32::from(i16::MAX)) as i16);
        }
        samples
    }

    /// One made up episode, listened to at its beginning only.
    fn an_episode(work_id: WorkId, before: f32, opening: &[i16], seed: u32) -> ListenedTo {
        let mut sound = a_tune(before, seed);
        sound.extend_from_slice(opening);
        sound.extend(a_tune(20.0, seed.wrapping_add(500)));
        ListenedTo {
            work_id,
            source_id: MediaSourceId::new(),
            file_name: format!("Distant.Signal.S01E{seed:02}.mkv"),
            beginning: Listened::of(&sound),
            ending: None,
        }
    }

    fn opening_of(found: &Found) -> Option<&MediaSegment> {
        found
            .segments
            .iter()
            .find(|segment| segment.kind == SegmentKind::Intro)
    }

    #[test]
    fn a_season_whose_episodes_share_an_opening_is_told_where_each_one_holds_it() {
        let opening = a_tune(14.0, 7);
        let episodes: Vec<ListenedTo> = [(1.4f32, 11u32), (4.9, 22), (3.1, 33)]
            .into_iter()
            .map(|(before, seed)| an_episode(WorkId::new(), before, &opening, seed))
            .collect();

        let found = what_a_season_shares(&episodes);
        assert_eq!(found.len(), 3);
        for (one, opens_at) in found.iter().zip([1.4f64, 4.9, 3.1]) {
            let opening = opening_of(one).unwrap_or_else(|| panic!("{one:?} has an opening"));
            assert_eq!(opening.origin, SegmentOrigin::Detected);
            assert!(
                (opening.start.as_seconds_f64() - opens_at).abs() < 1.0,
                "found where it really sits: {opening:?}"
            );
            assert!(
                (opening.end.saturating_sub(opening.start).as_seconds_f64() - 14.0).abs() < 1.5,
                "and for as long as it really lasts: {opening:?}"
            );
        }
    }

    /// What the comparison came to about the opening of one episode.
    fn how_the_opening_went(found: &Found) -> WhatWasShared {
        found
            .ends
            .iter()
            .find(|(kind, _)| *kind == SegmentKind::Intro)
            .map(|(_, what)| *what)
            .expect("every episode answers about its opening")
    }

    #[test]
    fn how_far_a_second_look_reaches_is_a_quarter_of_the_episode_and_ten_minutes_at_most() {
        // A sitcom: a quarter of it reaches past the four minutes read first.
        assert_eq!(
            how_far_in_to_look_again(Some(Millis::new(22 * 60 * 1_000))),
            Some(Millis::new(5 * 60 * 1_000 + 30_000))
        );
        // A long drama, where the ten minutes is what binds.
        assert_eq!(
            how_far_in_to_look_again(Some(Millis::new(60 * 60 * 1_000))),
            Some(THE_BEGINNING_AGAIN)
        );
        // Under sixteen minutes a quarter reaches no further than the four
        // minutes already read, so there is nothing deeper to look at.
        assert_eq!(
            how_far_in_to_look_again(Some(Millis::new(15 * 60 * 1_000))),
            None
        );
        assert_eq!(how_far_in_to_look_again(None), None);
    }

    #[test]
    fn an_opening_ending_hard_against_the_edge_of_what_was_read_asks_for_a_second_look() {
        let ending_at = |seconds: i64| {
            WhatWasShared::AStretch(Stretch {
                start: Millis::new(seconds * 1_000 - 20_000),
                end: Millis::new(seconds * 1_000),
            })
        };
        // Three minutes fifty nine, which is where some thirty files of a real
        // collection stopped: the edge, not the opening, is what ended it.
        assert!(ending_at(239).a_longer_look_might_help());
        // And one that ends well before the edge is the opening really ending.
        assert!(!ending_at(200).a_longer_look_might_help());
        // Every kind of nothing is worth looking further for.
        assert!(WhatWasShared::Nothing.a_longer_look_might_help());
        assert!(WhatWasShared::TooShort(Millis::new(3_000)).a_longer_look_might_help());
    }

    #[test]
    fn a_season_sharing_a_stretch_too_short_for_a_button_says_so_rather_than_saying_nothing() {
        // The defect this exists for: a season came away with no buttons and
        // the journal said its episodes shared nothing with their neighbours,
        // when in truth they shared a title card of a few seconds that was
        // thrown away without a word. The two are not the same thing, and the
        // only way to tell them apart was to ask the person who owns the
        // files. Now the line says which it was, and how long.
        let card = a_tune(5.0, 7);
        let episodes: Vec<ListenedTo> = [(1.4f32, 11u32), (4.9, 22), (3.1, 33)]
            .into_iter()
            .map(|(before, seed)| an_episode(WorkId::new(), before, &card, seed))
            .collect();

        for one in what_a_season_shares(&episodes) {
            assert!(
                one.segments.is_empty(),
                "five seconds is not a button: {one:?}"
            );
            let how = how_the_opening_went(&one);
            let WhatWasShared::TooShort(length) = how else {
                panic!("what they share is too short, and the line has to say so: {how:?}");
            };
            assert!(
                (length.as_seconds_f64() - 5.0).abs() < 1.5,
                "and how long it was: {how:?}"
            );
        }
    }

    #[test]
    fn a_season_whose_episodes_share_nothing_is_told_nothing() {
        let episodes: Vec<ListenedTo> = [11u32, 22, 33]
            .into_iter()
            .map(|seed| {
                an_episode(
                    WorkId::new(),
                    2.0,
                    &a_tune(14.0, seed.wrapping_add(900)),
                    seed,
                )
            })
            .collect();

        for one in what_a_season_shares(&episodes) {
            assert!(one.segments.is_empty(), "{one:?}");
            // And nothing is what it is told. These episodes do in fact agree
            // about eight tenths of a second somewhere, as any two stretches
            // of sound will, and calling that a refused title card would put
            // a misleading line under every season that shares nothing.
            let how = how_the_opening_went(&one);
            assert!(
                matches!(how, WhatWasShared::Nothing),
                "a coincidence is reported as the nothing it is: {how:?}"
            );
        }
    }

    #[test]
    fn two_copies_of_one_episode_are_never_compared_with_each_other() {
        // Two copies hold the same sound from end to end, so compared with
        // each other they share the whole episode. A season of two episodes
        // held in two copies each would otherwise come away with a button
        // over all of it.
        let one = WorkId::new();
        let other = WorkId::new();
        let episodes = vec![
            an_episode(one, 1.0, &a_tune(14.0, 700), 11),
            an_episode(one, 1.0, &a_tune(14.0, 700), 11),
            an_episode(other, 1.0, &a_tune(14.0, 800), 22),
            an_episode(other, 1.0, &a_tune(14.0, 800), 22),
        ];

        for found in what_a_season_shares(&episodes) {
            assert!(
                opening_of(&found).is_none(),
                "the two copies of one episode were compared with each other: {found:?}"
            );
        }
    }

    #[test]
    fn a_list_of_files_says_how_many_episodes_it_is_copies_of() {
        let one = WorkId::new();
        let other = WorkId::new();
        let files: Vec<EpisodeToListenTo> = [one, one, other]
            .into_iter()
            .map(|work_id| EpisodeToListenTo {
                work_id,
                number: None,
                source_id: MediaSourceId::new(),
                path: std::path::PathBuf::from("/x.mkv"),
                duration: None,
            })
            .collect();
        assert_eq!(how_many_episodes(&files), 2);
        assert_eq!(how_many_episodes(&[]), 0);
    }
}
