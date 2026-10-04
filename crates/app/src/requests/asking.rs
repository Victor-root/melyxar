//! An account asking for a title, seeing what it asked for, and taking a
//! request back while it is not accepted.

use std::sync::Arc;

use melyxar_core::id::RequestId;
use melyxar_core::user::User;
use melyxar_database::requests::{NewRequest, TitleRequest};
use melyxar_metadata::MetadataProvider;

use super::live::moved;
use super::rules::may_be_asked;
use super::{access, provider_language, telling, word_of, Catalogue, Refused, Result, Trouble};
use crate::AppState;

/// The longest a note added to a request may be, in characters.
const LONGEST_NOTE: usize = 500;

/// What an account asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Asking {
    pub catalogue: Catalogue,
    pub tmdb_id: String,
    /// For a series, the seasons wanted; none for the whole of it.
    pub seasons: Vec<i32>,
    pub note: String,
}

/// Asks for a title, described as the provider describes it in the
/// account's language, and tells the administrators.
pub async fn ask<P: MetadataProvider>(
    state: &AppState,
    provider: &Arc<P>,
    who: &User,
    mut asking: Asking,
    language: &str,
) -> Result<TitleRequest> {
    access::require(state, who).await?;
    let note = asking.note.trim();
    if note.chars().count() > LONGEST_NOTE {
        return Err(Trouble::Refused(Refused::NoteTooLong));
    }
    asking.seasons.sort_unstable();
    asking.seasons.dedup();

    let details = provider
        .details(asking.catalogue, &asking.tmdb_id, provider_language(language))
        .await?;
    let known: Vec<i32> = details.season_lengths.iter().map(|season| season.season).collect();
    let catalogue = word_of(asking.catalogue);
    let held = state
        .database()
        .held_titles(catalogue, std::slice::from_ref(&details.external_id))
        .await?;
    may_be_asked(
        asking.catalogue,
        &asking.seasons,
        &known,
        held.get(&details.external_id),
    )
    .map_err(Trouble::Refused)?;

    let written = state
        .database()
        .add_request(&NewRequest {
            user_id: who.id,
            catalogue,
            tmdb_id: &details.external_id,
            title: &details.title,
            year: details.release_year,
            poster_path: details.poster_path.as_deref(),
            overview: details.overview.as_deref(),
            seasons: &asking.seasons,
            note,
            created_at: melyxar_core::time::now(),
        })
        .await;
    let request = match written {
        Err(error) if error.is_a_duplicate() => return Err(Trouble::Refused(Refused::AlreadyAsked)),
        other => other?,
    };
    moved(state);
    telling::asked(state, &request, telling::poster_of(provider.as_ref(), &request)).await;
    Ok(request)
}

/// An account's requests, newest first.
pub async fn mine(state: &AppState, who: &User) -> Result<Vec<TitleRequest>> {
    access::require(state, who).await?;
    Ok(state.database().requests_of(who.id).await?)
}

/// Takes back one of an account's requests: cancelled while it waits,
/// cleared from its list once refused or added. An accepted one stays.
pub async fn withdraw(state: &AppState, who: &User, id: RequestId) -> Result<()> {
    access::require(state, who).await?;
    let database = state.database();
    if database.withdraw_request(who.id, id).await? {
        moved(state);
        return Ok(());
    }
    match database.request(id).await? {
        Some(request) if request.user_id == who.id => Err(Trouble::Refused(Refused::Accepted)),
        _ => Err(Trouble::Failed(melyxar_core::Error::not_found("request").into())),
    }
}

#[cfg(test)]
mod tests {
    use melyxar_core::user::Permissions;

    use super::super::testing::{a_film, a_series, requests_on, StandIn};
    use super::*;
    use crate::notifications::live::{follow, Change};

    fn provider() -> Arc<StandIn> {
        Arc::new(StandIn {
            films: vec![a_film("1", "Amber Field")],
            series: vec![a_series("7", "Salt Road", &[(1, 8), (2, 10)])],
        })
    }

    fn asking(catalogue: Catalogue, id: &str, seasons: &[i32]) -> Asking {
        Asking {
            catalogue,
            tmdb_id: id.to_string(),
            seasons: seasons.to_vec(),
            note: "  in French  ".to_string(),
        }
    }

