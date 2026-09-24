//! What the server is called, the logo it wears and what stands behind its
//! sign in screen, as its administrator chooses them.

use std::path::Path;

use melyxar_core::fingerprint;
use melyxar_core::orientation::Orientation;

use crate::{AppError, AppState};

/// What a server is called until its administrator names it, the name the
/// first migration gives it.
pub const DEFAULT_NAME: &str = "Melyxar";

/// The longest a server's name may be: it stands in the bar at the top beside
/// the logo and the way back, where a longer one would reach over the page,
/// and under an installed icon, where a longer one is cut.
pub const LONGEST_NAME: usize = 15;

/// Why a name was not taken, in a word the interface turns into a sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NameNeeded,
    NameTooLong,
    /// What was sent for a logo or a picture is not an image this server
    /// reads.
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

/// Gives the server back the name it came with.
pub async fn forget_name(state: &AppState) -> Result<(), AppError> {
    state.database().set_server_name(DEFAULT_NAME).await?;
    Ok(())
}

/// Makes these bytes the logo of the server, in place of the one it had.
/// Answers where it is, under the folder of what the administrator sent.
///
/// Kept whole, in its own shape and with what is see-through left so: a logo
/// is drawn on the page's own ground, never cut into a square or a round.
pub async fn set_logo(state: &AppState, bytes: &[u8]) -> Result<String, Trouble> {
    let name = made_from(state, bytes, Sent::Logo).await?;
    let before = state.database().set_logo(Some(&name)).await?;
    forget(
        &state.config().directories.uploads(),
        before.as_deref().filter(|before| *before != name),
    )
    .await;
    tracing::info!("the server was given a logo");
    Ok(name)
}

/// Takes the server's logo away, which puts Melyxar's own back.
pub async fn remove_logo(state: &AppState) -> Result<(), Trouble> {
    let before = state.database().set_logo(None).await?;
    forget(&state.config().directories.uploads(), before.as_deref()).await;
    Ok(())
}

pub use melyxar_database::settings::LoginBackground;

/// What stands behind the sign in screen: a picture the administrator put
/// there, which wins, and the drawn background worn when there is none.
pub struct Door {
    pub picture: Option<String>,
    pub background: LoginBackground,
}

/// What stands behind the sign in screen.
pub async fn door(state: &AppState) -> Result<Door, AppError> {
    let settings = state.database().server_settings().await?;
    Ok(Door {
        picture: settings.login_background_path,
        background: settings.login_background,
    })
}

/// Which drawn background the sign in screen wears when no picture was put
/// there.
pub async fn set_door_background(
    state: &AppState,
    background: LoginBackground,
) -> Result<(), AppError> {
    state.database().set_door_background(background).await?;
    Ok(())
}

/// Puts these bytes behind the sign in screen, in place of the picture that
/// was there. Answers where it is, under the folder of what the administrator
/// sent.
pub async fn set_door_picture(state: &AppState, bytes: &[u8]) -> Result<String, Trouble> {
    let name = made_from(state, bytes, Sent::DoorPicture).await?;
    let before = state.database().set_door_picture(Some(&name)).await?;
    if let Some(before) = before.filter(|before| *before != name) {
        tokio::fs::remove_file(state.config().directories.uploads().join(before))
            .await
            .ok();
    }
    tracing::info!("the sign in screen was given a picture");
    Ok(name)
}

/// Takes the picture away from behind the sign in screen, which puts the
/// drawn background back.
pub async fn remove_door_picture(state: &AppState) -> Result<(), Trouble> {
    if let Some(before) = state.database().set_door_picture(None).await? {
        tokio::fs::remove_file(state.config().directories.uploads().join(before))
            .await
            .ok();
    }
    Ok(())
}

/// What an image the administrator sent is made into.
#[derive(Debug, Clone, Copy)]
enum Sent {
    Logo,
    DoorPicture,
}

impl Sent {
    fn prefix(self) -> &'static str {
        match self {
            Self::Logo => "logo",
            Self::DoorPicture => "door",
        }
    }
}

