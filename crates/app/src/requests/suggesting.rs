//! What to suggest to an account that is about to ask for a title: the works
//! the provider finds most popular in the genres this account watches most,
//! and without a taste to go by, the most popular of all. Titles the
//! libraries hold already are left out.

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::future::join_all;
use melyxar_core::user::User;
use melyxar_database::taste::WatchedGenre;
use melyxar_metadata::{Candidate, Genre, MetadataProvider};

use super::search::{found_from, standing_of, Found};
use super::{access, provider_language, Catalogue, Result};
use crate::AppState;

/// How many genres are suggested from, the most watched first.
const SHELVES: usize = 3;

/// The most titles one shelf holds.
const TITLES_PER_SHELF: usize = 18;

/// One row of suggestions.
#[derive(Debug, Clone, PartialEq)]
pub struct Shelf {
    /// The genre it is taken from, in the language asked for. Nothing for
    /// what is popular whatever it is.
    pub genre: Option<String>,
    /// Nothing for a shelf of films and series together.
    pub catalogue: Option<Catalogue>,
    pub found: Vec<Found>,
}

/// A genre worth suggesting from, and how much it is watched.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Pick {
    catalogue: Catalogue,
    genre_id: String,
    name: String,
    titles: i64,
}

fn catalogue_of_kind(kind: &str) -> Catalogue {
    match kind {
        "series" => Catalogue::Series,
        _ => Catalogue::Films,
    }
}

/// The genres to suggest from. What an account watched is told in the words
/// of the library it was watched in, so each name is looked up in the genres
/// of that language to find which genre of the provider it is; the same genre
/// watched under two languages counts once, added up. The name shown is the
/// one in the language asked for.
fn picks(
    taste: &[WatchedGenre],
    lists: &HashMap<(Catalogue, String), Vec<Genre>>,
    shown_in: &str,
) -> Vec<Pick> {
    let mut found: HashMap<(Catalogue, String), Pick> = HashMap::new();
    for watched in taste {
        let catalogue = catalogue_of_kind(&watched.kind);
        let Some(genre) = lists
            .get(&(catalogue, watched.language.clone()))
            .and_then(|list| list.iter().find(|one| one.name.eq_ignore_ascii_case(&watched.name)))
        else {
            continue;
        };
        let name = lists
            .get(&(catalogue, shown_in.to_string()))
            .and_then(|list| list.iter().find(|one| one.id == genre.id))
            .map_or_else(|| watched.name.clone(), |one| one.name.clone());
        found
            .entry((catalogue, genre.id.clone()))
            .and_modify(|pick| pick.titles += watched.titles)
            .or_insert(Pick {
                catalogue,
                genre_id: genre.id.clone(),
                name,
                titles: watched.titles,
            });
    }
    let mut picks: Vec<Pick> = found.into_values().collect();
    picks.sort_by(|one, other| {
        other
            .titles
            .cmp(&one.titles)
            .then_with(|| one.name.cmp(&other.name))
            .then_with(|| (one.catalogue as u8).cmp(&(other.catalogue as u8)))
    });
    picks.truncate(SHELVES);
    picks
}

