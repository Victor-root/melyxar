//! Looking a title up at the provider, films and series together, by name,
//! by identifier or by a link pasted whole; and saying of each answer
//! whether it is here already, asked for already, or free to ask for.

use std::collections::HashMap;
use std::sync::Arc;

use melyxar_core::id::{RequestId, WorkId};
use melyxar_core::user::User;
use melyxar_database::requests::{HeldTitle, TitleRequest};
use melyxar_metadata::pointed::{pointed_at, Pointed};
use melyxar_metadata::{Candidate, MetadataProvider};

use super::{access, provider_language, word_of, Catalogue, Result};
use crate::AppState;

/// The most answers a search gives.
const ANSWERS_AT_MOST: usize = 30;

/// How wide the posters of a list are asked for, in pixels.
pub const POSTER_WIDTH: u32 = 342;

/// A title as the libraries this account sees hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Held {
    /// The work to open.
    pub work_id: WorkId,
    /// For a series, the seasons with at least one episode.
    pub seasons: Vec<i32>,
}

/// One answer, and where it stands.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub candidate: Candidate,
    /// Full address of its poster.
    pub poster: Option<String>,
    pub held: Option<Held>,
    /// How many accounts asked for it and wait.
    pub asked_by: usize,
    /// This account's own request for it, while it waits.
    pub mine: Option<RequestId>,
}

/// One season of a series, as it may be asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeasonChoice {
    /// Nought for the specials.
    pub number: i32,
    pub episodes: i32,
    pub held: bool,
}

/// What a line names, asked of the provider. A number alone may be a film,
/// a series or a title written in figures, and is asked as all three.
async fn candidates<P: MetadataProvider>(
    provider: &P,
    line: &str,
    language: &str,
) -> Result<Vec<Candidate>> {
    let by_name = || async {
        let (films, series) = tokio::join!(
            provider.search(Catalogue::Films, line, None, language),
            provider.search(Catalogue::Series, line, None, language),
        );
        let mut found = films?;
        found.extend(series?);
        found.sort_by(|one, other| other.popularity.total_cmp(&one.popularity));
        Ok::<_, melyxar_metadata::ProviderError>(found)
    };
    Ok(match pointed_at(line) {
        Some(Pointed::Provider(catalogue, id)) => {
            vec![provider.details(catalogue, &id, language).await?.as_candidate(catalogue)]
        }
        Some(Pointed::Imdb(id)) => provider.by_imdb_id(&id, language).await?.into_iter().collect(),
        Some(Pointed::Number(id)) => {
            let (film, series, named) = tokio::join!(
                provider.details(Catalogue::Films, &id, language),
                provider.details(Catalogue::Series, &id, language),
                by_name(),
            );
            // A number that is no film or no series is not a failure: it
            // was perhaps a title.
            let mut found: Vec<Candidate> = [(film, Catalogue::Films), (series, Catalogue::Series)]
                .into_iter()
                .filter_map(|(details, catalogue)| Some(details.ok()?.as_candidate(catalogue)))
                .collect();
            found.extend(named?);
            found
        }
        None => by_name().await?,
    })
}

/// What the libraries an account sees hold of some titles, and the requests
/// waiting on them, catalogue by catalogue.
pub(super) struct Standing {
    held: HashMap<(Catalogue, String), HeldTitle>,
    open: HashMap<(Catalogue, String), Vec<TitleRequest>>,
}

pub(super) async fn standing_of(
    state: &AppState,
    who: &User,
    found: &[Candidate],
) -> Result<Standing> {
    let database = state.database();
    let within = crate::reach::within(who);
    let mut standing = Standing {
        held: HashMap::new(),
        open: HashMap::new(),
    };
    for catalogue in [Catalogue::Films, Catalogue::Series] {
        let ids: Vec<String> = found
            .iter()
            .filter(|one| one.catalogue == catalogue)
            .map(|one| one.external_id.clone())
            .collect();
        for (id, held) in database
            .held_titles(word_of(catalogue), &ids, within.as_deref())
            .await?
        {
            standing.held.insert((catalogue, id), held);
        }
        for request in database.open_requests_for(word_of(catalogue), &ids).await? {
            standing
                .open
                .entry((catalogue, request.tmdb_id.clone()))
                .or_default()
                .push(request);
        }
    }
    Ok(standing)
}

