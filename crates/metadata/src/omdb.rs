//! OMDb, asked with the administrator's own key for what only it relays:
//! the share of critics Rotten Tomatoes gathers who liked a work.
//!
//! Everything else it holds is written in English only, and the texts of a
//! work stay those of its provider, in the language of its library.

use std::time::Duration;

use serde::Deserialize;

use crate::provider::{ProviderError, Result};

const BASE_URL: &str = "https://www.omdbapi.com/";

/// How long a single request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// The name Rotten Tomatoes goes by among the ratings OMDb relays.
const ROTTEN_TOMATOES: &str = "Rotten Tomatoes";

pub struct OmdbClient {
    client: reqwest::Client,
    key: String,
    base_url: String,
}

impl OmdbClient {
    /// Builds the client. Fails only if no client can be built at all.
    pub fn new(key: impl Into<String>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!("Melyxar/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        Ok(Self {
            client,
            key: key.into(),
            base_url: BASE_URL.to_string(),
        })
    }

    /// The share of critics who liked the work with this IMDb identifier, out
    /// of a hundred, or nothing when Rotten Tomatoes has no say on it.
    pub async fn critics_score(&self, imdb_id: &str) -> Result<Option<u8>> {
        let response = self
            .client
            .get(&self.base_url)
            .query(&[("apikey", self.key.as_str()), ("i", imdb_id)])
            .send()
            .await
            // The address never reaches the log: it carries the key.
            .map_err(|error| ProviderError::Unreachable(error.without_url().to_string()))?;
        let status = response.status().as_u16();
        if (500..600).contains(&status) {
            return Err(ProviderError::Unreachable(format!("OMDb answered {status}")));
        }
        // A refusal comes back with a sentence saying which, whatever the
        // status that goes with it.
        let answer: Answer = response
            .json()
            .await
            .map_err(|error| ProviderError::Unexpected(error.without_url().to_string()))?;
        answer.critics_score()
    }
}

#[derive(Debug, Deserialize)]
struct Answer {
    #[serde(rename = "Response", default)]
    response: String,
    #[serde(rename = "Error", default)]
    error: Option<String>,
    #[serde(rename = "Ratings", default)]
    ratings: Vec<RelayedRating>,
}

#[derive(Debug, Deserialize)]
struct RelayedRating {
    #[serde(rename = "Source")]
    source: String,
    #[serde(rename = "Value")]
    value: String,
}

impl Answer {
    fn critics_score(self) -> Result<Option<u8>> {
        if self.response != "True" {
            let error = self.error.unwrap_or_default();
            let said = error.to_lowercase();
            return if said.contains("api key") {
                Err(ProviderError::Unauthorised)
            } else if said.contains("limit") {
                Err(ProviderError::TooManyRequests {
                    retry_after_seconds: None,
                })
            } else {
                // Not found, or a title OMDb cannot describe: no rating.
                Ok(None)
            };
        }
        Ok(self
            .ratings
            .into_iter()
            .find(|rating| rating.source == ROTTEN_TOMATOES)
            .and_then(|rating| rating.value.strip_suffix('%')?.trim().parse::<u8>().ok())
            .filter(|share| *share <= 100))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(json: &str) -> Result<Option<u8>> {
        serde_json::from_str::<Answer>(json)
            .expect("read")
            .critics_score()
    }

    #[test]
    fn the_share_of_critics_is_read_among_the_ratings_relayed() {
        assert_eq!(
            read(
                r#"{"Response": "True", "Ratings": [
                    {"Source": "Internet Movie Database", "Value": "8.7/10"},
                    {"Source": "Rotten Tomatoes", "Value": "83%"},
                    {"Source": "Metacritic", "Value": "73/100"}]}"#
            )
            .expect("answered"),
            Some(83)
        );
        assert_eq!(
            read(r#"{"Response": "True", "Ratings": [{"Source": "Metacritic", "Value": "73/100"}]}"#)
                .expect("answered"),
            None,
            "a work the critics did not gather on has no share"
        );
        assert_eq!(
            read(r#"{"Response": "True", "Ratings": [{"Source": "Rotten Tomatoes", "Value": "N/A"}]}"#)
                .expect("answered"),
            None
        );
    }

    #[test]
    fn a_refused_key_and_a_spent_allowance_are_told_apart_from_a_title_unknown() {
        assert!(matches!(
            read(r#"{"Response": "False", "Error": "Invalid API key!"}"#),
            Err(ProviderError::Unauthorised)
        ));
        assert!(matches!(
            read(r#"{"Response": "False", "Error": "Request limit reached!"}"#),
            Err(ProviderError::TooManyRequests { .. })
        ));
        assert_eq!(
            read(r#"{"Response": "False", "Error": "Incorrect IMDb ID."}"#).expect("answered"),
            None
        );
    }
}
