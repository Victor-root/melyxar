//! What the server is called and the logo it wears, as its administrator
//! chooses them.

use std::path::Path;

use melyxar_core::fingerprint;

use crate::{AppError, AppState};

/// The longest a server's name may be, the same as a library's: it is carried
/// in a browser's tab and under an installed icon, where a longer one is cut.
pub const LONGEST_NAME: usize = 60;

/// Why a name was not taken, in a word the interface turns into a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NameNeeded,
    NameTooLong,
    /// What was sent for a logo is not an image this server reads.
    NotAPicture,
    /// It looked like an image and still could not be read as one.
    CouldNotBeRead,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NameNeeded => "name_needed",
            Self::NameTooLong => "name_too_long",
            Self::NotAPicture => "not_a_picture",
            Self::CouldNotBeRead => "could_not_be_read",
        }
    }

    /// Every one of them, so a test can check each has words on the screen.
    pub const ALL: [Self; 4] = [
        Self::NameNeeded,
        Self::NameTooLong,
        Self::NotAPicture,
        Self::CouldNotBeRead,
    ];
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

/// What the server is called, and the file of its logo when it was given one.
pub struct Identity {
    pub name: String,
    pub logo: Option<String>,
}

/// What the server is called and the logo it wears.
pub async fn identity(state: &AppState) -> Result<Identity, AppError> {
    let settings = state.database().server_settings().await?;
    Ok(Identity {
        name: settings.server_name,
        logo: settings.logo_path,
    })
}

/// Calls the server something else, and answers the name as it was kept.
pub async fn rename(state: &AppState, asked: &str) -> Result<String, Trouble> {
    let name = name_of(asked)?;
    state.database().set_server_name(&name).await?;
    Ok(name)
}

/// Makes these bytes the logo of the server, in place of the one it had.
/// Answers where it is, under the folder of what the administrator sent.
///
/// Kept whole, in its own shape and with what is see-through left so: a logo
/// is drawn on the page's own ground, never cut into a square or a round.
pub async fn set_logo(state: &AppState, bytes: &[u8]) -> Result<String, Trouble> {
    if !crate::avatars::is_a_picture(bytes) {
        return Err(Trouble::Refused(Refused::NotAPicture));
    }
    let Some(tools) = state.tools() else {
        return Err(Trouble::Failed(
            melyxar_core::Error::internal("no picture tool on this server").into(),
        ));
    };

    let folder = state.config().directories.uploads();
    tokio::fs::create_dir_all(&folder).await?;
    let name = format!("logo-{}.webp", fingerprint::of_bytes(bytes));
    let sent = folder.join(format!("{name}.source"));
    tokio::fs::write(&sent, bytes).await?;

    let reading = sent.clone();
    let orientation =
        tokio::task::spawn_blocking(move || melyxar_library::orientation::orientation_of(&reading))
            .await
            .unwrap_or_default();
    let made =
        melyxar_ffmpeg::images::logo(&tools.ffmpeg, &sent, orientation, &folder.join(&name)).await;
    tokio::fs::remove_file(&sent).await.ok();
    if let Err(error) = made {
        tracing::warn!(%error, "an image sent for the server's logo could not be read");
        return Err(Trouble::Refused(Refused::CouldNotBeRead));
    }

    let before = state.database().set_logo(Some(&name)).await?;
    forget(&folder, before.as_deref().filter(|before| *before != name)).await;
    tracing::info!("the server was given a logo");
    Ok(name)
}

/// Takes the server's logo away, which puts Melyxar's own back.
pub async fn remove_logo(state: &AppState) -> Result<(), Trouble> {
    let before = state.database().set_logo(None).await?;
    forget(&state.config().directories.uploads(), before.as_deref()).await;
    Ok(())
}

/// Deletes the file of a logo no longer worn. One already gone is fine.
async fn forget(folder: &Path, name: Option<&str>) {
    if let Some(name) = name {
        tokio::fs::remove_file(folder.join(name)).await.ok();
    }
}

/// The name a server is to be called, or a refusal saying why not.
fn name_of(asked: &str) -> Result<String, Trouble> {
    let name = asked.trim();
    if name.is_empty() {
        return Err(Trouble::Refused(Refused::NameNeeded));
    }
    if name.chars().count() > LONGEST_NAME {
        return Err(Trouble::Refused(Refused::NameTooLong));
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories};
    use melyxar_database::Database;

    #[tokio::test]
    async fn a_logo_is_kept_whole_and_the_one_before_it_is_deleted() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = Config {
            directories: Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
                ..Default::default()
            },
            ..Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let (tools, capabilities) = crate::startup::detect_media_tools(&config).await;
        let Some(ffmpeg) = tools.as_ref().map(|tools| tools.ffmpeg.clone()) else {
            eprintln!("no media tool here, a logo was not made");
            return;
        };
        let database = Database::open_in_memory().await.expect("database opens");
        let state = AppState::new(config, database, tools, capabilities);

        // A wide mark, larger than it is ever shown, see-through around it.
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
                    .arg(format!("color=c={colour}@0.5:s=1200x300,format=rgba"))
                    .args(["-frames:v", "1"])
                    .arg(&out)
                    .status()
                    .await
                    .expect("the tool runs");
                assert!(done.success());
                tokio::fs::read(&out).await.expect("picture read")
            }
        };
        let folder = state.config().directories.uploads();

        let first = set_logo(&state, &made("red").await).await.expect("worn");
        let probed = tokio::process::Command::new(ffmpeg.with_file_name("ffprobe"))
            .args([
                "-v",
                "error",
                "-show_entries",
                "stream=width,height,pix_fmt",
                "-of",
                "csv=p=0",
            ])
            .arg(folder.join(&first))
            .output()
            .await
            .expect("probed");
        assert_eq!(
            String::from_utf8_lossy(&probed.stdout).trim(),
            "512,128,yuva420p",
            "brought down in its own shape, still see-through"
        );

        let second = set_logo(&state, &made("blue").await).await.expect("worn");
        assert_ne!(first, second, "named after what it holds");
        assert!(!folder.join(&first).exists(), "the one before is deleted");

        assert!(matches!(
            set_logo(&state, b"<svg/>").await,
            Err(Trouble::Refused(Refused::NotAPicture))
        ));
        assert!(
            folder.join(&second).exists(),
            "a refusal leaves the logo worn"
        );

        remove_logo(&state).await.expect("taken away");
        assert!(!folder.join(&second).exists());
        assert_eq!(identity(&state).await.expect("read").logo, None);
    }

    fn refusal(asked: &str) -> Option<Refused> {
        match name_of(asked) {
            Err(Trouble::Refused(refused)) => Some(refused),
            _ => None,
        }
    }

    #[test]
    fn a_name_is_kept_without_the_spaces_around_it() {
        assert_eq!(name_of("  Home Cinema ").expect("taken"), "Home Cinema");
    }

    #[test]
    fn an_empty_name_is_refused() {
        assert_eq!(refusal("   "), Some(Refused::NameNeeded));
    }

    #[test]
    fn a_name_is_counted_in_letters_rather_than_bytes() {
        assert!(name_of(&"é".repeat(LONGEST_NAME)).is_ok());
        assert_eq!(
            refusal(&"é".repeat(LONGEST_NAME + 1)),
            Some(Refused::NameTooLong)
        );
    }
}