/// Titles a line could mean, best first, each with where it stands.
pub async fn look_for<P: MetadataProvider>(
    state: &AppState,
    provider: &Arc<P>,
    who: &User,
    line: &str,
    language: &str,
) -> Result<Vec<Found>> {
    access::require_a_look(state, who).await?;
    let line = line.trim();
    if line.is_empty() {
        return Ok(Vec::new());
    }
    let mut found = candidates(provider.as_ref(), line, provider_language(language)).await?;
    let mut seen = std::collections::HashSet::new();
    found.retain(|one| seen.insert((one.catalogue, one.external_id.clone())));
    found.truncate(ANSWERS_AT_MOST);

    let standing = standing_of(state, who, &found).await?;
    Ok(found
        .into_iter()
        .map(|candidate| found_from(candidate, &standing, who, provider.as_ref()))
        .collect())
}

/// An answer, with where it stands among what the libraries hold and what
/// was asked for.
pub(super) fn found_from<P: MetadataProvider>(
    candidate: Candidate,
    standing: &Standing,
    who: &User,
    provider: &P,
) -> Found {
    let key = (candidate.catalogue, candidate.external_id.clone());
    let open = standing.open.get(&key).map(Vec::as_slice).unwrap_or_default();
    Found {
        poster: candidate
            .poster_path
            .as_deref()
            .map(|path| provider.image_url_at(path, POSTER_WIDTH)),
        held: standing.held.get(&key).map(|held| Held {
            work_id: held.work_id,
            seasons: held.seasons.clone(),
        }),
        asked_by: open.len(),
        mine: open.iter().find(|one| one.user_id == who.id).map(|one| one.id),
        candidate,
    }
}

