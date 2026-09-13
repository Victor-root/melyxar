//! Background work: what is running, and asking for more.
//!
//! A scan started from a button and a scan started from a terminal are the
//! same job, written down the same way and stopped the same way. That is the
//! whole point of the job layer, and it is why nothing here does any work of
//! its own beyond translating.

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
            "/api/v1/works/{id}/candidates",
            axum::routing::get(candidates),
        )
        .route("/api/v1/works/{id}/identify", axum::routing::post(choose))
}

#[derive(Debug, Serialize)]
struct JobView {
    id: String,
    kind: &'static str,
    state: &'static str,
    /// What the job is about, such as a library.
    target: Option<String>,
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

async fn jobs(State(state): State<AppState>) -> Result<Json<JobsView>> {
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
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<StartedView>> {
    let library = library_of(&state, &id).await?;
    // A scan asked for from a screen looks up what it found, because a person
    // pressing one button expects one thing to happen and that thing is a
    // filled library, not a list of file names.
    let job = melyxar_app::scan::start_scan_and_identification(&state, library)
        .await
        .map_err(already_running)?;
    Ok(Json(StartedView {
        job_id: job.to_string(),
    }))
}

/// Asks for the works still waiting to be looked up.
async fn start_identification(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<StartedView>> {
    let library = library_of(&state, &id).await?;
    let provider = state.metadata_provider().ok_or_else(|| {
        ServerError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            melyxar_core::error::ErrorCode::ExternalServiceUnavailable,
            "no metadata provider is available",
        )
    })?;

    let job = melyxar_app::identify::start_identification(&state, provider, library)
        .await
        .map_err(already_running)?;
    Ok(Json(StartedView {
        job_id: job.id().to_string(),
    }))
}

#[derive(Debug, Deserialize)]
struct Asked {
    /// What to look for. Empty falls back to what the work is called, which is
    /// the first thing anybody would try.
    query: Option<String>,
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
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(asked): Query<Asked>,
) -> Result<Json<Vec<CandidateView>>> {
    let work_id = parse_work(&id)?;
    let provider = provider_of(&state)?;

    let query = match asked.query.map(|value| value.trim().to_string()) {
        Some(query) if !query.is_empty() => query,
        _ => {
            state
                .database()
                .work(work_id)
                .await
                .map_err(internal)?
                .ok_or_else(|| ServerError::not_found("no work with that identifier"))?
                .title
        }
    };

    let found = melyxar_app::identify::candidates_for(&state, &provider, work_id, &query).await?;
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
}

/// Records the film a person picked, which no later run undoes.
async fn choose(
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
    )
    .await?;

    Ok(Json(ChosenView { identified: true }))
}

#[derive(Debug, Serialize)]
struct ChosenView {
    identified: bool,
}

fn parse_work(id: &str) -> Result<melyxar_core::id::WorkId> {
    id.parse()
        .map_err(|_| ServerError::invalid_input("the work identifier is malformed"))
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

/// Asks a job to stop.
async fn cancel(
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
async fn forget(State(state): State<AppState>) -> Result<Json<ForgottenView>> {
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
    use melyxar_core::job::{Job, JobKind, JobPriority, JobState};

    fn job(done: i64, total: Option<i64>, state: JobState) -> Job {
        Job {
            id: JobId::new(),
            kind: JobKind::ScanLibrary,
            priority: JobPriority::REQUESTED,
            state,
            target_id: Some("films".to_string()),
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