/// Shelves of titles to ask for, for this account.
pub async fn suggestions<P: MetadataProvider>(
    state: &AppState,
    provider: &Arc<P>,
    who: &User,
    language: &str,
) -> Result<Vec<Shelf>> {
    access::require(state, who).await?;
    let shown_in = provider_language(language);
    let taste = state.database().watched_genres(who.id).await?;

    let mut needed: Vec<(Catalogue, String)> = Vec::new();
    for watched in &taste {
        let catalogue = catalogue_of_kind(&watched.kind);
        for language in [watched.language.as_str(), shown_in] {
            let key = (catalogue, language.to_string());
            if !needed.contains(&key) {
                needed.push(key);
            }
        }
    }
    let answers = join_all(needed.iter().map(|(catalogue, language)| provider.genres(*catalogue, language))).await;
    // A genre list the provider would not give leaves the genres written in
    // that language unmatched, and what is left still makes a suggestion.
    let lists: HashMap<(Catalogue, String), Vec<Genre>> = needed
        .into_iter()
        .zip(answers)
        .filter_map(|(key, list)| Some((key, list.ok()?)))
        .collect();
    let picks = picks(&taste, &lists, shown_in);

    let mut shelves: Vec<(Option<String>, Option<Catalogue>, Vec<Candidate>)> = Vec::new();
    if picks.is_empty() {
        let (films, series) = tokio::join!(
            provider.popular(Catalogue::Films, None, shown_in),
            provider.popular(Catalogue::Series, None, shown_in),
        );
        shelves.push((None, None, interleaved(films?, series?)));
    } else {
        let answers = join_all(
            picks
                .iter()
                .map(|pick| provider.popular(pick.catalogue, Some(&pick.genre_id), shown_in)),
        )
        .await;
        for (pick, answer) in picks.into_iter().zip(answers) {
            shelves.push((Some(pick.name), Some(pick.catalogue), answer?));
        }
    }

    let every: Vec<Candidate> = shelves
        .iter()
        .flat_map(|(_, _, candidates)| candidates.iter().cloned())
        .collect();
    let standing = standing_of(state, &every).await?;
    Ok(shelves
        .into_iter()
        .map(|(genre, catalogue, candidates)| Shelf {
            genre,
            catalogue,
            found: candidates
                .into_iter()
                .map(|candidate| found_from(candidate, &standing, who, provider.as_ref()))
                .filter(|found| found.held.is_none())
                .take(TITLES_PER_SHELF)
                .collect(),
        })
        .filter(|shelf| !shelf.found.is_empty())
        .collect())
}

