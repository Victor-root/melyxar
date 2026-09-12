//! Identifying works: asking a provider what a file is about.
//!
//! Kept apart from the scan on purpose. A scan touches the disk and must
//! finish quickly; identification talks to someone else's server and may well
//! fail today and work tomorrow. Splitting them is what lets a library be
//! browsable minutes after a first scan, and what lets identification be
//! replayed on its own and on a subset.
//!
//! Nothing here believes a provider outright. A file name suggests a title and
//! a year, the provider offers candidates, and the choice between them is made
//! here by rules that can be read and argued with.

use std::sync::Arc;

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::job::{JobKind, JobPriority, JobState};
use melyxar_core::library::Library;
use melyxar_core::privacy::MediaName;
use melyxar_core::work::Work;
use melyxar_database::metadata::{
    CollectionRecord, CreditRecord, IdentifiedWork, RemoteTrailerRecord,
};
use melyxar_jobs::{JobHandle, StartedJob};
use melyxar_library::naming;
use melyxar_metadata::{MetadataProvider, MovieCandidate, MovieDetails, ProviderError};

use crate::{AppError, AppState, Result};

/// What an identification run did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdentifyReport {
    pub identified: usize,
    /// Works the provider knew nothing about. They stay in the library with a
    /// marker rather than being set aside.
    pub unidentified: usize,
    /// Works left untouched because the provider could not be reached. They
    /// are still waiting, so the next run picks them up.
    pub postponed: usize,
    pub cancelled: bool,
}

/// How many works one run looks at.
///
/// A run that never ends cannot be followed, and the next one continues where
/// this one stopped.
const BATCH: i64 = 200;

/// Identifies the works of a library that are still waiting.
pub async fn identify_library(
    state: &AppState,
    provider: &impl MetadataProvider,
    library: &Library,
    handle: &JobHandle,
) -> Result<IdentifyReport> {
    let database = state.database();
    let waiting = database
        .works_awaiting_identification(library.id, BATCH)
        .await?;

    let mut report = IdentifyReport::default();
    if waiting.is_empty() {
        return Ok(report);
    }
    handle.set_total(waiting.len() as i64).await;

    for work in waiting {
        if handle.is_cancelled() {
            report.cancelled = true;
            break;
        }

        match identify_one(state, provider, library, &work).await? {
            Outcome::Identified => report.identified += 1,
            Outcome::Unknown => {
                database.mark_work_unidentified(work.id).await?;
                report.unidentified += 1;
            }
            Outcome::Postponed => report.postponed += 1,
            // Every other work would meet the same wall, and none of them are
            // the problem. Stopping here says what is actually wrong.
            Outcome::Refused => {
                return Err(AppError::Domain(melyxar_core::Error::invalid_input(
                    "the metadata provider refused the key; check it in the configuration",
                )))
            }
        }
        handle.advance(1).await;
    }

    if report.identified > 0 {
        database.bump_library_version(library.id).await?;
    }

    tracing::info!(
        library = library.name,
        identified = report.identified,
        unidentified = report.unidentified,
        postponed = report.postponed,
        cancelled = report.cancelled,
        "identification finished"
    );
    Ok(report)
}

enum Outcome {
    Identified,
    Unknown,
    /// The provider could not be reached. Nothing is written down, so the work
    /// is still waiting and the next run tries again.
    Postponed,
    /// The provider refused the key. Nothing is wrong with this work, and
    /// nothing will be right with the next one either.
    Refused,
}

async fn identify_one(
    state: &AppState,
    provider: &impl MetadataProvider,
    library: &Library,
    work: &Work,
) -> Result<Outcome> {
    let database = state.database();
    let language = &library.metadata_language;

    // An identifier already known beats any search: it was either read from a
    // description file or chosen by someone, and either way it is not a guess.
    let known_ids = database.work_external_ids(work.id).await?;
    let chosen = match known_id(&known_ids, provider.name()) {
        Some(external_id) => external_id,
        None => match find_candidate(provider, work, &known_ids, language).await {
            Ok(Some(candidate)) => candidate.external_id,
            Ok(None) => return Ok(Outcome::Unknown),
            Err(error) => return Ok(postpone(work, &error)),
        },
    };

    let details = match provider.movie_details(&chosen, language).await {
        Ok(details) => details,
        Err(error) => return Ok(postpone(work, &error)),
    };

    database
        .apply_identification(
            work.id,
            &to_record(&details, provider.name(), language),
            false,
        )
        .await
        .map_err(AppError::from)?;

    // The pictures follow at once, from what the provider already told us:
    // asking a second time for the same film would be a request for nothing.
    crate::images::store_provider_images(state, provider, work.id, &details).await;

    tracing::debug!(
        work = %MediaName::new(&details.title),
        provider = provider.name(),
        "work identified"
    );
    Ok(Outcome::Identified)
}

