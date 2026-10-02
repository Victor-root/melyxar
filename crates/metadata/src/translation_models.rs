//! The model that translates English into French, and fetching it.
//!
//! One model is offered, made of five small files. As for the models that
//! listen, each file is written down with its size and its checksum, and the
//! address they come from is fixed in this file and pinned to one revision, so
//! nobody can make the server fetch anything else and a file never changes
//! under it.

use std::path::Path;

use crate::speech_models::{DownloadError, fetch};

/// Where the model is published, at the revision the checksums below belong to.
const BASE_URL: &str =
    "https://huggingface.co/michaelfeil/ct2fast-opus-mt-en-fr/resolve/55bcef18761227161433d86b81bbbcd9d813b429";

/// One file of the model.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct File {
    pub name: &'static str,
    /// What the file weighs, to the byte.
    pub bytes: u64,
    /// The SHA-256 of the file, in lower case.
    pub sha256: &'static str,
}

/// Every file the model is made of, the weights first.
pub const FILES: [File; 5] = [
    File {
        name: "model.bin",
        bytes: 149_872_839,
        sha256: "636d926880a6e6e4b773403a5991b50d4dfdfabe49a230e148676a05e99f2bec",
    },
    File {
        name: "config.json",
        bytes: 159,
        sha256: "0c2f6fa2057c7264d052fb4a62ba3476eeae70487acddfa8e779a53a00cbf44c",
    },
    File {
        name: "shared_vocabulary.txt",
        bytes: 556_777,
        sha256: "e87446c025bbe57cc4f9ef90fa6e28ab303a2e31479b7355c2a2d3a6a3a43a0a",
    },
    File {
        name: "source.spm",
        bytes: 778_395,
        sha256: "173e9f493a668fe396d599e28d414a201193094e6ffd7a4678e5aab0f6d3d838",
    },
    File {
        name: "target.spm",
        bytes: 802_397,
        sha256: "78d0e717c77053f1c4b856d8661d9cb87c64f083a35418c087b9146300e4f585",
    },
];

/// What the whole model weighs.
pub fn total_bytes() -> u64 {
    FILES.iter().map(|file| file.bytes).sum()
}

/// Whether every file of the model is on the disk in `folder`: cheap to ask,
/// and enough to tell a download that finished from one that was cut. The
/// content was checked when each file arrived.
pub fn is_whole(folder: &Path) -> bool {
    FILES.iter().all(|file| {
        std::fs::metadata(folder.join(file.name))
            .is_ok_and(|found| found.is_file() && found.len() == file.bytes)
    })
}

/// Fetches the files that are missing to `folder`. `progress` is told how many
/// bytes of the whole model have arrived, files already there included, and
/// `stop` is asked between chunks whether to give up.
pub async fn download(
    folder: &Path,
    mut progress: impl FnMut(u64),
    stop: impl Fn() -> bool,
) -> Result<(), DownloadError> {
    download_from(BASE_URL, folder, &mut progress, &stop).await
}

async fn download_from(
    base: &str,
    folder: &Path,
    progress: &mut impl FnMut(u64),
    stop: &impl Fn() -> bool,
) -> Result<(), DownloadError> {
    tokio::fs::create_dir_all(folder)
        .await
        .map_err(|error| DownloadError::Unwritable(error.to_string()))?;
    let mut before = 0u64;
    for file in FILES {
        let to = folder.join(file.name);
        let already = std::fs::metadata(&to).is_ok_and(|found| found.is_file() && found.len() == file.bytes);
        if !already {
            fetch(
                &format!("{base}/{}", file.name),
                file.bytes,
                file.sha256,
                &to,
                |done| progress(before + done),
                stop,
            )
            .await?;
        }
        before += file.bytes;
        progress(before);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_model_weighs_what_its_files_weigh_together() {
        assert_eq!(total_bytes(), 152_010_567);
        assert_eq!(FILES[0].name, "model.bin", "the weights come first: the others are small");
    }

    #[test]
    fn a_folder_holds_the_model_only_when_every_file_is_there_whole() {
        let folder = tempfile::tempdir().expect("folder");
        assert!(!is_whole(folder.path()));
        for file in FILES {
            let made = std::fs::File::create(folder.path().join(file.name)).expect("created");
            made.set_len(file.bytes).expect("a sparse file");
        }
        assert!(is_whole(folder.path()));
        let cut = std::fs::File::options().write(true).open(folder.path().join("model.bin")).expect("open");
        cut.set_len(10).expect("cut");
        assert!(!is_whole(folder.path()), "a cut file is not the model");
    }

    #[tokio::test]
    async fn a_server_that_cannot_be_reached_leaves_nothing_in_the_folder() {
        let folder = tempfile::tempdir().expect("folder");
        let mut told = |_| {};
        let result = download_from("http://127.0.0.1:1", folder.path(), &mut told, &|| false).await;
        assert!(matches!(result, Err(DownloadError::Unreachable(_))));
        assert!(!is_whole(folder.path()));
        assert!(std::fs::read_dir(folder.path()).expect("read").next().is_none());
    }
}