/// Makes the image the administrator sent into what it was sent for, kept in
/// the folder of what they send under a name that changes with its content.
/// Answers that name.
///
/// The image itself is never kept: it is turned the way its camera said and
/// written again in the shape it is for, then thrown away.
async fn made_from(state: &AppState, bytes: &[u8], sent: Sent) -> Result<String, Trouble> {
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
    let name = format!("{}-{}.webp", sent.prefix(), fingerprint::of_bytes(bytes));
    let source = folder.join(format!("{name}.source"));
    tokio::fs::write(&source, bytes).await?;

    let reading = source.clone();
    let orientation =
        tokio::task::spawn_blocking(move || melyxar_library::orientation::orientation_of(&reading))
            .await
            .unwrap_or_default();
    let destination = folder.join(&name);
    let made = match sent {
        Sent::Logo => {
            melyxar_ffmpeg::images::logo(&tools.ffmpeg, &source, orientation, &destination).await
        }
        Sent::DoorPicture => {
            melyxar_ffmpeg::images::door_picture(&tools.ffmpeg, &source, orientation, &destination)
                .await
        }
    };
    tokio::fs::remove_file(&source).await.ok();
    if let Err(error) = made {
        tracing::warn!(%error, sent = sent.prefix(), "an image the administrator sent could not be read");
        return Err(Trouble::Refused(Refused::CouldNotBeRead));
    }
    Ok(name)
}

/// Deletes the files of a logo no longer worn, its icons included. One
/// already gone is fine.
async fn forget(folder: &Path, name: Option<&str>) {
    if let Some(name) = name {
        tokio::fs::remove_file(folder.join(name)).await.ok();
        for icon in [LogoIcon::Whole, LogoIcon::Inset] {
            tokio::fs::remove_file(folder.join(icon.file_of(stem_of(name))))
                .await
                .ok();
        }
    }
}

/// The two square icons made from the server's logo: what a browser shows in
/// its tab and an installed application wears.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogoIcon {
    /// The logo whole, as large as its shape lets it in the square.
    Whole,
    /// The logo small enough in the middle for a system that cuts icons to a
    /// round of its own, laid on a ground when it is served.
    Inset,
}

impl LogoIcon {
    fn file_of(self, stem: &str) -> String {
        match self {
            Self::Whole => format!("{stem}.png"),
            Self::Inset => format!("{stem}-inset.png"),
        }
    }
}

/// The name a logo's files share, which the addresses of its icons carry.
pub fn stem_of(logo: &str) -> &str {
    logo.strip_suffix(".webp").unwrap_or(logo)
}

