//! Two accounts on one server, and what each of them can reach.
//!
//! Everything else about this is checked on its own: that the right is read
//! back from the database as it was written, that a narrowing goes into the
//! statement a grid is read by, that a library nobody was granted answers as a
//! library that is not there. What none of that proves is the thing somebody
//! actually gets, which is that the person who was given the films and nothing
//! else cannot reach the series by any of the ways this server answers.
//!
//! So two libraries are built here with a work in each, and three accounts
//! meet them: the one that sees everything, the one granted the films alone,
//! and the one granted nothing at all. Every way in is tried with each of
//! them. A right applied in nine places out of ten is a right that does not
//! exist, and the tenth is the one somebody finds.

use melyxar_app::browse::BrowseRequest;
use melyxar_app::AppState;
use melyxar_config::{Config, Directories};
use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::library::LibraryKind;
use melyxar_core::time::Millis;
use melyxar_core::user::{Permissions, User};
use melyxar_core::work::{PlaybackState, WorkKind};
use melyxar_database::Database;
use std::path::PathBuf;

/// Everything the tests below are put to work against.
struct TwoLibraries {
    _directory: tempfile::TempDir,
    state: AppState,
    films: LibraryId,
    series: LibraryId,
    a_film: WorkId,
    a_series: WorkId,
    /// Sees every library there is.
    anybody: User,
    /// Granted the films and nothing else.
    granted_the_films: User,
    /// Granted nothing at all, which is not the same as granted nothing in
    /// particular.
    granted_nothing: User,
}

async fn two_libraries() -> TwoLibraries {
    let directory = tempfile::tempdir().expect("temporary directory");
    let config = Config {
        directories: Directories {
            data: directory.path().join("data"),
            cache: directory.path().join("cache"),
            transcodes: directory.path().join("cache/transcodes"),
        },
        ..Config::default()
    };
    melyxar_app::startup::prepare_directories(&config).expect("directories prepared");

    let database = Database::open_in_memory().await.expect("database opens");
    let films = database
        .create_library(
            "Films",
            LibraryKind::Movies,
            "fr",
            &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
        )
        .await
        .expect("library created");
    let series = database
        .create_library(
            "Series",
            LibraryKind::Series,
            "fr",
            &[("disk-two".to_string(), PathBuf::from("/mnt/two/Series"))],
        )
        .await
        .expect("library created");

    let a_film = database
        .create_work(
            films.id,
            WorkKind::Movie,
            "Quiet Harbour",
            "quiet harbour",
            Some(2019),
        )
        .await
        .expect("work created");
    let a_series = database
        .create_work(
            series.id,
            WorkKind::Series,
            "Amber Field",
            "amber field",
            Some(2021),
        )
        .await
        .expect("work created");

    let anybody = database
        .create_user("anybody", None, &Permissions::viewer())
        .await
        .expect("account created");
    let granted_the_films = database
        .create_user(
            "granted the films",
            None,
            &Permissions {
                sees_every_library: false,
                allowed_libraries: vec![films.id],
                ..Permissions::viewer()
            },
        )
        .await
        .expect("account created");
    let granted_nothing = database
        .create_user(
            "granted nothing",
            None,
            &Permissions {
                sees_every_library: false,
                allowed_libraries: Vec::new(),
                ..Permissions::viewer()
            },
        )
        .await
        .expect("account created");

    TwoLibraries {
        _directory: directory,
        state: AppState::new(config, database, None, None),
        films: films.id,
        series: series.id,
        a_film: a_film.id,
        a_series: a_series.id,
        anybody,
        granted_the_films,
        granted_nothing,
    }
}

/// The titles of a grid read with no library asked for.
async fn everything_seen_by(here: &TwoLibraries, who: &User) -> Vec<String> {
    melyxar_app::catalogue::browse(&here.state, &BrowseRequest::default(), who)
        .await
        .expect("read")
        .cards
        .into_iter()
        .map(|card| card.title)
        .collect()
}

#[tokio::test]
async fn the_menu_names_only_the_libraries_an_account_was_granted() {
    let here = two_libraries().await;

    let named = |summaries: Vec<melyxar_app::catalogue::LibrarySummary>| -> Vec<String> {
        summaries.into_iter().map(|library| library.name).collect()
    };

    assert_eq!(
        named(
            melyxar_app::catalogue::libraries(&here.state, &here.anybody)
                .await
                .expect("read")
        ),
        vec!["Films", "Series"]
    );
    // It is the list every screen of the interface is built from, so a library
    // left in it would be offered and then refused, which reads as a fault.
    assert_eq!(
        named(
            melyxar_app::catalogue::libraries(&here.state, &here.granted_the_films)
                .await
                .expect("read")
        ),
        vec!["Films"]
    );
    assert!(named(
        melyxar_app::catalogue::libraries(&here.state, &here.granted_nothing)
            .await
            .expect("read")
    )
    .is_empty());
}

