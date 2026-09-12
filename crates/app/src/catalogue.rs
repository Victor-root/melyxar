//! Reading the library: what a menu, a grid and a home page ask for.
//!
//! The use cases live here rather than in the layer above, so a handler reads
//! a request, calls one of these and renders the answer. It is also what lets
//! a second entry point, a command line or a television client, behave exactly
//! like the browser without any rule being written twice.

use melyxar_core::id::LibraryId;
use melyxar_core::library::{LibraryKind, RootAccess};

use crate::browse::{BrowseRequest, WorkOrder, WorkPage};
use crate::{AppState, Result};

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
    pub roots: Vec<RootSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootSummary {
    pub label: String,
    /// What the server may actually do with the folder, established by trying
    /// rather than by reading permission bits.
    pub access: RootAccess,
}

/// What the filter menu of one library offers.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Filters {
    pub genres: Vec<(String, i64)>,
    pub decades: Vec<(i32, i64)>,
}

/// What a home page leads with.
#[derive(Debug, Clone, PartialEq)]
pub struct Home {
    pub recently_added: WorkPage,
    pub works: i64,
    pub awaiting_identification: i64,
}

/// How many cards the home page leads with.
///
/// Enough to fill a wide screen twice over, few enough to arrive at once.
const RECENTLY_ADDED: i64 = 24;

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
                    label: root.label.clone(),
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
    })
}

/// One page of a grid.
pub async fn browse(state: &AppState, request: &BrowseRequest) -> Result<WorkPage> {
    Ok(state.database().browse_works(request).await?)
}

/// What a home page opens on.
pub async fn home(state: &AppState, library_id: Option<LibraryId>) -> Result<Home> {
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

    async fn state_with_films(titles: &[&str]) -> (tempfile::TempDir, AppState, LibraryId) {
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

        let state = AppState::new(config, database, None, None);
        (directory, state, library.id)
    }

    #[tokio::test]
    async fn a_menu_is_told_what_each_library_holds_and_what_it_can_reach() {
        let (_directory, state, library_id) =
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
    async fn a_home_page_leads_with_what_arrived_last() {
        let (_directory, state, library_id) =
            state_with_films(&["Quiet Harbour", "Amber Field", "Winter Signal"]).await;

        let page = home(&state, Some(library_id)).await.expect("read");
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
        let (_directory, state, library_id) =
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
        let (_directory, state, library_id) = state_with_films(&[]).await;
        assert_eq!(
            filters(&state, Some(library_id)).await.expect("read"),
            Filters::default()
        );
    }
}
