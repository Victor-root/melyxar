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
    Candidate, Catalogue, Collection, Credit, Details, EpisodeDetails, MetadataProvider,
    ProviderError, Result, SeasonDetails, Trailer,
};

const BASE_URL: &str = "https://api.themoviedb.org/3";

/// Where the provider keeps its pictures.
///
/// Fetched at their full size and shrunk here, so the sizes the interface
/// serves are ours to choose and do not change under us.
const IMAGE_BASE_URL: &str = "https://image.tmdb.org/t/p/original";

/// How long fetching one picture may take.
///
/// Longer than a question, since a backdrop is a few hundred kilobytes, but
/// still bounded: a picture that will not come is a picture for another day.
const IMAGE_TIMEOUT: Duration = Duration::from_secs(30);

/// Refuses a picture beyond any plausible size before reading it all.
const LARGEST_IMAGE: u64 = 16 * 1024 * 1024;

/// How long a single request may take.
///
/// Short on purpose. A film that takes ten seconds to identify is a film that
/// will not be identified today, and the job comes back to it later.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Which picture files the pictures asked for are worth reading.
///
/// The provider offers some title images as drawings rather than pixels, and
/// reading those needs a piece the media tool is not always built with. A
/// title image that shows up on one machine and not another is worse than none
/// at all, so only what any build reads is considered.
const READABLE_PICTURES: [&str; 4] = ["png", "jpg", "jpeg", "webp"];

pub struct TmdbProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    image_base_url: String,
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
            image_base_url: IMAGE_BASE_URL.to_string(),
        })
    }

    /// Points the provider somewhere else, which is what lets a test answer in
    /// place of the real thing.
    pub fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = base_url.into();
        self
    }

    /// Points the pictures somewhere else, for the same reason.
    pub fn with_image_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.image_base_url = base_url.into();
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

/// The word this provider puts in an address for one of its catalogues, and
/// the word it names a release date by.
///
/// The two catalogues answer the same shapes under different words, which is
/// why one reader serves both and only these differ.
const fn road_of(catalogue: Catalogue) -> &'static str {
    match catalogue {
        Catalogue::Films => "movie",
        Catalogue::Series => "tv",
    }
}

/// What comes back alongside a work, so one work costs one request rather than
/// five. The two catalogues name their age ratings differently and agree on
/// everything else.
const fn extras_of(catalogue: Catalogue) -> &'static str {
    match catalogue {
        Catalogue::Films => "credits,release_dates,videos,images",
        Catalogue::Series => "credits,content_ratings,videos,images",
    }
}

