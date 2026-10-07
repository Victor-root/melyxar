//! The whole page of one title the server may not hold: everything the
//! provider says of it, with where it stands here, so it can be read through
//! like a title the server does have before it is asked for.

use std::sync::Arc;

use melyxar_core::user::User;
use melyxar_metadata::{Details, MetadataProvider};

use super::search::{found_from, standing_of, Found, SeasonChoice};
use super::{access, provider_language, Catalogue, Result};
use crate::AppState;

/// One title, as the provider describes it and as this server stands to it.
#[derive(Debug, Clone, PartialEq)]
pub struct Described {
    pub found: Found,
    pub details: Details,
    /// For a series, every season the provider knows and which are here.
    pub seasons: Vec<SeasonChoice>,
}

pub async fn describe<P: MetadataProvider>(
    state: &AppState,
    provider: &Arc<P>,
    who: &User,
    catalogue: Catalogue,
    tmdb_id: &str,
    language: &str,
) -> Result<Described> {
    access::require_a_look(state, who).await?;
    let details = provider
        .details(catalogue, tmdb_id, provider_language(language))
        .await?;
    let candidate = details.as_candidate(catalogue);
    let standing = standing_of(state, who, std::slice::from_ref(&candidate)).await?;
    let found = found_from(candidate, &standing, who, provider.as_ref());
    let held = found.held.as_ref().map(|held| held.seasons.as_slice()).unwrap_or_default();
    let seasons = details
        .season_lengths
        .iter()
        .map(|season| SeasonChoice {
            number: season.season,
            episodes: season.episodes,
            held: held.contains(&season.season),
        })
        .collect();
    Ok(Described {
        found,
        details,
        seasons,
    })
}

#[cfg(test)]
mod tests {
    use super::super::testing::{a_film, a_series, requests_on, StandIn};
    use super::*;

    #[tokio::test]
    async fn a_title_comes_with_what_the_provider_says_and_where_it_stands() {
        let (_held, state, viewer) = requests_on().await;
        let provider = Arc::new(StandIn {
            films: vec![a_film("1", "Amber Field")],
            series: vec![a_series("7", "Salt Road", &[(1, 8), (2, 10)])],
        });

        let film = describe(&state, &provider, &viewer, Catalogue::Films, "1", "fr")
            .await
            .expect("described");
        assert_eq!(film.details.title, "Amber Field");
        assert_eq!(film.found.asked_by, 0);
        assert!(film.found.held.is_none());
        assert!(film.seasons.is_empty());

        let series = describe(&state, &provider, &viewer, Catalogue::Series, "7", "fr")
            .await
            .expect("described");
        assert_eq!(series.seasons.len(), 2);
        assert!(series.seasons.iter().all(|season| !season.held));

        assert!(describe(&state, &provider, &viewer, Catalogue::Films, "404", "fr").await.is_err());
    }
}
