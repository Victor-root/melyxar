//! What arrived in a library, announced once it has settled.
//!
//! Read from when each work was added rather than told by the scan, which
//! knows nothing of notifications: what a library added since its last
//! announcement is announced once nothing has been added for a while and
//! nothing of it still waits for a name or for the readings a film gets as it
//! arrives. A file replaced or a film named again keeps its work, and the day
//! it was added, so neither is news.

use std::collections::HashMap;

use melyxar_core::id::{LibraryId, UserId, WorkId};
use melyxar_core::library::LibraryKind;
use melyxar_core::time::Timestamp;
use melyxar_core::user::User;
use melyxar_database::notifications::{AnnouncingLibrary, Arrival};

use super::sending::{readers_of, SeriesArrived};
use super::{send, Audience, Level, Outgoing, Said};
use crate::{AppState, Result};

/// How long a library must have gone without an addition before what it
/// added is announced: a scan adding a season does so in one sitting.
const QUIET_FOR: time::Duration = time::Duration::minutes(15);

/// How long an announcement waits for a work still to be named. After that,
/// it goes without it.
const NAMED_WITHIN: time::Duration = time::Duration::days(1);

/// How many films an announcement names; the rest are counted.
const FILMS_NAMED: usize = 3;

/// Whether what arrives in libraries of this kind is announced. Songs and
/// home videos are not.
fn announced_kind(kind: LibraryKind) -> bool {
    matches!(
        kind,
        LibraryKind::Movies | LibraryKind::Series | LibraryKind::Anime | LibraryKind::Shows
    )
}

/// What to do about what a library added since its last announcement.
#[derive(Debug, PartialEq)]
enum Verdict<'a> {
    /// Nothing arrived.
    Nothing,
    /// Still arriving, or still being named.
    Wait,
    /// Settled: these are announced, and everything up to `until` is done
    /// with, announced or not.
    Settled {
        until: Timestamp,
        announced: Vec<&'a Arrival>,
    },
}

fn is_named(arrival: &Arrival) -> bool {
    matches!(arrival.identification.as_str(), "identified" | "manual")
}

fn weigh<'a>(arrivals: &'a [Arrival], latest: Option<Timestamp>, now: Timestamp) -> Verdict<'a> {
    let Some(until) = arrivals.iter().map(|arrival| arrival.added_at).max() else {
        return Verdict::Nothing;
    };
    if latest.is_some_and(|latest| now - latest < QUIET_FOR) {
        return Verdict::Wait;
    }
    let still_naming = arrivals
        .iter()
        .any(|arrival| arrival.identification == "pending" && now - arrival.added_at < NAMED_WITHIN);
    if still_naming {
        return Verdict::Wait;
    }
    Verdict::Settled {
        until,
        announced: arrivals
            .iter()
            .filter(|arrival| is_named(arrival) && arrival.has_poster)
            .collect(),
    }
}

/// What one announcement says, and the work whose poster it wears.
fn said_of(library: &AnnouncingLibrary, arrivals: &[&Arrival]) -> (Said, Option<WorkId>) {
    let films: Vec<&Arrival> = arrivals
        .iter()
        .copied()
        .filter(|arrival| arrival.series.is_none())
        .collect();
    let mut series: Vec<SeriesArrived> = Vec::new();
    for (id, title) in arrivals.iter().filter_map(|arrival| arrival.series.as_ref()) {
        match series.iter_mut().find(|one| one.id == *id) {
            Some(one) => one.episodes += 1,
            None => series.push(SeriesArrived {
                id: *id,
                title: title.clone(),
                episodes: 1,
            }),
        }
    }
    let shown = films
        .first()
        .map(|film| film.work_id)
        .or_else(|| series.first().map(|one| one.id));
    let said = Said::NewContent {
        library: library.id,
        library_name: library.name.clone(),
        films: films.len(),
        film_titles: films
            .iter()
            .take(FILMS_NAMED)
            .map(|film| film.title.clone())
            .collect(),
        series,
    };
    (said, shown)
}