/// One icon of the logo the server wears, made from it the first time it is
/// asked for. Nothing when the server wears no logo under that name, which is
/// also the answer for the icons of one taken away since.
///
/// Made from the logo as kept rather than from the image sent, which is never
/// kept: a logo given before icons were made has them too.
pub async fn logo_icon(
    state: &AppState,
    stem: &str,
    icon: LogoIcon,
) -> Result<Option<Vec<u8>>, AppError> {
    let settings = state.database().server_settings().await?;
    let Some(logo) = settings.logo_path.filter(|logo| stem_of(logo) == stem) else {
        return Ok(None);
    };
    let folder = state.config().directories.uploads();
    let path = folder.join(icon.file_of(stem));
    if let Ok(made) = tokio::fs::read(&path).await {
        return Ok(Some(made));
    }

    let Some(tools) = state.tools() else {
        return Err(melyxar_core::Error::internal("no picture tool on this server").into());
    };
    // Written aside and moved into place in one step, so a browser asking
    // meanwhile never reads half of one.
    let making = path.with_extension("part.png");
    let made = melyxar_ffmpeg::images::logo_icon(
        &tools.ffmpeg,
        &folder.join(&logo),
        Orientation::AsStored,
        icon == LogoIcon::Inset,
        &making,
    )
    .await;
    if let Err(error) = made {
        tokio::fs::remove_file(&making).await.ok();
        return Err(error.into());
    }
    tokio::fs::rename(&making, &path).await?;
    Ok(Some(tokio::fs::read(&path).await?))
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
    async fn the_door_picture_is_brought_down_and_the_one_before_it_deleted() {
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
            eprintln!("no media tool here, a picture was not made");
            return;
        };
        let database = Database::open_in_memory().await.expect("database opens");
        let state = AppState::new(config, database, tools, capabilities);

        // Wider than any screen needs.
        let made = |colour: &'static str| {
            let ffmpeg = ffmpeg.clone();
            let out = directory.path().join(format!("{colour}.png"));
            async move {
                let done = tokio::process::Command::new(&ffmpeg)
                    .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i"])
                    .arg(format!("color=c={colour}:s=3000x1000"))
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

        set_door_background(&state, LoginBackground::Library)
            .await
            .expect("chosen");
        let first = set_door_picture(&state, &made("red").await)
            .await
            .expect("put there");
        let probed = tokio::process::Command::new(ffmpeg.with_file_name("ffprobe"))
            .args(["-v", "error", "-show_entries", "stream=width", "-of", "csv=p=0"])
            .arg(folder.join(&first))
            .output()
            .await
            .expect("probed");
        assert_eq!(String::from_utf8_lossy(&probed.stdout).trim(), "2560");

        let second = set_door_picture(&state, &made("blue").await)
            .await
            .expect("put there");
        assert_ne!(first, second, "named after what it holds");
        assert!(!folder.join(&first).exists(), "the one before is deleted");
        assert!(matches!(
            set_door_picture(&state, b"<svg/>").await,
            Err(Trouble::Refused(Refused::NotAPicture))
        ));
        let behind = door(&state).await.expect("read");
        assert_eq!(behind.picture.as_deref(), Some(second.as_str()));
        assert_eq!(behind.background, LoginBackground::Library);

        remove_door_picture(&state).await.expect("taken away");
        assert!(!folder.join(&second).exists());
        let behind = door(&state).await.expect("read");
        assert_eq!(behind.picture, None);
        assert_eq!(behind.background, LoginBackground::Library, "the drawn one stays chosen");
    }

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
        let first_icon = logo_icon(&state, stem_of(&first), LogoIcon::Whole)
            .await
            .expect("made")
            .expect("the logo worn has an icon");
        assert!(first_icon.starts_with(b"\x89PNG"));
        assert!(folder.join(format!("{}.png", stem_of(&first))).exists());
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
        assert!(
            !folder.join(format!("{}.png", stem_of(&first))).exists(),
            "and its icons with it"
        );
        assert_eq!(
            logo_icon(&state, stem_of(&first), LogoIcon::Whole)
                .await
                .expect("asked"),
            None,
            "a logo no longer worn has no icon"
        );
        let inset = logo_icon(&state, stem_of(&second), LogoIcon::Inset)
            .await
            .expect("made")
            .expect("the logo worn has an icon");
        let inset_file = directory.path().join("inset.png");
        tokio::fs::write(&inset_file, &inset).await.expect("kept");
        let size = tokio::process::Command::new(ffmpeg.with_file_name("ffprobe"))
            .args([
                "-v",
                "error",
                "-show_entries",
                "stream=width,height",
                "-of",
                "csv=p=0",
            ])
            .arg(&inset_file)
            .output()
            .await
            .expect("probed");
        assert_eq!(String::from_utf8_lossy(&size.stdout).trim(), "512,512");

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
        assert!(!folder
            .join(format!("{}-inset.png", stem_of(&second)))
            .exists());
        assert_eq!(identity(&state).await.expect("read").logo, None);
    }

    #[tokio::test]
    async fn the_name_given_back_is_the_one_a_new_server_starts_with() {
        let database = Database::open_in_memory().await.expect("database opens");
        assert_eq!(
            database.server_settings().await.expect("read").server_name,
            DEFAULT_NAME
        );
        database
            .set_server_name("Home Cinema")
            .await
            .expect("renamed");
        let state = AppState::new(melyxar_config::Config::default(), database, None, None);
        forget_name(&state).await.expect("given back");
        assert_eq!(identity(&state).await.expect("read").name, DEFAULT_NAME);
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
