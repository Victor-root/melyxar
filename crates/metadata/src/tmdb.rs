//! The film database provider.
//!
//! One client is built and kept, so the connection to the provider stays open
//! instead of being made again for every film. Every request carries a
//! deadline: a provider that stops answering must slow a scan down, never stop
//! it.

use std::time::Duration;

use melyxar_core::time::Millis;
use serde::Deserialize;

use crate::provider::{
    Collection, Credit, MetadataProvider, MovieCandidate, MovieDetails, ProviderError, Result,
    Trailer,
};

const BASE_URL: &str = "https://api.themoviedb.org/3";

/// How long a single request may take.
///
/// Short on purpose. A film that takes ten seconds to identify is a film that
/// will not be identified today, and the job comes back to it later.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Everything asked for in one go, so one film costs one request rather than
/// four.
const DETAIL_EXTRAS: &str = "credits,release_dates,videos";

pub struct TmdbProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
}

impl TmdbProvider {
    /// Builds the provider. Fails only if no client can be built at all, which
    /// means the machine has no usable network stack.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("Melyxar/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;

        Ok(Self {
            client,
            api_key: api_key.into(),
            base_url: BASE_URL.to_string(),
        })
    }

    /// Points the provider somewhere else, which is what lets a test answer in
    /// place of the real thing.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    async fn get<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        query: &[(&str, String)],
    ) -> Result<T> {
        let url = format!("{}{path}", self.base_url);
        let mut request = self.client.get(&url).query(&[("api_key", &self.api_key)]);
        for (name, value) in query {
            request = request.query(&[(name, value)]);
        }

        let response = request.send().await.map_err(|error| {
            // The address never reaches the log: it carries the key.
            ProviderError::Unreachable(short_reason(&error))
        })?;

        match response.status().as_u16() {
            200 => response
                .json::<T>()
                .await
                .map_err(|error| ProviderError::Unexpected(short_reason(&error))),
            401 | 403 => Err(ProviderError::Unauthorised),
            429 => Err(ProviderError::TooManyRequests {
                retry_after_seconds: response
                    .headers()
                    .get("retry-after")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.parse().ok()),
            }),
            404 => Err(ProviderError::Unexpected("not found".to_string())),
            status if (500..600).contains(&status) => Err(ProviderError::Unreachable(format!(
                "the provider answered {status}"
            ))),
            status => Err(ProviderError::Unexpected(format!(
                "the provider answered {status}"
            ))),
        }
    }
}

impl MetadataProvider for TmdbProvider {
    fn name(&self) -> &'static str {
        "tmdb"
    }

    async fn search_movie(
        &self,
        title: &str,
        year: Option<i32>,
        language: &str,
    ) -> Result<Vec<MovieCandidate>> {
        let mut query = vec![
            ("query", title.to_string()),
            ("language", language.to_string()),
            // Films nobody asked for have no business turning up in a search
            // made from a file name.
            ("include_adult", "false".to_string()),
        ];
        if let Some(year) = year {
            query.push(("year", year.to_string()));
        }

        let found: SearchResponse = self.get("/search/movie", &query).await?;
        Ok(found.results.into_iter().map(candidate_from).collect())
    }

    async fn movie_details(&self, external_id: &str, language: &str) -> Result<MovieDetails> {
        let raw: DetailsResponse = self
            .get(
                &format!("/movie/{external_id}"),
                &[
                    ("language", language.to_string()),
                    ("append_to_response", DETAIL_EXTRAS.to_string()),
                ],
            )
            .await?;
        Ok(details_from(raw, language))
    }

    async fn movie_by_imdb_id(
        &self,
        imdb_id: &str,
        language: &str,
    ) -> Result<Option<MovieCandidate>> {
        let found: FindResponse = self
            .get(
                &format!("/find/{imdb_id}"),
                &[
                    ("external_source", "imdb_id".to_string()),
                    ("language", language.to_string()),
                ],
            )
            .await?;
        Ok(found.movie_results.into_iter().next().map(candidate_from))
    }
}

