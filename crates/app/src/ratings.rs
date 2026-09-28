//! The ratings a work carries from elsewhere than its provider: IMDb's, read
//! out of the file IMDb publishes each day, and the share of critics Rotten
//! Tomatoes gathers, asked of OMDb with the administrator's own key.
//!
//! Both are looked up by what IMDb calls a work. A film is named with it; a
//! series is not, so its provider is asked for it first. Nothing here can
//! fail a library: a rating is a comfort, and a work without one is still a
//! work.

use std::collections::HashSet;
use std::path::PathBuf;

use melyxar_core::id::WorkId;
use melyxar_core::job::{JobKind, JobPriority, JobStep};
use melyxar_core::library::Library;
use melyxar_core::rating::RatingSource;
use melyxar_core::time::{now, Timestamp};
use melyxar_database::ratings::{RatingFound, WorkToRate};
use melyxar_jobs::{JobHandle, StartedJob};
use melyxar_metadata::{Catalogue, MetadataProvider, OmdbClient, ProviderError};
use time::Duration;

use crate::identify::known_id;
use crate::{AppState, Result};

/// How long the file of IMDb ratings is used before it is fetched again.
/// IMDb publishes it once a day; a little less, so a run at the same hour the
/// next day always finds it stale.
const IMDB_FILE_KEPT_FOR: Duration = Duration::hours(20);

/// How long an IMDb rating read is trusted: until the next file.
const IMDB_READ_AGAIN_AFTER: Duration = IMDB_FILE_KEPT_FOR;

/// How long the critics' share is trusted, which moves little once a work is
/// out, and how long a work that had none waits to be asked again.
const OMDB_ASKED_AGAIN_AFTER: Duration = Duration::days(30);

/// How long a work its provider knew no IMDb identifier for waits to be asked
/// again.
const IMDB_ID_ASKED_AGAIN_AFTER: Duration = Duration::days(30);

/// How many questions OMDb is asked in a day. A free key allows a thousand;
/// the rest is room for the works somebody refreshes by hand.
const OMDB_QUESTIONS_A_DAY: i64 = 950;

/// A film every catalogue knows, which a key is tried on before it is kept.
const A_FILM_EVERYBODY_KNOWS: &str = "tt0133093";

/// What trying the administrator's OMDb key came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyTried {
    Kept,
    /// OMDb does not know it, or it was never switched on from the mail it
    /// sends.
    Refused,
    /// OMDb could not be asked just now, so nothing is known of the key.
    Unreachable,
}

impl KeyTried {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Kept => "kept",
            Self::Refused => "refused",
            Self::Unreachable => "unreachable",
        }
    }
}

/// Where the file of IMDb ratings is kept between two runs.
fn imdb_file(state: &AppState) -> PathBuf {
    state
        .config()
        .directories
        .cache
        .join("imdb-title-ratings.tsv.gz")
}

/// The client OMDb is asked through, when the administrator gave a key.
async fn omdb(state: &AppState) -> Result<Option<OmdbClient>> {
    Ok(match state.database().omdb_key().await? {
        Some(key) => OmdbClient::new(key).ok(),
        None => None,
    })
}

/// Whether the administrator gave an OMDb key.
pub async fn has_omdb_key(state: &AppState) -> Result<bool> {
    Ok(state.database().omdb_key().await?.is_some())
}

/// When the file of IMDb ratings in hand was fetched, if it ever was.
pub async fn imdb_fetched_at(state: &AppState) -> Option<Timestamp> {
    let modified = tokio::fs::metadata(imdb_file(state)).await.ok()?.modified().ok()?;
    Some(Timestamp::from(modified))
}

/// Keeps the OMDb key the administrator typed once OMDb has taken it, and
/// sets the ratings going with it.
pub async fn set_omdb_key(state: &AppState, key: &str) -> Result<KeyTried> {
    let key = key.trim();
    if key.is_empty() || !key.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Ok(KeyTried::Refused);
    }
    let Ok(client) = OmdbClient::new(key) else {
        return Ok(KeyTried::Unreachable);
    };
    match client.critics_score(A_FILM_EVERYBODY_KNOWS).await {
        // A spent allowance is still a key OMDb knows.
        Ok(_) | Err(ProviderError::TooManyRequests { .. }) => {}
        Err(ProviderError::Unauthorised) => return Ok(KeyTried::Refused),
        Err(error) => {
            tracing::warn!(%error, "OMDb could not be asked whether it knows the key");
            return Ok(KeyTried::Unreachable);
        }
    }
    state.database().set_omdb_key(Some(key)).await?;
    tracing::info!("an OMDb key was kept");
    crate::schedule::start_in_turn(
        state,
        vec![crate::schedule::ScheduledTask::Ratings],
        JobPriority::REQUESTED,
    );
    Ok(KeyTried::Kept)
}