/// Decides what to do about a provider that did not answer.
///
/// A failure worth retrying leaves the work alone, so the next run finds it
/// still waiting. Anything else is the provider saying no, which is an answer.
fn postpone(work: &Work, error: &ProviderError) -> Outcome {
    match error {
        ProviderError::Unauthorised => {
            tracing::error!("the metadata provider refused the key; nothing can be looked up");
            Outcome::Refused
        }
        error if error.is_worth_retrying() => {
            tracing::warn!(
                work = %MediaName::new(&work.title),
                reason = %error,
                "the provider could not be reached; this work stays on the waiting list"
            );
            Outcome::Postponed
        }
        error => {
            tracing::warn!(
                work = %MediaName::new(&work.title),
                reason = %error,
                "the provider had nothing for this work"
            );
            Outcome::Unknown
        }
    }
}

fn known_id(ids: &[(String, String)], provider: &str) -> Option<String> {
    ids.iter()
        .find(|(name, _)| name == provider)
        .map(|(_, id)| id.clone())
}

/// Finds the film a work is about.
///
/// An identifier from another site is tried first, then a search by title and
/// year, then the same search without the year.
async fn find_candidate(
    provider: &impl MetadataProvider,
    work: &Work,
    known_ids: &[(String, String)],
    language: &str,
) -> melyxar_metadata::provider::Result<Option<MovieCandidate>> {
    if let Some(imdb_id) = known_id(known_ids, "imdb") {
        if let Some(found) = provider.movie_by_imdb_id(&imdb_id, language).await? {
            return Ok(Some(found));
        }
    }

    let candidates = provider
        .search_movie(&work.title, work.release_year, language)
        .await?;
    if let Some(found) = choose(&candidates, work) {
        return Ok(Some(found.clone()));
    }

    // A year read off a file name is often the year of the copy rather than of
    // the film, so a search that found nothing is worth one more try without it.
    if work.release_year.is_some() {
        let without_year = provider.search_movie(&work.title, None, language).await?;
        if let Some(found) = choose(&without_year, work) {
            return Ok(Some(found.clone()));
        }
    }
    Ok(None)
}

/// Picks the candidate a person would pick.
///
/// A title that matches exactly wins, and among equals the year decides. The
/// provider's own ordering is the last word, never the first: it ranks by how
/// famous a film is, which says nothing about which one this file holds.
fn choose<'a>(candidates: &'a [MovieCandidate], work: &Work) -> Option<&'a MovieCandidate> {
    if candidates.is_empty() {
        return None;
    }
    let wanted = naming::sort_title(&work.title);

    let matches_title = |candidate: &MovieCandidate| {
        naming::sort_title(&candidate.title) == wanted
            || candidate
                .original_title
                .as_deref()
                .is_some_and(|title| naming::sort_title(title) == wanted)
    };
    let matches_year =
        |candidate: &MovieCandidate| match (work.release_year, candidate.release_year) {
            (Some(wanted), Some(found)) => wanted == found,
            _ => false,
        };

    candidates
        .iter()
        .find(|candidate| matches_title(candidate) && matches_year(candidate))
        .or_else(|| candidates.iter().find(|candidate| matches_title(candidate)))
        .or_else(|| {
            // No title matched. A year that matches is still something; with
            // neither, the provider's first answer is all there is.
            work.release_year
                .and_then(|_| candidates.iter().find(|candidate| matches_year(candidate)))
        })
        .or_else(|| candidates.first())
}

