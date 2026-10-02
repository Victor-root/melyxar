//! The models that listen, and fetching them.
//!
//! A model is one big file. The few the server offers are written down here
//! with their size and their checksum, which is what a download is checked
//! against: a file that is not exactly the one written down is never kept. The
//! address they come from is fixed in this file, not chosen by whoever asks,
//! so nobody can make the server fetch anything else.

use std::path::Path;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

/// Where the models are published.
const BASE_URL: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main";

/// How long a connection may go without answering. The download as a whole
/// has no deadline of its own: the biggest model is three gigabytes, and a
/// slow line is no reason to give up on it halfway.
const SILENCE: Duration = Duration::from_secs(60);

/// One model the server offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Model {
    /// The word the interface and the database know it by.
    pub id: &'static str,
    /// Its file, in the place it is published and in the server's folder.
    pub file: &'static str,
    /// What the file weighs, to the byte.
    pub bytes: u64,
    /// The SHA-256 of the file, in lower case.
    pub sha256: &'static str,
}

/// Every model offered, from the lightest to the most exact.
///
/// All of them hear every language and find which one is spoken, which the
/// versions made for English alone do not. The lightest listed is already
/// good enough for a clear voice; the others are for sound that is not.
pub const MODELS: [Model; 4] = [
    Model {
        id: "small",
        file: "ggml-small.bin",
        bytes: 487_601_967,
        sha256: "1be3a9b2063867b937e64e2ec7483364a79917e157fa98c5d94b5c1fffea987b",
    },
    Model {
        id: "medium",
        file: "ggml-medium.bin",
        bytes: 1_533_763_059,
        sha256: "6c14d5adee5f86394037b4e4e8b59f1673b6cee10e3cf0b11bbdbee79c156208",
    },
    Model {
        id: "large_v3_turbo",
        file: "ggml-large-v3-turbo.bin",
        bytes: 1_624_555_275,
        sha256: "1fc70f774d38eb169993ac391eea357ef47c88757ef72ee5943879b7e8e2bc69",
    },
    Model {
        id: "large_v3",
        file: "ggml-large-v3.bin",
        bytes: 3_095_033_483,
        sha256: "64d182b440b98d5203c4f9bd541544d84c605196c4f7b845dfa11fb23594d1e2",
    },
];

/// The model called by this word, if the server offers one.
pub fn model(id: &str) -> Option<&'static Model> {
    MODELS.iter().find(|model| model.id == id)
}

