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
use melyxar_core::job::{JobKind, JobPriority, JobState, JobStep};
use melyxar_core::library::Library;
use melyxar_core::privacy::MediaName;
use melyxar_core::work::{IdentificationNote, Work};
use melyxar_database::metadata::{
    CollectionRecord, CreditRecord, IdentifiedWork, RemoteTrailerRecord,
};
use melyxar_jobs::{JobHandle, StartedJob};
use melyxar_library::naming;
use melyxar_metadata::provider::Trailer;
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
    /// Works whose title was read again from their file name before anything
    /// was asked about them, and came out different.
    pub renamed: usize,
    /// Works that turned out to be another copy of a film already here.
    pub merged: usize,
    /// Films that had a name but no picture, and have one now.
    pub pictures_filled: usize,
    /// Films that had a name but no synopsis, and have one now.
    pub synopses_filled: usize,
    pub cancelled: bool,
}

/// How many works one run looks at.
///
/// Identifies the works of a library that are still waiting.
pub async fn identify_library<P>(
    state: &AppState,
    provider: &Arc<P>,
    library: &Library,
    handle: &JobHandle,
) -> Result<IdentifyReport>
where
    P: MetadataProvider + 'static,
{
    let database = state.database();
    // Before asking anyone about a film, make sure the question is the right
    // one. A work still waiting has never been given anything but the name of
    // its file, the rules that read those names get better, and a title read
    // by yesterday's rules is the commonest reason a provider answers nothing.
    handle.at_step(JobStep::ReadingNamesAgain).await;
    let reread = crate::scan::reread_names_of_nameless_works(state, library).await?;

    let mut report = IdentifyReport {
        renamed: reread.renamed,
        merged: reread.merged,
        ..IdentifyReport::default()
    };
    // Everything waiting, read once. A run deals with all of it: a library of
    // four disks must not need the button pressing three times, with nothing
    // to say why.
    let waiting = database.works_awaiting_identification(library.id).await?;
    handle.at_step(JobStep::AskingTheProvider).await;
    handle.set_total(waiting.len() as i64).await;

    for work in waiting {
        if handle.is_cancelled() {
            report.cancelled = true;
            break;
        }

        match identify_one(state, provider, library, &work).await? {
            Outcome::Identified => report.identified += 1,
            Outcome::Unknown(note) => {
                database.mark_work_unidentified(work.id).await?;
                database.set_identification_note(work.id, note).await?;
                report.unidentified += 1;
            }
            // Nothing about the film is written down, only why nobody has got
            // round to it: without that, a work that is still waiting looks
            // exactly like one that has never been tried.
            Outcome::Postponed(note) => {
                database.set_identification_note(work.id, note).await?;
                report.postponed += 1;
            }
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

    // Two copies can carry names nothing could ever match, and the provider
    // then answers the same film for both. That only becomes visible once both
    // have been asked about, which is here.
    report.merged +=
        join_what_the_provider_says_is_one_film(state, library, provider.name()).await?;

    // A picture is fetched when a film is named and never again, so one that
    // did not arrive that day would never arrive: the film keeps its title and
    // its grey rectangle for ever. Asked for here, where somebody has just
    // pressed the button that says look up what is missing.
    let filled = fill_in_what_is_missing(state, provider, library, handle).await?;
    report.pictures_filled = filled.pictures;
    report.synopses_filled = filled.synopses;

    if report.identified > 0
        || report.merged > 0
        || report.pictures_filled > 0
        || report.synopses_filled > 0
    {
        database.bump_library_version(library.id).await?;
    }

    tracing::info!(
        library = library.name,
        identified = report.identified,
        unidentified = report.unidentified,
        postponed = report.postponed,
        renamed = report.renamed,
        merged = report.merged,
        pictures_filled = report.pictures_filled,
        synopses_filled = report.synopses_filled,
        cancelled = report.cancelled,
        "identification finished"
    );
    Ok(report)
}

/// Joins the copies the provider says are one and the same film.
///
/// Two files can carry names no rule could ever bring together, and hold the
/// same film: one named after its original title and one after the title it
/// was released under here, one carrying a mark the other does not. Only the
/// provider can say they are one film, and it says so by answering the same
/// identifier for both.
///
/// Left apart they are the same title, the same poster and the same synopsis
/// twice in a grid, which is exactly what a version chooser exists to avoid.
async fn join_what_the_provider_says_is_one_film(
    state: &AppState,
    library: &Library,
    provider: &str,
) -> Result<usize> {
    let shared = state
        .database()
        .works_sharing_an_identity(library.id, provider)
        .await?;

    let mut joined = 0;
    for film in shared {
        for other in film.others {
            crate::scan::join_work_into(state, other, film.keep).await?;
            joined += 1;
        }
    }
    if joined > 0 {
        tracing::info!(
            copies = joined,
            "copies the provider calls one film were put together"
        );
    }
    Ok(joined)
}

/// The language a provider describes a film in when nobody translated it.
const THE_LANGUAGE_MOST_FILMS_ARE_DESCRIBED_IN: &str = "en";

/// Gives a film a synopsis when the language asked for has none.
///
/// Plenty of films are described in one language and not yet in another, and a
/// page with an empty half is worse than a page with a paragraph somebody can
/// read. The title stays in the language the library was asked for: it is the
/// synopsis that is missing, not the film.
async fn fill_in_the_synopsis(
    provider: &impl MetadataProvider,
    details: MovieDetails,
    language: &str,
) -> MovieDetails {
    let has_one = details
        .overview
        .as_deref()
        .is_some_and(|text| !text.trim().is_empty());
    if has_one || melyxar_core::media::normalise_language(language) == "eng" {
        return details;
    }

    match provider
        .movie_details(
            &details.external_id,
            THE_LANGUAGE_MOST_FILMS_ARE_DESCRIBED_IN,
        )
        .await
    {
        Ok(elsewhere) => {
            tracing::debug!(
                work = %MediaName::new(&details.title),
                "no synopsis in the language asked for; the original one is used"
            );
            MovieDetails {
                overview: elsewhere.overview,
                tagline: details.tagline.clone().or(elsewhere.tagline),
                ..details
            }
        }
        // A film with no synopsis is still a film.
        Err(_) => details,
    }
}

/// What one pass over the holes managed to fill.
#[derive(Debug, Default)]
struct Filled {
    pictures: usize,
    synopses: usize,
}

/// Asks again about films that have a name and are still missing something.
///
/// What a provider is asked for is only ever fetched when a film is named, so
/// anything that did not arrive that day would never arrive: a folder that
/// could not be written to, a disk that was full, a provider down for a
/// minute. The film keeps its title and its holes for ever, and nothing says
/// so. One question per film serves every hole it has.
///
/// Nothing here can fail the run: these are comforts, and a film without them
/// is still a film.
async fn fill_in_what_is_missing<P>(
    state: &AppState,
    provider: &Arc<P>,
    library: &Library,
    handle: &JobHandle,
) -> Result<Filled>
where
    P: MetadataProvider + 'static,
{
    let language = &library.metadata_language;
    // Every one of them, read once, for the same reason as the look up above.
    let waiting = state
        .database()
        .works_missing_their_metadata(library.id, provider.name(), language)
        .await?;
    if waiting.is_empty() {
        return Ok(Filled::default());
    }

    tracing::info!(
        films = waiting.len(),
        "some films have a name and are missing something; asking again"
    );

    handle.at_step(JobStep::FillingInWhatIsMissing).await;
    handle.set_total(waiting.len() as i64).await;

    let mut filled = Filled::default();
    for work in waiting {
        if handle.is_cancelled() {
            break;
        }
        let details = match provider.movie_details(&work.external_id, language).await {
            Ok(details) => fill_in_the_synopsis(provider.as_ref(), details, language).await,
            Err(error) => {
                tracing::warn!(reason = %error, "the provider would not describe a film again");
                handle.advance(1).await;
                continue;
            }
        };

        if work.wants_pictures
            && crate::images::store_provider_images(state, provider.as_ref(), work.id, &details)
                .await
                > 0
        {
            filled.pictures += 1;
        }

        if work.wants_a_synopsis {
            if let Some(synopsis) = details
                .overview
                .as_deref()
                .filter(|text| !text.trim().is_empty())
            {
                state
                    .database()
                    .set_work_synopsis(work.id, language, details.tagline.as_deref(), synopsis)
                    .await?;
                filled.synopses += 1;
            }
        }
        handle.advance(1).await;
    }
    Ok(filled)
}

enum Outcome {
    Identified,
    Unknown(IdentificationNote),
    /// The provider could not be reached. Nothing about the film is written
    /// down, so it is still waiting and the next run tries again.
    Postponed(IdentificationNote),
    /// The provider refused the key. Nothing is wrong with this work, and
    /// nothing will be right with the next one either.
    Refused,
}

async fn identify_one<P>(
    state: &AppState,
    provider: &Arc<P>,
    library: &Library,
    work: &Work,
) -> Result<Outcome>
where
    P: MetadataProvider + 'static,
{
    let database = state.database();
    let language = &library.metadata_language;

    // An identifier already known beats any search: it was either read from a
    // description file or chosen by someone, and either way it is not a guess.
    let known_ids = database.work_external_ids(work.id).await?;
    let chosen = match known_id(&known_ids, provider.name()) {
        Some(external_id) => external_id,
        None => match find_candidate(provider.as_ref(), work, &known_ids, language).await {
            Ok(Some(candidate)) => candidate.external_id,
            // The provider answered and offered nothing at all. The only thing
            // it was given is the title read off the file name, so that title
            // is what the line has to say.
            Ok(None) => {
                tracing::info!(
                    work = %MediaName::new(&work.title),
                    year = work.release_year,
                    provider = provider.name(),
                    "no film came back under this title; the name on disk is most likely not the name of the film"
                );
                return Ok(Outcome::Unknown(IdentificationNote::NoMatch));
            }
            Err(error) => return Ok(postpone(work, &error)),
        },
    };

    let details = match provider.movie_details(&chosen, language).await {
        Ok(details) => fill_in_the_synopsis(provider.as_ref(), details, language).await,
        Err(error) => return Ok(postpone(work, &error)),
    };

    let people = database
        .apply_identification(
            work.id,
            &to_record(&details, provider.name(), language),
            false,
        )
        .await
        .map_err(AppError::from)?;

    // The pictures follow at once, from what the provider already told us:
    // asking a second time for the same film would be a request for nothing.
    crate::images::store_provider_images(state, provider.as_ref(), work.id, &details).await;
    crate::images::store_person_photos(state, provider, &people).await;

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
        ProviderError::TooManyRequests {
            retry_after_seconds,
        } => {
            tracing::warn!(
                work = %MediaName::new(&work.title),
                retry_after_seconds,
                "the provider asked to be left alone; this work stays on the waiting list"
            );
            Outcome::Postponed(IdentificationNote::ProviderBusy)
        }
        error if error.is_worth_retrying() => {
            tracing::warn!(
                work = %MediaName::new(&work.title),
                reason = %error,
                "the provider could not be reached; this work stays on the waiting list"
            );
            Outcome::Postponed(IdentificationNote::ProviderUnreachable)
        }
        error => {
            tracing::warn!(
                work = %MediaName::new(&work.title),
                reason = %error,
                "the provider answered something that could not be read"
            );
            Outcome::Unknown(IdentificationNote::ProviderUnreadable)
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

    // An accent reaches us written one of two ways, and a provider that
    // matches text exactly answers nothing to the one it did not expect. The
    // two spellings look identical on any screen, so nobody would ever guess
    // that is what went wrong. Asking again in plain letters costs one request
    // and settles it, whichever way the file was written.
    if naming::carries_accents(&work.title) {
        let plain = naming::fold_accents(&work.title);
        let folded = provider.search_movie(&plain, None, language).await?;
        if let Some(found) = choose(&folded, work) {
            return Ok(Some(found.clone()));
        }
    }
    Ok(None)
}

/// How far a year read off a file name may be from the year a provider gives.
///
/// A film has several release dates: a festival, a country, a streaming
/// service, each a different year. Whoever named the file wrote down one of
/// them and the provider publishes another, so a year is trustworthy for
/// ruling out a film from another decade and worthless for telling apart two
/// films a year apart.
const YEARS_APART: i32 = 1;

/// Picks the candidate a person would pick, or none at all.
///
/// **The name is what identifies a film here, and nothing else does.** A
/// candidate is only ever taken when it carries the name that was searched
/// for, under the title it was released under or the one it was shot under.
///
/// The year then decides among films of that same name, and it is good at
/// that: a film from 1978 is not the one a file dated 2019 holds. It says
/// almost nothing about two films a year apart, since the two years are as
/// likely to be two dates of one film, so the provider's order settles those:
/// it ranks by how famous a film is, and on twenty films of one common title
/// one of them is the one nearly everybody means.
///
/// A name that is not quite the same name still has to be recognised, because
/// the two are written by different hands. Whoever named the file left out the
/// volume number, wrote the conjunction as a word, kept the episode number the
/// provider drops. So a candidate that is not word for word what was searched
/// for is taken when it is close enough **and** its year agrees: the year is
/// what stops a near miss from being a different film.
///
/// What is deliberately not done is taking the first answer when nothing
/// matches. A search made of a name the provider does not know still comes
/// back with something, and that something is a film picked at random as far
/// as this library is concerned. It used to be taken, which put a making-of on
/// two films of a series and then, both carrying one identifier, put those two
/// films on one page. A film nobody could name says so and waits, which is
/// visible, correctable, and the whole reason the report names them.
fn choose<'a>(candidates: &'a [MovieCandidate], work: &Work) -> Option<&'a MovieCandidate> {
    if candidates.is_empty() {
        return None;
    }
    let wanted = naming::matchable_title(&work.title);

    let names_of = |candidate: &MovieCandidate| {
        let mut names = vec![naming::matchable_title(&candidate.title)];
        if let Some(original) = candidate.original_title.as_deref() {
            names.push(naming::matchable_title(original));
        }
        names
    };
    let matches_title = |candidate: &&MovieCandidate| names_of(candidate).contains(&wanted);
    let near_the_year =
        |candidate: &&MovieCandidate| match (work.release_year, candidate.release_year) {
            (Some(wanted), Some(found)) => (wanted - found).abs() <= YEARS_APART,
            _ => false,
        };

    let carrying_the_name: Vec<&MovieCandidate> = candidates.iter().filter(matches_title).collect();

    let word_for_word = carrying_the_name
        .iter()
        .copied()
        .find(near_the_year)
        // Every film of this name is from another time. The name is still the
        // strongest thing there is, so the best of them is taken anyway.
        .or_else(|| carrying_the_name.first().copied());
    if word_for_word.is_some() {
        return word_for_word;
    }

    // Nothing carries the name exactly. The closest of those whose year agrees
    // is taken, and only if it is close enough to be the same film named by
    // two different hands. A year that is not known on both sides decides
    // nothing, so only the closeness is left to go on.
    let year_allows =
        |candidate: &&MovieCandidate| match (work.release_year, candidate.release_year) {
            (Some(_), Some(_)) => near_the_year(candidate),
            _ => true,
        };

    candidates
        .iter()
        .filter(year_allows)
        .map(|candidate| {
            let closeness = names_of(candidate)
                .iter()
                .map(|name| naming::how_alike(name, &wanted))
                .fold(0.0_f64, f64::max);
            (candidate, closeness)
        })
        .filter(|(_, closeness)| *closeness >= CLOSE_ENOUGH)
        // The closest wins, and on a tie the provider's order does: it puts
        // the film nearly everybody means first, so the first of equals stays.
        .fold(
            None,
            |best: Option<(&MovieCandidate, f64)>, next| match best {
                Some((_, closeness)) if closeness >= next.1 => best,
                _ => Some(next),
            },
        )
        .map(|(candidate, _)| candidate)
}

/// How much of their words two titles must share before they count as one
/// title written by two hands.
///
/// Half. Below that the words in common are the ordinary words any two titles
/// share, and above it a search that came back with something unrelated is
/// still refused: a name the provider never understood has almost nothing in
/// common with what it answered.
const CLOSE_ENOUGH: f64 = 0.5;

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
                photo_path: credit.photo_path.clone(),
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
        trailers: best_trailers(&details.trailers, language),
    }
}