/// The seasons of a series the provider knows, and which the libraries this
/// account sees hold.
pub async fn seasons_of<P: MetadataProvider>(
    state: &AppState,
    provider: &Arc<P>,
    who: &User,
    tmdb_id: &str,
    language: &str,
) -> Result<Vec<SeasonChoice>> {
    access::require_a_look(state, who).await?;
    let details = provider
        .details(Catalogue::Series, tmdb_id, provider_language(language))
        .await?;
    let held = state
        .database()
        .held_titles(
            word_of(Catalogue::Series),
            &[tmdb_id.to_string()],
            crate::reach::within(who).as_deref(),
        )
        .await?
        .remove(tmdb_id)
        .map(|held| held.seasons)
        .unwrap_or_default();
    Ok(details
        .season_lengths
        .iter()
        .map(|season| SeasonChoice {
            number: season.season,
            episodes: season.episodes,
            held: held.contains(&season.season),
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use melyxar_core::library::LibraryKind;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::WorkKind;
    use melyxar_database::requests::NewRequest;

    use super::super::testing::{a_film, a_series, kept_away, requests_on, StandIn};
    use super::*;

    #[tokio::test]
    async fn a_title_held_only_out_of_reach_reads_as_not_held() {
        let (_held, state, viewer) = requests_on().await;
        let kept = kept_away(&state).await;
        let provider = Arc::new(StandIn {
            films: vec![a_film("5", "Paper Moons")],
            series: vec![a_series("7", "Salt Road", &[(1, 8), (2, 10)])],
        });
        let held_for = |who: &User| {
            let state = state.clone();
            let provider = Arc::clone(&provider);
            let who = who.clone();
            async move {
                look_for(&state, &provider, &who, "anything", "en")
                    .await
                    .expect("found")
                    .into_iter()
                    .filter(|one| one.held.is_some())
                    .count()
            }
        };
        let seasons_held_for = |who: &User| {
            let state = state.clone();
            let provider = Arc::clone(&provider);
            let who = who.clone();
            async move {
                seasons_of(&state, &provider, &who, "7", "en")
                    .await
                    .expect("read")
                    .into_iter()
                    .filter(|season| season.held)
                    .count()
            }
        };

        assert_eq!(held_for(&viewer).await, 2);
        assert_eq!(seasons_held_for(&viewer).await, 1);
        // Here already, said of a title they cannot open, would tell them
        // what the library kept from them holds.
        assert_eq!(held_for(&kept.kept_from).await, 0);
        assert_eq!(seasons_held_for(&kept.kept_from).await, 0);
    }

    #[tokio::test]
    async fn an_answer_says_whether_it_is_here_asked_for_or_free() {
        let (_held, state, viewer) = requests_on().await;
        let database = state.database();
        let library = database
            .create_library("Films", LibraryKind::Movies, "en", &[])
            .await
            .expect("library");
        let film = database
            .create_work(library.id, WorkKind::Movie, "Amber Field", "amber field", None)
            .await
            .expect("film");
        database.set_work_external_id(film.id, "tmdb", "1").await.expect("named");
        sqlx::query("UPDATE works SET identification = 'identified' WHERE id = ?")
            .bind(film.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("identified");
        let other = database
            .create_user("other", None, &Permissions::viewer())
            .await
            .expect("account");
        for asker in [viewer.id, other.id] {
            database
                .add_request(&NewRequest {
                    user_id: asker,
                    catalogue: "films",
                    tmdb_id: "2",
                    title: "Salt Road",
                    year: None,
                    poster_path: None,
                    overview: None,
                    seasons: &[],
                    note: "",
                    created_at: melyxar_core::time::now(),
                })
                .await
                .expect("asked");
        }

        let provider = Arc::new(StandIn {
            films: vec![a_film("1", "Amber Field"), a_film("2", "Salt Road"), a_film("3", "Ash")],
            series: vec![a_series("1", "Amber Field", &[])],
        });
        let found = look_for(&state, &provider, &viewer, "amber", "fr").await.expect("found");
        let by = |catalogue: Catalogue, id: &str| {
            found
                .iter()
                .find(|one| one.candidate.catalogue == catalogue && one.candidate.external_id == id)
                .expect("answered")
        };
        assert_eq!(found.len(), 4, "films and series together");
        assert_eq!(
            by(Catalogue::Films, "1").held,
            Some(Held { work_id: film.id, seasons: Vec::new() })
        );
        assert_eq!(by(Catalogue::Series, "1").held, None, "a series does not share a film's number");
        let asked = by(Catalogue::Films, "2");
        assert_eq!(asked.asked_by, 2);
        assert!(asked.mine.is_some());
        assert_eq!(by(Catalogue::Films, "3").asked_by, 0);
        assert_eq!(
            by(Catalogue::Films, "3").poster.as_deref(),
            Some("https://pictures.invalid/3.jpg")
        );

        let pasted = look_for(&state, &provider, &viewer, "https://www.themoviedb.org/movie/3-ash", "fr")
            .await
            .expect("found");
        assert_eq!(pasted.len(), 1);
        assert_eq!(pasted[0].candidate.title, "Ash");
    }

    #[tokio::test]
    async fn the_seasons_of_a_series_say_which_are_here() {
        let (_held, state, viewer) = requests_on().await;
        let provider = Arc::new(StandIn {
            films: Vec::new(),
            series: vec![a_series("7", "Salt Road", &[(1, 8), (2, 10)])],
        });
        let seasons = seasons_of(&state, &provider, &viewer, "7", "fr").await.expect("read");
        assert_eq!(
            seasons,
            vec![
                SeasonChoice { number: 1, episodes: 8, held: false },
                SeasonChoice { number: 2, episodes: 10, held: false },
            ]
        );
    }
}