/// The accounts that hear about a library, put together by what each may
/// watch of what arrived: an account kept from a rating is told of the
/// rest, and never of what it may not watch.
fn by_what_they_may_watch<'a>(
    readers: &[User],
    arrivals: &[&'a Arrival],
) -> Vec<(Vec<UserId>, Vec<&'a Arrival>)> {
    let mut groups: HashMap<Vec<WorkId>, (Vec<UserId>, Vec<&'a Arrival>)> = HashMap::new();
    for reader in readers {
        let theirs: Vec<&Arrival> = arrivals
            .iter()
            .copied()
            .filter(|arrival| reader.permissions.may_watch_rating(arrival.age_rating))
            .collect();
        if theirs.is_empty() {
            continue;
        }
        let key = theirs.iter().map(|arrival| arrival.work_id).collect();
        groups
            .entry(key)
            .or_insert_with(|| (Vec::new(), theirs))
            .0
            .push(reader.id);
    }
    groups.into_values().collect()
}

/// The libraries whose arrivals may be announced, with whether they are.
pub async fn announcing(state: &AppState) -> Result<Vec<AnnouncingLibrary>> {
    Ok(state
        .database()
        .announcing_libraries()
        .await?
        .into_iter()
        .filter(|library| LibraryKind::parse(&library.kind).is_some_and(announced_kind))
        .collect())
}

/// Whether a library announces what arrives in it, from now on.
pub async fn set_announces(state: &AppState, library: LibraryId, announces: bool) -> Result<()> {
    if !announcing(state).await?.iter().any(|one| one.id == library) {
        return Err(melyxar_core::Error::not_found("no library announces what arrives under that name").into());
    }
    state
        .database()
        .set_library_announces(library, announces, melyxar_core::time::now())
        .await?;
    Ok(())
}

/// Looks at every library, and announces what has settled.
pub async fn announce(state: &AppState) -> Result<()> {
    announce_at(state, melyxar_core::time::now()).await
}

async fn announce_at(state: &AppState, now: Timestamp) -> Result<()> {
    let database = state.database();
    let libraries = database.list_libraries().await?;
    let requested = database.requested_works().await?;
    for library in announcing(state).await? {
        // A library met for the first time was added after the
        // notifications were: its first import is news.
        let since = library.announced_until.unwrap_or(library.created_at);
        let latest = database.latest_addition(library.id).await?;
        if library.announces == Some(false) {
            // Kept moving, so turning it back on does not announce
            // everything that came in meanwhile.
            if let Some(latest) = latest.filter(|latest| *latest > since) {
                database.set_announced_until(library.id, latest).await?;
            }
            continue;
        }
        let arrivals = database.arrivals_since(library.id, since).await?;
        let Verdict::Settled { until, announced } = weigh(&arrivals, latest, now) else {
            continue;
        };
        // Told only once the readings a film gets as it arrives are done: the
        // announcement sends people to a film that is ready to be played.
        if let Some(whole) = libraries.iter().find(|one| one.id == library.id)
            && crate::upkeep::is_still_taking_in(state, whole).await?
        {
            continue;
        }
        // A title somebody asked for is told to them by their request, and is
        // not news to be announced a second time.
        let announced: Vec<&Arrival> = announced
            .into_iter()
            .filter(|arrival| {
                !requested.contains(&arrival.work_id)
                    && !arrival.series.as_ref().is_some_and(|(id, _)| requested.contains(id))
            })
            .collect();
        let readers = readers_of(state, library.id).await?;
        for (accounts, theirs) in by_what_they_may_watch(&readers, &announced) {
            let (said, shown) = said_of(&library, &theirs);
            let outgoing = Outgoing {
                work: shown,
                ..Outgoing::new(said, Level::News, Audience::Accounts(accounts))
            };
            send(state, outgoing).await?;
        }
        tracing::info!(
            library = library.name,
            arrived = arrivals.len(),
            announced = announced.len(),
            "what arrived in a library was announced"
        );
        database.set_announced_until(library.id, until).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::WorkKind;
    use melyxar_database::images::StoredImage;
    use melyxar_database::metadata::IdentifiedWork;
    use time::macros::datetime;

    const NOON: Timestamp = datetime!(2026-10-03 12:00 UTC);

    fn a_film(title: &str, added_at: Timestamp, identification: &str) -> Arrival {
        Arrival {
            work_id: WorkId::new(),
            kind: "movie".to_string(),
            title: title.to_string(),
            added_at,
            series: None,
            identification: identification.to_string(),
            has_poster: true,
            age_rating: None,
        }
    }

    fn an_episode(series: (WorkId, &str), added_at: Timestamp) -> Arrival {
        Arrival {
            kind: "episode".to_string(),
            series: Some((series.0, series.1.to_string())),
            ..a_film("Pilot", added_at, "identified")
        }
    }

    #[test]
    fn a_library_still_adding_or_still_naming_waits() {
        let at = NOON - time::Duration::minutes(20);
        let arrivals = vec![a_film("Amber Field", at, "identified")];
        assert_eq!(weigh(&[], Some(at), NOON), Verdict::Nothing);
        assert_eq!(
            weigh(&arrivals, Some(NOON - time::Duration::minutes(5)), NOON),
            Verdict::Wait,
            "something was added five minutes ago"
        );

        let naming = vec![a_film("Amber Field", at, "identified"), a_film("Salt Road", at, "pending")];
        assert_eq!(weigh(&naming, Some(at), NOON), Verdict::Wait);
        let a_day_later = NOON + time::Duration::days(1);
        assert!(
            matches!(weigh(&naming, Some(at), a_day_later), Verdict::Settled { ref announced, .. } if announced.len() == 1),
            "after a day it goes without the one nobody could name"
        );
    }

    #[test]
    fn only_what_is_named_and_wears_a_poster_is_announced_but_everything_is_done_with() {
        let early = NOON - time::Duration::hours(2);
        let late = NOON - time::Duration::hours(1);
        let mut bare = a_film("Bare", late, "identified");
        bare.has_poster = false;
        let arrivals = vec![
            a_film("Amber Field", early, "identified"),
            a_film("By Hand", early, "manual"),
            a_film("Unknown", early, "unidentified"),
            bare,
        ];
        let Verdict::Settled { until, announced } = weigh(&arrivals, Some(late), NOON) else {
            panic!("settled");
        };
        assert_eq!(until, late);
        let titles: Vec<&str> = announced.iter().map(|one| one.title.as_str()).collect();
        assert_eq!(titles, vec!["Amber Field", "By Hand"]);
    }

    fn a_library() -> AnnouncingLibrary {
        AnnouncingLibrary {
            id: LibraryId::new(),
            name: "Films".to_string(),
            kind: "movies".to_string(),
            created_at: NOON,
            announces: None,
            announced_until: None,
        }
    }

    #[test]
    fn episodes_are_counted_under_their_series_and_a_lone_series_wears_its_poster() {
        let salt_road = (WorkId::new(), "Salt Road");
        let tides = (WorkId::new(), "Tides");
        let arrivals = [
            an_episode(salt_road, NOON),
            an_episode(tides, NOON),
            an_episode(salt_road, NOON),
        ];
        let library = a_library();
        let (said, shown) = said_of(&library, &arrivals.iter().collect::<Vec<_>>());
        assert_eq!(shown, Some(salt_road.0));
        assert_eq!(
            said,
            Said::NewContent {
                library: library.id,
                library_name: "Films".to_string(),
                films: 0,
                film_titles: Vec::new(),
                series: vec![
                    SeriesArrived { id: salt_road.0, title: "Salt Road".to_string(), episodes: 2 },
                    SeriesArrived { id: tides.0, title: "Tides".to_string(), episodes: 1 },
                ],
            }
        );
    }

    #[test]
    fn films_are_named_up_to_a_few_and_counted_beyond() {
        let arrivals: Vec<Arrival> = ["One", "Two", "Three", "Four"]
            .iter()
            .map(|title| a_film(title, NOON, "identified"))
            .collect();
        let (said, shown) = said_of(&a_library(), &arrivals.iter().collect::<Vec<_>>());
        assert_eq!(shown, Some(arrivals[0].work_id));
        let Said::NewContent { films, film_titles, .. } = said else {
            panic!("new content");
        };
        assert_eq!(films, 4);
        assert_eq!(film_titles, vec!["One", "Two", "Three"]);
    }

    fn a_reader(limit: Option<i32>) -> User {
        User {
            id: UserId::new(),
            name: "reader".to_string(),
            avatar_path: None,
            permissions: Permissions {
                max_age_rating: limit,
                ..Permissions::viewer()
            },
            preferences: Default::default(),
            created_at: NOON,
        }
    }

    #[test]
    fn an_account_kept_from_a_rating_is_told_of_the_rest_only() {
        let mut adult = a_film("Late Show", NOON, "identified");
        adult.age_rating = Some(18);
        let family = a_film("Picnic", NOON, "identified");
        let arrivals = vec![&adult, &family];
        let grown = a_reader(None);
        let other_grown = a_reader(Some(18));
        let child = a_reader(Some(10));

        let mut groups = by_what_they_may_watch(&[grown.clone(), child.clone(), other_grown.clone()], &arrivals);
        groups.sort_by_key(|(_, theirs)| theirs.len());
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].0, vec![child.id]);
        assert_eq!(groups[0].1, vec![&family]);
        assert_eq!(groups[1].0, vec![grown.id, other_grown.id]);

        let only_adult = vec![&adult];
        assert_eq!(by_what_they_may_watch(&[child], &only_adult), Vec::new());
    }

    #[test]
    fn songs_and_home_videos_are_never_announced() {
        assert!(announced_kind(LibraryKind::Movies));
        assert!(announced_kind(LibraryKind::Anime));
        assert!(!announced_kind(LibraryKind::Music));
        assert!(!announced_kind(LibraryKind::HomeMedia));
    }

    #[tokio::test]
    async fn a_settled_arrival_is_announced_once_to_who_may_read_it() {
        let (_held, state) = crate::an_empty_server().await;
        let database = state.database();
        let films = database
            .create_library("Films", LibraryKind::Movies, "fr", &[])
            .await
            .expect("library")
            .id;
        let reader = database
            .create_user("reader", None, &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let film = a_named_film_with_a_poster(database, films).await;
        let kept = || database.notifications_of(reader, None, 10);

        announce_at(&state, film.added_at + time::Duration::minutes(1))
            .await
            .expect("looked");
        assert!(kept().await.expect("read").is_empty(), "still arriving");

        let settled = film.added_at + QUIET_FOR + time::Duration::seconds(1);
        announce_at(&state, settled).await.expect("looked");
        let told = kept().await.expect("read");
        assert_eq!(told.len(), 1);
        assert_eq!(told[0].kind, "new_content");
        assert_eq!(told[0].level, "news");
        assert_eq!(told[0].work_id, Some(film.id));

        announce_at(&state, settled + time::Duration::hours(1))
            .await
            .expect("looked");
        assert_eq!(kept().await.expect("read").len(), 1, "announced once");
    }

    #[tokio::test]
    async fn an_arrival_waits_for_the_readings_it_gets_as_it_arrives() {
        use melyxar_core::job::{JobKind, JobPriority, JobState};

        let (_held, state) = crate::an_empty_server().await;
        let database = state.database();
        let films = database
            .create_library("Films", LibraryKind::Movies, "fr", &[])
            .await
            .expect("library")
            .id;
        let reader = database
            .create_user("reader", None, &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let film = a_named_film_with_a_poster(database, films).await;

        let target = films.to_string();
        let reading = database
            .create_job(JobKind::ReadKeyFrames, JobPriority::BACKGROUND, Some(&target))
            .await
            .expect("job");
        let settled = film.added_at + QUIET_FOR + time::Duration::seconds(1);
        announce_at(&state, settled).await.expect("looked");
        assert!(
            database.notifications_of(reader, None, 10).await.expect("read").is_empty(),
            "the film is still being read for where a jump can land"
        );

        database
            .finish_job(reading.id, JobState::Succeeded, None)
            .await
            .expect("finished");
        announce_at(&state, settled + time::Duration::minutes(1))
            .await
            .expect("looked");
        assert_eq!(database.notifications_of(reader, None, 10).await.expect("read").len(), 1);
    }

    /// A film the libraries could announce: named, and wearing a poster.
    async fn a_named_film_with_a_poster(
        database: &melyxar_database::Database,
        library: LibraryId,
    ) -> melyxar_core::work::Work {
        let film = database
            .create_work(library, WorkKind::Movie, "Amber Field", "amber field", None)
            .await
            .expect("film");
        database
            .apply_identification(film.id, &named("Amber Field"), false)
            .await
            .expect("named");
        let poster = StoredImage {
            owner_kind: "work".to_string(),
            owner_id: film.id.to_db_string(),
            image_kind: "poster".to_string(),
            relative_path: "works/amber/poster-400.webp".to_string(),
            width: Some(400),
            height: Some(600),
            fingerprint: "amber".to_string(),
            dominant_color: None,
        };
        database
            .replace_images("work", &film.id.to_db_string(), "poster", &[poster])
            .await
            .expect("poster");
        film
    }

    #[tokio::test]
    async fn a_title_somebody_asked_for_is_not_announced_as_news() {
        let (_held, state) = crate::an_empty_server().await;
        let database = state.database();
        let films = database
            .create_library("Films", LibraryKind::Movies, "fr", &[])
            .await
            .expect("library")
            .id;
        let reader = database
            .create_user("reader", None, &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let asked = a_named_film_with_a_poster(database, films).await;
        database
            .add_request(&melyxar_database::requests::NewRequest {
                user_id: reader,
                catalogue: "films",
                tmdb_id: "tmdb-Amber Field",
                title: "Amber Field",
                year: None,
                poster_path: None,
                overview: None,
                seasons: &[],
                note: "",
                created_at: NOON,
            })
            .await
            .expect("asked");
        let other = database
            .create_work(films, WorkKind::Movie, "Salt Road", "salt road", None)
            .await
            .expect("film");
        database
            .apply_identification(other.id, &IdentifiedWork { external_id: "tmdb-Salt".to_string(), ..named("Salt Road") }, false)
            .await
            .expect("named");
        let poster = StoredImage {
            owner_kind: "work".to_string(),
            owner_id: other.id.to_db_string(),
            image_kind: "poster".to_string(),
            relative_path: "works/salt/poster-400.webp".to_string(),
            width: Some(400),
            height: Some(600),
            fingerprint: "salt".to_string(),
            dominant_color: None,
        };
        database
            .replace_images("work", &other.id.to_db_string(), "poster", &[poster])
            .await
            .expect("poster");

        let settled = asked.added_at.max(other.added_at) + QUIET_FOR + time::Duration::seconds(1);
        announce_at(&state, settled).await.expect("looked");
        let told = database.notifications_of(reader, None, 10).await.expect("read");
        assert_eq!(told.len(), 1);
        assert_eq!(told[0].work_id, Some(other.id), "only the film nobody asked for");
    }

    fn named(title: &str) -> IdentifiedWork {
        IdentifiedWork {
            provider: "tmdb".to_string(),
            external_id: format!("tmdb-{title}"),
            imdb_id: None,
            language: "fr".to_string(),
            title: title.to_string(),
            sort_title: title.to_lowercase(),
            tagline: None,
            overview: None,
            release_year: Some(2019),
            release_date: None,
            end_date: None,
            runtime: None,
            community_rating: None,
            age_rating_label: None,
            genres: Vec::new(),
            studios: Vec::new(),
            credits: Vec::new(),
            collection: None,
            trailers: Vec::new(),
        }
    }
}
