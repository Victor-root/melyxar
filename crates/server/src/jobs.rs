//! Background work: what is running, and asking for more.
//!
//! A scan started from a button and a scan started from a terminal are the
//! same job, written down the same way and stopped the same way. That is the
//! whole point of the job layer, and it is why nothing here does any work of
//! its own beyond translating.

use crate::identifiers::parse_work;
use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use melyxar_app::metadata::MetadataProvider;
use melyxar_app::AppState;
use melyxar_core::id::{JobId, LibraryId};
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/jobs", axum::routing::get(jobs))
        .route("/api/v1/jobs/{id}/cancel", axum::routing::post(cancel))
        .route("/api/v1/jobs/finished", axum::routing::delete(forget))
        .route(
            "/api/v1/libraries/{id}/scan",
            axum::routing::post(start_scan),
        )
        .route(
            "/api/v1/libraries/{id}/identify",
            axum::routing::post(start_identification),
        )
        .route(
            "/api/v1/libraries/{id}/options",
            axum::routing::put(set_library_options),
        )
        .route("/api/v1/upkeep", axum::routing::get(upkeep))
        // One name, two verbs: reading what the server is set to do and
        // saying what it is to do from now on.
        .route(
            "/api/v1/settings/libraries",
            axum::routing::get(library_work).put(set_library_work),
        )
        .route("/api/v1/upkeep/run", axum::routing::post(run_the_upkeep))
        .route(
            "/api/v1/libraries/{id}/upkeep/{task}",
            axum::routing::post(run_one_upkeep_task),
        )
        .route(
            "/api/v1/works/{id}/candidates",
            axum::routing::get(candidates),
        )
        .route("/api/v1/works/{id}/identify", axum::routing::post(choose))
        .route(
            "/api/v1/copies/{id}/detach",
            axum::routing::post(detach_copy),
        )
        .route(
            "/api/v1/copies/{id}/read-again",
            axum::routing::post(read_copy_again),
        )
}

#[derive(Debug, Serialize)]
struct JobView {
    id: String,
    kind: &'static str,
    state: &'static str,
    /// What the job is about, such as a library.
    target: Option<String>,
    /// Which pass the job is on, when it has said. The counters below are
    /// counting that pass and nothing else, which is what a screen has to say
    /// out loud when a bar drops back to nothing and sets off again.
    step: Option<&'static str>,
    /// The name of the file it is on at this very moment, when the pass says
    /// so. A pass name and a bar do not tell a server that is working from one
    /// that is stuck on a four hour film; this does.
    doing: Option<String>,
    done: i64,
    /// Absent until the work has been sized up. A bar showing nothing beats a
    /// bar showing a number that was invented.
    total: Option<i64>,
    /// Between zero and one, absent for the same reason.
    ratio: Option<f64>,
    /// Set for a job that failed, and meant to be read.
    failure_reason: Option<String>,
}

#[derive(Debug, Serialize)]
struct JobsView {
    /// What is running or waiting, the awaited ones first.
    running: Vec<JobView>,
    /// The latest jobs whatever became of them, so a screen can show what
    /// happened rather than only what is happening.
    recent: Vec<JobView>,
}

/// How many finished jobs a screen is shown.
const RECENT: i64 = 20;

async fn jobs(_: crate::account::Administrator, State(state): State<AppState>) -> Result<Json<JobsView>> {
    let database = state.database();
    let running = database
        .unfinished_jobs()
        .await
        .map_err(internal)?
        .iter()
        .map(job_view)
        .collect();
    let recent = database
        .recent_jobs(RECENT)
        .await
        .map_err(internal)?
        .iter()
        .filter(|job| job.state.is_finished())
        .map(job_view)
        .collect();

    Ok(Json(JobsView { running, recent }))
}

fn job_view(job: &melyxar_core::job::Job) -> JobView {
    JobView {
        id: job.id.to_string(),
        kind: job.kind.as_str(),
        state: job.state.as_str(),
        target: job.target_id.clone(),
        step: job.step.map(|step| step.as_str()),
        doing: job.doing.clone(),
        done: job.progress_done,
        total: job.progress_total,
        ratio: job.ratio(),
        failure_reason: job.failure_reason.clone(),
    }
}