/// Forgets the OMDb key, which stops the critics being asked. What they
/// already said stays.
pub async fn forget_omdb_key(state: &AppState) -> Result<()> {
    state.database().set_omdb_key(None).await?;
    tracing::info!("the OMDb key was taken away");
    Ok(())
}

/// How much a library has waiting: works owed an IMDb identifier, an IMDb
/// rating, or, with a key, a word from the critics.
pub async fn waiting_on(state: &AppState, library: &Library) -> Result<i64> {
    let database = state.database();
    let at = now();
    let mut waiting = database
        .count_works_to_rate(library.id, RatingSource::Imdb, at - IMDB_READ_AGAIN_AFTER)
        .await?;
    if let Some(provider) = state.metadata_provider() {
        waiting += database
            .count_works_without_an_imdb_id(library.id, provider.name(), at - IMDB_ID_ASKED_AGAIN_AFTER)
            .await?;
    }
    if has_omdb_key(state).await? {
        waiting += database
            .count_works_to_rate(library.id, RatingSource::RottenTomatoes, at - OMDB_ASKED_AGAIN_AFTER)
            .await?;
    }
    Ok(waiting)
}

/// Starts the ratings of one library as a job of its own.
pub async fn start(state: &AppState, library: Library, priority: JobPriority) -> Result<StartedJob> {
    let owned = state.clone();
    let target = library.id.to_string();
    Ok(state
        .jobs()
        .clone()
        .start(JobKind::FetchRatings, priority, Some(target), move |handle| async move {
            rate_library(&owned, &library, &handle)
                .await
                .map(|_| ())
                .map_err(|error| error.to_string())
        })
        .await?)
}

/// Gives every work of a library the ratings it is owed, and answers how
/// many changed.
async fn rate_library(state: &AppState, library: &Library, handle: &JobHandle) -> Result<usize> {
    let database = state.database();
    let at = now();

    if let Some(provider) = state.metadata_provider() {
        let without = database
            .works_without_an_imdb_id(library.id, provider.name(), at - IMDB_ID_ASKED_AGAIN_AFTER)
            .await?;
        if !without.is_empty() {
            handle.at_step(JobStep::LookingUpImdbIds).await;
            handle.set_total(without.len() as i64).await;
            for work in without {
                if handle.is_cancelled() {
                    return Ok(0);
                }
                let Some(catalogue) = Catalogue::of(work.kind) else {
                    continue;
                };
                if !find_imdb_id(state, provider.as_ref(), work.id, catalogue, &work.external_id)
                    .await?
                {
                    break;
                }
                handle.advance(1).await;
            }
        }
    }

    let mut changed = 0;
    let owed = database
        .works_to_rate(library.id, RatingSource::Imdb, now() - IMDB_READ_AGAIN_AFTER, None)
        .await?;
    if !owed.is_empty() {
        handle.at_step(JobStep::ReadingImdbRatings).await;
        handle.set_total(owed.len() as i64).await;
        if fresh_imdb_file(state).await {
            changed += read_imdb_ratings(state, &owed).await?;
        }
        handle.advance(owed.len() as i64).await;
    }

    if let Some(omdb) = omdb(state).await? {
        let allowance = OMDB_QUESTIONS_A_DAY - omdb_questions_today(state).await?;
        let owed = database
            .works_to_rate(
                library.id,
                RatingSource::RottenTomatoes,
                now() - OMDB_ASKED_AGAIN_AFTER,
                Some(allowance.max(0)),
            )
            .await?;
        if !owed.is_empty() {
            handle.at_step(JobStep::AskingOmdb).await;
            handle.set_total(owed.len() as i64).await;
            for work in owed {
                if handle.is_cancelled() {
                    break;
                }
                match ask_the_critics(state, &omdb, &work).await? {
                    Some(moved) => changed += moved,
                    None => break,
                }
                handle.advance(1).await;
            }
        }
    }

    if changed > 0 {
        database.bump_library_version(library.id).await?;
    }
    tracing::info!(library = library.name, changed, "ratings from elsewhere brought up to date");
    Ok(changed)
}

/// Gives one work the ratings it is owed, when somebody named it by hand or
/// asked for its metadata again. The file of IMDb ratings is read as it
/// stands, never fetched for a single work: the nightly task keeps it fresh.
pub async fn rate_one<P>(state: &AppState, provider: &P, work_id: WorkId, catalogue: Catalogue)
where
    P: MetadataProvider,
{
    if let Err(error) = try_to_rate_one(state, provider, work_id, catalogue).await {
        tracing::warn!(%error, "the ratings of a work could not be brought up to date");
    }
}