/// A failure reduced to something worth logging.
///
/// The whole message of a request failure carries the address, and the address
/// carries the key.
fn short_reason(error: &impl std::error::Error) -> String {
    let message = error.to_string();
    match message.find("http") {
        Some(position) => message[..position].trim_end_matches(": ").to_string(),
        None => message,
    }
}

// ---------------------------------------------------------------------------
// What the provider actually sends
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<RawMovie>,
}

#[derive(Debug, Deserialize)]
struct FindResponse {
    #[serde(default)]
    movie_results: Vec<RawMovie>,
}

#[derive(Debug, Deserialize)]
struct RawMovie {
    id: i64,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    original_title: Option<String>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    poster_path: Option<String>,
    #[serde(default)]
    popularity: f64,
}

#[derive(Debug, Deserialize)]
struct DetailsResponse {
    id: i64,
    #[serde(default)]
    imdb_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    original_title: Option<String>,
    #[serde(default)]
    original_language: Option<String>,
    #[serde(default)]
    tagline: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    release_date: Option<String>,
    #[serde(default)]
    runtime: Option<i64>,
    #[serde(default)]
    vote_average: Option<f64>,
    #[serde(default)]
    poster_path: Option<String>,
    #[serde(default)]
    backdrop_path: Option<String>,
    #[serde(default)]
    genres: Vec<Named>,
    #[serde(default)]
    production_companies: Vec<Named>,
    #[serde(default)]
    belongs_to_collection: Option<RawCollection>,
    #[serde(default)]
    credits: Option<RawCredits>,
    #[serde(default)]
    release_dates: Option<RawReleaseDates>,
    #[serde(default)]
    videos: Option<RawVideos>,
}

