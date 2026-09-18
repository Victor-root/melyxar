//! Reading the library: what a menu, a grid and a home page ask for.
//!
//! The use cases live here rather than in the layer above, so a handler reads
//! a request, calls one of these and renders the answer. It is also what lets
//! a second entry point, a command line or a television client, behave exactly
//! like the browser without any rule being written twice.

use melyxar_core::id::{LibraryId, UserId};
use melyxar_core::library::{LibraryKind, LibraryOptions, RootAccess};

use crate::browse::{BrowseRequest, WorkOrder, WorkPage};
use crate::{AppState, Result};
use melyxar_database::playback::WorkToCarryOn;

/// A library as a menu shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibrarySummary {
    pub id: LibraryId,
    pub name: String,
    pub kind: LibraryKind,
    pub works: i64,
    /// Moves whenever anything in the library moves, so a client can ask
    /// whether to refetch instead of refetching.
    pub version: i64,
    /// What a scan of this library does in one sitting, so the screen that
    /// offers the switches shows where they stand.
    pub options: LibraryOptions,
    /// The language its films are described in, for the same screen.
    pub metadata_language: String,
    pub roots: Vec<RootSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootSummary {
    pub id: melyxar_core::id::LibraryRootId,
    pub label: String,
    /// Its whole path on the server's disk. Shown on the one screen an
    /// administrator manages roots from, where knowing exactly which folder a
    /// label stands for is the point of being there; a log line still shows
    /// only the label, which is the rule everywhere else.
    pub path: std::path::PathBuf,
    /// What the server may actually do with the folder, established by trying
    /// rather than by reading permission bits.
    pub access: RootAccess,
}

/// What the filter menu of one library offers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filters {
    pub genres: Vec<(String, i64)>,
    pub decades: Vec<(i32, i64)>,
    /// The letters titles actually start with, with how many start with each.
    pub initials: Vec<(String, i64)>,
}

/// What a home page leads with.
#[derive(Debug, Clone, PartialEq)]
pub struct Home {
    /// Films this viewer started and has not finished, the latest first.
    ///
    /// First on the page, before anything else: a film left halfway is the one
    /// thing somebody comes back for, and finding it meant remembering its
    /// title and hunting it down in the whole library.
    pub carry_on: Vec<WorkToCarryOn>,
    pub recently_added: WorkPage,
    pub works: i64,
    pub awaiting_identification: i64,
}

/// How many cards the home page leads with.
///
/// Enough to fill a wide screen twice over, few enough to arrive at once.
const RECENTLY_ADDED: i64 = 24;

/// How many unfinished films the home page offers.
///
/// A row, not a library: past a certain number these are films somebody
/// abandoned rather than films they mean to come back to.
const CARRY_ON: i64 = 20;

/// Every library, with what it holds and what the server can reach.
pub async fn libraries(state: &AppState) -> Result<Vec<LibrarySummary>> {
    let database = state.database();
    let access = database.roots_with_access().await?;

    let mut summaries = Vec::new();
    for library in database.list_libraries().await? {
        summaries.push(LibrarySummary {
            works: database.count_browsable(Some(library.id)).await?,
            version: database.library_version(library.id).await?,
            roots: library
                .roots
                .iter()
                .map(|root| RootSummary {
                    id: root.id,
                    label: root.label.clone(),
                    path: root.path.clone(),
                    // A root nobody has tested counts as missing, the most
                    // cautious of the four states.
                    access: access
                        .iter()
                        .find(|entry| entry.root.id == root.id)
                        .map(|entry| entry.access)
                        .unwrap_or(RootAccess::Missing),
                })
                .collect(),
            id: library.id,
            name: library.name,
            kind: library.kind,
            options: library.options,
            metadata_language: library.metadata_language,
        });
    }
    Ok(summaries)
}

/// What can be filtered on in one library.
pub async fn filters(state: &AppState, library_id: Option<LibraryId>) -> Result<Filters> {
    let database = state.database();
    Ok(Filters {
        genres: database.genres_in_use(library_id).await?,
        decades: database.decades_in_use(library_id).await?,
        initials: database.initials_in_use(library_id).await?,
    })
}

/// One page of a grid.
pub async fn browse(state: &AppState, request: &BrowseRequest) -> Result<WorkPage> {
    Ok(state.database().browse_works(request).await?)
}

