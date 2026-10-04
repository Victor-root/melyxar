//! What a request tells, through the notifications: the administrators a
//! new one, the account that made it what became of it. A notification
//! that could not be sent leaves the request as it is, and is said in the
//! technical journal.

use melyxar_core::id::UserId;
use melyxar_metadata::MetadataProvider;
use melyxar_database::requests::TitleRequest;

use crate::notifications::{send, Audience, Level, Outgoing, RequestNews, Said};
use crate::AppState;

/// How wide the poster told in a notification is asked for, in pixels.
const POSTER_WIDTH: u32 = 154;

/// Where a small copy of the poster of a request is, at the provider.
pub(super) fn poster_of<P: MetadataProvider>(provider: &P, request: &TitleRequest) -> Option<String> {
    request
        .poster_path
        .as_deref()
        .map(|path| provider.image_url_at(path, POSTER_WIDTH))
}

/// A request told: the title as the account asked for it, with its year, its
/// synopsis and its poster, to those it concerns.
async fn tell(
    state: &AppState,
    request: &TitleRequest,
    poster: Option<String>,
    news: RequestNews,
    level: Level,
    to: Vec<UserId>,
) {
    let said = Said::Request {
        title: request.title.clone(),
        year: request.year,
        overview: request.overview.as_deref().filter(|text| !text.is_empty()).map(cut_short),
        poster,
        news,
    };
    let outgoing = Outgoing {
        work: request.work_id,
        ..Outgoing::new(said, level, Audience::Accounts(to))
    };
    if let Err(error) = send(state, outgoing).await {
        tracing::warn!(%error, title = request.title, "a request could not be told");
    }
}

/// The most characters of a synopsis a notification carries.
const OVERVIEW_AT_MOST: usize = 280;

/// A synopsis cut on a word, the dots saying there was more.
fn cut_short(text: &str) -> String {
    if text.chars().count() <= OVERVIEW_AT_MOST {
        return text.to_string();
    }
    let kept: String = text.chars().take(OVERVIEW_AT_MOST).collect();
    let on_a_word = kept.rsplit_once(' ').map_or(kept.as_str(), |(before, _)| before);
    format!("{}…", on_a_word.trim_end_matches([',', ';', ':', '.', ' ']))
}

/// Tells the administrators an account asked for a title, the one who asked
/// included when it is one: it may be the only administrator.
pub(super) async fn asked(state: &AppState, request: &TitleRequest, poster: Option<String>) {
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
    tell(state, request, poster, news, Level::News, administrators).await;
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
    let poster = state
        .metadata_provider()
        .and_then(|provider| poster_of(provider.as_ref(), request));
    tell(state, request, poster, news, level, vec![request.user_id]).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_synopsis_is_cut_on_a_word_and_a_short_one_is_left() {
        assert_eq!(cut_short("A keeper and a lantern."), "A keeper and a lantern.");
        let long = "word ".repeat(100);
        let cut = cut_short(&long);
        assert!(cut.ends_with("word…"), "{cut}");
        assert!(cut.chars().count() <= OVERVIEW_AT_MOST + 1);
    }
}
