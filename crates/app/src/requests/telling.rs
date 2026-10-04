//! What a request tells, through the notifications: the administrators a
//! new one, the account that made it what became of it. A notification
//! that could not be sent leaves the request as it is, and is said in the
//! technical journal.

use melyxar_core::id::{UserId, WorkId};
use melyxar_database::requests::TitleRequest;

use crate::notifications::{send, Audience, Level, Outgoing, RequestNews, Said};
use crate::AppState;

async fn tell(state: &AppState, title: &str, news: RequestNews, level: Level, to: Vec<UserId>, work: Option<WorkId>) {
    let said = Said::Request {
        title: title.to_string(),
        news,
    };
    let outgoing = Outgoing {
        work,
        ..Outgoing::new(said, level, Audience::Accounts(to))
    };
    if let Err(error) = send(state, outgoing).await {
        tracing::warn!(%error, title, "a request could not be told");
    }
}

/// Tells the administrators an account asked for a title, the one who asked
/// included when it is one: it may be the only administrator.
pub(super) async fn asked(state: &AppState, request: &TitleRequest) {
    let administrators = match state.database().list_users().await {
        Ok(users) => users
            .into_iter()
            .filter(|user| user.permissions.is_administrator)
            .map(|user| user.id)
            .collect(),
        Err(error) => {
            tracing::warn!(%error, "the administrators could not be told of a request");
            return;
        }
    };
    let news = RequestNews::Asked {
        by: request.user_name.clone(),
        seasons: request.seasons.clone(),
    };
    tell(state, &request.title, news, Level::News, administrators, None).await;
}

/// Tells the account that asked what became of its request.
pub(super) async fn decided(state: &AppState, request: &TitleRequest) {
    let (news, level) = match request.state.as_str() {
        "accepted" => (RequestNews::Accepted, Level::Ok),
        "refused" => (
            RequestNews::Refused {
                answer: request.answer.clone(),
            },
            Level::Attention,
        ),
        _ => (RequestNews::Added, Level::News),
    };
    tell(state, &request.title, news, level, vec![request.user_id], request.work_id).await;
}