#[tokio::test]
async fn a_grid_over_every_library_holds_only_what_an_account_may_see() {
    let here = two_libraries().await;

    assert_eq!(
        everything_seen_by(&here, &here.anybody).await,
        vec!["Amber Field", "Quiet Harbour"]
    );
    assert_eq!(
        everything_seen_by(&here, &here.granted_the_films).await,
        vec!["Quiet Harbour"],
        "asking for no library in particular must not mean asking for all of them"
    );
    assert!(everything_seen_by(&here, &here.granted_nothing)
        .await
        .is_empty());
}

#[tokio::test]
async fn a_library_an_account_was_not_granted_is_a_library_that_is_not_there() {
    let here = two_libraries().await;

    for (who, refused) in [
        (&here.granted_the_films, here.series),
        (&here.granted_nothing, here.films),
        (&here.granted_nothing, here.series),
    ] {
        let outcome = melyxar_app::catalogue::browse(
            &here.state,
            &BrowseRequest {
                library_id: Some(refused),
                ..Default::default()
            },
            who,
        )
        .await;
        assert!(outcome.is_err(), "{} reached a grid of it", who.name);

        assert!(
            melyxar_app::catalogue::filters(&here.state, Some(refused), who)
                .await
                .is_err(),
            "{} reached the menus of it",
            who.name
        );

        assert!(
            melyxar_app::catalogue::home(&here.state, Some(refused), who)
                .await
                .is_err(),
            "{} reached a home page of it",
            who.name
        );
    }

    // And the one it was granted still answers, so the narrowing is a
    // narrowing rather than a wall.
    assert!(melyxar_app::catalogue::filters(
        &here.state,
        Some(here.films),
        &here.granted_the_films
    )
    .await
    .is_ok());
}

#[tokio::test]
async fn a_work_of_a_library_an_account_may_not_see_is_a_work_that_is_not_there() {
    let here = two_libraries().await;

    assert!(
        melyxar_app::detail::work_detail(&here.state, &here.anybody, here.a_series)
            .await
            .expect("read")
            .is_some()
    );
    assert!(
        melyxar_app::detail::work_detail(&here.state, &here.granted_the_films, here.a_film)
            .await
            .expect("read")
            .is_some(),
        "the films are theirs to read"
    );
    assert!(
        melyxar_app::detail::work_detail(&here.state, &here.granted_the_films, here.a_series)
            .await
            .expect("read")
            .is_none(),
        "a page of a series must read as a page of something that is not there"
    );
    assert!(
        melyxar_app::detail::work_detail(&here.state, &here.granted_nothing, here.a_film)
            .await
            .expect("read")
            .is_none()
    );
}

#[tokio::test]
async fn nothing_about_a_work_out_of_reach_can_be_written_either() {
    let here = two_libraries().await;

    // Liking it, and where somebody got to in it. Both are writes against a
    // work, and a right that only holds for reading is not a right.
    assert!(melyxar_app::playback::set_favourite(
        &here.state,
        &here.granted_the_films,
        here.a_series,
        true
    )
    .await
    .is_err());
    assert!(melyxar_app::playback::record_position(
        &here.state,
        &here.granted_the_films,
        here.a_series,
        Millis::new(60_000),
        melyxar_core::time::now(),
    )
    .await
    .is_err());

    // The same two against the library they were granted go through.
    assert!(
        melyxar_app::playback::set_favourite(
            &here.state,
            &here.granted_the_films,
            here.a_film,
            true
        )
        .await
        .expect("written"),
        "what it is now, which is what the button is drawn from"
    );
}

#[tokio::test]
async fn what_one_account_watched_is_not_what_another_one_carries_on_with() {
    let here = two_libraries().await;

    here.state
        .database()
        .record_playback_progress(
            here.anybody.id,
            here.a_film,
            Millis::new(600_000),
            PlaybackState::InProgress,
            melyxar_core::time::now(),
        )
        .await
        .expect("recorded");

    let carried_on_by = |who: &User| {
        let state = here.state.clone();
        let who = who.clone();
        async move {
            melyxar_app::catalogue::home(&state, None, &who)
                .await
                .expect("read")
                .carry_on
                .len()
        }
    };

    assert_eq!(carried_on_by(&here.anybody).await, 1);
    assert_eq!(
        carried_on_by(&here.granted_the_films).await,
        0,
        "a film somebody else left halfway is not on this person's home page"
    );
}
