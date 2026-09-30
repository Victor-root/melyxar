//! Deezer, for the photos of artists: free, with no account and no key, and
//! the one source of them that asks for neither (see
//! `docs/architecture/06-musique.md`). Only an artist whose name it knows
//! exactly is taken, and only when it holds a photo of them.

use std::time::Duration;

use serde::Deserialize;

use crate::provider::{ProviderError, Result};

const SEARCH_URL: &str = "https://api.deezer.com/search/artist";

/// Far enough apart to stay well under the fifty questions in five seconds
/// Deezer allows.
pub const BETWEEN_QUESTIONS: Duration = Duration::from_millis(250);

/// Long enough for a slow answer, short enough that a job moves on.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// Where a photo is kept, an artist without one having an empty place there.
const PHOTOS: &str = "/images/artist/";

pub struct DeezerClient {
    client: reqwest::Client,
}

impl DeezerClient {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .user_agent(concat!(
                "Melyxar/",
                env!("CARGO_PKG_VERSION"),
                " (self-hosted media server)"
            ))
            .build()
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        Ok(Self { client })
    }

    /// Where the photo of an artist of this name is, when Deezer has one.
    pub async fn artist_photo(&self, name: &str) -> Result<Option<String>> {
        let response = self
            .client
            .get(SEARCH_URL)
            .query(&[("q", name), ("limit", "10")])
            .send()
            .await
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        let status = response.status().as_u16();
        let body = response
            .text()
            .await
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        match status {
            200 => photo_in(&body, name),
            429 => Err(ProviderError::TooManyRequests {
                retry_after_seconds: None,
            }),
            500..=599 => Err(ProviderError::Unreachable(format!(
                "Deezer answered {status}"
            ))),
            _ => Err(ProviderError::Unexpected(format!(
                "Deezer answered {status}"
            ))),
        }
    }

    pub async fn photo(&self, url: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        match response.status().as_u16() {
            200 => Ok(response
                .bytes()
                .await
                .map_err(|error| ProviderError::Unreachable(error.to_string()))?
                .to_vec()),
            status @ 500..=599 => Err(ProviderError::Unreachable(format!(
                "Deezer answered {status}"
            ))),
            status => Err(ProviderError::Unexpected(format!(
                "Deezer answered {status}"
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Search {
    #[serde(default)]
    data: Vec<Found>,
    /// Deezer answers its refusals with a success and this in the body.
    error: Option<Refusal>,
}

#[derive(Debug, Deserialize)]
struct Refusal {
    #[serde(default)]
    code: u32,
    #[serde(default)]
    message: String,
}

#[derive(Debug, Deserialize)]
struct Found {
    name: String,
    picture_xl: Option<String>,
}

/// Deezer's answer to asking too often.
const QUOTA_EXCEEDED: u32 = 4;

/// The photo of the first artist of a search named exactly as asked, letter
/// case and surrounding spaces aside, when it has one: a name that only
/// looks like it would be somebody else's face.
fn photo_in(body: &str, name: &str) -> Result<Option<String>> {
    let search: Search =
        serde_json::from_str(body).map_err(|error| ProviderError::Unexpected(error.to_string()))?;
    if let Some(refusal) = search.error {
        return Err(if refusal.code == QUOTA_EXCEEDED {
            ProviderError::TooManyRequests {
                retry_after_seconds: None,
            }
        } else {
            ProviderError::Unexpected(refusal.message)
        });
    }
    let asked = name.trim().to_lowercase();
    Ok(search
        .data
        .into_iter()
        .find(|found| found.name.trim().to_lowercase() == asked)
        .and_then(|found| found.picture_xl)
        .filter(|url| has_a_photo(url)))
}

/// Whether a photo address names a photo, and not the empty place of an
/// artist who has none.
fn has_a_photo(url: &str) -> bool {
    url.split_once(PHOTOS)
        .is_some_and(|(_, rest)| !rest.starts_with('/'))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH: &str = r#"{"data":[
        {"id":1,"name":"The Lanterns - Live","picture_xl":"https://cdn.example/images/artist/aaaa/1000x1000.jpg"},
        {"id":2,"name":"the lanterns","picture_xl":"https://cdn.example/images/artist/bbbb/1000x1000.jpg"},
        {"id":3,"name":"The Lanterns","picture_xl":"https://cdn.example/images/artist/cccc/1000x1000.jpg"}
    ],"total":3}"#;

    #[test]
    fn only_an_artist_named_exactly_as_asked_is_taken() {
        assert_eq!(
            photo_in(SEARCH, " The Lanterns ").expect("read"),
            Some("https://cdn.example/images/artist/bbbb/1000x1000.jpg".to_string())
        );
        assert_eq!(photo_in(SEARCH, "Lanterns").expect("read"), None);
        assert_eq!(
            photo_in(r#"{"data":[],"total":0}"#, "Anyone").expect("read"),
            None
        );
        assert!(photo_in("not json", "Anyone").is_err());
    }

    #[test]
    fn an_artist_without_a_photo_is_not_given_the_empty_one() {
        let without = r#"{"data":[{"name":"Amber Field","picture_xl":"https://cdn.example/images/artist//1000x1000.jpg"}]}"#;
        assert_eq!(photo_in(without, "Amber Field").expect("read"), None);
    }

    #[test]
    fn asking_too_often_is_worth_trying_again_later() {
        let refused = r#"{"error":{"type":"Exception","message":"Quota limit exceeded","code":4}}"#;
        assert!(matches!(
            photo_in(refused, "Anyone"),
            Err(ProviderError::TooManyRequests { .. })
        ));
    }
}