#[derive(Debug, Serialize)]
struct StartedView {
    job_id: String,
}

/// Asks for a scan of one library.
///
/// Answers with the job rather than with the result: a scan takes minutes and
/// a request that waits for it is a request that times out. The job is then
/// followed, and stopped, like any other.
async fn start_scan(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(asked): Query<HowMuch>,
) -> Result<Json<StartedView>> {
    let library = library_of(&state, &id).await?;
    // A scan asked for from a screen looks up what it found, because a person
    // pressing one button expects one thing to happen and that thing is a
    // filled library, not a list of file names.
    let job = melyxar_app::scan::start_scan_and_identification(
        &state,
        library,
        melyxar_core::job::JobPriority::REQUESTED,
        asked.mode()?,
    )
    .await
    .map_err(already_running)?;
    Ok(Json(StartedView {
        job_id: job.to_string(),
    }))
}

/// How much of a library a run is asked to go over.
///
/// Left out means the usual one. A word nobody knows is refused rather than
/// read as the usual one: somebody who wrote a mode meant a mode, and quietly
/// doing something else is how a library ends up not being refreshed while
/// every screen says it was.
#[derive(Debug, Deserialize)]
struct HowMuch {
    mode: Option<String>,
}

impl HowMuch {
    fn mode(&self) -> Result<melyxar_core::refresh::RefreshMode> {
        match self.mode.as_deref() {
            None => Ok(melyxar_core::refresh::RefreshMode::default()),
            Some(word) => melyxar_core::refresh::RefreshMode::parse(word)
                .ok_or_else(|| ServerError::invalid_input("that is not a way of refreshing")),
        }
    }
}

/// Asks for the works still waiting to be looked up.
async fn start_identification(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(asked): Query<HowMuch>,
) -> Result<Json<StartedView>> {
    let library = library_of(&state, &id).await?;
    let provider = provider_of(&state)?;

    let job = melyxar_app::identify::start_identification(&state, provider, library, asked.mode()?)
        .await
        .map_err(already_running)?;
    Ok(Json(StartedView {
        job_id: job.id().to_string(),
    }))
}

