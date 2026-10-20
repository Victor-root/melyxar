//! The cameras of a library of cameras: declared by the administrator, seen by
//! the accounts granted the library, and opened live by their applications.

use melyxar_auth::sealing::SealingKey;
use melyxar_core::camera::{is_a_camera_address, opened_with};
use melyxar_core::id::{CameraId, LibraryId};
use melyxar_core::library::{Library, LibraryKind};
use melyxar_core::user::User;
use melyxar_database::cameras::{Camera, CameraFields};

use crate::{AppError, AppState, Result};

/// The longest name a camera may be given.
const LONGEST_NAME: usize = 120;

/// Why a camera, as written in the administration, was refused. A word and
/// never a sentence: the screen words it, naming the box to put right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NameNeeded,
    /// Not an RTSP address naming a machine, or one carrying its account,
    /// which is written in the boxes beside it.
    NotACameraAddress,
}

impl Refused {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NameNeeded => "name_needed",
            Self::NotACameraAddress => "not_a_camera_address",
        }
    }

    /// Every one of them, so a test can find a refusal nobody worded.
    pub const ALL: [Self; 2] = [Self::NameNeeded, Self::NotACameraAddress];
}

/// What can go wrong declaring or changing a camera: something somebody
/// typed, or the server itself.
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

impl From<melyxar_auth::AuthError> for Trouble {
    fn from(error: melyxar_auth::AuthError) -> Self {
        Self::Failed(AppError::from(error))
    }
}

/// What the administration says of a camera.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Declared {
    pub name: String,
    pub address: String,
    pub username: String,
}

/// What becomes of a camera's password when it is changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NewPassword {
    Kept,
    Replaced(String),
    Forgotten,
}

/// A camera as a screen shows it. Its address and account only to an
/// administrator, and its password to nobody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shown {
    pub id: CameraId,
    pub name: String,
    pub address: Option<String>,
    pub username: Option<String>,
    pub has_password: Option<bool>,
}

/// The cameras of a library this account was granted, by name.
pub async fn of_the_library(state: &AppState, who: &User, library: LibraryId) -> Result<Vec<Shown>> {
    let library = library_of_cameras(state, library).await?;
    crate::reach::may_read(who, library.id)?;
    let administrator = who.permissions.is_administrator;
    Ok(state
        .database()
        .cameras_of(library.id)
        .await?
        .into_iter()
        .map(|camera| Shown {
            id: camera.id,
            name: camera.name,
            address: administrator.then_some(camera.address),
            username: administrator.then_some(camera.username),
            has_password: administrator.then_some(camera.sealed_password.is_some()),
        })
        .collect())
}

/// Declares a camera in a library of cameras.
pub async fn declare(
    state: &AppState,
    library: LibraryId,
    declared: &Declared,
    password: Option<&str>,
) -> std::result::Result<CameraId, Trouble> {
    let library = library_of_cameras(state, library).await?;
    let fields = checked(declared).map_err(Trouble::Refused)?;
    let database = state.database();
    let camera = database.add_camera(library.id, fields, None).await?;
    if let Some(password) = password.filter(|password| !password.is_empty()) {
        let sealed = sealing_key(state).await?.seal(password, &camera.id.to_string())?;
        database.set_camera_password(camera.id, Some(&sealed)).await?;
    }
    database.bump_library_version(library.id).await?;
    tracing::info!(library = library.name, camera = camera.name, "a camera was declared");
    Ok(camera.id)
}

/// Says something else of a camera.
pub async fn change(
    state: &AppState,
    id: CameraId,
    declared: &Declared,
    password: NewPassword,
) -> std::result::Result<(), Trouble> {
    let camera = camera(state, id).await?;
    let database = state.database();
    database
        .update_camera(camera.id, checked(declared).map_err(Trouble::Refused)?)
        .await?;
    match password {
        NewPassword::Kept => {}
        NewPassword::Forgotten => {
            database.set_camera_password(camera.id, None).await?;
        }
        NewPassword::Replaced(password) => {
            let sealed = sealing_key(state).await?.seal(&password, &camera.id.to_string())?;
            database.set_camera_password(camera.id, Some(&sealed)).await?;
        }
    }
    database.bump_library_version(camera.library_id).await?;
    tracing::info!(camera = declared.name, "a camera was changed");
    Ok(())
}