/// How many trailers are worth keeping for one film.
///
/// A page offers a trailer, not a list of five. One is what gets watched; a
/// couple more are kept in case the first has been taken down.
const TRAILERS_KEPT: usize = 3;

/// Orders the trailers the way a viewer would want them and keeps a few.
///
/// The one in the language the library is in comes first, and an official one
/// beats a fan edit. Without this the interface would show whichever the
/// provider happened to list first, which is often neither.
fn best_trailers(trailers: &[Trailer], language: &str) -> Vec<RemoteTrailerRecord> {
    let wanted = melyxar_core::media::normalise_language(language);

    let mut ordered: Vec<&Trailer> = trailers.iter().collect();
    ordered.sort_by_key(|trailer| {
        let speaks_the_language = trailer.language.as_deref() == Some(wanted.as_str());
        // Sorting is ascending, so false comes first; negating puts the ones
        // that matter at the top.
        (!speaks_the_language, !trailer.is_official)
    });

    ordered
        .into_iter()
        .filter_map(|trailer| {
            Some(RemoteTrailerRecord {
                name: trailer.name.clone(),
                url: trailer.watch_url()?,
            })
        })
        .take(TRAILERS_KEPT)
        .collect()
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
                match identify_library(&state, &provider, &library, &handle).await {
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

/// Films a person could mean, for a work no provider recognised.
///
/// The rules that read a file name do their best and sometimes there is
/// nothing to be done: a copy named after the wrong film, a title the provider
/// spells differently, a name that is only a marker. Somebody looking at the
/// film knows what it is, and this is how they say so.
pub async fn candidates_for<P>(
    state: &AppState,
    provider: &Arc<P>,
    work_id: WorkId,
    query: &str,
) -> Result<Vec<MovieCandidate>>
where
    P: MetadataProvider + 'static,
{
    let work = state
        .database()
        .work(work_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("work")))?;

    let language = state
        .database()
        .list_libraries()
        .await?
        .into_iter()
        .find(|library| library.id == work.library_id)
        .map(|library| library.metadata_language)
        .unwrap_or_else(|| "en".to_string());

    // Whatever was typed, and never the year: a person searching by hand is
    // already saying the automatic attempt was wrong, and the year it used
    // came from the same file name that was wrong.
    provider
        .search_movie(query, None, &language)
        .await
        .map_err(|error| AppError::Domain(melyxar_core::Error::invalid_input(error.to_string())))
}

/// Chooses a match by hand, and remembers that a person chose it.
pub async fn identify_by_hand<P>(
    state: &AppState,
    provider: &Arc<P>,
    library_id: LibraryId,
    work_id: WorkId,
    external_id: &str,
) -> Result<()>
where
    P: MetadataProvider + 'static,
{
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

    let people = state
        .database()
        .apply_identification(
            work_id,
            &to_record(&details, provider.name(), &library.metadata_language),
            true,
        )
        .await?;

    // A film someone identified by hand gets its pictures like any other: the
    // provider has just described it, so asking again would be a request for
    // nothing.
    crate::images::store_provider_images(state, provider.as_ref(), work_id, &details).await;
    crate::images::store_person_photos(state, provider, &people).await;

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
        /// Every picture actually asked for, so a test can show that nothing
        /// is fetched twice and that a long cast does not mean a long wait.
        fetched: Mutex<Vec<String>>,
        /// Bytes handed back for any picture asked for, when there are any.
        picture: Option<Vec<u8>>,
        /// Whether a title has to be spelled exactly as held to match.
        exact: bool,
        /// Whether this film has only ever been described in English, which
        /// is the ordinary state of a film nobody has translated yet.
        only_in_english: bool,
    }

    impl StandIn {
        fn new(candidates: Vec<MovieCandidate>, details: Vec<MovieDetails>) -> Self {
            Self {
                candidates,
                details,
                failure: None,
                searches: Mutex::new(Vec::new()),
                fetched: Mutex::new(Vec::new()),
                picture: None,
                exact: false,
                only_in_english: false,
            }
        }

        /// A provider that answers the way the real one was measured to:
        /// accents folded on its side, so a title spelled with them or without
        /// matches, and nothing at all for an accent written as a separate
        /// mark, which it does not know how to read.
        fn matching_exactly(candidates: Vec<MovieCandidate>, details: Vec<MovieDetails>) -> Self {
            Self {
                exact: true,
                ..Self::new(candidates, details)
            }
        }

        /// A film described in English and in no other language.
        fn described_only_in_english(mut self) -> Self {
            self.only_in_english = true;
            self
        }

        fn failing(failure: fn() -> ProviderError) -> Self {
            Self {
                candidates: Vec::new(),
                details: Vec::new(),
                failure: Some(failure),
                searches: Mutex::new(Vec::new()),
                fetched: Mutex::new(Vec::new()),
                picture: None,
                exact: false,
                only_in_english: false,
            }
        }

        fn searches(&self) -> Vec<(String, Option<i32>)> {
            self.searches.lock().expect("free").clone()
        }

        fn fetched(&self) -> Vec<String> {
            self.fetched.lock().expect("free").clone()
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
            if self.exact {
                let asked = melyxar_library::naming::fold_accents(title);
                let holds_an_unread_mark =
                    title.chars().any(|c| ('\u{300}'..='\u{36f}').contains(&c));
                if holds_an_unread_mark
                    || !self.candidates.iter().any(|candidate| {
                        melyxar_library::naming::fold_accents(&candidate.title) == asked
                    })
                {
                    return Ok(Vec::new());
                }
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
            language: &str,
        ) -> melyxar_metadata::provider::Result<MovieDetails> {
            if let Some(failure) = self.failure {
                return Err(failure());
            }
            let found = self
                .details
                .iter()
                .find(|details| details.external_id == external_id)
                .cloned()
                .ok_or_else(|| ProviderError::Unexpected("not found".into()))?;

            // A provider answers with what it holds in the language it was
            // asked for, and with nothing where nobody has written it yet.
            match self.only_in_english && language != "en" {
                true => Ok(MovieDetails {
                    overview: None,
                    tagline: None,
                    ..found
                }),
                false => Ok(found),
            }
        }

        fn image_url(&self, path: &str) -> String {
            format!("https://pictures.invalid{path}")
        }

        async fn fetch_image(&self, path: &str) -> melyxar_metadata::provider::Result<Vec<u8>> {
            self.fetched.lock().expect("free").push(path.to_string());
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

    /// A work as it comes out of a file name, without going near a database:
    /// the rule that picks a film among several answers is pure, and testing
    /// it through a provider hides which of its clauses actually decided.
    fn work_named(title: &str, year: Option<i32>) -> Work {
        Work {
            id: WorkId::new(),
            library_id: LibraryId::new(),
            parent_id: None,
            kind: WorkKind::Movie,
            sort_title: naming::sort_title(title),
            title: title.to_string(),
            release_year: year,
            runtime: None,
            community_rating: None,
            age_rating_label: None,
            identification: IdentificationState::Pending,
            identification_note: None,
            dominant_color: None,
            added_at: melyxar_core::time::now(),
            updated_at: melyxar_core::time::now(),
        }
    }

    #[test]
    fn the_title_and_the_year_together_beat_either_on_its_own() {
        let wanted = work_named("Quiet Harbour", Some(2019));
        let offered = vec![
            candidate("1", "Quiet Harbour", Some(1978)),
            candidate("2", "Amber Field", Some(2019)),
            candidate("3", "Quiet Harbour", Some(2019)),
        ];
        assert_eq!(
            choose(&offered, &wanted).expect("one of them").external_id,
            "3"
        );
    }

    #[test]
    fn a_title_that_matches_beats_a_year_that_matches() {
        let wanted = work_named("Quiet Harbour", Some(2019));
        let offered = vec![
            candidate("1", "Amber Field", Some(2019)),
            candidate("2", "Quiet Harbour", Some(1978)),
        ];
        assert_eq!(
            choose(&offered, &wanted).expect("one of them").external_id,
            "2",
            "a remake is still the film someone named, a different film is not"
        );
    }

    #[test]
    fn a_year_that_fits_is_not_an_identification_on_its_own() {
        // Two films of that year came back, and neither is called anything
        // like what was searched for. Taking one used to be what happened,
        // and a film wrongly named looks exactly like a film correctly named.
        let wanted = work_named("Quiet Harbour", Some(2019));
        let offered = vec![
            candidate("1", "Something Invented", Some(2019)),
            candidate("2", "Something Else", Some(2019)),
        ];
        assert!(
            choose(&offered, &wanted).is_none(),
            "a date in common is a coincidence, not a name"
        );
    }

    #[test]
    fn a_file_named_after_the_original_title_still_finds_its_film() {
        // A copy named in the language it was shot in, offered under its
        // French title. Without this clause the name would match nothing.
        let wanted = work_named("Quiet Harbour", Some(2019));
        let offered = vec![
            candidate("1", "Un Autre Film", Some(2019)),
            MovieCandidate {
                original_title: Some("Quiet Harbour".to_string()),
                ..candidate("2", "Port Tranquille", Some(2019))
            },
        ];
        assert_eq!(
            choose(&offered, &wanted).expect("one of them").external_id,
            "2"
        );
    }

    #[test]
    fn a_common_title_is_settled_by_the_provider_and_not_by_a_year_off_a_file() {
        // Twenty films share a common word for a title. One of them is the one
        // nearly everybody means, and the provider lists it first. A file
        // dated by its festival showing matches the year of an obscure other
        // one exactly, which used to be enough to pick it.
        let wanted = work_named("Quiet Harbour", Some(2024));
        let offered = vec![
            candidate("1", "Quiet Harbour", Some(2025)),
            candidate("2", "Quiet Harbour", Some(2022)),
            candidate("3", "Quiet Harbour", Some(2024)),
        ];
        assert_eq!(
            choose(&offered, &wanted).expect("one of them").external_id,
            "1",
            "a year and a half apart tells two dates of one film from two films"
        );
    }

    #[test]
    fn a_film_from_another_time_is_still_ruled_out_by_the_year() {
        // What the year is good at, and the reason it is asked first.
        let wanted = work_named("Quiet Harbour", Some(2019));
        let offered = vec![
            candidate("1", "Quiet Harbour", Some(1978)),
            candidate("2", "Quiet Harbour", Some(2019)),
        ];
        assert_eq!(
            choose(&offered, &wanted).expect("one of them").external_id,
            "2"
        );
    }

    #[test]
    fn when_every_film_of_that_name_is_from_another_time_the_name_still_wins() {
        let wanted = work_named("Quiet Harbour", Some(2019));
        let offered = vec![
            candidate("1", "Amber Field", Some(2019)),
            candidate("2", "Quiet Harbour", Some(1978)),
        ];
        assert_eq!(
            choose(&offered, &wanted).expect("one of them").external_id,
            "2",
            "a remake is still the film somebody named, a different film is not"
        );
    }

    #[test]
    fn with_nothing_to_go_on_no_film_is_chosen_at_all() {
        // What a search made of a name the provider does not know comes back
        // with: films, because a search always comes back with films. Taking
        // the first of them is picking one at random, and it is how a
        // making-of ended up on two films of a series at once.
        let wanted = work_named("Something Invented", None);
        let offered = vec![
            candidate("1", "Amber Field", Some(2020)),
            candidate("2", "Winter Signal", Some(2021)),
        ];
        assert!(
            choose(&offered, &wanted).is_none(),
            "a film nobody could name says so and waits, which is visible and correctable"
        );
        assert!(choose(&[], &wanted).is_none());
    }

    #[test]
    fn a_name_written_by_another_hand_is_still_recognised_when_the_year_agrees() {
        // The commonest shape of all: whoever named the file left out a word
        // the provider keeps, or kept one it drops.
        let wanted = work_named("Quiet Harbour 2", Some(2019));
        let offered = vec![
            candidate("1", "Something Else Entirely", Some(2019)),
            candidate("2", "Quiet Harbour Vol. 2", Some(2019)),
        ];
        assert_eq!(
            choose(&offered, &wanted).expect("one of them").external_id,
            "2"
        );
    }

    #[test]
    fn the_closest_of_several_near_misses_is_the_one_taken() {
        // A making-of carries the whole title of its film and several words
        // more. Both are close, and only one of them is the film.
        let wanted = work_named("Quiet Harbour Rising Tide", Some(2019));
        let offered = vec![
            candidate(
                "1",
                "Quiet Harbour Rising Tide, The Making Of It",
                Some(2019),
            ),
            candidate("2", "Quiet Harbour & Rising Tide", Some(2019)),
        ];
        assert_eq!(
            choose(&offered, &wanted).expect("one of them").external_id,
            "2"
        );
    }

    #[test]
    fn a_near_miss_from_another_time_is_not_the_film() {
        // The year is what stops a name that is nearly right from being a
        // different film altogether.
        let wanted = work_named("Quiet Harbour 2", Some(2019));
        let offered = vec![candidate("1", "Quiet Harbour Vol. 2", Some(1994))];
        assert!(choose(&offered, &wanted).is_none());
    }

    #[test]
    fn with_no_year_on_the_file_the_name_alone_has_to_be_close_enough() {
        let wanted = work_named("The Descent 2", None);
        let offered = vec![
            candidate("1", "The Descent: Part 2", Some(2009)),
            candidate("2", "The Longest Descent, Part 2, To The Sea", Some(2020)),
        ];
        assert_eq!(
            choose(&offered, &wanted).expect("one of them").external_id,
            "1"
        );
    }

    #[test]
    fn a_word_in_common_is_not_a_name_in_common() {
        // What the closeness is really there to refuse: a search the provider
        // never understood answers films that share an ordinary word.
        let wanted = work_named("Studio Invented Harbour", Some(2009));
        let offered = vec![
            candidate("1", "The Making Of Something And Harbour", Some(2009)),
            candidate("2", "Amber Field And Harbour", Some(2009)),
        ];
        assert!(choose(&offered, &wanted).is_none());
    }

    #[test]
    fn a_title_spelled_with_other_punctuation_is_the_same_title() {
        // The reason the name can be made to decide on its own: whoever named
        // the file dropped a colon, and no provider ever does.
        let wanted = work_named("Quiet Harbour Rising Tide", Some(2019));
        let offered = vec![candidate("1", "Quiet Harbour: Rising Tide", Some(2019))];
        assert_eq!(
            choose(&offered, &wanted).expect("one of them").external_id,
            "1"
        );
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
            credits: vec![
                Credit {
                    external_id: "1".to_string(),
                    name: "Alix Moreau".to_string(),
                    role: "actor".to_string(),
                    character: Some("Camille".to_string()),
                    ordinal: 0,
                    photo_path: Some("/alix.jpg".to_string()),
                },
                Credit {
                    external_id: "3".to_string(),
                    name: "Sacha Nord".to_string(),
                    role: "director".to_string(),
                    character: None,
                    ordinal: 0,
                    photo_path: Some("/sacha.jpg".to_string()),
                },
            ],
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

    async fn run(state: &AppState, provider: &Arc<StandIn>, library: &Library) -> IdentifyReport {
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
    async fn two_films_the_provider_knows_nothing_about_stay_two_films() {
        // Two different films of one series, whose names carry a word the
        // provider has never heard of. A search always comes back with
        // something, and taking that something named both after the same
        // stranger; both then carried one identifier, so they were put on one
        // page, and one film of the series was gone from the library.
        let (_directory, state, library, _) =
            state_with_work("Studio Invented Harbour", Some(2009)).await;
        state
            .database()
            .create_work(
                library.id,
                WorkKind::Movie,
                "Studio Invented Harbour",
                "studio invented harbour 2013",
                Some(2013),
            )
            .await
            .expect("work created");

        let provider = Arc::new(StandIn::new(
            vec![candidate("999", "The Making Of Something Else", Some(2024))],
            vec![details("999", "The Making Of Something Else", Some(2024))],
        ));

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.identified, 0, "neither is that film");
        assert_eq!(report.unidentified, 2);
        assert_eq!(report.merged, 0, "two films are two films");
        assert_eq!(
            state
                .database()
                .count_works(library.id)
                .await
                .expect("read"),
            2
        );
    }

    #[tokio::test]
    async fn two_copies_the_provider_calls_one_film_become_one_film() {
        // One copy named after the title the film was shot under, the other
        // after the title it was released under here. No rule reading names
        // could ever bring those two together; the provider answers the same
        // film for both, and that is the whole proof.
        let (_directory, state, library, first) =
            state_with_work("Quiet Harbour", Some(2019)).await;
        let second = state
            .database()
            .create_work(
                library.id,
                WorkKind::Movie,
                "Port Tranquille",
                "port tranquille",
                Some(2019),
            )
            .await
            .expect("work created");
        for (work_id, name) in [
            (first.id, "Quiet Harbour 2019 1080p.mkv"),
            (second.id, "Port Tranquille 2019 1080p.mkv"),
        ] {
            state
                .database()
                .insert_source(
                    work_id,
                    library.roots[0].id,
                    std::path::Path::new(name),
                    1_000,
                    melyxar_core::time::now(),
                )
                .await
                .expect("source recorded");
        }

        let provider = Arc::new(StandIn::new(
            vec![
                candidate("111", "Quiet Harbour", Some(2019)),
                candidate("111", "Port Tranquille", Some(2019)),
            ],
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.identified, 2);
        assert_eq!(report.merged, 1);

        assert_eq!(
            state
                .database()
                .count_works(library.id)
                .await
                .expect("read"),
            1,
            "the same title, poster and synopsis twice in a grid is the defect this avoids"
        );
        let kept = state
            .database()
            .work(first.id)
            .await
            .expect("read")
            .expect("the one that has been here longest stays");
        let sources = state
            .database()
            .sources_of_root(library.roots[0].id)
            .await
            .expect("read");
        assert_eq!(sources.len(), 2, "not one copy is lost in the move");
        assert!(sources.iter().all(|source| source.work_id == kept.id));

        // And a second run has nothing left to put together.
        assert_eq!(run(&state, &provider, &library).await.merged, 0);
    }

    #[tokio::test]
    async fn a_film_the_provider_knows_gets_everything_a_page_shows() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));

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
        let provider = Arc::new(StandIn::new(
            vec![
                candidate("999", "Quiet Harbour", Some(1978)),
                candidate("111", "Quiet Harbour", Some(2019)),
            ],
            vec![
                details("999", "Quiet Harbour", Some(1978)),
                details("111", "Quiet Harbour", Some(2019)),
            ],
        ));

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
        let provider = Arc::new(StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));

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

        let provider = Arc::new(StandIn::new(
            Vec::new(),
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));
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

        let provider = Arc::new(StandIn::new(
            Vec::new(),
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));
        let report = run(&state, &provider, &library).await;

        assert_eq!(report.identified, 1);
        assert!(provider.searches().is_empty());
    }

    #[tokio::test]
    async fn an_accent_written_as_a_mark_of_its_own_still_finds_its_film() {
        // The file carries the plain letter followed by the accent, which
        // looks the same on any screen and is not the same text. A provider
        // that matches exactly answers nothing to it, so the title is asked
        // again in plain letters.
        let (_directory, state, library, work) =
            state_with_work("La rue\u{301}e vers l'or", Some(1925)).await;
        let provider = Arc::new(StandIn::matching_exactly(
            vec![candidate("111", "La ru\u{e9}e vers l'or", Some(1925))],
            vec![details("111", "La ru\u{e9}e vers l'or", Some(1925))],
        ));

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.identified, 1, "searches: {:?}", provider.searches());
        assert_eq!(
            state
                .database()
                .work(work.id)
                .await
                .expect("read")
                .expect("present")
                .identification,
            IdentificationState::Identified
        );

        let asked: Vec<String> = provider
            .searches()
            .into_iter()
            .map(|(title, _)| title)
            .collect();
        assert!(
            asked.iter().any(|title| title.is_ascii()),
            "the last question is asked in plain letters: {asked:?}"
        );
    }

    #[tokio::test]
    async fn a_film_nobody_recognised_stays_in_the_library_with_a_marker() {
        let (_directory, state, library, work) =
            state_with_work("Something Invented", Some(2019)).await;
        let provider = Arc::new(StandIn::new(Vec::new(), Vec::new()));

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.unidentified, 1);

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.identification, IdentificationState::Unidentified);
        assert_eq!(
            stored.identification_note,
            Some(IdentificationNote::NoMatch),
            "the film says why nobody named it, so nobody has to read a log"
        );
        assert!(
            stored.identification.may_be_looked_up_again(),
            "a film nobody recognised today may be recognised tomorrow"
        );
    }

    #[tokio::test]
    async fn a_provider_that_answers_nonsense_is_not_worth_waiting_for() {
        // Told apart from a provider that is down: asking again changes
        // nothing, so the film is marked rather than left on the waiting list
        // for ever.
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(StandIn::failing(|| {
            ProviderError::Unexpected("not json at all".into())
        }));

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.unidentified, 1);
        assert_eq!(report.postponed, 0);

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.identification, IdentificationState::Unidentified);
        assert_eq!(
            stored.identification_note,
            Some(IdentificationNote::ProviderUnreadable),
            "a film blamed for someone else's broken answer is a film nobody can fix"
        );
    }

    #[tokio::test]
    async fn a_provider_asking_to_be_left_alone_says_so_on_the_film_itself() {
        // The likeliest reason a whole library stays nameless after one run:
        // told apart from a provider that is down, because the answer to one
        // is to wait and the answer to the other is to look at the network.
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(StandIn::failing(|| ProviderError::TooManyRequests {
            retry_after_seconds: Some(3),
        }));

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.postponed, 1);
        assert_eq!(report.unidentified, 0);

        let stored = state
            .database()
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.identification, IdentificationState::Pending);
        assert_eq!(
            stored.identification_note,
            Some(IdentificationNote::ProviderBusy)
        );
    }

    #[tokio::test]
    async fn a_library_that_learnt_something_says_so_to_whoever_is_reading_it() {
        // The counter is how a client knows its copy of the grid is stale. A
        // run that changed nothing must not move it, or every client throws
        // away what it holds for nothing.
        let (_directory, state, library, _) = state_with_work("Quiet Harbour", Some(2019)).await;
        let before = state
            .database()
            .library_version(library.id)
            .await
            .expect("read");

        let nothing_found = Arc::new(StandIn::new(Vec::new(), Vec::new()));
        run(&state, &nothing_found, &library).await;
        assert_eq!(
            state
                .database()
                .library_version(library.id)
                .await
                .expect("read"),
            before,
            "nothing was learnt, so nothing is stale"
        );

        // A film nobody recognised today may be recognised tomorrow, so the
        // same film is looked up again, this time by a provider that knows it.
        let provider = Arc::new(StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));
        assert_eq!(run(&state, &provider, &library).await.identified, 1);
        assert!(
            state
                .database()
                .library_version(library.id)
                .await
                .expect("read")
                > before,
            "a film that gained a title changed what every grid shows"
        );
    }

    #[tokio::test]
    async fn a_provider_that_is_down_leaves_the_work_waiting_rather_than_marking_it() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(StandIn::failing(|| {
            ProviderError::Unreachable("timed out".into())
        }));

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
            stored.identification_note,
            Some(IdentificationNote::ProviderUnreachable),
            "a film still waiting must not look like one nobody has tried"
        );
        assert_eq!(
            state
                .database()
                .works_awaiting_identification(library.id)
                .await
                .expect("read")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn a_key_the_provider_refuses_stops_the_run_and_blames_the_key() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(StandIn::failing(|| ProviderError::Unauthorised));

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
    async fn a_library_bigger_than_one_handful_is_named_in_one_run() {
        // Four disks make a library several times the size of what is read
        // from the table at once. Stopping at the first handful would mean
        // pressing the button again and again, with nothing to say why.
        let (_directory, state, library, _) = state_with_work("Quiet Harbour", Some(2019)).await;

        let more = 240;
        let mut candidates = vec![candidate("0", "Quiet Harbour", Some(2019))];
        let mut described = vec![details("0", "Quiet Harbour", Some(2019))];
        for index in 1..more {
            let title = format!("Invented Film {index}");
            state
                .database()
                .create_work(
                    library.id,
                    WorkKind::Movie,
                    &title,
                    &naming::sort_title(&title),
                    Some(2019),
                )
                .await
                .expect("work created");
            candidates.push(candidate(&index.to_string(), &title, Some(2019)));
            described.push(details(&index.to_string(), &title, Some(2019)));
        }

        let provider = Arc::new(StandIn::new(candidates, described));
        let report = run(&state, &provider, &library).await;

        assert_eq!(report.identified, more, "every one of them, in one run");
        assert!(
            state
                .database()
                .works_awaiting_identification(library.id)
                .await
                .expect("read")
                .is_empty(),
            "nothing is left waiting"
        );
    }

    #[tokio::test]
    async fn a_provider_that_is_down_stops_the_run_rather_than_reading_for_ever() {
        // Every film comes back postponed, so every film stays in the table.
        // Reading the table again would hand back the same ones for ever.
        let (_directory, state, library, _) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(StandIn::failing(|| {
            ProviderError::Unreachable("timed out".into())
        }));

        let report = run(&state, &provider, &library).await;
        assert_eq!(report.postponed, 1);
        assert_eq!(report.identified, 0);
    }

    #[tokio::test]
    async fn a_work_already_identified_is_left_alone() {
        let (_directory, state, library, _) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));

        assert_eq!(run(&state, &provider, &library).await.identified, 1);
        let second = run(&state, &provider, &library).await;
        assert_eq!(second.identified, 0);
        assert_eq!(second.unidentified, 0);
    }

    #[tokio::test]
    async fn a_match_picked_by_hand_is_never_undone_by_a_later_run() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(StandIn::new(
            vec![candidate("999", "Quiet Harbour", Some(2019))],
            vec![
                details("999", "Quiet Harbour", Some(2019)),
                details("111", "Quiet Harbour", Some(2019)),
            ],
        ));

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

    fn trailer(name: &str, language: Option<&str>, official: bool) -> Trailer {
        Trailer {
            name: name.to_string(),
            site: "YouTube".to_string(),
            key: name.to_lowercase().replace(' ', "-"),
            is_official: official,
            language: language.map(str::to_string),
        }
    }

    #[test]
    fn the_trailer_a_viewer_would_pick_comes_first() {
        let found = best_trailers(
            &[
                trailer("Fan edit", None, false),
                trailer("Official English trailer", Some("eng"), true),
                trailer("Bande annonce officielle", Some("fre"), true),
                trailer("Extrait francais", Some("fre"), false),
            ],
            "fr",
        );

        assert_eq!(found[0].name, "Bande annonce officielle");
        assert_eq!(
            found[1].name, "Extrait francais",
            "the language matters more than the stamp of approval"
        );
    }

    #[test]
    fn a_page_offers_a_trailer_rather_than_a_list_of_five() {
        let many: Vec<Trailer> = (0..8)
            .map(|index| trailer(&format!("Trailer {index}"), Some("fre"), true))
            .collect();
        assert_eq!(best_trailers(&many, "fr").len(), 3);
    }

    #[test]
    fn a_trailer_on_a_site_nobody_knows_is_left_out_rather_than_linked_wrongly() {
        let elsewhere = Trailer {
            site: "SomeSite".to_string(),
            ..trailer("Ailleurs", Some("fre"), true)
        };
        assert!(best_trailers(&[elsewhere], "fr").is_empty());
    }

    #[tokio::test]
    async fn a_trailer_hosted_elsewhere_is_kept_as_a_link() {
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));
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
        let provider = Arc::new(
            StandIn::new(
                vec![candidate("111", "Quiet Harbour", Some(2019))],
                vec![details("111", "Quiet Harbour", Some(2019))],
            )
            .serving(picture),
        );

        // The count is what the log reports, so it is worth an answer of its
        // own: one picture prepared the first time, none the second, since the
        // poster has not changed.
        let details = details("111", "Quiet Harbour", Some(2019));
        assert_eq!(
            crate::images::store_provider_images(&state, provider.as_ref(), work.id, &details)
                .await,
            1,
            "the film has a poster and no backdrop, so one picture is prepared"
        );
        assert_eq!(
            crate::images::store_provider_images(&state, provider.as_ref(), work.id, &details)
                .await,
            0,
            "a picture already here is not prepared a second time"
        );

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
    async fn the_faces_of_the_cast_are_prepared_and_nobody_elses() {
        let Some(picture) = a_real_poster() else {
            eprintln!("no media tool here, the preparation of a picture was not exercised");
            return;
        };

        let (_directory, state, library, work) =
            state_with_tools("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(
            StandIn::new(
                vec![candidate("111", "Quiet Harbour", Some(2019))],
                vec![details("111", "Quiet Harbour", Some(2019))],
            )
            .serving(picture),
        );

        run(&state, &provider, &library).await;

        let detail = crate::detail::work_detail(&state, work.id)
            .await
            .expect("read")
            .expect("present");
        let actor = detail
            .credits
            .iter()
            .find(|credit| credit.role == "actor")
            .expect("an actor was credited");
        assert_eq!(
            actor.photo.len(),
            2,
            "one size per width a face is shown at"
        );
        assert!(actor
            .photo
            .iter()
            .all(|image| image.relative_path.starts_with("people/")
                && image.relative_path.ends_with(".webp")));

        let root = state.config().directories.images();
        for image in &actor.photo {
            assert!(
                root.join(&image.relative_path).exists(),
                "a row without its file is a broken picture"
            );
        }

        let director = detail
            .credits
            .iter()
            .find(|credit| credit.role == "director")
            .expect("a director was credited");
        assert!(
            director.photo.is_empty(),
            "a page reads the crew as names, so their faces are never fetched"
        );
    }

    #[tokio::test]
    async fn a_film_with_a_long_cast_does_not_fetch_a_face_nobody_scrolls_to() {
        let Some(picture) = a_real_poster() else {
            return;
        };
        let (_directory, state, library, _) = state_with_tools("Quiet Harbour", Some(2019)).await;

        let mut crowded = details("111", "Quiet Harbour", Some(2019));
        crowded.credits = (0..40)
            .map(|index| Credit {
                external_id: format!("p{index}"),
                name: format!("Invented Name {index}"),
                role: "actor".to_string(),
                character: Some(format!("Part {index}")),
                ordinal: index,
                photo_path: Some(format!("/face-{index}.jpg")),
            })
            .collect();

        let provider = Arc::new(
            StandIn::new(
                vec![candidate("111", "Quiet Harbour", Some(2019))],
                vec![crowded],
            )
            .serving(picture),
        );
        run(&state, &provider, &library).await;

        let faces: Vec<String> = provider
            .fetched()
            .into_iter()
            .filter(|path| path.starts_with("/face-"))
            .collect();
        assert_eq!(
            faces.len(),
            18,
            "a page shows the leads, not the call sheet"
        );
        assert!(
            faces.contains(&"/face-0.jpg".to_string())
                && !faces.contains(&"/face-39.jpg".to_string()),
            "the faces fetched are the ones billed first: {faces:?}"
        );
    }

    #[tokio::test]
    async fn a_face_already_fetched_is_not_fetched_again_for_the_next_film() {
        let Some(picture) = a_real_poster() else {
            return;
        };
        let (_directory, state, library, work) =
            state_with_tools("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(
            StandIn::new(
                vec![candidate("111", "Quiet Harbour", Some(2019))],
                vec![details("111", "Quiet Harbour", Some(2019))],
            )
            .serving(picture),
        );

        run(&state, &provider, &library).await;
        let people = state
            .database()
            .apply_identification(
                work.id,
                &to_record(&details("111", "Quiet Harbour", Some(2019)), "tmdb", "fr"),
                false,
            )
            .await
            .expect("applied");

        assert_eq!(
            crate::images::store_person_photos(&state, &provider, &people).await,
            0,
            "a face that did not change must not be fetched or written again"
        );

        // The same people credited on another film: still nobody new, so
        // still nothing prepared, while a face nobody has yet is prepared.
        let (_elsewhere, other_state, other_library, other_work) =
            state_with_tools("Amber Field", Some(2020)).await;
        let _ = other_library;
        let fresh = other_state
            .database()
            .apply_identification(
                other_work.id,
                &to_record(&details("111", "Quiet Harbour", Some(2019)), "tmdb", "fr"),
                false,
            )
            .await
            .expect("applied");
        assert_eq!(
            crate::images::store_person_photos(&other_state, &provider, &fresh).await,
            1,
            "one actor with a photo, so one face prepared"
        );
    }

    #[tokio::test]
    async fn a_poster_that_has_not_changed_is_not_fetched_again() {
        let Some(picture) = a_real_poster() else {
            return;
        };
        let (_directory, state, library, work) =
            state_with_tools("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(
            StandIn::new(
                vec![candidate("111", "Quiet Harbour", Some(2019))],
                vec![details("111", "Quiet Harbour", Some(2019))],
            )
            .serving(picture),
        );

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
            provider.as_ref(),
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
    async fn a_film_nobody_translated_still_gets_a_synopsis() {
        // Plenty of films are described in one language and not yet in
        // another. A page with an empty half is worse than a page with a
        // paragraph somebody can read.
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let provider = Arc::new(
            StandIn::new(
                vec![candidate("111", "Quiet Harbour", Some(2019))],
                vec![details("111", "Quiet Harbour", Some(2019))],
            )
            .described_only_in_english(),
        );

        assert_eq!(run(&state, &provider, &library).await.identified, 1);

        let detail = crate::detail::work_detail(&state, work.id)
            .await
            .expect("read")
            .expect("present");
        assert!(
            detail.overview.is_some_and(|text| !text.is_empty()),
            "the library is in French and the film is described in English, which is a synopsis"
        );
    }

    #[tokio::test]
    async fn every_film_with_a_hole_in_it_is_dealt_with_however_many_there_are() {
        // The same bound as the look up, and the same reason it must not stop
        // a run: a library of four disks is several handfuls. Told through the
        // synopsis rather than the pictures, which are the same loop and cost
        // a run of the media tool each.
        let (_directory, state, library, _) = state_with_work("Quiet Harbour", Some(2019)).await;

        let more = 205;
        let mut candidates = vec![candidate("0", "Quiet Harbour", Some(2019))];
        let mut described = vec![details("0", "Quiet Harbour", Some(2019))];
        for index in 1..more {
            let title = format!("Invented Film {index}");
            state
                .database()
                .create_work(
                    library.id,
                    WorkKind::Movie,
                    &title,
                    &naming::sort_title(&title),
                    Some(2019),
                )
                .await
                .expect("work created");
            candidates.push(candidate(&index.to_string(), &title, Some(2019)));
            described.push(details(&index.to_string(), &title, Some(2019)));
        }

        // Named on a day the provider had no words for any of them.
        let wordless: Vec<MovieDetails> = described
            .iter()
            .cloned()
            .map(|details| MovieDetails {
                overview: None,
                ..details
            })
            .collect();
        let silent = Arc::new(StandIn::new(candidates.clone(), wordless));
        assert_eq!(run(&state, &silent, &library).await.identified, more);

        let talking = Arc::new(StandIn::new(candidates, described));
        assert_eq!(
            run(&state, &talking, &library).await.synopses_filled,
            more,
            "every one of them, in one run"
        );
    }

    #[tokio::test]
    async fn a_synopsis_that_did_not_arrive_that_day_is_asked_for_again() {
        // The same hole as a missing picture, and the same reason nothing ever
        // filled it: a provider is only ever asked at the moment a film is
        // named. A film described in another language since, or one whose
        // answer came back empty that day, stays empty for ever.
        let (_directory, state, library, work) = state_with_work("Quiet Harbour", Some(2019)).await;
        let mut wordless = details("111", "Quiet Harbour", Some(2019));
        wordless.overview = None;
        let silent = Arc::new(StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![wordless],
        ));
        assert_eq!(run(&state, &silent, &library).await.identified, 1);
        assert!(crate::detail::work_detail(&state, work.id)
            .await
            .expect("read")
            .expect("present")
            .overview
            .is_none());

        // The same film, a day the provider has words for it.
        let talking = Arc::new(StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));
        let report = run(&state, &talking, &library).await;
        assert_eq!(report.identified, 0);
        assert_eq!(report.synopses_filled, 1);

        let detail = crate::detail::work_detail(&state, work.id)
            .await
            .expect("read")
            .expect("present");
        assert!(detail.overview.is_some_and(|text| !text.is_empty()));
        assert_eq!(
            detail.work.title, "Quiet Harbour",
            "only the hole is filled; nothing else about the film is written again"
        );

        assert_eq!(run(&state, &talking, &library).await.synopses_filled, 0);
    }

    #[tokio::test]
    async fn a_picture_that_did_not_arrive_that_day_is_asked_for_again() {
        // The case this exists for: the film was named on a day the pictures
        // could not be written, so it kept its title and a grey rectangle, and
        // nothing ever asked a second time.
        let Some(picture) = a_real_poster() else {
            eprintln!("no media tool here, the preparation of a picture was not exercised");
            return;
        };
        let (_directory, state, library, work) =
            state_with_tools("Quiet Harbour", Some(2019)).await;

        // Named by a provider that could serve no picture at all.
        let empty_handed = Arc::new(StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));
        assert_eq!(run(&state, &empty_handed, &library).await.identified, 1);
        assert!(state
            .database()
            .images_of("work", &work.id.to_db_string())
            .await
            .expect("read")
            .is_empty());

        // The same library on a day the pictures can be had. Nothing is left
        // to identify, so only the pictures are the point.
        let serving = Arc::new(
            StandIn::new(
                vec![candidate("111", "Quiet Harbour", Some(2019))],
                vec![details("111", "Quiet Harbour", Some(2019))],
            )
            .serving(picture),
        );
        let report = run(&state, &serving, &library).await;
        assert_eq!(report.identified, 0);
        assert_eq!(report.pictures_filled, 1);

        let posters: Vec<_> = state
            .database()
            .images_of("work", &work.id.to_db_string())
            .await
            .expect("read")
            .into_iter()
            .filter(|image| image.image_kind == "poster")
            .collect();
        assert_eq!(posters.len(), 3, "one size per width the interface serves");

        // And a third run has nothing left to do, so nobody is asked again.
        assert_eq!(run(&state, &serving, &library).await.pictures_filled, 0);
    }

    #[tokio::test]
    async fn a_picture_that_will_not_come_never_costs_the_film_its_identification() {
        let (_directory, state, library, work) =
            state_with_tools("Quiet Harbour", Some(2019)).await;
        // The stand-in serves no picture at all.
        let provider = Arc::new(StandIn::new(
            vec![candidate("111", "Quiet Harbour", Some(2019))],
            vec![details("111", "Quiet Harbour", Some(2019))],
        ));

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
        // The identifier is what an activity page follows the run by, so it
        // has to name a row that is really there.
        let followed = job.id();
        let (job_state, report) = job.wait().await;

        assert_eq!(job_state, JobState::Succeeded);
        assert_eq!(report.expect("a finished run has a report").identified, 1);
        assert_eq!(
            state
                .database()
                .job(followed)
                .await
                .expect("read")
                .expect("the run was written down before it started")
                .state,
            JobState::Succeeded
        );
    }
}