async fn try_to_rate_one<P>(
    state: &AppState,
    provider: &P,
    work_id: WorkId,
    catalogue: Catalogue,
) -> Result<()>
where
    P: MetadataProvider,
{
    let database = state.database();
    let ids = database.work_external_ids(work_id).await?;
    if known_id(&ids, "imdb").is_none() {
        if let Some(external_id) = known_id(&ids, provider.name()) {
            find_imdb_id(state, provider, work_id, catalogue, &external_id).await?;
        }
    }
    let Some(imdb_id) = known_id(&database.work_external_ids(work_id).await?, "imdb") else {
        return Ok(());
    };
    let work = [WorkToRate {
        id: work_id,
        imdb_id,
    }];
    if tokio::fs::try_exists(imdb_file(state)).await.unwrap_or(false) {
        read_imdb_ratings(state, &work).await?;
    }
    if let Some(omdb) = omdb(state).await? {
        if omdb_questions_today(state).await? < OMDB_QUESTIONS_A_DAY {
            ask_the_critics(state, &omdb, &work[0]).await?;
        }
    }
    Ok(())
}

/// Asks the provider what IMDb calls one work and writes the answer down.
/// Answers false when asking again today is pointless.
async fn find_imdb_id<P>(
    state: &AppState,
    provider: &P,
    work_id: WorkId,
    catalogue: Catalogue,
    external_id: &str,
) -> Result<bool>
where
    P: MetadataProvider,
{
    match provider.imdb_id(catalogue, external_id).await {
        Ok(imdb_id) => {
            state.database().set_imdb_id(work_id, imdb_id.as_deref()).await?;
            Ok(true)
        }
        Err(error) => {
            tracing::warn!(%error, "the provider would not say what IMDb calls a work");
            Ok(!matches!(
                error,
                ProviderError::Unauthorised
                    | ProviderError::Unreachable(_)
                    | ProviderError::TooManyRequests { .. }
            ))
        }
    }
}

/// Whether the file of IMDb ratings is here and recent, fetching it when it
/// is not. A file that cannot be fetched leaves the ratings as they were.
async fn fresh_imdb_file(state: &AppState) -> bool {
    let file = imdb_file(state);
    let age = tokio::fs::metadata(&file)
        .await
        .and_then(|found| found.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok());
    if age.is_some_and(|age| age < IMDB_FILE_KEPT_FOR) {
        return true;
    }
    if let Some(folder) = file.parent() {
        if let Err(error) = tokio::fs::create_dir_all(folder).await {
            tracing::warn!(%error, folder = %folder.display(), "the folder of the IMDb ratings could not be made");
            return false;
        }
    }
    match melyxar_metadata::imdb::download_ratings(&file).await {
        Ok(()) => {
            tracing::info!("the file of IMDb ratings was fetched");
            true
        }
        Err(error) => {
            tracing::warn!(%error, "the file of IMDb ratings could not be fetched");
            false
        }
    }
}

/// Reads the IMDb ratings of these works out of the file, and writes down
/// what it says of each, nothing for a work it does not rate.
async fn read_imdb_ratings(state: &AppState, works: &[WorkToRate]) -> Result<usize> {
    let file = imdb_file(state);
    let wanted: Vec<String> = works.iter().map(|work| work.imdb_id.clone()).collect();
    // Well over a million lines: read away from the threads that answer.
    let read = tokio::task::spawn_blocking(move || {
        let wanted: HashSet<&str> = wanted.iter().map(String::as_str).collect();
        std::fs::File::open(&file)
            .and_then(|opened| melyxar_metadata::imdb::read_ratings(opened, &wanted))
    })
    .await
    .unwrap_or_else(|error| Err(std::io::Error::other(error)));
    let ratings = match read {
        Ok(ratings) => ratings,
        Err(error) => {
            tracing::warn!(%error, "the file of IMDb ratings could not be read");
            return Ok(0);
        }
    };
    let found: Vec<RatingFound> = works
        .iter()
        .map(|work| {
            let rating = ratings
                .get(&work.imdb_id)
                .map(|(average, votes)| (*average, Some(*votes)));
            (work.id, rating)
        })
        .collect();
    Ok(state.database().write_ratings(RatingSource::Imdb, &found).await?)
}