/// Takes a camera away.
pub async fn remove(state: &AppState, id: CameraId) -> Result<()> {
    let camera = camera(state, id).await?;
    state.database().remove_camera(camera.id).await?;
    state.database().bump_library_version(camera.library_id).await?;
    tracing::info!(camera = camera.name, "a camera was taken away");
    Ok(())
}

/// The address an application opens a camera at, its account written in, for
/// an account granted the camera's library.
pub async fn live_address(state: &AppState, who: &User, id: CameraId) -> Result<String> {
    let camera = camera(state, id).await?;
    crate::reach::may_read(who, camera.library_id)?;
    let password = match &camera.sealed_password {
        Some(sealed) => Some(sealing_key(state).await?.open(sealed, &camera.id.to_string())?),
        None => None,
    };
    tracing::info!(camera = camera.name, user = who.name, "a camera is opened live");
    Ok(opened_with(&camera.address, &camera.username, password.as_deref()))
}

fn checked(declared: &Declared) -> std::result::Result<CameraFields<'_>, Refused> {
    let name = declared.name.trim();
    if name.is_empty() || name.chars().count() > LONGEST_NAME {
        return Err(Refused::NameNeeded);
    }
    let address = declared.address.trim();
    if !is_a_camera_address(address) {
        return Err(Refused::NotACameraAddress);
    }
    Ok(CameraFields {
        name,
        address,
        username: declared.username.trim(),
    })
}

async fn camera(state: &AppState, id: CameraId) -> Result<Camera> {
    state
        .database()
        .camera(id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("no camera with that identifier")))
}

async fn library_of_cameras(state: &AppState, id: LibraryId) -> Result<Library> {
    state
        .database()
        .list_libraries()
        .await?
        .into_iter()
        .find(|library| library.id == id && library.kind == LibraryKind::Cameras)
        .ok_or_else(|| {
            AppError::Domain(melyxar_core::Error::not_found("no library of cameras with that identifier"))
        })
}

/// The server's sealing key, drawn and kept the first time one is needed.
///
/// Readable by the server alone. Two requests drawing it at once both try to
/// write it as a new file, and the one that loses reads the other's.
async fn sealing_key(state: &AppState) -> Result<SealingKey> {
    let path = state.config().directories.sealing_key();
    let key = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        match std::fs::read(&path) {
            Ok(key) => Ok(key),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let drawn = SealingKey::draw()?;
                match write_only_for_the_server(&path, &drawn) {
                    Ok(()) => {
                        tracing::info!("a key to seal the server's secrets with was drawn");
                        Ok(drawn.to_vec())
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        Ok(std::fs::read(&path)?)
                    }
                    Err(error) => Err(error.into()),
                }
            }
            Err(error) => Err(error.into()),
        }
    })
    .await
    .map_err(|error| AppError::Domain(melyxar_core::Error::internal(error.to_string())))??;
    Ok(SealingKey::from_bytes(&key)?)
}

fn write_only_for_the_server(path: &std::path::Path, key: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(key)?;
    file.sync_all()
}

#[cfg(test)]
mod tests {
    use melyxar_core::user::Permissions;

    use super::*;

    const DOOR: &str = "rtsp://192.168.1.20:554/stream1";

    fn declared(name: &str) -> Declared {
        Declared {
            name: name.to_string(),
            address: DOOR.to_string(),
            username: "viewer".to_string(),
        }
    }

    async fn a_library_of_cameras() -> (tempfile::TempDir, AppState, LibraryId) {
        let (directory, state) = crate::an_empty_server().await;
        let library = state
            .database()
            .create_library("Cameras", LibraryKind::Cameras, "fr", &[])
            .await
            .expect("library created");
        (directory, state, library.id)
    }

    fn an_administrator() -> User {
        User {
            permissions: Permissions::administrator(),
            ..crate::an_ordinary_account(melyxar_core::id::UserId::new())
        }
    }

