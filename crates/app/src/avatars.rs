//! The picture an account chose for itself.
//!
//! The one file this server receives from a browser. See the decisions on the
//! profile picture for what is accepted and why: an image recognised by what
//! it holds rather than by its name, turned the right way up, cut square and
//! written once at the one size it is shown at. The image sent is never kept.

use std::path::Path;

use melyxar_core::fingerprint;
use melyxar_core::id::UserId;

use crate::{AppError, AppState};

/// The most an image sent for a profile picture may weigh, which the server
/// holds a request to before it reaches here. A photo straight off a phone is
/// a few megabytes; this leaves room for a large one.
pub const LARGEST: usize = 20 * 1024 * 1024;

/// Why a picture was not taken, in a word the interface turns into a
/// sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    /// What was sent is not an image this server reads.
    NotAPicture,
    /// It looked like an image and still could not be read as one.
    CouldNotBeRead,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotAPicture => "not_a_picture",
            Self::CouldNotBeRead => "could_not_be_read",
        }
    }

    /// Every one of them, so a test can check each has words on the screen.
    pub const ALL: [Self; 2] = [Self::NotAPicture, Self::CouldNotBeRead];
}

/// What can go wrong: a refusal to put into words, or the server itself.
#[derive(Debug, thiserror::Error)]
pub enum Trouble {
    #[error("refused: {}", .0.as_str())]
    Refused(Refused),
    #[error(transparent)]
    Failed(#[from] AppError),
}

impl From<melyxar_database::DatabaseError> for Trouble {
    fn from(error: melyxar_database::DatabaseError) -> Self {
        Self::Failed(AppError::from(error))
    }
}

impl From<std::io::Error> for Trouble {
    fn from(error: std::io::Error) -> Self {
        Self::Failed(AppError::from(error))
    }
}

type Result<T> = std::result::Result<T, Trouble>;

/// Whether these bytes open the way an image this server reads does: JPEG,
/// PNG, WebP or GIF. Read from what the file holds, never from its name.
fn is_a_picture(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
        || bytes.starts_with(b"\x89PNG\r\n\x1a\n")
        || bytes.starts_with(b"GIF87a")
        || bytes.starts_with(b"GIF89a")
        || (bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP")
}

/// Makes these bytes the picture of this account, in place of the one it had.
/// Answers where it is, under the folder of pictures of accounts.
pub async fn set(state: &AppState, user_id: UserId, bytes: &[u8]) -> Result<String> {
    if !is_a_picture(bytes) {
        return Err(Trouble::Refused(Refused::NotAPicture));
    }
    let Some(tools) = state.tools() else {
        return Err(Trouble::Failed(
            melyxar_core::Error::internal("no picture tool on this server").into(),
        ));
    };

    let root = state.config().directories.avatars();
    let folder = root.join(user_id.to_string());
    tokio::fs::create_dir_all(&folder).await?;
    let name = format!("avatar-{}.webp", fingerprint::of_bytes(bytes));
    let sent = folder.join(format!("{name}.source"));
    tokio::fs::write(&sent, bytes).await?;

    let reading = sent.clone();
    let orientation =
        tokio::task::spawn_blocking(move || melyxar_library::orientation::orientation_of(&reading))
            .await
            .unwrap_or_default();
    let made =
        melyxar_ffmpeg::images::avatar(&tools.ffmpeg, &sent, orientation, &folder.join(&name))
            .await;
    tokio::fs::remove_file(&sent).await.ok();
    if let Err(error) = made {
        tracing::warn!(%error, "an image sent for a profile picture could not be read");
        return Err(Trouble::Refused(Refused::CouldNotBeRead));
    }

    let path = format!("{user_id}/{name}");
    let before = state.database().set_avatar(user_id, Some(&path)).await?;
    forget(&root, before.as_deref().filter(|before| *before != path)).await;
    tracing::info!(account = %user_id, "a profile picture was chosen");
    Ok(path)
}

/// Takes the picture of this account away.
pub async fn remove(state: &AppState, user_id: UserId) -> Result<()> {
    let before = state.database().set_avatar(user_id, None).await?;
    forget(&state.config().directories.avatars(), before.as_deref()).await;
    Ok(())
}

/// Deletes every picture an account ever had, once the account is gone.
pub async fn forget_every_one_of(state: &AppState, user_id: UserId) {
    let folder = state
        .config()
        .directories
        .avatars()
        .join(user_id.to_string());
    tokio::fs::remove_dir_all(folder).await.ok();
}

/// Deletes the file of a picture no longer worn. One already gone is fine.
async fn forget(root: &Path, path: Option<&str>) {
    if let Some(path) = path {
        tokio::fs::remove_file(root.join(path)).await.ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories};
    use melyxar_database::Database;

    #[tokio::test]
    async fn a_picture_sent_is_worn_cut_square_and_the_one_before_it_is_deleted() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = Config {
            directories: Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
            },
            ..Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let (tools, capabilities) = crate::startup::detect_media_tools(&config).await;
        let Some(ffmpeg) = tools.as_ref().map(|tools| tools.ffmpeg.clone()) else {
            eprintln!("no media tool here, a picture was not made");
            return;
        };
        let database = Database::open_in_memory().await.expect("database opens");
        let zoe = database
            .create_user("Zoe", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created");
        let state = AppState::new(config, database, tools, capabilities);

        // A wide picture, as a phone held sideways takes one.
        let made = |colour: &'static str| {
            let ffmpeg = ffmpeg.clone();
            let out = directory.path().join(format!("{colour}.png"));
            async move {
                let done = tokio::process::Command::new(&ffmpeg)
                    .args([
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-y",
                        "-f",
                        "lavfi",
                        "-i",
                    ])
                    .arg(format!("color=c={colour}:s=400x300"))
                    .args(["-frames:v", "1"])
                    .arg(&out)
                    .status()
                    .await
                    .expect("the tool runs");
                assert!(done.success());
                tokio::fs::read(&out).await.expect("picture read")
            }
        };
        let root = state.config().directories.avatars();

        let first = set(&state, zoe.id, &made("red").await).await.expect("worn");
        let size = tokio::process::Command::new(ffmpeg.with_file_name("ffprobe"))
            .args([
                "-v",
                "error",
                "-show_entries",
                "stream=width,height",
                "-of",
                "csv=p=0",
            ])
            .arg(root.join(&first))
            .output()
            .await
            .expect("probed");
        assert_eq!(String::from_utf8_lossy(&size.stdout).trim(), "256,256");

        let second = set(&state, zoe.id, &made("blue").await)
            .await
            .expect("worn");
        assert_ne!(first, second, "named after what it holds");
        assert!(!root.join(&first).exists(), "the one before is deleted");
        assert!(root.join(&second).exists());

        assert!(matches!(
            set(&state, zoe.id, b"<svg/>").await,
            Err(Trouble::Refused(Refused::NotAPicture))
        ));
        assert!(
            root.join(&second).exists(),
            "a refusal leaves the picture worn"
        );

        remove(&state, zoe.id).await.expect("taken away");
        assert!(!root.join(&second).exists());
        let (read, _) = state
            .database()
            .user_by_name("Zoe")
            .await
            .expect("read")
            .expect("account found");
        assert_eq!(read.avatar_path, None);
    }

    #[test]
    fn an_image_is_known_by_what_it_holds_and_not_by_its_name() {
        assert!(is_a_picture(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0]));
        assert!(is_a_picture(b"\x89PNG\r\n\x1a\n...."));
        assert!(is_a_picture(b"GIF89a...."));
        assert!(is_a_picture(b"RIFF\0\0\0\0WEBPVP8 "));
        assert!(
            !is_a_picture(b"RIFF\0\0\0\0AVI LIST"),
            "a video in the same wrapper"
        );
        assert!(!is_a_picture(
            b"<svg xmlns=\"http://www.w3.org/2000/svg\"/>"
        ));
        assert!(!is_a_picture(b""));
    }
}
