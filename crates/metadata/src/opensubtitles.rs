//! OpenSubtitles, asked with the administrator's own application key for the
//! subtitles of a film or an episode, and for one of them to download.
//!
//! A work is looked up by what TMDb calls it, never by the name of its file:
//! a title close to another one is how a subtitle of the wrong film arrives.
//! Downloads are counted per day; signed in with the administrator's account
//! the day allows more, and OpenSubtitles says how many are left with each.

use std::time::Duration;

use serde::Deserialize;
use tokio::sync::Mutex;

use crate::provider::{ProviderError, Result};

const BASE_URL: &str = "https://api.opensubtitles.com/api/v1";

/// How long a single request may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);

/// A film every catalogue knows, which a key alone is tried on.
const A_FILM_EVERYBODY_KNOWS: &str = "603";

/// Refuses a subtitle beyond any plausible size before reading it all.
const LARGEST_SUBTITLE: usize = 5 * 1024 * 1024;

/// What a search is about: a film, or an episode of a series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Searched<'a> {
    Film { tmdb_id: &'a str },
    Episode { series_tmdb_id: &'a str, season: i32, episode: i32 },
}

/// One subtitle OpenSubtitles offers.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleOffer {
    /// What its file is asked for by.
    pub file_id: i64,
    /// The language, as OpenSubtitles writes it: two letters, sometimes with
    /// a region.
    pub language: String,
    /// The release it was timed on, which is how one is told from another.
    pub release: String,
    pub downloads: i64,
    pub hearing_impaired: bool,
    /// Written by a program rather than by somebody.
    pub machine_translated: bool,
    /// Uploaded by somebody OpenSubtitles trusts.
    pub trusted: bool,
}

/// A subtitle downloaded, and how many more the day allows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Downloaded {
    pub contents: Vec<u8>,
    pub remaining: Option<i64>,
}

pub struct OpenSubtitlesClient {
    client: reqwest::Client,
    key: String,
    login: Option<(String, String)>,
    /// The token the account signed in with, once it has.
    token: Mutex<Option<String>>,
    base_url: String,
}