#[derive(Debug, Deserialize)]
struct Named {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RawCollection {
    id: i64,
    name: String,
    #[serde(default)]
    poster_path: Option<String>,
    #[serde(default)]
    backdrop_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawCredits {
    #[serde(default)]
    cast: Vec<RawCast>,
    #[serde(default)]
    crew: Vec<RawCrew>,
}

#[derive(Debug, Deserialize)]
struct RawCast {
    id: i64,
    name: String,
    #[serde(default)]
    character: Option<String>,
    #[serde(default)]
    order: i32,
    #[serde(default)]
    profile_path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RawCrew {
    id: i64,
    name: String,
    #[serde(default)]
    job: Option<String>,
    #[serde(default)]
    profile_path: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawReleaseDates {
    #[serde(default)]
    results: Vec<RawCountryReleases>,
}

#[derive(Debug, Deserialize)]
struct RawCountryReleases {
    #[serde(default)]
    iso_3166_1: String,
    #[serde(default)]
    release_dates: Vec<RawRelease>,
}

#[derive(Debug, Deserialize)]
struct RawRelease {
    #[serde(default)]
    certification: String,
}

#[derive(Debug, Default, Deserialize)]
struct RawVideos {
    #[serde(default)]
    results: Vec<RawVideo>,
}

#[derive(Debug, Deserialize)]
struct RawVideo {
    #[serde(default)]
    name: String,
    #[serde(default)]
    site: String,
    #[serde(default)]
    key: String,
    #[serde(default, rename = "type")]
    kind: String,
    #[serde(default)]
    official: bool,
    #[serde(default)]
    iso_639_1: Option<String>,
}

// ---------------------------------------------------------------------------
// Turning it into what the rest of the server speaks
// ---------------------------------------------------------------------------

fn candidate_from(raw: RawMovie) -> MovieCandidate {
    MovieCandidate {
        external_id: raw.id.to_string(),
        title: raw
            .title
            .or_else(|| raw.original_title.clone())
            .unwrap_or_default(),
        original_title: raw.original_title,
        release_year: year_of(raw.release_date.as_deref()),
        overview: raw.overview.filter(|value| !value.trim().is_empty()),
        poster_path: raw.poster_path,
        popularity: raw.popularity,
    }
}

fn details_from(raw: DetailsResponse, language: &str) -> MovieDetails {
    let credits = raw.credits.unwrap_or_default();
    let mut people: Vec<Credit> = credits
        .cast
        .into_iter()
        .map(|person| Credit {
            external_id: person.id.to_string(),
            name: person.name,
            role: "actor".to_string(),
            character: person.character.filter(|value| !value.trim().is_empty()),
            ordinal: person.order,
            photo_path: person.profile_path,
        })
        .collect();

    // Only the parts a page actually shows are kept: a full crew runs to
    // hundreds of people and none of them are ever displayed.
    people.extend(
        credits
            .crew
            .into_iter()
            .filter_map(|person| {
                let role = match person.job.as_deref()? {
                    "Director" => "director",
                    "Screenplay" | "Writer" | "Story" => "writer",
                    "Producer" => "producer",
                    "Original Music Composer" => "composer",
                    _ => return None,
                };
                Some(Credit {
                    external_id: person.id.to_string(),
                    name: person.name,
                    role: role.to_string(),
                    character: None,
                    ordinal: 0,
                    photo_path: person.profile_path,
                })
            })
            .collect::<Vec<_>>(),
    );

    let trailers = raw
        .videos
        .unwrap_or_default()
        .results
        .into_iter()
        .filter(|video| video.kind == "Trailer" || video.kind == "Teaser")
        .map(|video| Trailer {
            name: video.name,
            site: video.site,
            key: video.key,
            is_official: video.official,
            language: video
                .iso_639_1
                .map(|tag| melyxar_core::media::normalise_language(&tag)),
        })
        .collect();

    MovieDetails {
        external_id: raw.id.to_string(),
        imdb_id: raw.imdb_id.filter(|value| value.starts_with("tt")),
        title: raw
            .title
            .clone()
            .or_else(|| raw.original_title.clone())
            .unwrap_or_default(),
        original_title: raw.original_title,
        original_language: raw
            .original_language
            .map(|tag| melyxar_core::media::normalise_language(&tag)),
        tagline: raw.tagline.filter(|value| !value.trim().is_empty()),
        overview: raw.overview.filter(|value| !value.trim().is_empty()),
        release_year: year_of(raw.release_date.as_deref()),
        runtime: raw
            .runtime
            .filter(|minutes| *minutes > 0)
            .map(|minutes| Millis::new(minutes * 60_000)),
        community_rating: raw.vote_average.filter(|value| *value > 0.0),
        age_rating_label: age_rating(raw.release_dates.unwrap_or_default(), language),
        genres: raw.genres.into_iter().map(|value| value.name).collect(),
        studios: raw
            .production_companies
            .into_iter()
            .map(|value| value.name)
            .collect(),
        credits: people,
        collection: raw.belongs_to_collection.map(|value| Collection {
            external_id: value.id.to_string(),
            name: value.name,
            poster_path: value.poster_path,
            backdrop_path: value.backdrop_path,
        }),
        poster_path: raw.poster_path,
        backdrop_path: raw.backdrop_path,
        trailers,
    }
}

/// The year a date string carries.
fn year_of(date: Option<&str>) -> Option<i32> {
    date?.get(..4)?.parse().ok()
}

/// The age rating, from the country that goes with the language asked for.
///
/// A rating means nothing without the country that issued it, so one country
/// is chosen rather than the first that happens to be listed. The country of
/// the language comes first, then the one most films carry.
fn age_rating(releases: RawReleaseDates, language: &str) -> Option<String> {
    let preferred = country_for_language(language);
    let pick = |country: &str| -> Option<String> {
        releases
            .results
            .iter()
            .find(|entry| entry.iso_3166_1.eq_ignore_ascii_case(country))?
            .release_dates
            .iter()
            .map(|release| release.certification.trim())
            .find(|certification| !certification.is_empty())
            .map(str::to_string)
    };
    pick(preferred).or_else(|| pick("US"))
}

fn country_for_language(language: &str) -> &'static str {
    match language.split(['-', '_']).next().unwrap_or(language) {
        "fr" | "fre" | "fra" => "FR",
        "de" | "ger" | "deu" => "DE",
        "es" | "spa" => "ES",
        "it" | "ita" => "IT",
        _ => "US",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shaped like a real answer, with invented content.
    const SEARCH: &str = r#"{
        "page": 1,
        "results": [
            {"id": 111, "title": "Quiet Harbour", "original_title": "Quiet Harbour",
             "release_date": "2019-05-17", "overview": "Un port, une nuit.",
             "poster_path": "/poster.jpg", "popularity": 12.5},
            {"id": 222, "title": "Quiet Harbour Revisited", "release_date": "2021-01-01",
             "overview": "", "popularity": 3.0}
        ]
    }"#;

    const DETAILS: &str = r#"{
        "id": 111,
        "imdb_id": "tt7654321",
        "title": "Quiet Harbour",
        "original_title": "Quiet Harbour",
        "original_language": "en",
        "tagline": "La mer ne rend rien.",
        "overview": "Un port, une nuit.",
        "release_date": "2019-05-17",
        "runtime": 118,
        "vote_average": 7.4,
        "poster_path": "/poster.jpg",
        "backdrop_path": "/backdrop.jpg",
        "genres": [{"id": 18, "name": "Drame"}, {"id": 53, "name": "Thriller"}],
        "production_companies": [{"id": 9, "name": "Invented Pictures"}],
        "belongs_to_collection": {"id": 77, "name": "Harbour Trilogy",
                                  "poster_path": "/c.jpg", "backdrop_path": null},
        "credits": {
            "cast": [
                {"id": 1, "name": "Alix Moreau", "character": "Camille", "order": 0,
                 "profile_path": "/a.jpg"},
                {"id": 2, "name": "Bruno Keller", "character": "Le gardien", "order": 1}
            ],
            "crew": [
                {"id": 3, "name": "Sacha Nord", "job": "Director"},
                {"id": 4, "name": "Rene Falk", "job": "Screenplay"},
                {"id": 5, "name": "Someone Else", "job": "Best Boy"}
            ]
        },
        "release_dates": {
            "results": [
                {"iso_3166_1": "US", "release_dates": [{"certification": "R", "type": 3}]},
                {"iso_3166_1": "FR", "release_dates": [{"certification": "12", "type": 3}]}
            ]
        },
        "videos": {
            "results": [
                {"name": "Bande annonce", "site": "YouTube", "key": "abc", "type": "Trailer",
                 "official": true, "iso_639_1": "fr"},
                {"name": "Interview", "site": "YouTube", "key": "def", "type": "Featurette"}
            ]
        }
    }"#;

