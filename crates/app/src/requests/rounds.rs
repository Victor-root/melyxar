//! What the requests do of their own accord, every minute: notice the titles
//! asked for that arrived in a library, were identified and are done being
//! taken in, mark their requests added, and tell each account that asked.

use std::collections::HashMap;

use melyxar_core::id::{RequestId, UserId, WorkId};
use melyxar_database::requests::TitleRequest;

use super::live::moved;
use super::rules::has_arrived;
use super::{catalogue_of, telling, word_of, Catalogue, Result};
use crate::AppState;

/// How often the round is made.
const EVERY: std::time::Duration = std::time::Duration::from_secs(60);

/// Marks added every open request whose title arrived, and tells whoever
/// asked. Nothing while requests are off.
pub async fn settle_arrivals(state: &AppState) -> Result<()> {
    let database = state.database();
    if !database.requests_enabled().await? {
        return Ok(());
    }
    let open = database.open_requests().await?;
    let libraries = database.list_libraries().await?;
    // Each account's requests are answered from the libraries it sees: a
    // title held where it cannot look has not arrived for it.
    let mut asked_by: HashMap<UserId, Vec<&TitleRequest>> = HashMap::new();
    for request in &open {
        asked_by.entry(request.user_id).or_default().push(request);
    }
    let mut arrived: HashMap<WorkId, Vec<RequestId>> = HashMap::new();
    for (user, theirs) in asked_by {
        let Some(who) = database.user(user).await? else {
            continue;
        };
        let within = crate::reach::within(&who);
        for catalogue in [Catalogue::Films, Catalogue::Series] {
            let asked: Vec<_> = theirs
                .iter()
                .filter(|request| catalogue_of(&request.catalogue) == Some(catalogue))
                .collect();
            let ids: Vec<String> = asked.iter().map(|request| request.tmdb_id.clone()).collect();
            let held = database
                .held_titles(word_of(catalogue), &ids, within.as_deref())
                .await?;
            for request in asked {
                if let Some(title) = held.get(&request.tmdb_id)
                    && has_arrived(&request.seasons, title)
                {
                    // Said added once the library is done taking it in, so the
                    // title somebody is sent to is ready to be played.
                    let taking_in = match libraries.iter().find(|one| one.id == title.library_id) {
                        Some(library) => crate::upkeep::is_still_taking_in(state, library).await?,
                        None => false,
                    };
                    if !taking_in {
                        arrived.entry(title.work_id).or_default().push(request.id);
                    }
                }
            }
        }
    }
    if arrived.is_empty() {
        return Ok(());
    }

    let now = melyxar_core::time::now();
    let mut added = Vec::new();
    for (work, requests) in arrived {
        added.extend(database.requests_arrived(&requests, work, now).await?);
    }
    moved(state);
    for request in &added {
        telling::decided(state, request).await;
    }
    Ok(())
}

/// Makes the round for as long as the server runs.
pub fn keep_watching_for_arrivals(state: &AppState) -> tokio::task::JoinHandle<()> {
    let state = state.clone();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(EVERY).await;
            if let Err(error) = settle_arrivals(&state).await {
                tracing::warn!(%error, "the titles asked for could not be looked for");
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use melyxar_core::library::LibraryKind;
    use melyxar_core::work::WorkKind;

    use super::super::asking::{ask, mine, Asking};
    use super::super::testing::{a_film, a_series, held, kept_away, requests_on, StandIn};
    use super::*;
    use crate::notifications::live::{follow, Change};

    #[tokio::test]
    async fn a_title_arrives_for_somebody_only_where_they_can_see_it() {
        let (_held, state, _viewer) = requests_on().await;
        let kept = kept_away(&state).await;
        let provider = Arc::new(StandIn {
            films: vec![a_film("5", "Paper Moons")],
            series: Vec::new(),
        });
        let asking = Asking {
            catalogue: Catalogue::Films,
            tmdb_id: "5".to_string(),
            seasons: Vec::new(),
            note: String::new(),
        };
        ask(&state, &provider, &kept.kept_from, asking, "en").await.expect("asked");

        settle_arrivals(&state).await.expect("looked");
        assert_eq!(
            mine(&state, &kept.kept_from).await.expect("read")[0].state,
            "pending",
            "held where they cannot look, it has not arrived for them"
        );

        let theirs = held(&state, kept.open, WorkKind::Movie, "5").await;
        settle_arrivals(&state).await.expect("looked");
        let request = &mine(&state, &kept.kept_from).await.expect("read")[0];
        assert_eq!(request.state, "added");
        assert_eq!(request.work_id, Some(theirs), "the copy they can open");
    }

    #[tokio::test]
    async fn a_request_is_added_once_every_season_it_asked_for_is_here() {
        let (_held, state, viewer) = requests_on().await;
        let database = state.database();
        let provider = Arc::new(StandIn {
            films: Vec::new(),
            series: vec![a_series("7", "Salt Road", &[(1, 8), (2, 10)])],
        });
        let asking = Asking {
            catalogue: Catalogue::Series,
            tmdb_id: "7".to_string(),
            seasons: vec![2],
            note: String::new(),
        };
        ask(&state, &provider, &viewer, asking, "en").await.expect("asked");

        let library = database
            .create_library("Series", LibraryKind::Series, "en", &[])
            .await
            .expect("library");
        let series = database
            .create_work(library.id, WorkKind::Series, "Salt Road", "salt road", None)
            .await
            .expect("series");
        database.set_work_external_id(series.id, "tmdb", "7").await.expect("named");
        sqlx::query("UPDATE works SET identification = 'identified' WHERE id = ?")
            .bind(series.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("identified");
        let place_season = |number: i32| {
            let database = database.clone();
            async move {
                let season = database
                    .create_child_work(library.id, series.id, number, WorkKind::Season, "Season", "season")
                    .await
                    .expect("season");
                database
                    .create_child_work(library.id, season.id, 1, WorkKind::Episode, "Episode", "episode")
                    .await
                    .expect("episode");
            }
        };

        place_season(1).await;
        settle_arrivals(&state).await.expect("looked");
        assert_eq!(mine(&state, &viewer).await.expect("read")[0].state, "pending");

        place_season(2).await;
        let reading = database
            .create_job(
                melyxar_core::job::JobKind::ReadKeyFrames,
                melyxar_core::job::JobPriority::BACKGROUND,
                Some(&library.id.to_string()),
            )
            .await
            .expect("job");
        settle_arrivals(&state).await.expect("looked");
        assert_eq!(
            mine(&state, &viewer).await.expect("read")[0].state,
            "pending",
            "the title is still being taken in"
        );
        database
            .finish_job(reading.id, melyxar_core::job::JobState::Succeeded, None)
            .await
            .expect("finished");
        let mut line = follow(&state);
        settle_arrivals(&state).await.expect("looked");
        let request = &mine(&state, &viewer).await.expect("read")[0];
        assert_eq!(request.state, "added");
        assert_eq!(request.work_id, Some(series.id));
        let told = line.try_recv().expect("told");
        assert_eq!(told.user, viewer.id);
        let Change::Arrived { told, .. } = told.change else {
            panic!("a notification arrives");
        };
        assert_eq!(told.notification.work_id, Some(series.id), "it opens the series");

        settle_arrivals(&state).await.expect("looked");
        assert!(line.try_recv().is_err(), "told once");
    }
}