/// Two lists, one title of each in turn.
fn interleaved(films: Vec<Candidate>, series: Vec<Candidate>) -> Vec<Candidate> {
    let mut films = films.into_iter();
    let mut series = series.into_iter();
    let mut both = Vec::new();
    loop {
        let (film, one) = (films.next(), series.next());
        if film.is_none() && one.is_none() {
            return both;
        }
        both.extend(film);
        both.extend(one);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use melyxar_core::id::WorkId;
    use melyxar_core::time::Millis;
    use melyxar_core::work::{PlaybackState, WorkKind};
    use melyxar_core::library::LibraryKind;

    use super::*;
    use crate::requests::testing::{a_film, requests_on, StandIn};

    fn watched(kind: &str, language: &str, name: &str, titles: i64) -> WatchedGenre {
        WatchedGenre {
            kind: kind.to_string(),
            language: language.to_string(),
            name: name.to_string(),
            titles,
        }
    }

    fn list(entries: &[(&str, &str)]) -> Vec<Genre> {
        entries
            .iter()
            .map(|(id, name)| Genre { id: id.to_string(), name: name.to_string() })
            .collect()
    }

    #[test]
    fn the_most_watched_genres_are_picked_by_their_names_in_the_language_of_the_library() {
        let mut lists = HashMap::new();
        lists.insert(
            (Catalogue::Films, "fr".to_string()),
            list(&[("27", "Horreur"), ("35", "Comédie"), ("53", "Thriller")]),
        );
        lists.insert(
            (Catalogue::Films, "en".to_string()),
            list(&[("27", "Horror"), ("35", "Comedy"), ("53", "Thriller")]),
        );
        let taste = vec![
            watched("movie", "fr", "Horreur", 5),
            watched("movie", "fr", "Comédie", 2),
            watched("movie", "fr", "Un genre fait main", 9),
            watched("movie", "fr", "thriller", 3),
        ];
        let picked = picks(&taste, &lists, "en");
        assert_eq!(
            picked.iter().map(|pick| (pick.genre_id.as_str(), pick.name.as_str())).collect::<Vec<_>>(),
            vec![("27", "Horror"), ("53", "Thriller"), ("35", "Comedy")],
            "ordered by how much is watched, a name nobody knows left out, shown in the language asked for"
        );
    }

    #[test]
    fn a_genre_watched_in_two_languages_counts_once_added_up() {
        let mut lists = HashMap::new();
        lists.insert((Catalogue::Films, "fr".to_string()), list(&[("27", "Horreur")]));
        lists.insert((Catalogue::Films, "en".to_string()), list(&[("27", "Horror")]));
        let taste = vec![watched("movie", "fr", "Horreur", 2), watched("movie", "en", "Horror", 4)];
        let picked = picks(&taste, &lists, "en");
        assert_eq!(picked.len(), 1);
        assert_eq!(picked[0].titles, 6);
    }

    #[test]
    fn films_and_series_of_one_genre_name_stay_apart() {
        let mut lists = HashMap::new();
        lists.insert((Catalogue::Films, "en".to_string()), list(&[("10", "Drama")]));
        lists.insert((Catalogue::Series, "en".to_string()), list(&[("18", "Drama")]));
        let taste = vec![watched("movie", "en", "Drama", 2), watched("series", "en", "Drama", 3)];
        let picked = picks(&taste, &lists, "en");
        assert_eq!(picked.len(), 2);
        assert_eq!(picked[0].catalogue, Catalogue::Series);
    }

    #[test]
    fn two_lists_are_read_one_title_of_each_in_turn() {
        let candidate = |id: &str| Candidate {
            external_id: id.to_string(),
            catalogue: Catalogue::Films,
            title: id.to_string(),
            original_title: None,
            release_year: None,
            overview: None,
            poster_path: None,
            popularity: 1.0,
        };
        let both = interleaved(
            vec![candidate("f1"), candidate("f2"), candidate("f3")],
            vec![candidate("s1")],
        );
        assert_eq!(
            both.iter().map(|one| one.external_id.as_str()).collect::<Vec<_>>(),
            vec!["f1", "s1", "f2", "f3"]
        );
    }

    #[tokio::test]
    async fn an_account_is_suggested_what_it_watches_without_what_is_here() {
        let (_held, state, viewer) = requests_on().await;
        let database = state.database();
        let library = database
            .create_library("Films", LibraryKind::Movies, "en", &[("disk".to_string(), std::path::PathBuf::from("/m"))])
            .await
            .expect("library")
            .id;
        let seen = database
            .create_work(library, WorkKind::Movie, "Seen", "seen", None)
            .await
            .expect("film");
        let genre: WorkId = seen.id;
        sqlx::query("INSERT INTO genres (id, name) VALUES ('g1', 'Horror')")
            .execute(database.writer())
            .await
            .expect("genre");
        sqlx::query("INSERT INTO work_genres (work_id, genre_id) VALUES (?, 'g1')")
            .bind(genre.to_db_string())
            .execute(database.writer())
            .await
            .expect("linked");
        database
            .record_playback_progress(viewer.id, seen.id, Millis::new(0), PlaybackState::Watched, melyxar_core::time::now())
            .await
            .expect("watched");

        let mut scary = a_film("1", "Scary");
        scary.genres = vec!["Horror".to_string()];
        let mut funny = a_film("2", "Funny");
        funny.genres = vec!["Comedy".to_string()];
        let provider = Arc::new(StandIn { films: vec![scary, funny], series: Vec::new() });

        let shelves = suggestions(&state, &provider, &viewer, "en").await.expect("suggested");
        assert_eq!(shelves.len(), 1);
        assert_eq!(shelves[0].genre.as_deref(), Some("Horror"));
        assert_eq!(
            shelves[0].found.iter().map(|one| one.candidate.title.as_str()).collect::<Vec<_>>(),
            vec!["Scary"],
            "only the genre watched"
        );
    }

    #[tokio::test]
    async fn an_account_that_watched_nothing_is_suggested_what_is_popular() {
        let (_held, state, viewer) = requests_on().await;
        let provider = Arc::new(StandIn {
            films: vec![a_film("1", "Scary"), a_film("2", "Funny")],
            series: vec![crate::requests::testing::a_series("3", "Long", &[])],
        });
        let shelves = suggestions(&state, &provider, &viewer, "en").await.expect("suggested");
        assert_eq!(shelves.len(), 1);
        assert_eq!(shelves[0].genre, None);
        assert_eq!(shelves[0].found.len(), 3, "films and series together");
    }
}
