//! The page of one person: their life in a few lines, and what of theirs this
//! server holds.
//!
//! The life is asked of the provider the first time the page is opened and
//! kept from then on. Asking ahead for everybody a library credits would be
//! thousands of requests for pages nobody opens; asking on the way in costs
//! one wait, once, to whoever opens the page first.

use melyxar_core::id::PersonId;
use melyxar_core::user::User;
use melyxar_database::browse::WorkCard;
use melyxar_database::images::StoredImage;
use melyxar_database::people::{Person, PersonDescription};
use melyxar_metadata::MetadataProvider;

use crate::{AppState, Result};

/// What the language of a person's page falls back to when nothing of theirs
/// says which, the one the provider has the most of.
const WHEN_NOTHING_SAYS: &str = "en";

/// Everything the page of one person shows.
#[derive(Debug, Clone, PartialEq)]
pub struct PersonPage {
    pub person: Person,
    /// Every size of their face, largest first.
    pub photo: Vec<StoredImage>,
    /// What of theirs this account can open, newest first.
    pub works: Vec<WorkCard>,
}

/// Reads the page of one person, asking the provider about them first if it
/// never has been.
///
/// Nothing at all for somebody this account meets in no work it may open:
/// told apart from somebody who is not there, one identifier after another
/// would say what the libraries it was not given hold.
pub async fn person_page(state: &AppState, who: &User, id: PersonId) -> Result<Option<PersonPage>> {
    let provider = state.metadata_provider();
    person_page_asking(state, who, id, provider.as_deref()).await
}