/// What to look for. Every part of it narrows, and all of it is optional: a
/// name left empty falls back to what the work is called, which is the first
/// thing anybody would try.
#[derive(Debug, Deserialize)]
struct Asked {
    query: Option<String>,
    year: Option<i32>,
    imdb_id: Option<String>,
    provider_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct CandidateView {
    external_id: String,
    title: String,
    original_title: Option<String>,
    year: Option<i32>,
    overview: Option<String>,
    /// Full address of a small poster, so the list is recognisable at a glance
    /// rather than being a column of titles that all look alike.
    poster: Option<String>,
}

/// Films a person could mean, for a work nobody recognised.
async fn candidates(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(asked): Query<Asked>,
) -> Result<Json<Vec<CandidateView>>> {
    let work_id = parse_work(&id)?;
    let provider = provider_of(&state)?;

    let found = melyxar_app::identify::candidates_for(
        &state,
        &provider,
        work_id,
        melyxar_app::identify::SearchCriteria {
            name: asked.query,
            year: asked.year,
            imdb_id: asked.imdb_id,
            provider_id: asked.provider_id,
        },
    )
    .await?;
    Ok(Json(
        found
            .iter()
            .map(|candidate| CandidateView {
                external_id: candidate.external_id.clone(),
                title: candidate.title.clone(),
                original_title: candidate.original_title.clone(),
                year: candidate.release_year,
                overview: candidate.overview.clone(),
                poster: candidate
                    .poster_path
                    .as_deref()
                    .map(|path| provider.image_url(path)),
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
struct Chosen {
    external_id: String,
    /// Whether the pictures already held are made again from the work just
    /// chosen. Left out, they are: correcting a work that was wholly the wrong
    /// work is the ordinary case, and a poster of the wrong film is the most
    /// visible part of it.
    #[serde(default = "yes")]
    replace_pictures: bool,
}

/// What a field left out of the body means, where leaving it out has to mean
/// something rather than nothing.
const fn yes() -> bool {
    true
}

/// Records the film a person picked, which no later run undoes.
async fn choose(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(chosen): Json<Chosen>,
) -> Result<Json<ChosenView>> {
    let work_id = parse_work(&id)?;
    let provider = provider_of(&state)?;
    let work = state
        .database()
        .work(work_id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::not_found("no work with that identifier"))?;

    melyxar_app::identify::identify_by_hand(
        &state,
        &provider,
        work.library_id,
        work_id,
        &chosen.external_id,
        chosen.replace_pictures,
    )
    .await?;

    Ok(Json(ChosenView { identified: true }))
}

#[derive(Debug, Serialize)]
struct ChosenView {
    identified: bool,
}

/// Takes one copy away from the film it sits on, as a film of its own.
///
/// Copies are put together without anybody asking, so somebody has to be able
/// to say it was wrong from the page that shows it.
async fn detach_copy(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DetachedView>> {
    let source_id = id
        .parse()
        .map_err(|_| ServerError::invalid_input("the copy identifier is malformed"))?;

    let detached = melyxar_app::scan::detach_copy(&state, source_id)
        .await?
        .ok_or_else(|| {
            ServerError::invalid_input("this film holds one copy, so there is nothing to take away")
        })?;

    Ok(Json(DetachedView {
        work_id: detached.to_string(),
    }))
}

/// Reads one file again for what it says about itself.
///
/// Its own route rather than a scan of the whole library, because a scan opens
/// only what changed on disk and would walk straight past this file. What a
/// person wants here is one file read again, now.
async fn read_copy_again(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<StartedView>> {
    let source_id = id
        .parse()
        .map_err(|_| ServerError::invalid_input("the copy identifier is malformed"))?;

    let job = melyxar_app::scan::read_copy_again(&state, source_id)
        .await
        .map_err(already_running)?
        .ok_or_else(|| ServerError::invalid_input("there is no such copy"))?;

    Ok(Json(StartedView {
        job_id: job.to_string(),
    }))
}

#[derive(Debug, Serialize)]
struct DetachedView {
    /// Where the copy went, so a page can go and look at it.
    work_id: String,
}

fn provider_of(
    state: &AppState,
) -> Result<std::sync::Arc<impl melyxar_app::metadata::MetadataProvider + 'static>> {
    state.metadata_provider().ok_or_else(|| {
        ServerError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            melyxar_core::error::ErrorCode::ExternalServiceUnavailable,
            "no metadata provider is available",
        )
    })
}

#[derive(Debug, Deserialize)]
struct OptionsAsked {
    key_frames_during_scan: bool,
    thumbnails_during_scan: bool,
    /// The language this library's films are described in, as a two letter
    /// code. Changing it asks the provider about every film again.
    metadata_language: String,
}

#[derive(Debug, Serialize)]
struct OptionsView {
    key_frames_during_scan: bool,
    thumbnails_during_scan: bool,
    metadata_language: String,
    /// Whether anything really moved. A screen that sent what was already
    /// there gets a plain no rather than a second copy of the same answer.
    changed: bool,
    /// How many films went back in the queue to be described again, when the
    /// language is what changed. A number somebody is owed: it is the size of
    /// what they just set going.
    asked_about_again: Option<u64>,
}

/// A language code the provider can be asked in.
///
/// Two letters, which is what the provider takes and what a library has always
/// been declared with. Refused rather than corrected: a code nobody can use
/// would leave a library described in nothing at all, and the screen offers a
/// list rather than a text field, so anything else is a client with a defect.
fn language_of(asked: &str) -> Result<String> {
    let language = asked.trim().to_lowercase();
    if language.len() != 2 || !language.chars().all(|letter| letter.is_ascii_lowercase()) {
        return Err(ServerError::invalid_input(
            "a language is two letters, such as fr or en",
        ));
    }
    Ok(language)
}

/// Says what a scan of this library does, and what language it is described in.
///
/// Everything travels together, because it is one screen and one answer:
/// sending half of it would leave the other half to be guessed at, and the
/// guess would be wrong every other time.
async fn set_library_options(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(asked): Json<OptionsAsked>,
) -> Result<Json<OptionsView>> {
    let library = library_of(&state, &id).await?;
    let language = language_of(&asked.metadata_language)?;
    let options = melyxar_core::library::LibraryOptions {
        key_frames_during_scan: asked.key_frames_during_scan,
        thumbnails_during_scan: asked.thumbnails_during_scan,
    };
    let changed = state
        .database()
        .set_library_options(library.id, options)
        .await
        .map_err(internal)?;

    // After the switches and not before: a language that changes sets a run
    // going, and a run that started while the switches failed to be written
    // would be a run under settings nobody asked for.
    let asked_about_again =
        melyxar_app::identify::change_the_language_of(&state, &library, &language).await?;

    // A switch flipped on a screen and a switch the server took are two
    // different things, and the only difference a person sees is a scan that
    // behaves as it did before.
    tracing::debug!(
        library = library.name,
        key_frames_during_scan = options.key_frames_during_scan,
        thumbnails_during_scan = options.thumbnails_during_scan,
        metadata_language = language,
        changed,
        asked_about_again,
        "what a scan of this library does was set"
    );

    Ok(Json(OptionsView {
        key_frames_during_scan: options.key_frames_during_scan,
        thumbnails_during_scan: options.thumbnails_during_scan,
        metadata_language: language,
        changed: changed || asked_about_again.is_some(),
        asked_about_again,
    }))
}

#[derive(Debug, Serialize)]
struct UpkeepTaskView {
    task: &'static str,
    library: String,
    library_name: String,
    /// Files still waiting. Nought means there is nothing to start.
    waiting: i64,
    /// Files already done, so a screen says four hundred of four hundred and
    /// ten rather than ten.
    done: i64,
    /// Whether the two numbers above count seasons rather than files.
    ///
    /// Listening for the titles a season shares is done season by season, so
    /// a screen saying files would be counting something nobody can act on.
    counts_seasons: bool,
    /// Whether the scan of that library does this reading itself.
    during_the_scan: bool,
    /// Whether it is running right now, so a screen offers to watch rather
    /// than to start.
    under_way: bool,
    /// When this reading last ran to an end here, as an instant, or nothing
    /// when it never has. A reading that never ran and one that ran last night
    /// and found nothing look alike without it.
    last_run: Option<String>,
    /// How that run ended, in the words every other job uses.
    last_run_state: Option<&'static str>,
    /// How long it took, in seconds.
    last_run_seconds: Option<i64>,
}

#[derive(Debug, Serialize)]
struct UpkeepView {
    tasks: Vec<UpkeepTaskView>,
    /// When the upkeep next runs on its own, written as an instant so that
    /// whoever reads it sees it in their own hour rather than in the server's.
    /// Absent when it never does.
    next_run: Option<String>,
    /// What the server does with a library, which this screen also sets.
    settings: WorkView,
}

/// What the upkeep has left to do, and when it will next do it on its own.
async fn upkeep(_: crate::account::Administrator, State(state): State<AppState>) -> Result<Json<UpkeepView>> {
    let work = state.database().library_work().await.map_err(internal)?;
    let left = melyxar_app::upkeep::what_is_left(&state).await?;

    Ok(Json(UpkeepView {
        tasks: left
            .into_iter()
            .map(|entry| UpkeepTaskView {
                task: entry.task.as_str(),
                library: entry.library.to_string(),
                library_name: entry.library_name,
                waiting: entry.waiting,
                done: entry.done,
                counts_seasons: entry.task.counts_seasons(),
                during_the_scan: entry.during_the_scan,
                under_way: entry.under_way,
                last_run: entry
                    .last_run
                    .as_ref()
                    .map(|last| melyxar_core::time::to_text(last.at)),
                last_run_state: entry.last_run.as_ref().map(|last| last.state.as_str()),
                last_run_seconds: entry.last_run.as_ref().and_then(|last| last.took_seconds),
            })
            .collect(),
        next_run: work.upkeep_nightly.then(|| {
            melyxar_core::time::to_text(melyxar_core::time::next_occurrence_of_utc_minutes(
                work.upkeep_at_utc_minutes,
            ))
        }),
        settings: work_view(work),
    }))
}

/// What the server does with a library, on its own and in what shape.
///
/// Every one of these was a line of the configuration file, which meant a
/// terminal, a text editor and a restart to change one. None of them is a
/// property of the machine: they are what somebody wants done with their
/// films.
#[derive(Debug, Serialize, Deserialize)]
struct WorkView {
    /// Read the description files some collections keep next to a film.
    read_companion_files: bool,
    /// Whether the thumbnails of the playback bar are made at all.
    thumbnails_enabled: bool,
    /// How far apart in the film two of them stand.
    thumbnails_every_seconds: i64,
    thumbnails_height: i64,
    thumbnails_columns: i64,
    thumbnails_rows: i64,
    /// Whether the upkeep runs on its own of a night.
    upkeep_nightly: bool,
    /// When it does, in minutes since midnight, **in UTC**. Whatever shows it
    /// turns it into the time of whoever is looking; the server keeps the one
    /// clock it can read with certainty.
    upkeep_at_utc_minutes: i64,
}

fn work_view(work: melyxar_app::settings::LibraryWork) -> WorkView {
    WorkView {
        read_companion_files: work.read_companion_files,
        thumbnails_enabled: work.thumbnails_enabled,
        thumbnails_every_seconds: work.thumbnails_every_seconds,
        thumbnails_height: work.thumbnails_height,
        thumbnails_columns: work.thumbnails_columns,
        thumbnails_rows: work.thumbnails_rows,
        upkeep_nightly: work.upkeep_nightly,
        upkeep_at_utc_minutes: work.upkeep_at_utc_minutes,
    }
}

/// What the server is set to do with a library.
///
/// Its own route rather than a corner of the upkeep's: a screen of settings
/// wants one row of the settings, and asking the upkeep would count every
/// film of every library to answer it.
async fn library_work(_: crate::account::Administrator, State(state): State<AppState>) -> Result<Json<WorkView>> {
    Ok(Json(work_view(
        state.database().library_work().await.map_err(internal)?,
    )))
}

/// Says what the server is to do with a library from now on.
///
/// Every value travels together, because they are one screen and one answer.
/// What comes back is what was kept, which is not always what was asked for: a
/// shape that cannot hold a thumbnail is brought back into range rather than
/// refused, so a screen with a defect cannot leave a server making nothing.
///
/// Changing the shape of the thumbnails puts every film back in front of the
/// upkeep, since what is already made no longer answers what is asked for. The
/// screen says so before the change; nothing is thrown away, and a shape
/// somebody changes back is found again as it stands.
async fn set_library_work(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Json(asked): Json<WorkView>,
) -> Result<Json<WorkView>> {
    let kept = state
        .database()
        .save_library_work(melyxar_app::settings::LibraryWork {
            read_companion_files: asked.read_companion_files,
            thumbnails_enabled: asked.thumbnails_enabled,
            thumbnails_every_seconds: asked.thumbnails_every_seconds,
            thumbnails_height: asked.thumbnails_height,
            thumbnails_columns: asked.thumbnails_columns,
            thumbnails_rows: asked.thumbnails_rows,
            upkeep_nightly: asked.upkeep_nightly,
            upkeep_at_utc_minutes: asked.upkeep_at_utc_minutes,
        })
        .await
        .map_err(internal)?;

    tracing::debug!(
        read_companion_files = kept.read_companion_files,
        thumbnails_enabled = kept.thumbnails_enabled,
        thumbnails_every_seconds = kept.thumbnails_every_seconds,
        thumbnails_height = kept.thumbnails_height,
        thumbnails_columns = kept.thumbnails_columns,
        thumbnails_rows = kept.thumbnails_rows,
        upkeep_nightly = kept.upkeep_nightly,
        upkeep_at_utc_minutes = kept.upkeep_at_utc_minutes,
        "what the server does with a library was set"
    );
    Ok(Json(work_view(kept)))
}

#[derive(Debug, Serialize)]
struct StartedManyView {
    /// How many jobs this started. Nought is an answer: there was nothing
    /// waiting, which is what somebody pressing the button wanted to know.
    started: usize,
}

/// Starts everything the upkeep has waiting, now, on every library.
///
/// At the priority of something asked for: whoever pressed this is not waiting
/// for the night, which is the whole reason the button exists.
async fn run_the_upkeep(_: crate::account::Administrator, State(state): State<AppState>) -> Result<Json<StartedManyView>> {
    let started = melyxar_app::upkeep::start_what_is_waiting(
        &state,
        melyxar_core::job::JobPriority::REQUESTED,
    )
    .await;
    // Nought is the commonest answer here and the one worth writing down: the
    // button did work, there was simply nothing waiting, and from the outside
    // that is indistinguishable from a button that does nothing.
    tracing::debug!(jobs = started, "the upkeep was asked for from a screen");
    Ok(Json(StartedManyView { started }))
}

/// Starts one of the two readings on one library, now.
async fn run_one_upkeep_task(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path((id, task)): Path<(String, String)>,
) -> Result<Json<StartedView>> {
    let task = melyxar_app::upkeep::UpkeepTask::parse(&task)
        .ok_or_else(|| ServerError::invalid_input("there is no such upkeep task"))?;
    let library = library_of(&state, &id).await?;

    let job = melyxar_app::upkeep::start(
        &state,
        task,
        library,
        melyxar_core::job::JobPriority::REQUESTED,
    )
    .await
    .map_err(already_running)?;

    Ok(Json(StartedView {
        job_id: job.id.to_string(),
    }))
}

/// Asks a job to stop.
async fn cancel(
    _: crate::account::Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<StoppedView>> {
    let job_id: JobId = id
        .parse()
        .map_err(|_| ServerError::invalid_input("the job identifier is malformed"))?;

    Ok(Json(StoppedView {
        // A job that is not running is not an error: it may have finished
        // between the screen being drawn and the button being pressed.
        stopped: state.jobs().cancel(job_id),
    }))
}

#[derive(Debug, Serialize)]
struct StoppedView {
    stopped: bool,
}

/// Forgets the work that is over.
///
/// A history that cannot be cleared stops being read: the run that matters is
/// the last one, not the four hundred before it. What is still running stays,
/// because it is not history yet.
async fn forget(_: crate::account::Administrator, State(state): State<AppState>) -> Result<Json<ForgottenView>> {
    Ok(Json(ForgottenView {
        forgotten: state
            .database()
            .forget_finished_jobs()
            .await
            .map_err(internal)?,
    }))
}

#[derive(Debug, Serialize)]
struct ForgottenView {
    forgotten: u64,
}

async fn library_of(state: &AppState, id: &str) -> Result<melyxar_core::library::Library> {
    let library_id: LibraryId = id
        .parse()
        .map_err(|_| ServerError::invalid_input("the library identifier is malformed"))?;

    state
        .database()
        .list_libraries()
        .await
        .map_err(internal)?
        .into_iter()
        .find(|library| library.id == library_id)
        .ok_or_else(|| ServerError::not_found("no library with that identifier"))
}

/// A second scan of the same library is refused rather than run alongside the
/// first, and that refusal is the answer, not a fault.
fn already_running(error: melyxar_app::AppError) -> ServerError {
    match error {
        melyxar_app::AppError::Jobs(melyxar_jobs::JobError::AlreadyUnderWay) => ServerError::new(
            axum::http::StatusCode::CONFLICT,
            melyxar_core::error::ErrorCode::Conflict,
            "a job of that kind is already under way on this library",
        ),
        other => ServerError::internal(other.to_string()),
    }
}

fn internal(error: impl std::fmt::Display) -> ServerError {
    ServerError::internal(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_job_says_which_file_it_is_on_right_now() {
        // A pass name and a bar do not tell a server that is working from one
        // stuck on a four hour film. Only this does, and until it was here the
        // one way to know was to open the journal.
        let view = job_view(&job(3, Some(10), JobState::Running));
        assert_eq!(view.doing.as_deref(), Some("Quiet.Harbour.2019.mkv"));

        let mut between_films = job(3, Some(10), JobState::Running);
        between_films.doing = None;
        assert_eq!(
            job_view(&between_films).doing,
            None,
            "a pass that has not said is a row with nothing in that place"
        );
    }

    /// Every name a job can carry has a sentence in the interface, in both
    /// languages.
    ///
    /// A name with no sentence behind it reaches the screen as the name
    /// itself. That is exactly what happened the evening a pass was added to
    /// the scan: the activity screen read `jobs.step.making_thumbnails` while
    /// three hundred films were being read, and nothing else said what was
    /// going on. The words live in the interface and the names live here, so
    /// nothing but a test crossing from one to the other can catch it.
    #[test]
    fn every_name_a_job_carries_has_words_in_both_languages() {
        let words = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/i18n.ts"),
        )
        .expect("the words of the interface");

        let said_twice = |key: &str| {
            assert_eq!(
                words.matches(&format!("\"{key}\":")).count(),
                2,
                "{key} needs a sentence in English and one in French"
            );
        };
        for kind in melyxar_core::job::JobKind::ALL {
            said_twice(&format!("jobs.{}", kind.as_str()));
        }
        for step in melyxar_core::job::JobStep::ALL {
            said_twice(&format!("jobs.step.{}", step.as_str()));
        }
        for state in [
            "queued",
            "running",
            "succeeded",
            "failed",
            "cancelled",
            "interrupted",
        ] {
            said_twice(&format!("jobs.state.{state}"));
        }
        // A mode reaches a screen as a choice somebody has to make, and one
        // with no words behind it reaches it as `refresh.what_is_missing`.
        // Both the name and the sentence saying what it costs: these three
        // differ by hours of work, and Jellyfin's own users have asked for
        // years what its three actually do.
        for mode in melyxar_core::refresh::RefreshMode::ALL {
            said_twice(&format!("refresh.{}", mode.as_str()));
            said_twice(&format!("refresh.{}_why", mode.as_str()));
        }
        for task in melyxar_app::upkeep::UpkeepTask::ALL {
            said_twice(&format!("upkeep.{}", task.as_str()));
            said_twice(&format!("upkeep.{}_why", task.as_str()));
        }
    }

    use melyxar_core::job::{Job, JobKind, JobPriority, JobState};

    fn job(done: i64, total: Option<i64>, state: JobState) -> Job {
        Job {
            id: JobId::new(),
            kind: JobKind::ScanLibrary,
            priority: JobPriority::REQUESTED,
            state,
            target_id: Some("films".to_string()),
            step: Some(melyxar_core::job::JobStep::AnalysingFiles),
            doing: Some("Quiet.Harbour.2019.mkv".to_string()),
            progress_done: done,
            progress_total: total,
            failure_reason: None,
            created_at: melyxar_core::time::now(),
            started_at: None,
            finished_at: None,
        }
    }

    #[test]
    fn a_job_that_has_been_sized_up_carries_how_far_it_got() {
        let view = job_view(&job(25, Some(50), JobState::Running));
        assert_eq!(view.done, 25);
        assert_eq!(view.total, Some(50));
        assert_eq!(view.ratio, Some(0.5));
        assert_eq!(view.state, "running");
        assert_eq!(
            view.step,
            Some("analysing_files"),
            "the numbers are counting a pass, and the pass has to travel with them"
        );
    }

    #[test]
    fn a_job_nobody_has_sized_up_shows_nothing_rather_than_a_number_it_invented() {
        let view = job_view(&job(3, None, JobState::Running));
        assert_eq!(view.total, None);
        assert_eq!(view.ratio, None);
    }

    #[test]
    fn a_job_that_failed_hands_over_the_reason_to_be_read() {
        let mut failed = job(0, None, JobState::Failed);
        failed.failure_reason = Some("the root is not usable".to_string());
        let view = job_view(&failed);

        assert_eq!(view.state, "failed");
        assert_eq!(
            view.failure_reason.as_deref(),
            Some("the root is not usable")
        );
    }

    #[test]
    fn a_second_scan_of_one_library_is_answered_rather_than_treated_as_a_fault() {
        let refused = already_running(melyxar_app::AppError::Jobs(
            melyxar_jobs::JobError::AlreadyUnderWay,
        ));
        let rendered = format!("{:?}", refused.code());
        assert_eq!(rendered, "\"conflict\"");
    }
}
