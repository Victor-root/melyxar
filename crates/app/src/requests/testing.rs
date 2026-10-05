//! What the tests of requests are set up with: a server with requests on and
//! an account allowed to ask, and a provider that knows a few titles.

use melyxar_core::user::{Permissions, User};
use melyxar_core::work::SeasonLength;
use melyxar_metadata::provider::Result;
use melyxar_metadata::{
    Candidate, Catalogue, Details, Genre, MetadataProvider, OfferedPicture, PersonDetails, ProviderError,
    SeasonDetails,
};

use crate::AppState;

/// A server with requests on, and a viewer given the right to ask.
pub(super) async fn requests_on() -> (tempfile::TempDir, AppState, User) {
    let (directory, state) = crate::an_empty_server().await;
    let database = state.database();
    database.set_requests_enabled(true).await.expect("switched on");
    let viewer = database
        .create_user("viewer", None, &Permissions::viewer())
        .await
        .expect("account");
    database.set_request_right(viewer.id, true).await.expect("allowed");
    (directory, state, viewer)
}

fn described(id: &str, title: &str, seasons: &[(i32, i32)]) -> Details {
    Details {
        external_id: id.to_string(),
        imdb_id: None,
        title: title.to_string(),
        original_title: None,
        original_language: None,
        tagline: None,
        overview: Some(format!("All about {title}.")),
        release_year: Some(2020),
        release_date: None,
        end_date: None,
        runtime: None,
        community_rating: None,
        age_rating_label: None,
        genres: Vec::new(),
        studios: Vec::new(),
        credits: Vec::new(),
        collection: None,
        poster_path: Some(format!("/{id}.jpg")),
        backdrop_path: None,
        logo_path: None,
        thumb_path: None,
        trailers: Vec::new(),
        season_lengths: seasons
            .iter()
            .map(|&(season, episodes)| SeasonLength { season, episodes })
            .collect(),
    }
}

pub(super) fn a_film(id: &str, title: &str) -> Details {
    described(id, title, &[])
}

pub(super) fn a_series(id: &str, title: &str, seasons: &[(i32, i32)]) -> Details {
    described(id, title, seasons)
}

/// A provider that knows these films and series, and answers every one of
/// them to any search.
pub(super) struct StandIn {
    pub films: Vec<Details>,
    pub series: Vec<Details>,
}

impl StandIn {
    fn of(&self, catalogue: Catalogue) -> &[Details] {
        match catalogue {
            Catalogue::Films => &self.films,
            Catalogue::Series => &self.series,
        }
    }
}

impl MetadataProvider for StandIn {
    fn name(&self) -> &'static str {
        "tmdb"
    }

    async fn search(
        &self,
        catalogue: Catalogue,
        _title: &str,
        _year: Option<i32>,
        _language: &str,
    ) -> Result<Vec<Candidate>> {
        Ok(self
            .of(catalogue)
            .iter()
            .map(|details| details.as_candidate(catalogue))
            .collect())
    }

    async fn details(&self, catalogue: Catalogue, external_id: &str, _language: &str) -> Result<Details> {
        self.of(catalogue)
            .iter()
            .find(|details| details.external_id == external_id)
            .cloned()
            .ok_or_else(|| ProviderError::Unexpected("not known".into()))
    }

    async fn person(&self, _external_id: &str, _language: &str) -> Result<PersonDetails> {
        Ok(PersonDetails::default())
    }

    async fn season(&self, _series_id: &str, _season_number: i32, _language: &str) -> Result<SeasonDetails> {
        Err(ProviderError::Unexpected("not asked".into()))
    }

    fn image_url(&self, path: &str) -> String {
        format!("https://pictures.invalid{path}")
    }

    async fn fetch_image(&self, _path: &str) -> Result<Vec<u8>> {
        Err(ProviderError::Unexpected("not asked".into()))
    }

    async fn pictures(&self, _catalogue: Catalogue, _external_id: &str, _language: &str) -> Result<Vec<OfferedPicture>> {
        Ok(Vec::new())
    }

    async fn imdb_id(&self, _catalogue: Catalogue, _external_id: &str) -> Result<Option<String>> {
        Ok(None)
    }

    async fn by_imdb_id(&self, _imdb_id: &str, _language: &str) -> Result<Option<Candidate>> {
        Ok(None)
    }

    /// The genres its titles carry, each known by its own name.
    async fn genres(&self, catalogue: Catalogue, _language: &str) -> Result<Vec<Genre>> {
        let mut names: Vec<&String> = self.of(catalogue).iter().flat_map(|details| &details.genres).collect();
        names.sort();
        names.dedup();
        Ok(names
            .into_iter()
            .map(|name| Genre { id: name.clone(), name: name.clone() })
            .collect())
    }

    async fn popular(
        &self,
        catalogue: Catalogue,
        genre_id: Option<&str>,
        _released_by: &str,
        page: u32,
        _language: &str,
    ) -> Result<Vec<Candidate>> {
        if page > 1 {
            return Ok(Vec::new());
        }
        Ok(self
            .of(catalogue)
            .iter()
            .filter(|details| genre_id.is_none_or(|genre| details.genres.iter().any(|one| one == genre)))
            .map(|details| details.as_candidate(catalogue))
            .collect())
    }
}