/// Turns what the provider said into what the storage takes.
fn to_record(details: &MovieDetails, provider: &str, language: &str) -> IdentifiedWork {
    IdentifiedWork {
        provider: provider.to_string(),
        external_id: details.external_id.clone(),
        imdb_id: details.imdb_id.clone(),
        language: language.to_string(),
        sort_title: naming::sort_title(&details.title),
        title: details.title.clone(),
        tagline: details.tagline.clone(),
        overview: details.overview.clone(),
        release_year: details.release_year,
        runtime: details.runtime,
        community_rating: details.community_rating,
        age_rating_label: details.age_rating_label.clone(),
        genres: details.genres.clone(),
        studios: details.studios.clone(),
        credits: details
            .credits
            .iter()
            .map(|credit| CreditRecord {
                external_id: credit.external_id.clone(),
                sort_name: naming::sort_title(&credit.name),
                name: credit.name.clone(),
                role: credit.role.clone(),
                character: credit.character.clone(),
                ordinal: credit.ordinal,
            })
            .collect(),
        collection: details
            .collection
            .as_ref()
            .map(|collection| CollectionRecord {
                external_id: collection.external_id.clone(),
                sort_name: naming::sort_title(&collection.name),
                name: collection.name.clone(),
            }),
        trailers: details
            .trailers
            .iter()
            .filter_map(|trailer| {
                Some(RemoteTrailerRecord {
                    name: trailer.name.clone(),
                    url: trailer.watch_url()?,
                })
            })
            .collect(),
    }
}

/// An identification running in the background.
pub struct IdentifyJob {
    started: StartedJob,
    outcome: Arc<std::sync::Mutex<Option<IdentifyReport>>>,
}

impl IdentifyJob {
    pub fn id(&self) -> melyxar_core::id::JobId {
        self.started.id
    }

    /// Waits for the run to end, and says what it did.
    pub async fn wait(self) -> (JobState, Option<IdentifyReport>) {
        let state = self.started.completion.await.unwrap_or_else(|error| {
            tracing::error!(error = %error, "the identification task ended unexpectedly");
            JobState::Failed
        });
        let report = self
            .outcome
            .lock()
            .expect("the report lock is never held across an await")
            .clone();
        (state, report)
    }
}

/// Starts an identification run as a background job.
pub async fn start_identification<P>(
    state: &AppState,
    provider: Arc<P>,
    library: Library,
) -> Result<IdentifyJob>
where
    P: MetadataProvider + 'static,
{
    let outcome: Arc<std::sync::Mutex<Option<IdentifyReport>>> =
        Arc::new(std::sync::Mutex::new(None));
    let recorded = Arc::clone(&outcome);
    let state = state.clone();
    let target = library.id.to_string();

    let started = state
        .jobs()
        .clone()
        .start(
            JobKind::IdentifyWork,
            JobPriority::BACKGROUND,
            Some(target),
            move |handle| async move {
                match identify_library(&state, provider.as_ref(), &library, &handle).await {
                    Ok(report) => {
                        *recorded
                            .lock()
                            .expect("the report lock is never held across an await") = Some(report);
                        Ok(())
                    }
                    Err(error) => Err(error.to_string()),
                }
            },
        )
        .await?;

    Ok(IdentifyJob { started, outcome })
}