/// What a home page opens on.
pub async fn home(state: &AppState, library_id: Option<LibraryId>, viewer: UserId) -> Result<Home> {
    let database = state.database();
    let recently_added = database
        .browse_works(&BrowseRequest {
            library_id,
            order: WorkOrder::AddedAt,
            descending: true,
            limit: RECENTLY_ADDED,
            ..Default::default()
        })
        .await?;

    Ok(Home {
        carry_on: database.works_to_carry_on(viewer, CARRY_ON).await?,
        recently_added,
        works: database.count_browsable(library_id).await?,
        awaiting_identification: database.catalogue_summary().await?.awaiting_identification,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories, LibraryConfig, RootConfig};
    use melyxar_core::work::WorkKind;
    use melyxar_database::Database;

    async fn state_with_films(titles: &[&str]) -> (tempfile::TempDir, AppState, LibraryId, UserId) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let media = directory.path().join("films");
        std::fs::create_dir_all(&media).expect("media folder");

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
                    path: media,
                }],
            }],
            ..Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");

        let database = Database::open_in_memory().await.expect("database opens");
        crate::startup::reconcile_libraries(&database, &config)
            .await
            .expect("libraries reconciled");
        crate::startup::refresh_root_access(&database)
            .await
            .expect("access refreshed");
        let library = database
            .library_by_name("Films")
            .await
            .expect("read")
            .expect("declared");

        for title in titles {
            database
                .create_work(
                    library.id,
                    WorkKind::Movie,
                    title,
                    &melyxar_library::sort_title(title),
                    Some(2019),
                )
                .await
                .expect("work created");
        }

        // A home page shows what somebody left halfway, so there has to be a
        // somebody. The server makes this account for itself at first start.
        let viewer = database
            .create_user(
                crate::startup::DEFAULT_ACCOUNT_NAME,
                None,
                &melyxar_core::user::Permissions::administrator(),
            )
            .await
            .expect("account created");

        let state = AppState::new(config, database, None, None);
        (directory, state, library.id, viewer.id)
    }

    #[tokio::test]
    async fn a_menu_is_told_what_each_library_holds_and_what_it_can_reach() {
        let (_directory, state, library_id, _viewer) =
            state_with_films(&["Quiet Harbour", "Amber Field"]).await;

        let summaries = libraries(&state).await.expect("read");
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, library_id);
        assert_eq!(summaries[0].works, 2);
        assert_eq!(summaries[0].roots.len(), 1);
        assert!(
            summaries[0].roots[0].access.is_usable(),
            "the folder was created, so the server can reach it"
        );
        assert_eq!(
            summaries[0].roots[0].access.explanation_code(),
            "root_readable_and_writable"
        );
    }

    #[tokio::test]
    async fn a_root_that_cannot_be_reached_says_so_rather_than_looking_fine() {
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
                    label: "disk-two".into(),
                    path: std::path::PathBuf::from("/nowhere/at/all"),
                }],
            }],
            ..Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let database = Database::open_in_memory().await.expect("database opens");
        crate::startup::reconcile_libraries(&database, &config)
            .await
            .expect("libraries reconciled");
        crate::startup::refresh_root_access(&database)
            .await
            .expect("access refreshed");

        let state = AppState::new(config, database, None, None);
        let summaries = libraries(&state).await.expect("read");
        assert_eq!(summaries[0].roots[0].access, RootAccess::Missing);
        assert_eq!(
            summaries[0].roots[0].access.explanation_code(),
            "root_missing_or_not_mounted"
        );
    }

    #[tokio::test]
    async fn a_home_page_offers_back_what_was_left_halfway() {
        let (_directory, state, library_id, viewer) =
            state_with_films(&["Quiet Harbour", "Amber Field"]).await;
        let started = state
            .database()
            .recent_works(library_id, 10)
            .await
            .expect("read")
            .pop()
            .expect("a film");

        assert!(
            home(&state, Some(library_id), viewer)
                .await
                .expect("read")
                .carry_on
                .is_empty(),
            "nothing was started, so there is nothing to carry on"
        );

        state
            .database()
            .record_playback_progress(
                viewer,
                started.id,
                melyxar_core::time::Millis::new(1_800_000),
                melyxar_core::work::PlaybackState::InProgress,
                melyxar_core::time::now(),
            )
            .await
            .expect("recorded");

        let page = home(&state, Some(library_id), viewer).await.expect("read");
        assert_eq!(page.carry_on.len(), 1);
        assert_eq!(page.carry_on[0].card.id, started.id);
        assert_eq!(
            page.carry_on[0].position,
            melyxar_core::time::Millis::new(1_800_000),
            "a card has to know how far in it is without asking again per film"
        );
    }

    #[tokio::test]
    async fn a_home_page_leads_with_what_arrived_last() {
        let (_directory, state, library_id, viewer) =
            state_with_films(&["Quiet Harbour", "Amber Field", "Winter Signal"]).await;

        let page = home(&state, Some(library_id), viewer).await.expect("read");
        assert_eq!(page.works, 3);
        assert_eq!(page.recently_added.cards.len(), 3);
        assert_eq!(
            page.recently_added.cards[0].title, "Winter Signal",
            "the one added last comes first"
        );
        assert_eq!(
            page.awaiting_identification, 3,
            "a page can say why the films have no titles yet"
        );
    }

    #[tokio::test]
    async fn a_grid_reads_one_page_at_a_time() {
        let (_directory, state, library_id, _viewer) =
            state_with_films(&["Quiet Harbour", "Amber Field", "Winter Signal"]).await;

        let page = browse(
            &state,
            &BrowseRequest {
                library_id: Some(library_id),
                limit: 2,
                ..Default::default()
            },
        )
        .await
        .expect("read");
        assert_eq!(page.cards.len(), 2);
        assert!(page.next.is_some());
    }

    #[tokio::test]
    async fn a_filter_menu_of_an_empty_library_offers_nothing_rather_than_failing() {
        let (_directory, state, library_id, _viewer) = state_with_films(&[]).await;
        assert_eq!(
            filters(&state, Some(library_id)).await.expect("read"),
            Filters::default()
        );
    }

    #[tokio::test]
    async fn a_filter_menu_offers_what_the_library_actually_holds() {
        let (_directory, state, library_id, _viewer) = state_with_films(&["Quiet Harbour"]).await;
        let work = state
            .database()
            .browse_works(&BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("read")
            .cards[0]
            .id;

        state
            .database()
            .apply_identification(
                work,
                &melyxar_database::metadata::IdentifiedWork {
                    provider: "tmdb".to_string(),
                    external_id: "111".to_string(),
                    imdb_id: None,
                    language: "fr".to_string(),
                    title: "Quiet Harbour".to_string(),
                    sort_title: "quiet harbour".to_string(),
                    tagline: None,
                    overview: None,
                    release_year: Some(2019),
                    runtime: None,
                    community_rating: None,
                    age_rating_label: None,
                    genres: vec!["Drame".to_string(), "Thriller".to_string()],
                    studios: Vec::new(),
                    credits: Vec::new(),
                    collection: None,
                    trailers: Vec::new(),
                },
                false,
            )
            .await
            .expect("identification applied");

        let offered = filters(&state, Some(library_id)).await.expect("read");
        assert_eq!(
            offered
                .genres
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["Drame", "Thriller"]
        );
        assert_eq!(offered.decades, vec![(2010, 1)]);
    }

    #[tokio::test]
    async fn a_home_page_shows_one_library_rather_than_the_whole_server() {
        // Two libraries, and the page was asked about one of them: a home page
        // that answers with everything puts films from elsewhere on it.
        let (_directory, state, library_id, viewer) = state_with_films(&["Quiet Harbour"]).await;
        let elsewhere = state
            .database()
            .create_library(
                "Animes",
                melyxar_core::library::LibraryKind::Anime,
                "fr",
                &[(
                    "disk-two".to_string(),
                    std::path::PathBuf::from("/mnt/two/Animes"),
                )],
            )
            .await
            .expect("library created");
        state
            .database()
            .create_work(
                elsewhere.id,
                WorkKind::Movie,
                "Amber Field",
                "amber field",
                Some(2020),
            )
            .await
            .expect("work created");

        let page = home(&state, Some(library_id), viewer).await.expect("read");
        assert_eq!(page.works, 1);
        assert_eq!(page.recently_added.cards.len(), 1);
        assert_eq!(page.recently_added.cards[0].title, "Quiet Harbour");

        let everything = home(&state, None, viewer).await.expect("read");
        assert_eq!(
            everything.works, 2,
            "without a library named, a home page covers the whole server"
        );
    }

    #[tokio::test]
    async fn a_home_page_shows_a_handful_of_the_newest_and_not_the_whole_library() {
        // Titles in the reverse of the order they arrive, so a page ordered by
        // name rather than by arrival is told apart from one that is not.
        let titles: Vec<String> = (0..30)
            .map(|index| format!("Invented Title {:02}", 29 - index))
            .collect();
        let borrowed: Vec<&str> = titles.iter().map(String::as_str).collect();
        let (_directory, state, library_id, viewer) = state_with_films(&borrowed).await;

        let page = home(&state, Some(library_id), viewer).await.expect("read");
        assert_eq!(page.works, 30, "the count covers everything");
        assert_eq!(
            page.recently_added.cards.len(),
            RECENTLY_ADDED as usize,
            "a home page opens on a handful, not on the whole library"
        );
        assert_eq!(
            page.recently_added.cards[0].title, "Invented Title 00",
            "newest first, which is the one that arrived last and not the last by name"
        );
    }
}