impl MetadataProvider for TmdbProvider {
    fn name(&self) -> &'static str {
        "tmdb"
    }

    async fn search(
        &self,
        catalogue: Catalogue,
        title: &str,
        year: Option<i32>,
        language: &str,
    ) -> Result<Vec<Candidate>> {
        let mut query = vec![
            ("query", title.to_string()),
            ("language", language.to_string()),
            // Works nobody asked for have no business turning up in a search
            // made from a file name.
            ("include_adult", "false".to_string()),
        ];
        if let Some(year) = year {
            // The two catalogues narrow by year under different words, and the
            // wrong one is ignored rather than refused: a search that silently
            // stops narrowing is a search that answers the wrong film.
            query.push(match catalogue {
                Catalogue::Films => ("year", year.to_string()),
                Catalogue::Series => ("first_air_date_year", year.to_string()),
            });
        }

        let found: SearchResponse = self
            .get(&format!("/search/{}", road_of(catalogue)), &query)
            .await?;
        Ok(found
            .results
            .into_iter()
            .map(|raw| candidate_from(raw, catalogue))
            .collect())
    }

    async fn details(
        &self,
        catalogue: Catalogue,
        external_id: &str,
        language: &str,
    ) -> Result<Details> {
        let raw: DetailsResponse = self
            .get(
                &format!("/{}/{external_id}", road_of(catalogue)),
                &[
                    ("language", language.to_string()),
                    ("append_to_response", extras_of(catalogue).to_string()),
                    ("include_image_language", image_languages(language)),
                ],
            )
            .await?;
        Ok(details_from(raw, language))
    }

    async fn season(
        &self,
        series_id: &str,
        season_number: i32,
        language: &str,
    ) -> Result<SeasonDetails> {
        let raw: RawSeason = self
            .get(
                &format!("/tv/{series_id}/season/{season_number}"),
                &[("language", language.to_string())],
            )
            .await?;
        Ok(season_from(raw, season_number))
    }

    fn image_url(&self, path: &str) -> String {
        format!("{}{}", self.image_base_url, path)
    }

    async fn fetch_image(&self, path: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(self.image_url(path))
            .timeout(IMAGE_TIMEOUT)
            .send()
            .await
            .map_err(|error| ProviderError::Unreachable(short_reason(&error)))?;

        match response.status().as_u16() {
            200 => {}
            401 | 403 => return Err(ProviderError::Unauthorised),
            429 => {
                return Err(ProviderError::TooManyRequests {
                    retry_after_seconds: None,
                })
            }
            status if (500..600).contains(&status) => {
                return Err(ProviderError::Unreachable(format!(
                    "the provider answered {status}"
                )))
            }
            status => {
                return Err(ProviderError::Unexpected(format!(
                    "the provider answered {status}"
                )))
            }
        }

        // A picture announcing an absurd size is refused before a single byte
        // of it is kept, rather than after the disk has filled up.
        if response
            .content_length()
            .is_some_and(|size| size > LARGEST_IMAGE)
        {
            return Err(ProviderError::Unexpected(
                "the picture is larger than any picture has business being".to_string(),
            ));
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|error| ProviderError::Unreachable(short_reason(&error)))?;
        if bytes.len() as u64 > LARGEST_IMAGE {
            return Err(ProviderError::Unexpected(
                "the picture is larger than any picture has business being".to_string(),
            ));
        }
        Ok(bytes.to_vec())
    }

    async fn by_imdb_id(&self, imdb_id: &str, language: &str) -> Result<Option<Candidate>> {
        let found: FindResponse = self
            .get(
                &format!("/find/{imdb_id}"),
                &[
                    ("external_source", "imdb_id".to_string()),
                    ("language", language.to_string()),
                ],
            )
            .await?;
        // One answer carries both catalogues, and the identifier says which it
        // belongs to by which list it turns up in.
        Ok(found
            .movie_results
            .into_iter()
            .next()
            .map(|raw| candidate_from(raw, Catalogue::Films))
            .or_else(|| {
                found
                    .tv_results
                    .into_iter()
                    .next()
                    .map(|raw| candidate_from(raw, Catalogue::Series))
            }))
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
    #[serde(default)]
    tv_results: Vec<RawMovie>,
}

/// One work as it comes back from either catalogue.
///
/// The words differ and the shapes do not: a film has a title and a release
/// date, a series has a name and a first air date, and every other field is
/// spelled the same. Read under both spellings rather than copied into a
/// second set of structs, because a field added to one copy and forgotten in
/// the other is a series that quietly stops carrying it.
#[derive(Debug, Deserialize)]
struct RawMovie {
    id: i64,
    #[serde(default, alias = "name")]
    title: Option<String>,
    #[serde(default, alias = "original_name")]
    original_title: Option<String>,
    #[serde(default, alias = "first_air_date")]
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
    #[serde(default, alias = "name")]
    title: Option<String>,
    #[serde(default, alias = "original_name")]
    original_title: Option<String>,
    #[serde(default)]
    original_language: Option<String>,
    #[serde(default)]
    tagline: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default, alias = "first_air_date")]
    release_date: Option<String>,
    #[serde(default)]
    runtime: Option<i64>,
    /// A series says how long its episodes run rather than how long it does,
    /// and says it as a list because the answer has changed over the years.
    #[serde(default)]
    episode_run_time: Vec<i64>,
    /// How many seasons the provider counts. Absent for a film.
    #[serde(default)]
    number_of_seasons: Option<i32>,
    #[serde(default)]
    content_ratings: Option<RawContentRatings>,
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
    #[serde(default)]
    images: Option<RawImages>,
}

#[derive(Debug, Deserialize)]
struct Named {
    name: String,
}

/// Only the title images are read here. The posters and backdrops come along
/// in the same answer and are ignored: the film's own two pictures are named
/// on the film itself, where the provider has already made the choice.
#[derive(Debug, Default, Deserialize)]
struct RawImages {
    #[serde(default)]
    logos: Vec<RawImage>,
}