/// Chooses a match by hand, and remembers that a person chose it.
pub async fn identify_by_hand(
    state: &AppState,
    provider: &impl MetadataProvider,
    library_id: LibraryId,
    work_id: WorkId,
    external_id: &str,
) -> Result<()> {
    let library = state
        .database()
        .list_libraries()
        .await?
        .into_iter()
        .find(|library| library.id == library_id)
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("library")))?;

    let details = provider
        .movie_details(external_id, &library.metadata_language)
        .await
        .map_err(|error| AppError::Domain(melyxar_core::Error::invalid_input(error.to_string())))?;

    state
        .database()
        .apply_identification(
            work_id,
            &to_record(&details, provider.name(), &library.metadata_language),
            true,
        )
        .await?;
    state.database().bump_library_version(library_id).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories, LibraryConfig, RootConfig};
    use melyxar_core::work::{IdentificationState, WorkKind};
    use melyxar_database::Database;
    use melyxar_metadata::provider::{Collection, Credit, Trailer};
    use std::sync::Mutex;

    /// A provider that answers from memory, so the rules can be tested without
    /// anyone else's server being involved.
    struct StandIn {
        candidates: Vec<MovieCandidate>,
        details: Vec<MovieDetails>,
        /// What to fail with instead of answering, if anything.
        failure: Option<fn() -> ProviderError>,
        searches: Mutex<Vec<(String, Option<i32>)>>,
        /// Bytes handed back for any picture asked for, when there are any.
        picture: Option<Vec<u8>>,
    }

    impl StandIn {
        fn new(candidates: Vec<MovieCandidate>, details: Vec<MovieDetails>) -> Self {
            Self {
                candidates,
                details,
                failure: None,
                searches: Mutex::new(Vec::new()),
                picture: None,
            }
        }

        fn failing(failure: fn() -> ProviderError) -> Self {
            Self {
                candidates: Vec::new(),
                details: Vec::new(),
                failure: Some(failure),
                searches: Mutex::new(Vec::new()),
                picture: None,
            }
        }

        fn searches(&self) -> Vec<(String, Option<i32>)> {
            self.searches.lock().expect("free").clone()
        }

        /// Hands back a real picture, so the whole preparation can be exercised.
        fn serving(mut self, picture: Vec<u8>) -> Self {
            self.picture = Some(picture);
            self
        }
    }

    impl MetadataProvider for StandIn {
        fn name(&self) -> &'static str {
            "tmdb"
        }

        async fn search_movie(
            &self,
            title: &str,
            year: Option<i32>,
            _language: &str,
        ) -> melyxar_metadata::provider::Result<Vec<MovieCandidate>> {
            self.searches
                .lock()
                .expect("free")
                .push((title.to_string(), year));
            if let Some(failure) = self.failure {
                return Err(failure());
            }
            // A search with a year only returns what matches it, the way the
            // provider itself behaves.
            Ok(match year {
                None => self.candidates.clone(),
                Some(year) => self
                    .candidates
                    .iter()
                    .filter(|candidate| candidate.release_year == Some(year))
                    .cloned()
                    .collect(),
            })
        }

        async fn movie_details(
            &self,
            external_id: &str,
            _language: &str,
        ) -> melyxar_metadata::provider::Result<MovieDetails> {
            if let Some(failure) = self.failure {
                return Err(failure());
            }
            self.details
                .iter()
                .find(|details| details.external_id == external_id)
                .cloned()
                .ok_or_else(|| ProviderError::Unexpected("not found".into()))
        }

        fn image_url(&self, path: &str) -> String {
            format!("https://pictures.invalid{path}")
        }

        async fn fetch_image(&self, _path: &str) -> melyxar_metadata::provider::Result<Vec<u8>> {
            match &self.picture {
                Some(bytes) => Ok(bytes.clone()),
                None => Err(ProviderError::Unexpected("no picture here".into())),
            }
        }

        async fn movie_by_imdb_id(
            &self,
            imdb_id: &str,
            _language: &str,
        ) -> melyxar_metadata::provider::Result<Option<MovieCandidate>> {
            if let Some(failure) = self.failure {
                return Err(failure());
            }
            Ok(self
                .details
                .iter()
                .find(|details| details.imdb_id.as_deref() == Some(imdb_id))
                .map(|details| {
                    candidate(&details.external_id, &details.title, details.release_year)
                }))
        }
    }

    fn candidate(id: &str, title: &str, year: Option<i32>) -> MovieCandidate {
        MovieCandidate {
            external_id: id.to_string(),
            title: title.to_string(),
            original_title: None,
            release_year: year,
            overview: None,
            poster_path: None,
            popularity: 1.0,
        }
    }

    fn details(id: &str, title: &str, year: Option<i32>) -> MovieDetails {
        MovieDetails {
            external_id: id.to_string(),
            imdb_id: Some(format!("tt{id}")),
            title: title.to_string(),
            original_title: None,
            original_language: Some("eng".to_string()),
            tagline: Some("La mer ne rend rien.".to_string()),
            overview: Some("Un port, une nuit.".to_string()),
            release_year: year,
            runtime: Some(melyxar_core::time::Millis::new(118 * 60_000)),
            community_rating: Some(7.4),
            age_rating_label: Some("12".to_string()),
            genres: vec!["Drame".to_string()],
            studios: vec!["Invented Pictures".to_string()],
            credits: vec![Credit {
                external_id: "1".to_string(),
                name: "Alix Moreau".to_string(),
                role: "actor".to_string(),
                character: Some("Camille".to_string()),
                ordinal: 0,
                photo_path: None,
            }],
            collection: Some(Collection {
                external_id: "77".to_string(),
                name: "Harbour Trilogy".to_string(),
                poster_path: None,
                backdrop_path: None,
            }),
            poster_path: Some("/poster.jpg".to_string()),
            backdrop_path: None,
            trailers: vec![Trailer {
                name: "Bande annonce".to_string(),
                site: "YouTube".to_string(),
                key: "abc".to_string(),
                is_official: true,
                language: Some("fre".to_string()),
            }],
        }
    }

    /// The folder comes back with the state so it lives exactly as long as the
    /// test does, rather than being leaked to keep it alive.
    async fn state_with_work(
        title: &str,
        year: Option<i32>,
    ) -> (tempfile::TempDir, AppState, Library, Work) {
        build_state(title, year, false).await
    }

    /// The same, with the media tools, which the preparation of a picture
    /// needs and the rest of the rules do not.
    async fn state_with_tools(
        title: &str,
        year: Option<i32>,
    ) -> (tempfile::TempDir, AppState, Library, Work) {
        build_state(title, year, true).await
    }

    async fn build_state(
        title: &str,
        year: Option<i32>,
        with_tools: bool,
    ) -> (tempfile::TempDir, AppState, Library, Work) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = Config {
            directories: Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
            },
            libraries: vec![LibraryConfig {
                name: "Films".into(),
                kind: "movies".into(),
                metadata_language: "fr".into(),
                roots: vec![RootConfig {
                    label: "disk-one".into(),
                    path: directory.path().join("films"),
                }],
            }],
            ..Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");

        let database = Database::open_in_memory().await.expect("database opens");
        crate::startup::reconcile_libraries(&database, &config)
            .await
            .expect("libraries reconciled");
        let library = database
            .library_by_name("Films")
            .await
            .expect("read")
            .expect("declared");
        let work = database
            .create_work(
                library.id,
                WorkKind::Movie,
                title,
                &naming::sort_title(title),
                year,
            )
            .await
            .expect("work created");

        let (tools, capabilities) = match with_tools {
            true => crate::startup::detect_media_tools(&config).await,
            false => (None, None),
        };

        (
            directory,
            AppState::new(config, database, tools, capabilities),
            library,
            work,
        )
    }

    async fn run(state: &AppState, provider: &StandIn, library: &Library) -> IdentifyReport {
        let runner = melyxar_jobs::JobRunner::new(state.database().clone());
        let holder: Arc<Mutex<Option<JobHandle>>> = Arc::new(Mutex::new(None));
        let kept = Arc::clone(&holder);
        runner
            .start(
                JobKind::FetchImages,
                JobPriority::BACKGROUND,
                None,
                move |handle| async move {
                    *kept.lock().expect("free") = Some(handle);
                    Ok(())
                },
            )
            .await
            .expect("job started")
            .completion
            .await
            .expect("the job ran");
        let handle = holder.lock().expect("free").clone().expect("a handle");

        identify_library(state, provider, library, &handle)
            .await
            .expect("the run finished")
    }

    #[tokio::test]
    async fn a_film_the_provider_knows_gets_everything_a_page_shows() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        );

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.identified, 1);

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.identification, IdentificationState::Identified);
        assert_eq!(stored.release_year, Some(2019));
        assert_eq!(
            state.database().work_genres(work.id).await.expect("read"),
            vec!["Drame"]
        );
        assert_eq!(
            state
                .database()
                .work_collection(work.id)
                .await
                .expect("read")
                .as_deref(),
            Some("Harbour Trilogy")
        );
    }

    #[tokio::test]
    async fn the_year_decides_between_a_film_and_its_remake() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = StandIn::new(
            vec![
                candidate("999", "Quiet Harbour", Some(1978)),
                candidate("111", "Quiet Harbour", Some(2019)),
            ],
            vec![
                details("999", "Quiet Harbour", Some(1978)),
                details("111", "Quiet Harbour", Some(2019)),
            ],
        );

        run(&state, &provider, &library).await;
        assert_eq!(
            state
                .database()
                .work_external_ids(work.id)
                .await
                .expect("read")
                .iter()
                .find(|(provider, _)| provider == "tmdb")
                .map(|(_, id)| id.as_str()),
            Some("111")
        );
    }

    #[tokio::test]
    async fn a_year_that_belongs_to_the_copy_rather_than_the_film_is_not_the_last_word() {
        // The file said 2020, the film came out in 2019: a search with the
        // year finds nothing, and the one without it finds the film.
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2020)).await;
        let provider = StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        );

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.identified, 1);
        assert_eq!(
            provider.searches(),
            vec![
                ("Quiet Harbour".to_string(), Some(2020)),
                ("Quiet Harbour".to_string(), None),
            ]
        );

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.release_year, Some(2019));
    }

    #[tokio::test]
    async fn an_identifier_already_known_is_used_instead_of_a_search() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        state
            .database()
            .set_work_external_id(work.id, "tmdb", "111")
            .await
            .expect("identifier stored");

        let provider = StandIn::new(
            Vec::new(),
            vec![details("111", "Quiet Harbour", Some(2019))],
        );
        let report = run(&state, &provider, &library).await;

        assert_eq!(report.identified, 1);
        assert!(
            provider.searches().is_empty(),
            "an identifier read from a file or chosen by someone is not a guess to be checked"
        );
    }

    #[tokio::test]
    async fn an_identifier_from_another_site_finds_the_film_without_a_search_by_name() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", None).await;
        state
            .database()
            .set_work_external_id(work.id, "imdb", "tt111")
            .await
            .expect("identifier stored");

        let provider = StandIn::new(
            Vec::new(),
            vec![details("111", "Quiet Harbour", Some(2019))],
        );
        let report = run(&state, &provider, &library).await;

        assert_eq!(report.identified, 1);
        assert!(provider.searches().is_empty());
    }

    #[tokio::test]
    async fn a_film_nobody_recognised_stays_in_the_library_with_a_marker() {
        let (_directory, state, library, work) =
            state_with_work("Something Invented", Some(2019)).await;
        let provider = StandIn::new(Vec::new(), Vec::new());

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.unidentified, 1);

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.identification, IdentificationState::Unidentified);
        assert!(
            stored.identification.may_be_looked_up_again(),
            "a film nobody recognised today may be recognised tomorrow"
        );
    }

    #[tokio::test]
    async fn a_provider_that_is_down_leaves_the_work_waiting_rather_than_marking_it() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = StandIn::failing(|| ProviderError::Unreachable("timed out".into()));

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.postponed, 1);
        assert_eq!(report.unidentified, 0);

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            stored.identification,
            IdentificationState::Pending,
            "a provider that is down says nothing about the film"
        );
        assert_eq!(
            state
                .database()
                .works_awaiting_identification(library.id, 10)
                .await
                .expect("read")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn a_key_the_provider_refuses_stops_the_run_and_blames_the_key() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = StandIn::failing(|| ProviderError::Unauthorised);

        let runner = melyxar_jobs::JobRunner::new(state.database().clone());
        let holder: Arc<Mutex<Option<JobHandle>>> = Arc::new(Mutex::new(None));
        let kept = Arc::clone(&holder);
        runner
            .start(
                JobKind::FetchImages,
                JobPriority::BACKGROUND,
                None,
                move |handle| async move {
                    *kept.lock().expect("free") = Some(handle);
                    Ok(())
                },
            )
            .await
            .expect("job started")
            .completion
            .await
            .expect("the job ran");
        let handle = holder.lock().expect("free").clone().expect("a handle");

        let error = identify_library(&state, &provider, &library, &handle)
            .await
            .expect_err("a refused key is a failure of the run");
        assert!(error.to_string().contains("key"));

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            stored.identification,
            IdentificationState::Pending,
            "the film is not the problem, and nothing about it was learnt"
        );
    }

    #[tokio::test]
    async fn a_work_already_identified_is_left_alone() {
        let (_directory, state, library, _) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        );

        assert_eq!(run(&state, &provider, &library).await.identified, 1);
        let second = run(&state, &provider, &library).await;
        assert_eq!(second.identified, 0);
        assert_eq!(second.unidentified, 0);
    }

    #[tokio::test]
    async fn a_match_picked_by_hand_is_never_undone_by_a_later_run() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = StandIn::new(
            vec![candidate("999", "Quiet Harbour", Some(2019))],
            vec![
                details("999", "Quiet Harbour", Some(2019)),
                details("111", "Quiet Harbour", Some(2019)),
            ],
        );

        identify_by_hand(&state, &provider, library.id, work.id, "111")
            .await
            .expect("chosen by hand");

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.identification, IdentificationState::Manual);

        let report = run(&state, &provider, &library).await;
        assert_eq!(
            report.identified, 0,
            "a choice someone made is not revisited"
        );
        assert_eq!(
            state
                .database()
                .work_external_ids(work.id)
                .await
                .expect("read")
                .iter()
                .find(|(provider, _)| provider == "tmdb")
                .map(|(_, id)| id.as_str()),
            Some("111")
        );
    }

    #[tokio::test]
    async fn a_trailer_hosted_elsewhere_is_kept_as_a_link() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        );
        run(&state, &provider, &library).await;

        let row: (String,) = sqlx::query_as(
            "SELECT remote_url FROM extra_videos WHERE work_id = ? AND remote_url IS NOT NULL",
        )
        .bind(work.id.to_db_string())
        .fetch_one(state.database().reader())
        .await
        .expect("read");
        assert_eq!(row.0, "https://www.youtube.com/watch?v=abc");
    }

    /// A real picture, made by the tool the server itself uses.
    fn a_real_poster() -> Option<Vec<u8>> {
        let file = tempfile::Builder::new()
            .suffix(".jpg")
            .tempfile()
            .expect("temporary file");
        let made = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=#c81e1e:s=600x900",
                "-frames:v",
                "1",
            ])
            .arg(file.path())
            .output()
            .ok()?;
        made.status
            .success()
            .then(|| std::fs::read(file.path()).ok())?
    }

    #[tokio::test]
    async fn a_poster_is_prepared_in_every_size_the_interface_serves() {
        let Some(picture) = a_real_poster() else {
            eprintln!("no media tool here, the preparation of a picture was not exercised");
            return;
        };

        let (_directory, state, library, work) =
            state_with_tools("Quiet Harbour", Some(2019)).await;
        let provider = StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        )
        .serving(picture);

        run(&state, &provider, &library).await;

        let images = state
            .database()
            .images_of("work", &work.id.to_db_string())
            .await
            .expect("read");
        let posters: Vec<_> = images
            .iter()
            .filter(|image| image.image_kind == "poster")
            .collect();
        assert_eq!(posters.len(), 3, "one size per width the interface serves");
        assert!(posters
            .iter()
            .all(|image| image.relative_path.ends_with(".webp")));
        assert_eq!(
            posters[0].height,
            Some(posters[0].width.expect("a width") * 3 / 2),
            "the shape of the poster is kept"
        );

        let root = state.config().directories.images();
        for image in &posters {
            assert!(
                root.join(&image.relative_path).exists(),
                "a row without its file is a broken picture"
            );
        }
        assert!(
            !root
                .join("works")
                .join(work.id.to_db_string())
                .read_dir()
                .expect("the folder is there")
                .filter_map(std::result::Result::ok)
                .any(|entry| entry
                    .path()
                    .extension()
                    .is_some_and(|value| value == "source")),
            "the picture as it arrived has done its work and is not kept"
        );

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        let colour = stored.dominant_color.expect("a card has a colour to show");
        assert!(colour.starts_with('#') && colour.len() == 7, "{colour}");
    }

    #[tokio::test]
    async fn a_poster_that_has_not_changed_is_not_fetched_again() {
        let Some(picture) = a_real_poster() else {
            return;
        };
        let (_directory, state, library, work) =
            state_with_tools("Quiet Harbour", Some(2019)).await;
        let provider = StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        )
        .serving(picture);

        run(&state, &provider, &library).await;
        let before = state
            .database()
            .images_of("work", &work.id.to_db_string())
            .await
            .expect("read");

        // Asking again is what a refresh does. The pictures are the same, so
        // nothing is fetched and nothing is written.
        state
            .database()
            .apply_identification(
                work.id,
                &to_record(&details("111", "Quiet Harbour", Some(2019)), "tmdb", "fr"),
                false,
            )
            .await
            .expect("applied");
        crate::images::store_provider_images(
            &state,
            &provider,
            work.id,
            &details("111", "Quiet Harbour", Some(2019)),
        )
        .await;

        assert_eq!(
            state
                .database()
                .images_of("work", &work.id.to_db_string())
                .await
                .expect("read"),
            before,
            "a poster that did not change must not be fetched or written again"
        );
    }

    #[tokio::test]
    async fn a_picture_that_will_not_come_never_costs_the_film_its_identification() {
        let (_directory, state, library, work) =
            state_with_tools("Quiet Harbour", Some(2019)).await;
        // The stand-in serves no picture at all.
        let provider = StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        );

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.identified, 1);

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.identification, IdentificationState::Identified);
        assert!(state
            .database()
            .images_of("work", &work.id.to_db_string())
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn an_identification_started_as_a_job_reports_what_it_did() {
        let (_directory, state, library, _) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));

        let job = start_identification(&state, provider, library)
            .await
            .expect("job started");
        let (job_state, report) = job.wait().await;

        assert_eq!(job_state, JobState::Succeeded);
        assert_eq!(report.expect("a finished run has a report").identified, 1);
    }
}