impl Model {
    /// Where the model is fetched from.
    pub fn url(&self) -> String {
        format!("{BASE_URL}/{}", self.file)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum DownloadError {
    #[error("the models could not be reached: {0}")]
    Unreachable(String),
    #[error("the file could not be written: {0}")]
    Unwritable(String),
    #[error("the file received is not the model it was meant to be")]
    NotTheModel,
    #[error("the download was stopped")]
    Stopped,
}

/// Fetches a model to `to`, whole and checked or not at all.
///
/// It is written beside the place it is going, hashed as it arrives, and put
/// in its place with one rename once its size and its checksum are exactly
/// those written down. Anything else removes what was written. `progress` is
/// told how many bytes have arrived, and `stop` is asked between chunks
/// whether to give up.
pub async fn download(
    model: &Model,
    to: &Path,
    progress: impl FnMut(u64),
    stop: impl Fn() -> bool,
) -> Result<(), DownloadError> {
    download_from(&model.url(), model, to, progress, stop).await
}

async fn download_from(
    url: &str,
    model: &Model,
    to: &Path,
    progress: impl FnMut(u64),
    stop: impl Fn() -> bool,
) -> Result<(), DownloadError> {
    fetch(url, model.bytes, model.sha256, to, progress, stop).await
}

/// Fetches one file of a known size and checksum to `to`, whole and checked
/// or not at all. What `download` does for a model, for any file written down.
pub(crate) async fn fetch(
    url: &str,
    bytes: u64,
    sha256: &str,
    to: &Path,
    mut progress: impl FnMut(u64),
    stop: impl Fn() -> bool,
) -> Result<(), DownloadError> {
    let unreachable = |error: reqwest::Error| DownloadError::Unreachable(error.to_string());
    let client = reqwest::Client::builder()
        .read_timeout(SILENCE)
        .connect_timeout(SILENCE)
        .user_agent(concat!("Melyxar/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(unreachable)?;
    let mut response = client.get(url).send().await.map_err(unreachable)?;
    if response.status().as_u16() != 200 {
        return Err(DownloadError::Unreachable(format!(
            "the answer was {}",
            response.status().as_u16()
        )));
    }

    let written_to = to.with_extension("part");
    let unwritable = |error: std::io::Error| DownloadError::Unwritable(error.to_string());
    let mut file = tokio::fs::File::create(&written_to).await.map_err(unwritable)?;
    let mut hash = Sha256::new();
    let mut size = 0u64;
    let outcome = async {
        while let Some(chunk) = response.chunk().await.map_err(unreachable)? {
            if stop() {
                return Err(DownloadError::Stopped);
            }
            size += chunk.len() as u64;
            // More than the model weighs is not the model, and is refused
            // before it fills the disk.
            if size > bytes {
                return Err(DownloadError::NotTheModel);
            }
            hash.update(&chunk);
            file.write_all(&chunk).await.map_err(unwritable)?;
            progress(size);
        }
        file.flush().await.map_err(unwritable)
    }
    .await;
    drop(file);

    let checked = outcome.and_then(|()| {
        let digest = as_hexadecimal(&hash.finalize());
        if size == bytes && digest == sha256 {
            Ok(())
        } else {
            Err(DownloadError::NotTheModel)
        }
    });
    match checked {
        Ok(()) => tokio::fs::rename(&written_to, to).await.map_err(unwritable),
        Err(error) => {
            let _ = tokio::fs::remove_file(&written_to).await;
            Err(error)
        }
    }
}

/// Bytes as the text a checksum is written down in.
fn as_hexadecimal(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let mut written = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(written, "{byte:02x}");
    }
    written
}

/// Whether the file on the disk is the whole model: cheap to ask, and enough
/// to tell one that was put there by a download that finished from one that
/// is missing or was cut. The content was checked when it arrived.
pub fn is_whole(model: &Model, file: &Path) -> bool {
    std::fs::metadata(file).is_ok_and(|found| found.is_file() && found.len() == model.bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// What the test models weigh and hash to.
    const HELLO: Model = Model {
        id: "test",
        file: "ggml-test.bin",
        bytes: 5,
        sha256: "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824",
    };

    /// A server that answers one request with these bytes.
    async fn serving(body: &'static [u8], status: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bound");
        let address = listener.local_addr().expect("address");
        tokio::spawn(async move {
            if let Ok((mut stream, _)) = listener.accept().await {
                let mut asked = [0u8; 1024];
                let _ = stream.read(&mut asked).await;
                let head = format!(
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = stream.write_all(head.as_bytes()).await;
                let _ = stream.write_all(body).await;
            }
        });
        format!("http://{address}/ggml-test.bin")
    }

    #[test]
    fn the_models_offered_are_found_by_their_word_and_each_has_an_address() {
        for model in MODELS {
            assert_eq!(super::model(model.id), Some(&model));
            assert!(model.url().starts_with("https://huggingface.co/"));
            assert!(model.url().ends_with(model.file));
            assert_eq!(model.sha256.len(), 64, "{}", model.id);
        }
        assert_eq!(super::model("everything"), None);
    }

    #[tokio::test]
    async fn the_model_is_kept_once_it_is_exactly_the_one_written_down() {
        let folder = tempfile::tempdir().expect("folder");
        let to = folder.path().join(HELLO.file);
        let url = serving(b"hello", "200 OK").await;
        let mut told = Vec::new();
        download_from(&url, &HELLO, &to, |done| told.push(done), || false)
            .await
            .expect("downloaded");
        assert_eq!(std::fs::read(&to).expect("kept"), b"hello");
        assert_eq!(told.last(), Some(&5));
        assert!(is_whole(&HELLO, &to));
        assert!(!to.with_extension("part").exists());
    }

    #[tokio::test]
    async fn a_file_that_is_not_the_model_is_never_kept() {
        let folder = tempfile::tempdir().expect("folder");
        let to = folder.path().join(HELLO.file);
        // Right size, wrong content.
        let url = serving(b"world", "200 OK").await;
        let error = download_from(&url, &HELLO, &to, |_| (), || false).await;
        assert!(matches!(error, Err(DownloadError::NotTheModel)));
        // Too much of it.
        let url = serving(b"hello and more", "200 OK").await;
        let error = download_from(&url, &HELLO, &to, |_| (), || false).await;
        assert!(matches!(error, Err(DownloadError::NotTheModel)));
        assert!(!to.exists(), "nothing is put in place");
        assert!(!to.with_extension("part").exists(), "and nothing is left beside it");
    }

    #[tokio::test]
    async fn a_download_that_is_stopped_leaves_nothing_and_a_refusal_is_said() {
        let folder = tempfile::tempdir().expect("folder");
        let to = folder.path().join(HELLO.file);
        let url = serving(b"hello", "200 OK").await;
        let error = download_from(&url, &HELLO, &to, |_| (), || true).await;
        assert!(matches!(error, Err(DownloadError::Stopped)));
        assert!(!to.exists() && !to.with_extension("part").exists());

        let url = serving(b"", "404 Not Found").await;
        let error = download_from(&url, &HELLO, &to, |_| (), || false).await;
        assert!(matches!(error, Err(DownloadError::Unreachable(_))));
    }

    #[test]
    fn a_file_that_is_missing_or_cut_is_not_a_whole_model() {
        let folder = tempfile::tempdir().expect("folder");
        let file = folder.path().join(HELLO.file);
        assert!(!is_whole(&HELLO, &file));
        std::fs::write(&file, b"hel").expect("written");
        assert!(!is_whole(&HELLO, &file));
        std::fs::write(&file, b"hello").expect("written");
        assert!(is_whole(&HELLO, &file));
    }
}