#[derive(Debug, Deserialize)]
struct RawImage {
    #[serde(default)]
    file_path: Option<String>,
    #[serde(default)]
    iso_639_1: Option<String>,
    #[serde(default)]
    vote_average: f64,
    #[serde(default)]
    vote_count: i64,
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

/// What a series answers where a film answers its release dates.
#[derive(Debug, Default, Deserialize)]
struct RawContentRatings {
    #[serde(default)]
    results: Vec<RawContentRating>,
}

#[derive(Debug, Deserialize)]
struct RawContentRating {
    #[serde(default)]
    iso_3166_1: String,
    #[serde(default)]
    rating: String,
}

/// One season, with the episodes under it.
#[derive(Debug, Deserialize)]
struct RawSeason {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    poster_path: Option<String>,
    #[serde(default)]
    episodes: Vec<RawEpisode>,
}

#[derive(Debug, Deserialize)]
struct RawEpisode {
    #[serde(default)]
    id: i64,
    #[serde(default)]
    episode_number: i32,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    overview: Option<String>,
    #[serde(default)]
    still_path: Option<String>,
    #[serde(default)]
    air_date: Option<String>,
    #[serde(default)]
    runtime: Option<i64>,
    #[serde(default)]
    vote_average: Option<f64>,
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

fn candidate_from(raw: RawMovie, catalogue: Catalogue) -> Candidate {
    Candidate {
        external_id: raw.id.to_string(),
        catalogue,
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

fn details_from(raw: DetailsResponse, language: &str) -> Details {
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

    Details {
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
        // A film says how long it runs. A series says how long its episodes
        // run, as a list, because the answer has changed over the years; the
        // first is the one a page means by "an episode of this".
        runtime: raw
            .runtime
            .or_else(|| raw.episode_run_time.first().copied())
            .filter(|minutes| *minutes > 0)
            .map(|minutes| Millis::new(minutes * 60_000)),
        community_rating: raw.vote_average.filter(|value| *value > 0.0),
        age_rating_label: match raw.content_ratings {
            Some(ratings) => rating_for_series(ratings, language),
            None => age_rating(raw.release_dates.unwrap_or_default(), language),
        },
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
        logo_path: best_logo(raw.images.unwrap_or_default(), language),
        trailers,
        season_count: raw.number_of_seasons,
    }
}

/// One season and its episodes, as the rest of the server speaks of them.
fn season_from(raw: RawSeason, season_number: i32) -> SeasonDetails {
    SeasonDetails {
        external_id: raw.id.to_string(),
        season_number,
        name: raw.name.filter(|value| !value.trim().is_empty()),
        overview: raw.overview.filter(|value| !value.trim().is_empty()),
        poster_path: raw.poster_path,
        episodes: raw
            .episodes
            .into_iter()
            .map(|episode| EpisodeDetails {
                external_id: episode.id.to_string(),
                episode_number: episode.episode_number,
                name: episode.name.filter(|value| !value.trim().is_empty()),
                overview: episode.overview.filter(|value| !value.trim().is_empty()),
                still_path: episode.still_path,
                release_year: year_of(episode.air_date.as_deref()),
                runtime: episode
                    .runtime
                    .filter(|minutes| *minutes > 0)
                    .map(|minutes| Millis::new(minutes * 60_000)),
                community_rating: episode.vote_average.filter(|value| *value > 0.0),
            })
            .collect(),
    }
}

/// The year a date string carries.
fn year_of(date: Option<&str>) -> Option<i32> {
    date?.get(..4)?.parse().ok()
}

/// The languages title images are asked for.
///
/// The library's own first, and English behind it: a great many films are
/// drawn under an English title and under no other, so a library that asked
/// for its language alone would show a plain line of text for most of its
/// shelf. Pictures carrying no language at all are left out on purpose: a
/// title image is a title, so it has one, and asking for them drags in every
/// wordless backdrop the film has, measured at twice the answer for nothing.
fn image_languages(language: &str) -> String {
    let wanted = short_language(language);
    match wanted == "en" {
        true => wanted.to_string(),
        false => format!("{wanted},en"),
    }
}

/// A language as the provider writes it on a picture: two letters, no country.
fn short_language(language: &str) -> String {
    language
        .split(['-', '_'])
        .next()
        .unwrap_or(language)
        .to_lowercase()
}

/// The title image a film is shown under, among those the provider offers.
///
/// The language decides first: a shelf kept in one language wants the title
/// drawn in that language, however well thought of another is. Among those
/// left, the provider's own voters decide, which is the same thing that
/// settles two films sharing a name.
fn best_logo(images: RawImages, language: &str) -> Option<String> {
    let wanted = short_language(language);
    let mut offered: Vec<(u8, f64, i64, String)> = images
        .logos
        .into_iter()
        .filter_map(|image| {
            let path = image.file_path?;
            if !is_readable_picture(&path) {
                return None;
            }
            let tongue = image.iso_639_1.unwrap_or_default().to_lowercase();
            let rank = match tongue.as_str() {
                spoken if spoken == wanted => 0,
                "en" => 1,
                _ => return None,
            };
            Some((rank, image.vote_average, image.vote_count, path))
        })
        .collect();

    offered.sort_by(|left, right| {
        left.0
            .cmp(&right.0)
            .then(right.1.total_cmp(&left.1))
            .then(right.2.cmp(&left.2))
    });
    offered.into_iter().next().map(|(_, _, _, path)| path)
}

/// Whether a picture is in a form the media tool reads whatever it was built
/// with.
fn is_readable_picture(path: &str) -> bool {
    let extension = match path.rsplit_once('.') {
        Some((_, extension)) => extension.to_lowercase(),
        None => return false,
    };
    READABLE_PICTURES.contains(&extension.as_str())
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

/// The same reading, of what a series answers instead.
///
/// The same rule either way: the country that speaks the language being asked
/// for, and failing that the one whose ratings everybody recognises.
fn rating_for_series(ratings: RawContentRatings, language: &str) -> Option<String> {
    let pick = |country: &str| -> Option<String> {
        ratings
            .results
            .iter()
            .filter(|entry| entry.iso_3166_1.eq_ignore_ascii_case(country))
            .map(|entry| entry.rating.trim())
            .find(|rating| !rating.is_empty())
            .map(str::to_string)
    };
    pick(country_for_language(language)).or_else(|| pick("US"))
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
        },
        "images": {
            "posters": [{"file_path": "/another-poster.jpg", "iso_639_1": "fr"}],
            "backdrops": [],
            "logos": [
                {"file_path": "/drawn.svg", "iso_639_1": "fr",
                 "vote_average": 9.0, "vote_count": 30},
                {"file_path": "/english-favourite.png", "iso_639_1": "en",
                 "vote_average": 8.0, "vote_count": 20},
                {"file_path": "/french.png", "iso_639_1": "fr",
                 "vote_average": 1.2, "vote_count": 2},
                {"file_path": "/french-unloved.png", "iso_639_1": "fr",
                 "vote_average": 1.2, "vote_count": 1},
                {"file_path": "/spanish.png", "iso_639_1": "es",
                 "vote_average": 9.5, "vote_count": 50}
            ]
        }
    }"#;

    fn details(language: &str) -> Details {
        let raw: DetailsResponse = serde_json::from_str(DETAILS).expect("the answer parses");
        details_from(raw, language)
    }

    #[test]
    fn a_search_answer_becomes_candidates_in_the_order_it_arrived() {
        let found: SearchResponse = serde_json::from_str(SEARCH).expect("the answer parses");
        let candidates: Vec<Candidate> = found
            .results
            .into_iter()
            .map(|raw| candidate_from(raw, Catalogue::Films))
            .collect();

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
    fn a_picture_is_asked_for_at_its_full_size_so_the_sizes_served_stay_ours() {
        let provider = TmdbProvider::new("key").expect("a client");
        assert_eq!(
            provider.image_url("/poster.jpg"),
            "https://image.tmdb.org/t/p/original/poster.jpg"
        );
    }

    #[test]
    fn a_year_is_read_off_a_date_and_never_guessed() {
        assert_eq!(year_of(Some("2019-05-17")), Some(2019));
        assert_eq!(year_of(Some("")), None);
        assert_eq!(year_of(None), None);
        assert_eq!(year_of(Some("bientot")), None);
    }

    #[test]
    fn the_title_image_is_the_one_drawn_in_the_language_of_the_shelf() {
        assert_eq!(
            details("fr").logo_path.as_deref(),
            Some("/french.png"),
            "a shelf kept in one language wants its own title, \
             however well thought of another is"
        );
        assert_eq!(
            details("en").logo_path.as_deref(),
            Some("/english-favourite.png")
        );
    }

    #[test]
    fn a_title_image_the_media_tool_might_not_read_is_not_chosen() {
        assert_ne!(
            details("fr").logo_path.as_deref(),
            Some("/drawn.svg"),
            "reading a drawing needs a piece the media tool is not always \
             built with, so it would show on one machine and not another"
        );
    }

    #[test]
    fn a_title_image_in_a_language_nobody_here_reads_is_left_alone() {
        let raw: DetailsResponse = serde_json::from_str(
            r#"{"id": 1, "images": {"logos": [
                {"file_path": "/spanish.png", "iso_639_1": "es",
                 "vote_average": 9.5, "vote_count": 50}]}}"#,
        )
        .expect("parses");
        assert_eq!(
            details_from(raw, "fr").logo_path,
            None,
            "a title nobody can read is worse than the title written out"
        );
    }

