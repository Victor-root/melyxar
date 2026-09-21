//! Reading the library: what a menu, a grid and a home page ask for.
//!
//! The use cases live here rather than in the layer above, so a handler reads
//! a request, calls one of these and renders the answer. It is also what lets
//! a second entry point, a command line or a television client, behave exactly
//! like the browser without any rule being written twice.

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::user::User;
use melyxar_core::library::{LibraryKind, LibraryOptions, RootAccess};

use crate::browse::{BrowseRequest, WorkCard, WorkOrder, WorkPage};
use crate::{AppState, Result};
use melyxar_database::playback::{UpNext, WorkToCarryOn};

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
    /// What the page leads with, at most five, largest of all.
    pub hero: Vec<HeroItem>,
    /// Films this viewer started and has not finished, the latest first.
    ///
    /// A film left halfway is the one thing somebody comes back for, and
    /// finding it meant remembering its title and hunting it down in the whole
    /// library.
    pub carry_on: Vec<WorkToCarryOn>,
    /// The episode each started series is waiting on. Distinct from carrying
    /// on: what somebody left halfway is not what they have not started.
    pub up_next: Vec<UpNext>,
    /// Everything newest, whatever kind it is.
    pub recently_added: WorkPage,
    /// One row per kind of library this server really holds, newest first.
    pub shelves: Vec<Shelf>,
    pub works: i64,
    pub awaiting_identification: i64,
}

/// One row of the home page, for one kind of library.
///
/// Built from the libraries that are really there rather than from a written
/// list of kinds: a server with no anime has no row of anime, and a server
/// with two libraries of films has one row holding both.
#[derive(Debug, Clone, PartialEq)]
pub struct Shelf {
    pub kind: LibraryKind,
    pub cards: Vec<WorkCard>,
}

/// One of the few works the page opens on, and why it is there.
#[derive(Debug, Clone, PartialEq)]
pub struct HeroItem {
    pub card: WorkCard,
    pub because: Because,
}

/// Why a work is in the hero, which decides what its button says.
///
/// Carried rather than worked out again by the interface: whether something
/// is there to be carried on or to be discovered is what the server decided
/// when it filled the row, and two places deciding it separately would
/// eventually decide it differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Because {
    /// Left halfway. Its button says carry on, and it comes first.
    Started,
    /// An administrator put it there.
    Pinned,
    /// Newly arrived.
    New,
    /// Nobody chose it: the server offers it.
    Suggested,
}

impl Because {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Pinned => "pinned",
            Self::New => "new",
            Self::Suggested => "suggested",
        }
    }
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

/// How many series the page says are waiting.
///
/// The same size as the row above it: both are rows of the same shape, and a
/// screen showing twenty of one and five of the other looks broken.
const UP_NEXT: i64 = 20;

/// How many works the page opens on, which the maintainer set.
const IN_THE_HERO: i64 = 5;

/// How many cards one kind's row holds.
const ON_A_SHELF: i64 = 24;