async fn person_page_asking(
    state: &AppState,
    who: &User,
    id: PersonId,
    provider: Option<&impl MetadataProvider>,
) -> Result<Option<PersonPage>> {
    let database = state.database();
    let Some(mut person) = database.person(id).await? else {
        return Ok(None);
    };
    let within = crate::reach::within(who);
    let works = database
        .works_crediting(who.id, id, within.as_deref())
        .await?;
    let Some(first) = works.first() else {
        return Ok(None);
    };

    if let (None, Some(external_id), Some(provider)) =
        (person.described_at, person.tmdb_id.as_deref(), provider)
    {
        // The language of the library their newest work is in, which is the
        // language that library's pages are read in.
        let language = database
            .list_libraries()
            .await?
            .into_iter()
            .find(|library| library.id == first.library_id)
            .map(|library| library.metadata_language)
            .unwrap_or_else(|| WHEN_NOTHING_SAYS.to_string());

        match provider.person(external_id, &language).await {
            Ok(said) => {
                let description = PersonDescription {
                    biography: said.biography,
                    born_on: said.born_on,
                    died_on: said.died_on,
                    birthplace: said.birthplace,
                };
                database.describe_person(id, &description).await?;
                if let Some(described) = database.person(id).await? {
                    person = described;
                }
            }
            // The page shows what is known, and the next visit asks again.
            Err(error) => tracing::warn!(
                person = %person.name,
                error = %error,
                "the provider could not say who this is"
            ),
        }
    }

    let photo = database
        .images_of("person", &id.to_db_string())
        .await?
        .into_iter()
        .filter(|image| image.image_kind == "photo")
        .collect();

    Ok(Some(PersonPage {
        person,
        photo,
        works,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::work::WorkKind;
    use melyxar_database::metadata::{CreditRecord, IdentifiedWork};
    use melyxar_metadata::provider::{
        Candidate, Catalogue, Details, OfferedPicture, PersonDetails, ProviderError,
        Result as Answer, SeasonDetails,
    };
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// A provider that knows one life and counts how often it is asked.
    #[derive(Default)]
    struct Biographer {
        asked: Mutex<Vec<(String, String)>>,
        down: bool,
    }

    fn nothing_else() -> ProviderError {
        ProviderError::Unexpected("only people are asked here".to_string())
    }

    impl MetadataProvider for Biographer {
        fn name(&self) -> &'static str {
            "tmdb"
        }

        async fn person(&self, external_id: &str, language: &str) -> Answer<PersonDetails> {
            self.asked
                .lock()
                .expect("free")
                .push((external_id.to_string(), language.to_string()));
            if self.down {
                return Err(ProviderError::Unreachable("timed out".to_string()));
            }
            Ok(PersonDetails {
                biography: Some("Born by the sea.".to_string()),
                born_on: Some("1970-04-02".to_string()),
                died_on: None,
                birthplace: Some("Harbourtown".to_string()),
            })
        }

        async fn search(
            &self,
            _: Catalogue,
            _: &str,
            _: Option<i32>,
            _: &str,
        ) -> Answer<Vec<Candidate>> {
            Err(nothing_else())
        }

        async fn details(&self, _: Catalogue, _: &str, _: &str) -> Answer<Details> {
            Err(nothing_else())
        }

        async fn season(&self, _: &str, _: i32, _: &str) -> Answer<SeasonDetails> {
            Err(nothing_else())
        }

        fn image_url(&self, path: &str) -> String {
            path.to_string()
        }

        async fn fetch_image(&self, _: &str) -> Answer<Vec<u8>> {
            Err(nothing_else())
        }

        async fn pictures(&self, _: Catalogue, _: &str, _: &str) -> Answer<Vec<OfferedPicture>> {
            Err(nothing_else())
        }

        async fn by_imdb_id(&self, _: &str, _: &str) -> Answer<Option<Candidate>> {
            Err(nothing_else())
        }
    }

    /// One film in a library read in French, crediting one actress.
    async fn one_film() -> (AppState, User, PersonId) {
        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let work = database
            .create_work(library.id, WorkKind::Movie, "Salt Road", "salt road", Some(2011))
            .await
            .expect("work created");
        database
            .apply_identification(
                work.id,
                &IdentifiedWork {
                    provider: "tmdb".to_string(),
                    external_id: "111".to_string(),
                    imdb_id: None,
                    language: "fr".to_string(),
                    title: "Salt Road".to_string(),
                    sort_title: "salt road".to_string(),
                    tagline: None,
                    overview: None,
                    release_year: Some(2011),
                    runtime: None,
                    community_rating: None,
                    age_rating_label: None,
                    genres: Vec::new(),
                    studios: Vec::new(),
                    credits: vec![CreditRecord {
                        external_id: "42".to_string(),
                        name: "Ada Moss".to_string(),
                        sort_name: "ada moss".to_string(),
                        role: "actor".to_string(),
                        character: None,
                        ordinal: 0,
                        photo_path: None,
                    }],
                    collection: None,
                    trailers: Vec::new(),
                },
                false,
            )
            .await
            .expect("identified");
        let ada = database
            .work_credits(work.id)
            .await
            .expect("credits read")
            .remove(0)
            .person_id;
        let viewer = database
            .create_user("Viewer", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created")
            .id;
        let state = AppState::new(melyxar_config::Config::default(), database, None, None);
        (state, crate::an_ordinary_account(viewer), ada)
    }

    #[tokio::test]
    async fn a_life_is_asked_for_once_in_the_language_of_the_library() {
        let (state, who, ada) = one_film().await;
        let biographer = Biographer::default();

        let page = person_page_asking(&state, &who, ada, Some(&biographer))
            .await
            .expect("read")
            .expect("present");
        assert_eq!(page.person.biography.as_deref(), Some("Born by the sea."));
        assert_eq!(page.works.len(), 1);

        person_page_asking(&state, &who, ada, Some(&biographer))
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            *biographer.asked.lock().expect("free"),
            vec![("42".to_string(), "fr".to_string())],
            "asked once, in French, and read from the database after"
        );
    }

    #[tokio::test]
    async fn a_provider_that_does_not_answer_still_leaves_a_page_and_asks_next_time() {
        let (state, who, ada) = one_film().await;
        let down = Biographer {
            down: true,
            ..Default::default()
        };

        let page = person_page_asking(&state, &who, ada, Some(&down))
            .await
            .expect("read")
            .expect("present");
        assert_eq!(page.person.name, "Ada Moss");
        assert_eq!(page.person.biography, None);

        person_page_asking(&state, &who, ada, Some(&down))
            .await
            .expect("read");
        assert_eq!(down.asked.lock().expect("free").len(), 2, "asked again");
    }

    #[tokio::test]
    async fn somebody_met_only_in_libraries_not_granted_is_nobody() {
        let (state, mut who, ada) = one_film().await;
        who.permissions.sees_every_library = false;
        who.permissions.allowed_libraries = Vec::new();

        assert_eq!(
            person_page_asking(&state, &who, ada, None::<&Biographer>)
                .await
                .expect("read"),
            None
        );
    }
}