    #[tokio::test]
    async fn a_camera_is_opened_at_its_address_with_its_account_written_in() {
        let (_directory, state, library) = a_library_of_cameras().await;
        let door = declare(&state, library, &declared("Front door"), Some("p@ss"))
            .await
            .expect("declared");

        let viewer = crate::an_ordinary_account(melyxar_core::id::UserId::new());
        assert_eq!(
            live_address(&state, &viewer, door).await.expect("opened"),
            "rtsp://viewer:p%40ss@192.168.1.20:554/stream1"
        );
        let kept = state.database().camera(door).await.expect("read").expect("there");
        assert!(
            !kept.sealed_password.expect("sealed").windows(4).any(|part| part == b"p@ss"),
            "the password is never kept as it was typed"
        );
        assert!(state.config().directories.sealing_key().exists(), "the key was drawn and kept");
    }

    #[tokio::test]
    async fn an_account_not_granted_the_library_neither_sees_nor_opens_its_cameras() {
        let (_directory, state, library) = a_library_of_cameras().await;
        let door = declare(&state, library, &declared("Front door"), None).await.expect("declared");
        let kept_out = User {
            permissions: Permissions {
                sees_every_library: false,
                allowed_libraries: Vec::new(),
                ..Permissions::viewer()
            },
            ..crate::an_ordinary_account(melyxar_core::id::UserId::new())
        };
        assert!(of_the_library(&state, &kept_out, library).await.is_err());
        assert!(live_address(&state, &kept_out, door).await.is_err());
    }

    #[tokio::test]
    async fn only_an_administrator_is_shown_where_a_camera_is_and_nobody_its_password() {
        let (_directory, state, library) = a_library_of_cameras().await;
        declare(&state, library, &declared("Front door"), Some("secret")).await.expect("declared");

        let viewer = crate::an_ordinary_account(melyxar_core::id::UserId::new());
        let seen = of_the_library(&state, &viewer, library).await.expect("listed");
        assert_eq!(seen[0].name, "Front door");
        assert_eq!((seen[0].address.clone(), seen[0].username.clone(), seen[0].has_password), (None, None, None));

        let seen = of_the_library(&state, &an_administrator(), library).await.expect("listed");
        assert_eq!(seen[0].address.as_deref(), Some(DOOR));
        assert_eq!(seen[0].has_password, Some(true));
    }

    #[tokio::test]
    async fn a_camera_changed_keeps_its_password_unless_given_another_or_none() {
        let (_directory, state, library) = a_library_of_cameras().await;
        let door = declare(&state, library, &declared("Front door"), Some("first")).await.expect("declared");
        let viewer = crate::an_ordinary_account(melyxar_core::id::UserId::new());

        change(&state, door, &declared("Porch"), NewPassword::Kept).await.expect("changed");
        assert!(live_address(&state, &viewer, door).await.expect("opened").contains(":first@"));
        change(&state, door, &declared("Porch"), NewPassword::Replaced("second".into()))
            .await
            .expect("changed");
        assert!(live_address(&state, &viewer, door).await.expect("opened").contains(":second@"));
        change(&state, door, &declared("Porch"), NewPassword::Forgotten).await.expect("changed");
        assert_eq!(
            live_address(&state, &viewer, door).await.expect("opened"),
            "rtsp://viewer@192.168.1.20:554/stream1"
        );
    }

    #[tokio::test]
    async fn a_camera_is_declared_only_with_a_name_and_an_address_of_a_camera() {
        let (_directory, state, library) = a_library_of_cameras().await;
        let unnamed = Declared { name: "  ".into(), ..declared("x") };
        assert!(declare(&state, library, &unnamed, None).await.is_err());
        let not_rtsp = Declared { address: "http://192.168.1.20/".into(), ..declared("Door") };
        assert!(declare(&state, library, &not_rtsp, None).await.is_err());
        let films = state
            .database()
            .create_library("Films", LibraryKind::Movies, "fr", &[])
            .await
            .expect("library created");
        assert!(declare(&state, films.id, &declared("Door"), None).await.is_err(), "not a library of cameras");
    }
}