/// Every library this viewer may see, with what it holds and what the server
/// can reach.
pub async fn libraries(state: &AppState, who: &User) -> Result<Vec<LibrarySummary>> {
    let database = state.database();
    let access = database.roots_with_access().await?;

    let mut summaries = Vec::new();
    for library in database.list_libraries().await? {
        // A library somebody was not granted is one they are never told
        // about: it is the list this narrowing has to happen in, since every
        // screen of the interface is built from it.
        if !who.permissions.may_access_library(library.id) {
            continue;
        }
        summaries.push(LibrarySummary {
            works: crate::counted::counted(state, Some(library.id)).await?.browsable,
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
///
/// Every one of these is a walk through the whole library, so none of them is
/// walked on the way to a page: they are counted once per change and read
/// back. See [`crate::counted`] for why.
pub async fn filters(
    state: &AppState,
    library_id: Option<LibraryId>,
    who: &User,
) -> Result<Filters> {
    Ok(crate::reach::counted_for(state, who, library_id)
        .await?
        .filters
        .clone())
}

/// One page of a grid, read only from what this viewer may see.
pub async fn browse(
    state: &AppState,
    request: &BrowseRequest,
    who: &User,
) -> Result<WorkPage> {
    if let Some(library_id) = request.library_id {
        crate::reach::may_read(who, library_id)?;
    }
    // Both of these are put on here rather than sent by the client: what an
    // account may read and who it is are the use case's to know, and a client
    // that could name either could read a library it was never given or wear
    // somebody else's marks.
    let request = BrowseRequest {
        within: crate::reach::within(who),
        viewer: Some(who.id),
        ..request.clone()
    };
    Ok(state.database().browse_works(&request).await?)
}

/// Puts a work in front of everybody, or takes it back off.
///
/// An administrator's doing and nobody else's, which the route says by asking
/// for one: this is the server's own shelf, not a bookmark. Each person's own
/// list is the favourites.
///
/// Answers what it is now rather than what was asked for, so a button pressed
/// twice in a second cannot end up saying one thing while the server says
/// another.
pub async fn set_pinned(
    state: &AppState,
    who: &User,
    work_id: WorkId,
    pinned: bool,
) -> Result<bool> {
    crate::reach::may_read_the_work(state, who, work_id).await?;
    match pinned {
        true => state.database().pin_work(work_id).await?,
        false => {
            state.database().unpin_work(work_id).await?;
        }
    }
    Ok(pinned)
}

/// What a home page opens on.
///
/// Every row is bounded by what this account was granted, and a row with
/// nothing in it comes back empty rather than absent: what to do with an empty
/// row is the interface's business, and a server deciding to hide one would be
/// deciding how the page looks.
pub async fn home(state: &AppState, library_id: Option<LibraryId>, who: &User) -> Result<Home> {
    let database = state.database();
    let within = crate::reach::within(who);
    let granted = within.as_deref();

    let recently_added = browse(
        state,
        &BrowseRequest {
            library_id,
            order: WorkOrder::AddedAt,
            descending: true,
            limit: RECENTLY_ADDED,
            ..Default::default()
        },
        who,
    )
    .await?;

    let carry_on = database
        .works_to_carry_on(who.id, granted, CARRY_ON)
        .await?;
    let up_next = database.up_next(who.id, granted, UP_NEXT).await?;

    // Counted once per change rather than on the way here: both of these walk
    // the whole collection, and a home page that counts a hundred thousand
    // works to print two numbers is a home page nobody waits for.
    let counted = crate::reach::counted_for(state, who, library_id).await?;

    Ok(Home {
        hero: the_hero(state, who, granted, &carry_on, &recently_added).await?,
        carry_on,
        up_next,
        shelves: the_shelves(state, who).await?,
        recently_added,
        works: counted.browsable,
        awaiting_identification: counted.awaiting_identification,
    })
}

/// The few works the page opens on.
///
/// What somebody left halfway comes first, and the maintainer chose that
/// knowing it puts the same films in the hero and in the row just below it.
/// The reason is written in the decisions: reaching for the film you were
/// watching is what people come here to do, and meeting it twice costs less
/// than looking for it once.
///
/// Then what an administrator put there, then what has just arrived, then
/// what the server offers. Each source is asked only for what the ones before
/// it left room for, and nothing appears twice.
async fn the_hero(
    state: &AppState,
    who: &User,
    granted: Option<&[LibraryId]>,
    carry_on: &[WorkToCarryOn],
    recently_added: &WorkPage,
) -> Result<Vec<HeroItem>> {
    let mut hero: Vec<HeroItem> = Vec::with_capacity(IN_THE_HERO as usize);
    let mut already: std::collections::HashSet<WorkId> = std::collections::HashSet::new();

    let mut take = |card: &WorkCard, because: Because, hero: &mut Vec<HeroItem>| {
        if hero.len() < IN_THE_HERO as usize && already.insert(card.id) {
            hero.push(HeroItem {
                card: card.clone(),
                because,
            });
        }
    };

    for entry in carry_on {
        take(&entry.card, Because::Started, &mut hero);
    }
    if hero.len() < IN_THE_HERO as usize {
        for card in state
            .database()
            .pinned_works(who.id, granted, IN_THE_HERO)
            .await?
        {
            take(&card, Because::Pinned, &mut hero);
        }
    }
    for card in &recently_added.cards {
        take(card, Because::New, &mut hero);
    }
    if hero.len() < IN_THE_HERO as usize {
        for card in state
            .database()
            .suggestions(who.id, granted, IN_THE_HERO)
            .await?
        {
            take(&card, Because::Suggested, &mut hero);
        }
    }
    Ok(hero)
}

/// One row per kind of library this server really holds.
///
/// Read from the libraries themselves rather than from a written list of
/// kinds, so a server with no anime has no row of anime and nobody has to
/// remember to add one the day a kind is invented. A kind holding several
/// libraries gets one row across all of them, which is what somebody means by
/// "films" when their films are spread over four disks.
async fn the_shelves(state: &AppState, who: &User) -> Result<Vec<Shelf>> {
    let mut shelves = Vec::new();
    let mut seen: Vec<LibraryKind> = Vec::new();

    for library in libraries(state, who).await? {
        if seen.contains(&library.kind) {
            continue;
        }
        seen.push(library.kind);
        // Music has no card to show yet: the model is there, nothing fills it.
        if library.kind == LibraryKind::Music {
            continue;
        }
        let page = browse(
            state,
            &BrowseRequest {
                library_kind: Some(library.kind),
                order: WorkOrder::AddedAt,
                descending: true,
                limit: ON_A_SHELF,
                ..Default::default()
            },
            who,
        )
        .await?;
        shelves.push(Shelf {
            kind: library.kind,
            cards: page.cards,
        });
    }
    Ok(shelves)
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories, LibraryConfig, RootConfig};
    use melyxar_core::user::User;
    use melyxar_core::work::WorkKind;
    use melyxar_database::Database;

    async fn state_with_films(
        titles: &[&str],
    ) -> (tempfile::TempDir, AppState, LibraryId, User) {
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
        // somebody.
        let viewer = database
            .create_user(
                "victor",
                None,
                &melyxar_core::user::Permissions::administrator(),
            )
            .await
            .expect("account created");

        let state = AppState::new(config, database, None, None);
        (directory, state, library.id, viewer)
    }

    #[tokio::test]
    async fn a_menu_is_told_what_each_library_holds_and_what_it_can_reach() {
        let (_directory, state, library_id, viewer) =
            state_with_films(&["Quiet Harbour", "Amber Field"]).await;

        let summaries = libraries(&state, &viewer).await.expect("read");
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
        let viewer = crate::an_ordinary_account(melyxar_core::id::UserId::new());
        let summaries = libraries(&state, &viewer).await.expect("read");
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
            home(&state, Some(library_id), &viewer)
                .await
                .expect("read")
                .carry_on
                .is_empty(),
            "nothing was started, so there is nothing to carry on"
        );

        state
            .database()
            .record_playback_progress(
                viewer.id,
                started.id,
                melyxar_core::time::Millis::new(1_800_000),
                melyxar_core::work::PlaybackState::InProgress,
                melyxar_core::time::now(),
            )
            .await
            .expect("recorded");

        let page = home(&state, Some(library_id), &viewer).await.expect("read");
        assert_eq!(page.carry_on.len(), 1);
        assert_eq!(page.carry_on[0].card.id, started.id);
        assert_eq!(
            page.carry_on[0].position,
            melyxar_core::time::Millis::new(1_800_000),
            "a card has to know how far in it is without asking again per film"
        );
    }

    #[tokio::test]
    async fn the_hero_leads_with_what_was_left_halfway_then_what_is_new() {
        let (_directory, state, library_id, viewer) =
            state_with_films(&["Quiet Harbour", "Amber Field", "Winter Signal"]).await;
        let newest = state
            .database()
            .recent_works(library_id, 10)
            .await
            .expect("read");
        let started = newest.last().expect("a film").id;

        let page = home(&state, Some(library_id), &viewer).await.expect("read");
        assert!(
            page.hero.iter().all(|entry| entry.because == Because::New),
            "with nothing started and nothing pinned, the hero is what arrived"
        );

        state
            .database()
            .record_playback_progress(
                viewer.id,
                started,
                melyxar_core::time::Millis::new(1_800_000),
                melyxar_core::work::PlaybackState::InProgress,
                melyxar_core::time::now(),
            )
            .await
            .expect("recorded");

        let page = home(&state, Some(library_id), &viewer).await.expect("read");
        assert_eq!(page.hero[0].card.id, started);
        assert_eq!(page.hero[0].because, Because::Started);
        assert_eq!(
            page.hero.len(),
            3,
            "three films, and the one carried on is not shown twice"
        );
        let mut every: Vec<_> = page.hero.iter().map(|entry| entry.card.id).collect();
        every.sort();
        every.dedup();
        assert_eq!(every.len(), page.hero.len(), "nothing appears twice");
    }

    #[tokio::test]
    async fn what_an_administrator_pinned_comes_before_what_is_merely_new() {
        let (_directory, state, library_id, viewer) =
            state_with_films(&["Quiet Harbour", "Amber Field", "Winter Signal"]).await;
        let oldest = state
            .database()
            .recent_works(library_id, 10)
            .await
            .expect("read")
            .pop()
            .expect("a film")
            .id;

        state.database().pin_work(oldest).await.expect("pinned");

        let page = home(&state, Some(library_id), &viewer).await.expect("read");
        assert_eq!(page.hero[0].card.id, oldest);
        assert_eq!(page.hero[0].because, Because::Pinned);
    }

    #[tokio::test]
    async fn a_row_exists_for_each_kind_of_library_and_for_no_other() {
        let (_directory, state, _, viewer) = state_with_films(&["Quiet Harbour"]).await;

        let page = home(&state, None, &viewer).await.expect("read");
        assert_eq!(
            page.shelves.iter().map(|shelf| shelf.kind).collect::<Vec<_>>(),
            vec![LibraryKind::Movies],
            "a server with only films has one row, and no empty row of anime"
        );
        assert_eq!(page.shelves[0].cards.len(), 1);
    }

    #[tokio::test]
    async fn a_home_page_leads_with_what_arrived_last() {
        let (_directory, state, library_id, viewer) =
            state_with_films(&["Quiet Harbour", "Amber Field", "Winter Signal"]).await;

        let page = home(&state, Some(library_id), &viewer).await.expect("read");
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
        let (_directory, state, library_id, viewer) =
            state_with_films(&["Quiet Harbour", "Amber Field", "Winter Signal"]).await;

        let page = browse(
            &state,
            &BrowseRequest {
                library_id: Some(library_id),
                limit: 2,
                ..Default::default()
            },
            &viewer,
        )
        .await
        .expect("read");
        assert_eq!(page.cards.len(), 2);
        assert!(page.next.is_some());
    }

    #[tokio::test]
    async fn a_filter_menu_of_an_empty_library_offers_nothing_rather_than_failing() {
        let (_directory, state, library_id, viewer) = state_with_films(&[]).await;
        assert_eq!(
            filters(&state, Some(library_id), &viewer).await.expect("read"),
            Filters::default()
        );
    }

    #[tokio::test]
    async fn a_filter_menu_offers_what_the_library_actually_holds() {
        let (_directory, state, library_id, viewer) = state_with_films(&["Quiet Harbour"]).await;
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

        let offered = filters(&state, Some(library_id), &viewer).await.expect("read");
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

        let page = home(&state, Some(library_id), &viewer).await.expect("read");
        assert_eq!(page.works, 1);
        assert_eq!(page.recently_added.cards.len(), 1);
        assert_eq!(page.recently_added.cards[0].title, "Quiet Harbour");

        let everything = home(&state, None, &viewer).await.expect("read");
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

        let page = home(&state, Some(library_id), &viewer).await.expect("read");
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