    #[tokio::test]
    async fn a_request_is_described_by_the_provider_and_told_to_the_administrators() {
        let (_held, state, viewer) = requests_on().await;
        let admin = state
            .database()
            .create_user("admin", None, &Permissions::administrator())
            .await
            .expect("account");
        let mut line = follow(&state);
        let moved = super::super::live::follow(&state);

        let request = ask(&state, &provider(), &viewer, asking(Catalogue::Films, "1", &[]), "fr")
            .await
            .expect("asked");
        assert_eq!(request.title, "Amber Field");
        assert_eq!(request.note, "in French");
        assert_eq!(request.poster_path.as_deref(), Some("/1.jpg"));
        assert!(moved.has_changed().expect("open"));

        let told = line.try_recv().expect("told");
        assert_eq!(told.user, admin.id);
        let Change::Arrived { told, .. } = told.change else {
            panic!("a notification arrives");
        };
        assert_eq!(told.notification.kind, "request");
        assert!(told.notification.data.contains("\"by\":\"viewer\""));
        assert!(told.notification.data.contains("All about Amber Field."), "the synopsis comes along");
        assert!(told.notification.data.contains("https://pictures.invalid/1.jpg"), "and the poster");

        assert!(matches!(
            ask(&state, &provider(), &viewer, asking(Catalogue::Films, "1", &[]), "fr").await,
            Err(Trouble::Refused(Refused::AlreadyAsked))
        ));
        assert_eq!(mine(&state, &viewer).await.expect("read").len(), 1);
    }

    #[tokio::test]
    async fn an_administrator_asking_is_told_of_it_like_the_others() {
        let (_held, state, _) = requests_on().await;
        let admin = state
            .database()
            .create_user("admin", None, &Permissions::administrator())
            .await
            .expect("account");
        let mut line = follow(&state);
        ask(&state, &provider(), &admin, asking(Catalogue::Films, "1", &[]), "fr")
            .await
            .expect("asked");
        let told = line.try_recv().expect("told");
        assert_eq!(told.user, admin.id);
    }

    #[tokio::test]
    async fn seasons_are_asked_of_a_series_only_and_among_those_it_has() {
        let (_held, state, viewer) = requests_on().await;
        let request = ask(&state, &provider(), &viewer, asking(Catalogue::Series, "7", &[2, 1, 2]), "fr")
            .await
            .expect("asked");
        assert_eq!(request.seasons, vec![1, 2]);
        assert!(matches!(
            ask(&state, &provider(), &viewer, asking(Catalogue::Films, "1", &[1]), "fr").await,
            Err(Trouble::Refused(Refused::NoSuchSeason))
        ));
        let too_long = Asking {
            note: "a".repeat(LONGEST_NOTE + 1),
            ..asking(Catalogue::Films, "1", &[])
        };
        assert!(matches!(
            ask(&state, &provider(), &viewer, too_long, "fr").await,
            Err(Trouble::Refused(Refused::NoteTooLong))
        ));
    }

    #[tokio::test]
    async fn an_account_without_the_right_may_not_ask_and_a_request_is_withdrawn_by_its_own() {
        let (_held, state, viewer) = requests_on().await;
        let other = state
            .database()
            .create_user("other", None, &Permissions::viewer())
            .await
            .expect("account");
        assert!(matches!(
            ask(&state, &provider(), &other, asking(Catalogue::Films, "1", &[]), "fr").await,
            Err(Trouble::Refused(Refused::NotAllowed))
        ));

        let request = ask(&state, &provider(), &viewer, asking(Catalogue::Films, "1", &[]), "fr")
            .await
            .expect("asked");
        state.database().set_request_right(other.id, true).await.expect("allowed");
        assert!(matches!(withdraw(&state, &other, request.id).await, Err(Trouble::Failed(_))));

        state
            .database()
            .decide_requests("films", "1", melyxar_database::requests::Decision::Accepted, "", melyxar_core::time::now())
            .await
            .expect("accepted");
        assert!(matches!(
            withdraw(&state, &viewer, request.id).await,
            Err(Trouble::Refused(Refused::Accepted))
        ));
    }
}