    #[test]
    fn among_titles_of_the_same_language_the_provider_settles_it() {
        let raw: DetailsResponse = serde_json::from_str(
            r#"{"id": 1, "images": {"logos": [
                {"file_path": "/seen-once.png", "iso_639_1": "fr",
                 "vote_average": 1.2, "vote_count": 1},
                {"file_path": "/seen-often.png", "iso_639_1": "fr",
                 "vote_average": 1.2, "vote_count": 9}]}}"#,
        )
        .expect("parses");
        assert_eq!(
            details_from(raw, "fr").logo_path.as_deref(),
            Some("/seen-often.png"),
            "two titles rated alike are told apart by how many said so"
        );
    }

    #[test]
    fn a_film_drawn_under_no_title_at_all_simply_has_none() {
        let raw: DetailsResponse = serde_json::from_str(r#"{"id": 1}"#).expect("parses");
        assert_eq!(details_from(raw, "fr").logo_path, None);
    }

    #[test]
    fn title_images_are_asked_for_in_the_language_of_the_shelf_and_in_english() {
        assert_eq!(image_languages("fr"), "fr,en");
        assert_eq!(image_languages("fr-FR"), "fr,en");
        assert_eq!(
            image_languages("en"),
            "en",
            "asking for English twice asks for nothing more"
        );
        assert!(
            !image_languages("fr").contains("null"),
            "pictures carrying no language drag in every wordless backdrop \
             the film has, for a title image that has a language by nature"
        );
    }

    #[test]
    fn only_pictures_any_build_of_the_media_tool_reads_are_kept() {
        assert!(is_readable_picture("/a.png"));
        assert!(is_readable_picture("/a.PNG"));
        assert!(is_readable_picture("/a.jpg"));
        assert!(!is_readable_picture("/a.svg"));
        assert!(!is_readable_picture("/no-extension-at-all"));
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
            .search(Catalogue::Films, "Quiet Harbour", Some(2019), "fr")
            .await;

        match outcome {
            Err(ProviderError::Unauthorised) => {}
            other => panic!("a refused key must be recognised as such, got {other:?}"),
        }
    }

    /// The one thing no invented answer can check: that the provider still
    /// sends title images in the shape read here, and that a well known film
    /// really comes back with one.
    #[tokio::test]
    #[ignore = "needs the network"]
    async fn a_famous_film_really_comes_back_with_a_title_image() {
        let Some(key) = crate::defaults::provider_key() else {
            eprintln!("no key carried here, the provider was not asked");
            return;
        };
        let provider = TmdbProvider::new(key).expect("a client");

        // A film picked from a public catalogue for being well illustrated,
        // and drawn under a title in both languages asked for here.
        let film = provider
            .details(Catalogue::Films, "603", "fr")
            .await
            .expect("the provider answered");

        let path = film.logo_path.expect("a title image came back");
        assert!(
            is_readable_picture(&path),
            "a title image must be in a form any build of the media tool \
             reads, got {path}"
        );
    }

    #[tokio::test]
    #[ignore = "needs the network"]
    async fn a_provider_that_is_not_there_is_a_delay_rather_than_a_failure() {
        let provider = TmdbProvider::new("whatever")
            .expect("a client")
            .with_base_url("https://melyxar.invalid/3");
        let error = provider
            .search(Catalogue::Films, "Quiet Harbour", None, "fr")
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
