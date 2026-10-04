//! What the administrator sees of the requests and decides about them: the
//! open ones gathered by title, and one decision for every account waiting
//! on the same title.

use melyxar_database::requests::{Decision, TitleRequest};

use super::live::moved;
use super::{telling, word_of, Catalogue, Result, Trouble};
use crate::AppState;

/// The longest the word given with a refusal may be, in characters.
const LONGEST_ANSWER: usize = 500;

/// One title asked for, and every open request for it, oldest first.
#[derive(Debug, Clone, PartialEq)]
pub struct Waiting {
    pub catalogue: Catalogue,
    pub tmdb_id: String,
    pub requests: Vec<TitleRequest>,
}

impl Waiting {
    /// Whether the administrator accepted it already.
    pub fn accepted(&self) -> bool {
        self.requests.iter().any(|request| request.state == "accepted")
    }
}

/// The open requests, gathered by title, the title asked for first first.
pub async fn waiting(state: &AppState) -> Result<Vec<Waiting>> {
    let mut gathered: Vec<Waiting> = Vec::new();
    for request in state.database().open_requests().await? {
        let Some(catalogue) = super::catalogue_of(&request.catalogue) else {
            continue;
        };
        match gathered
            .iter_mut()
            .find(|one| one.catalogue == catalogue && one.tmdb_id == request.tmdb_id)
        {
            Some(one) => one.requests.push(request),
            None => gathered.push(Waiting {
                catalogue,
                tmdb_id: request.tmdb_id.clone(),
                requests: vec![request],
            }),
        }
    }
    Ok(gathered)
}

/// Decides every open request for a title, and tells each account that
/// asked. The answer is kept with a refusal only.
pub async fn decide(
    state: &AppState,
    catalogue: Catalogue,
    tmdb_id: &str,
    decision: Decision,
    answer: &str,
) -> Result<Vec<TitleRequest>> {
    let answer = match decision {
        Decision::Refused => answer.trim(),
        _ => "",
    };
    if answer.chars().count() > LONGEST_ANSWER {
        return Err(Trouble::Refused(super::Refused::NoteTooLong));
    }
    let decided = state
        .database()
        .decide_requests(word_of(catalogue), tmdb_id, decision, answer, melyxar_core::time::now())
        .await?;
    if decided.is_empty() {
        return Err(Trouble::Failed(melyxar_core::Error::not_found("request").into()));
    }
    moved(state);
    for request in &decided {
        telling::decided(state, request).await;
    }
    Ok(decided)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use melyxar_core::user::Permissions;

    use super::super::asking::{ask, Asking};
    use super::super::testing::{a_film, requests_on, StandIn};
    use super::*;
    use crate::notifications::live::{follow, Change};

    #[tokio::test]
    async fn one_decision_reaches_every_account_waiting_on_the_title() {
        let (_held, state, viewer) = requests_on().await;
        let database = state.database();
        let other = database
            .create_user("other", None, &Permissions::viewer())
            .await
            .expect("account");
        database.set_request_right(other.id, true).await.expect("allowed");
        let provider = Arc::new(StandIn {
            films: vec![a_film("1", "Amber Field"), a_film("2", "Salt Road")],
            series: Vec::new(),
        });
        for (who, id) in [(&viewer, "1"), (&other, "1"), (&other, "2")] {
            let asking = Asking {
                catalogue: Catalogue::Films,
                tmdb_id: id.to_string(),
                seasons: Vec::new(),
                note: String::new(),
            };
            ask(&state, &provider, who, asking, "en").await.expect("asked");
        }

        let gathered = waiting(&state).await.expect("read");
        assert_eq!(gathered.len(), 2);
        assert_eq!(gathered[0].tmdb_id, "1");
        assert_eq!(gathered[0].requests.len(), 2);
        assert!(!gathered[0].accepted());

        let mut line = follow(&state);
        let refused = decide(&state, Catalogue::Films, "1", Decision::Refused, " Not to be found ")
            .await
            .expect("decided");
        assert_eq!(refused.len(), 2);
        let mut told = Vec::new();
        while let Ok(news) = line.try_recv() {
            if let Change::Arrived { told: arrived, .. } = news.change {
                assert!(arrived.notification.data.contains("Not to be found"));
                told.push(news.user);
            }
        }
        told.sort();
        let mut expected = vec![viewer.id, other.id];
        expected.sort();
        assert_eq!(told, expected);

        assert_eq!(waiting(&state).await.expect("read").len(), 1);
        assert!(matches!(
            decide(&state, Catalogue::Films, "1", Decision::Accepted, "").await,
            Err(Trouble::Failed(_))
        ));
        let accepted = decide(&state, Catalogue::Films, "2", Decision::Accepted, "ignored")
            .await
            .expect("decided");
        assert_eq!(accepted[0].answer, "");
        assert!(waiting(&state).await.expect("read")[0].accepted());
    }
}