    fn details(language: &str) -> MovieDetails {
        let raw: DetailsResponse = serde_json::from_str(DETAILS).expect("the answer parses");
        details_from(raw, language)
    }

    #[test]
    fn a_search_answer_becomes_candidates_in_the_order_it_arrived() {
        let found: SearchResponse = serde_json::from_str(SEARCH).expect("the answer parses");
        let candidates: Vec<MovieCandidate> =
            found.results.into_iter().map(candidate_from).collect();

        assert_eq!(candidates.len(), 2);
        assert_eq!(candidates[0].external_id, "111");
        assert_eq!(candidates[0].title, "Quiet Harbour");
        assert_eq!(candidates[0].release_year, Some(2019));
        assert_eq!(candidates[1].overview, None, "an empty text is no text");
    }

    #[test]
    fn a_film_gives_up_what_a_page_shows() {
        let film = details("fr");
        assert_eq!(film.external_id, "111");
        assert_eq!(film.imdb_id.as_deref(), Some("tt7654321"));
        assert_eq!(film.release_year, Some(2019));
        assert_eq!(film.runtime, Some(Millis::new(118 * 60_000)));
        assert_eq!(film.community_rating, Some(7.4));
        assert_eq!(film.genres, vec!["Drame", "Thriller"]);
        assert_eq!(film.studios, vec!["Invented Pictures"]);
        assert_eq!(
            film.collection.expect("a collection").name,
            "Harbour Trilogy"
        );
        assert_eq!(film.original_language.as_deref(), Some("eng"));
    }

    #[test]
    fn only_the_parts_a_page_shows_are_kept() {
        let film = details("fr");
        let roles: Vec<&str> = film
            .credits
            .iter()
            .map(|credit| credit.role.as_str())
            .collect();

        assert_eq!(roles, vec!["actor", "actor", "director", "writer"]);
        assert!(
            !film
                .credits
                .iter()
                .any(|credit| credit.name == "Someone Else"),
            "a full crew runs to hundreds of people and none of them are shown"
        );
        assert_eq!(film.credits[0].character.as_deref(), Some("Camille"));
        assert_eq!(film.credits[0].ordinal, 0);
    }