impl OpenSubtitlesClient {
    /// Builds the client. Fails only if no client can be built at all.
    pub fn new(key: impl Into<String>, login: Option<(String, String)>) -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            // OpenSubtitles refuses a request that does not say which
            // program it comes from, with its version.
            .user_agent(concat!("Melyxar v", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
        Ok(Self {
            client,
            key: key.into(),
            login,
            token: Mutex::new(None),
            base_url: BASE_URL.to_string(),
        })
    }

    /// Whether the key, and the account when there is one, are taken.
    pub async fn check(&self) -> Result<()> {
        if self.login.is_some() {
            self.signed_in().await.map(|_| ())
        } else {
            self.search(Searched::Film { tmdb_id: A_FILM_EVERYBODY_KNOWS }, &["en".to_string()])
                .await
                .map(|_| ())
        }
    }

    /// The subtitles offered for a work in these languages, the most
    /// downloaded first.
    pub async fn search(&self, searched: Searched<'_>, languages: &[String]) -> Result<Vec<SubtitleOffer>> {
        let mut query: Vec<(&str, String)> = vec![("languages", languages.join(","))];
        match searched {
            Searched::Film { tmdb_id } => {
                query.push(("tmdb_id", tmdb_id.to_string()));
                query.push(("type", "movie".to_string()));
            }
            Searched::Episode { series_tmdb_id, season, episode } => {
                query.push(("parent_tmdb_id", series_tmdb_id.to_string()));
                query.push(("season_number", season.to_string()));
                query.push(("episode_number", episode.to_string()));
                query.push(("type", "episode".to_string()));
            }
        }
        let response = self
            .client
            .get(format!("{}/subtitles", self.base_url))
            .header("Api-Key", &self.key)
            .query(&query)
            .send()
            .await
            .map_err(unreachable)?;
        let found: Found = read(response).await?;
        let mut offers = found.offers();
        offers.sort_by(|one, other| other.downloads.cmp(&one.downloads));
        Ok(offers)
    }

    /// Downloads one subtitle offered, signed in when there is an account.
    pub async fn download(&self, file_id: i64) -> Result<Downloaded> {
        let token = match self.login {
            Some(_) => Some(self.signed_in().await?),
            None => None,
        };
        let mut request = self
            .client
            .post(format!("{}/download", self.base_url))
            .header("Api-Key", &self.key)
            .json(&serde_json::json!({ "file_id": file_id }));
        if let Some(token) = &token {
            request = request.bearer_auth(token);
        }
        let link: DownloadLink = read(request.send().await.map_err(unreachable)?).await?;
        let response = self
            .client
            .get(&link.link)
            .send()
            .await
            .map_err(unreachable)?;
        if !response.status().is_success() {
            return Err(ProviderError::Unreachable(format!(
                "the subtitle file answered {}",
                response.status().as_u16()
            )));
        }
        let contents = response.bytes().await.map_err(unreachable)?;
        if contents.len() > LARGEST_SUBTITLE {
            return Err(ProviderError::Unexpected("the subtitle is far too large".to_string()));
        }
        Ok(Downloaded {
            contents: contents.to_vec(),
            remaining: link.remaining,
        })
    }

    /// The token of the account, signing in the first time it is needed.
    async fn signed_in(&self) -> Result<String> {
        let mut held = self.token.lock().await;
        if let Some(token) = held.as_ref() {
            return Ok(token.clone());
        }
        let Some((username, password)) = &self.login else {
            return Err(ProviderError::Unauthorised);
        };
        let response = self
            .client
            .post(format!("{}/login", self.base_url))
            .header("Api-Key", &self.key)
            .json(&serde_json::json!({ "username": username, "password": password }))
            .send()
            .await
            .map_err(unreachable)?;
        let signed: SignedIn = read(response).await?;
        *held = Some(signed.token.clone());
        Ok(signed.token)
    }
}

/// A request that did not reach OpenSubtitles, said without its address.
fn unreachable(error: reqwest::Error) -> ProviderError {
    ProviderError::Unreachable(error.without_url().to_string())
}

/// Reads an answer, telling a refused key or account and a spent day apart
/// from everything else.
async fn read<T: serde::de::DeserializeOwned>(response: reqwest::Response) -> Result<T> {
    match response.status().as_u16() {
        200 => response
            .json::<T>()
            .await
            .map_err(|error| ProviderError::Unexpected(error.without_url().to_string())),
        401 | 403 => Err(ProviderError::Unauthorised),
        // A day's downloads spent, or too many questions at once.
        406 | 429 => Err(ProviderError::TooManyRequests {
            retry_after_seconds: None,
        }),
        status if (500..600).contains(&status) => Err(ProviderError::Unreachable(format!(
            "OpenSubtitles answered {status}"
        ))),
        status => Err(ProviderError::Unexpected(format!("OpenSubtitles answered {status}"))),
    }
}

#[derive(Debug, Deserialize)]
struct SignedIn {
    token: String,
}

#[derive(Debug, Deserialize)]
struct DownloadLink {
    link: String,
    #[serde(default)]
    remaining: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct Found {
    #[serde(default)]
    data: Vec<FoundSubtitle>,
}

#[derive(Debug, Deserialize)]
struct FoundSubtitle {
    attributes: Attributes,
}

#[derive(Debug, Deserialize)]
struct Attributes {
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    release: Option<String>,
    #[serde(default)]
    download_count: i64,
    #[serde(default)]
    hearing_impaired: bool,
    #[serde(default)]
    machine_translated: bool,
    #[serde(default)]
    ai_translated: bool,
    #[serde(default)]
    from_trusted: bool,
    #[serde(default)]
    files: Vec<FoundFile>,
}

#[derive(Debug, Deserialize)]
struct FoundFile {
    file_id: i64,
    #[serde(default)]
    file_name: Option<String>,
}

impl Found {
    /// Every subtitle offered as one file, which is all a track can be. A
    /// subtitle cut into several files, one per disc of an old release, is
    /// left out.
    fn offers(self) -> Vec<SubtitleOffer> {
        self.data
            .into_iter()
            .filter_map(|found| {
                let attributes = found.attributes;
                let [file] = <[FoundFile; 1]>::try_from(attributes.files).ok()?;
                Some(SubtitleOffer {
                    file_id: file.file_id,
                    language: attributes.language?,
                    release: attributes
                        .release
                        .filter(|release| !release.trim().is_empty())
                        .or(file.file_name)
                        .unwrap_or_default(),
                    downloads: attributes.download_count,
                    hearing_impaired: attributes.hearing_impaired,
                    machine_translated: attributes.machine_translated || attributes.ai_translated,
                    trusted: attributes.from_trusted,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subtitle_is_offered_as_one_file_with_what_tells_it_apart() {
        let found: Found = serde_json::from_str(
            r#"{"data": [
                {"attributes": {"language": "fr", "release": "Quiet.Harbour.2019.1080p",
                    "download_count": 1200, "hearing_impaired": true, "ai_translated": true,
                    "from_trusted": true, "files": [{"file_id": 7, "file_name": "a.srt"}]}},
                {"attributes": {"language": "fr", "release": "",
                    "download_count": 3, "files": [{"file_id": 8, "file_name": "b.srt"}]}},
                {"attributes": {"language": "fr", "release": "Two.Discs",
                    "files": [{"file_id": 9}, {"file_id": 10}]}},
                {"attributes": {"release": "No.Language", "files": [{"file_id": 11}]}}
            ]}"#,
        )
        .expect("read");
        assert_eq!(
            found.offers(),
            vec![
                SubtitleOffer {
                    file_id: 7,
                    language: "fr".to_string(),
                    release: "Quiet.Harbour.2019.1080p".to_string(),
                    downloads: 1200,
                    hearing_impaired: true,
                    machine_translated: true,
                    trusted: true,
                },
                SubtitleOffer {
                    file_id: 8,
                    language: "fr".to_string(),
                    release: "b.srt".to_string(),
                    downloads: 3,
                    hearing_impaired: false,
                    machine_translated: false,
                    trusted: false,
                },
            ],
            "a release in several files, or of no language, is left out"
        );
    }
}
