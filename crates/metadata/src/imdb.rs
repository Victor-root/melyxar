//! The ratings IMDb publishes for everyone, once a day, as one file.
//!
//! IMDb answers no program's questions, but it hands out the ratings of every
//! title it knows in a compressed table of three columns: its identifier, its
//! average and how many voted. Each server fetches the file itself, so no key
//! is shared and no allowance runs out, and reads out of it only the titles it
//! holds.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::path::Path;
use std::time::Duration;

use tokio::io::AsyncWriteExt;

use crate::provider::{ProviderError, Result};

/// Where IMDb publishes the file.
const RATINGS_URL: &str = "https://datasets.imdbws.com/title.ratings.tsv.gz";

/// How long fetching the file may take. It weighs a few megabytes, and a
/// slow line is no reason to give up on a task that runs at night.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Refuses a file beyond any plausible size before it fills the disk.
const LARGEST_FILE: u64 = 256 * 1024 * 1024;

/// An IMDb rating: the average out of ten, and how many voted.
pub type ImdbRating = (f64, i64);

/// Fetches the file of IMDb ratings to `to`, whole or not at all: it is
/// written beside it first and put in its place once complete.
pub async fn download_ratings(to: &Path) -> Result<()> {
    let client = reqwest::Client::builder()
        .timeout(DOWNLOAD_TIMEOUT)
        .user_agent(concat!("Melyxar/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
    let mut response = client
        .get(RATINGS_URL)
        .send()
        .await
        .map_err(|error| ProviderError::Unreachable(error.to_string()))?;
    let status = response.status().as_u16();
    if status != 200 {
        return Err(if (500..600).contains(&status) {
            ProviderError::Unreachable(format!("IMDb answered {status}"))
        } else {
            ProviderError::Unexpected(format!("IMDb answered {status}"))
        });
    }

    let written_to = to.with_extension("part");
    let unwritable = |error: std::io::Error| ProviderError::Unexpected(error.to_string());
    let mut file = tokio::fs::File::create(&written_to).await.map_err(unwritable)?;
    let mut size = 0u64;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| ProviderError::Unreachable(error.to_string()))?
    {
        size += chunk.len() as u64;
        if size > LARGEST_FILE {
            drop(file);
            let _ = tokio::fs::remove_file(&written_to).await;
            return Err(ProviderError::Unexpected("the file of ratings is far too large".into()));
        }
        file.write_all(&chunk).await.map_err(unwritable)?;
    }
    file.flush().await.map_err(unwritable)?;
    drop(file);
    tokio::fs::rename(&written_to, to).await.map_err(unwritable)
}

/// Reads the ratings of the titles wanted out of the compressed file, and
/// nothing else: the file holds well over a million of them.
///
/// A line that cannot be read is passed over rather than failing the lot.
pub fn read_ratings(
    compressed: impl Read,
    wanted: &HashSet<&str>,
) -> std::io::Result<HashMap<String, ImdbRating>> {
    let lines = BufReader::new(flate2::read::GzDecoder::new(compressed));
    let mut found = HashMap::new();
    for line in lines.lines() {
        let line = line?;
        let mut columns = line.split('\t');
        let (Some(id), Some(average), Some(votes)) =
            (columns.next(), columns.next(), columns.next())
        else {
            continue;
        };
        if !wanted.contains(id) {
            continue;
        }
        if let (Ok(average), Ok(votes)) = (average.parse::<f64>(), votes.parse::<i64>())
            && (0.0..=10.0).contains(&average)
            && votes > 0
        {
            found.insert(id.to_string(), (average, votes));
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn compressed(text: &str) -> Vec<u8> {
        let mut encoder =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(text.as_bytes()).expect("written");
        encoder.finish().expect("finished")
    }

    #[test]
    fn only_the_titles_wanted_are_read_and_a_broken_line_is_passed_over() {
        let file = compressed(
            "tconst\taverageRating\tnumVotes\n\
             tt0000001\t5.7\t2100\n\
             tt0000002\tbroken\t10\n\
             tt0000003\t8.4\t950000\n\
             tt0000004\t6.1\t40\n\
             tt0000005\n",
        );
        let wanted = HashSet::from(["tt0000002", "tt0000003", "tt0000005", "tt0000009"]);
        let found = read_ratings(file.as_slice(), &wanted).expect("read");
        assert_eq!(
            found,
            HashMap::from([("tt0000003".to_string(), (8.4, 950000))]),
            "a title not wanted, a broken line and a title missing are all left out"
        );
    }

    #[test]
    fn a_file_that_is_not_compressed_is_an_error_not_an_empty_answer() {
        let wanted = HashSet::from(["tt0000001"]);
        assert!(read_ratings(b"tt0000001\t5.7\t2100\n".as_slice(), &wanted).is_err());
    }
}