/// Asks OMDb about one work and writes the answer down, answering how many
/// ratings changed, or nothing when asking any more today is pointless.
async fn ask_the_critics(
    state: &AppState,
    omdb: &OmdbClient,
    work: &WorkToRate,
) -> Result<Option<usize>> {
    match omdb.critics_score(&work.imdb_id).await {
        Ok(share) => Ok(Some(
            state
                .database()
                .write_ratings(
                    RatingSource::RottenTomatoes,
                    &[(work.id, share.map(|share| (f64::from(share), None)))],
                )
                .await?,
        )),
        Err(error) => {
            tracing::warn!(%error, "OMDb would not say what the critics made of a work");
            Ok(None)
        }
    }
}

/// How many questions OMDb was asked since midnight, UTC.
async fn omdb_questions_today(state: &AppState) -> Result<i64> {
    let midnight: Timestamp = melyxar_core::time::at_utc_minutes_on(now(), 0);
    Ok(state
        .database()
        .ratings_checked_since(RatingSource::RottenTomatoes, midnight)
        .await?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::work::WorkKind;
    use melyxar_database::metadata::IdentifiedWork;
    use melyxar_database::Database;
    use std::io::Write;
    use std::path::PathBuf;

    async fn a_server() -> (tempfile::TempDir, AppState, Library) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = melyxar_config::Config {
            directories: melyxar_config::Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
                ..Default::default()
            },
            ..melyxar_config::Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        (directory, AppState::new(config, database, None, None), library)
    }

    async fn a_named_film(state: &AppState, library: &Library, title: &str, imdb_id: &str) -> WorkId {
        let database = state.database();
        let work = database
            .create_work(library.id, WorkKind::Movie, title, &title.to_lowercase(), None)
            .await
            .expect("work created");
        database
            .apply_identification(
                work.id,
                &IdentifiedWork {
                    provider: "tmdb".to_string(),
                    external_id: format!("tmdb-{title}"),
                    imdb_id: Some(imdb_id.to_string()),
                    language: "fr".to_string(),
                    title: title.to_string(),
                    sort_title: title.to_lowercase(),
                    tagline: None,
                    overview: None,
                    release_year: Some(2019),
                    release_date: None,
                    end_date: None,
                    runtime: None,
                    community_rating: None,
                    age_rating_label: None,
                    genres: Vec::new(),
                    studios: Vec::new(),
                    credits: Vec::new(),
                    collection: None,
                    trailers: Vec::new(),
                },
                false,
            )
            .await
            .expect("named");
        work.id
    }

    #[tokio::test]
    async fn a_library_is_rated_out_of_the_file_imdb_publishes() {
        let (_directory, state, library) = a_server().await;
        let harbour = a_named_film(&state, &library, "Quiet Harbour", "tt0000001").await;
        let lantern = a_named_film(&state, &library, "Lantern Row", "tt0000002").await;
        assert_eq!(waiting_on(&state, &library).await.expect("counted"), 2);

        // Here and fresh, so nothing is fetched.
        let mut file = flate2::write::GzEncoder::new(
            std::fs::File::create(imdb_file(&state)).expect("file made"),
            flate2::Compression::default(),
        );
        file.write_all(b"tconst\taverageRating\tnumVotes\ntt0000001\t7.8\t4200\ntt0000009\t5.0\t10\n")
            .expect("written");
        file.finish().expect("finished");

        let version = state.database().library_version(library.id).await.expect("read");
        start(&state, library.clone(), JobPriority::REQUESTED)
            .await
            .expect("started")
            .completion
            .await
            .expect("ended");

        let ratings = state.database().work_ratings(harbour).await.expect("read");
        assert_eq!(ratings.len(), 1);
        assert_eq!((ratings[0].source, ratings[0].value, ratings[0].votes), (RatingSource::Imdb, 7.8, Some(4200)));
        assert!(
            state.database().work_ratings(lantern).await.expect("read").is_empty(),
            "a film IMDb does not rate carries nothing"
        );
        assert_eq!(
            waiting_on(&state, &library).await.expect("counted"),
            0,
            "both were asked about, and neither is asked about again today"
        );
        assert_ne!(
            state.database().library_version(library.id).await.expect("read"),
            version,
            "the screens showing the library read it again"
        );
    }

    #[tokio::test]
    async fn a_key_that_is_not_one_is_refused_before_anything_is_asked() {
        let (_directory, state, _library) = a_server().await;
        for typed in ["not a key!", "   "] {
            assert_eq!(set_omdb_key(&state, typed).await.expect("tried"), KeyTried::Refused);
        }
        assert!(!has_omdb_key(&state).await.expect("read"));
    }
}