    #[test]
    fn the_age_rating_comes_from_the_country_that_goes_with_the_language() {
        assert_eq!(details("fr").age_rating_label.as_deref(), Some("12"));
        assert_eq!(
            details("en").age_rating_label.as_deref(),
            Some("R"),
            "a rating means nothing without the country that issued it"
        );
    }

    #[test]
    fn a_missing_rating_for_the_country_falls_back_rather_than_inventing_one() {
        let raw: DetailsResponse = serde_json::from_str(
            r#"{"id": 1, "release_dates": {"results":
                [{"iso_3166_1": "US", "release_dates": [{"certification": "PG-13"}]}]}}"#,
        )
        .expect("parses");
        assert_eq!(
            details_from(raw, "fr").age_rating_label.as_deref(),
            Some("PG-13")
        );
    }

    #[test]
    fn an_answer_holding_almost_nothing_is_still_read() {
        let raw: DetailsResponse = serde_json::from_str(r#"{"id": 42}"#).expect("parses");
        let film = details_from(raw, "fr");

        assert_eq!(film.external_id, "42");
        assert_eq!(film.title, "");
        assert_eq!(film.runtime, None);
        assert_eq!(film.community_rating, None);
        assert!(film.credits.is_empty());
        assert!(film.trailers.is_empty());
    }

    #[test]
    fn only_trailers_are_taken_for_trailers() {
        let film = details("fr");
        assert_eq!(film.trailers.len(), 1);
        assert_eq!(film.trailers[0].name, "Bande annonce");
        assert!(film.trailers[0].is_official);
        assert_eq!(film.trailers[0].language.as_deref(), Some("fre"));
        assert_eq!(
            film.trailers[0].watch_url().as_deref(),
            Some("https://www.youtube.com/watch?v=abc")
        );
    }

    #[test]
    fn a_runtime_of_zero_is_not_a_runtime() {
        let raw: DetailsResponse =
            serde_json::from_str(r#"{"id": 1, "runtime": 0, "vote_average": 0.0}"#)
                .expect("parses");
        let film = details_from(raw, "fr");
        assert_eq!(film.runtime, None);
        assert_eq!(film.community_rating, None);
    }

    #[test]
    fn a_year_is_read_off_a_date_and_never_guessed() {
        assert_eq!(year_of(Some("2019-05-17")), Some(2019));
        assert_eq!(year_of(Some("")), None);
        assert_eq!(year_of(None), None);
        assert_eq!(year_of(Some("bientot")), None);
    }
}

/// Checks made against the provider itself.
///
/// Left out of the ordinary run: they need the network, and a server that is
/// simply offline must not look like a server that is broken. Run them on
/// purpose with `cargo test -p melyxar-metadata -- --ignored`.
#[cfg(test)]
mod live {
    use super::*;

    #[tokio::test]
    #[ignore = "needs the network"]
    async fn a_key_the_provider_refuses_is_told_apart_from_a_provider_that_is_down() {
        let provider = TmdbProvider::new("definitely-not-a-key").expect("a client");
        let outcome = provider
            .search_movie("Quiet Harbour", Some(2019), "fr")
            .await;

        match outcome {
            Err(ProviderError::Unauthorised) => {}
            other => panic!("a refused key must be recognised as such, got {other:?}"),
        }
    }

    #[tokio::test]
    #[ignore = "needs the network"]
    async fn a_provider_that_is_not_there_is_a_delay_rather_than_a_failure() {
        let provider = TmdbProvider::new("whatever")
            .expect("a client")
            .with_base_url("https://melyxar.invalid/3");
        let error = provider
            .search_movie("Quiet Harbour", None, "fr")
            .await
            .expect_err("nothing answers there");

        assert!(
            error.is_worth_retrying(),
            "a provider that is down is tried again later, not given up on"
        );
        assert!(
            !error.to_string().contains("whatever"),
            "a failure must never carry the key into a log"
        );
    }
}
