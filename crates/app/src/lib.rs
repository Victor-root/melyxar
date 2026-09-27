//! Use cases: where the modules are assembled.
//!
//! Every action the server can take lives here as a plain function, so the
//! HTTP layer is only a translator and a second entry point such as a command
//! line reuses the same code rather than reimplementing it.
//!
//! Nothing here knows about HTTP, and nothing here writes SQL.

#![forbid(unsafe_code)]

pub mod accounts;
pub mod activity;
pub mod attention;
pub mod avatars;
pub mod bench;
pub mod calibration;
pub mod catalogue;
pub mod counted;
pub mod deletion;
pub mod detail;
pub mod diagnostics;
pub mod episodes;
pub mod folder_watch;
pub mod identify;
pub mod images;
pub mod libraries;
pub mod mark;
pub mod marks;
pub mod measures;
pub mod openings;
pub mod overview;
pub mod people;
mod own;
pub mod playback;
pub mod reach;
pub mod preferences;
pub mod scan;
pub mod schedule;
pub mod segments;
pub mod server;
pub mod startup;
pub mod state;
pub mod subtitles;
pub mod thumbnails;
pub mod upkeep;
pub mod watching;

pub use state::AppState;

/// What browsing a library looks like, for whoever asks.
///
/// Re-exported here so the layer above speaks to the use cases and never to
/// the storage, which is what keeps the dependencies pointing one way.
pub mod browse {
    pub use melyxar_database::browse::{
        BrowseRequest, Initial, WorkCard, WorkOrder, WorkPage, DEFAULT_PAGE, LARGEST_PAGE,
    };
}

/// Pictures the server has prepared, as they are stored.
pub mod picture {
    pub use melyxar_database::images::StoredImage;
}

/// What the server does with a library, as it is stored.
///
/// Re-exported for the same reason as the rest: the layer above asks this
/// crate what the server is set to do, and never reaches past it to the
/// storage.
pub mod settings {
    pub use melyxar_database::settings::LibraryWork;
}

/// What a metadata provider is and what it answers.
///
/// Re-exported for the same reason as the rest: the layer above asks this
/// crate for a provider and never reaches past it for the trait it satisfies.
pub mod metadata {
    pub use melyxar_metadata::{Candidate, MetadataProvider, OfferedPicture, PictureKind};
}

/// An ordinary account, for the tests that are about something else.
///
/// Every use case that reads the library now takes the account asking rather
/// than only its identifier, because what that account may see is part of the
/// question. A test about a scan or about a playback decision is not about
/// that, so it hands over the account this server mostly answers: the one
/// nothing has been kept from.
#[cfg(test)]
pub(crate) fn an_ordinary_account(id: melyxar_core::id::UserId) -> melyxar_core::user::User {
    melyxar_core::user::User {
        id,
        name: "somebody".to_string(),
        avatar_path: None,
        permissions: melyxar_core::user::Permissions::viewer(),
        preferences: melyxar_core::user::Preferences::default(),
        created_at: melyxar_core::time::now(),
    }
}

/// A server the way a test needs one: its data under `directory`, and one
/// library called "Films" holding `kind` over these folders, declared the way
/// the configuration file declares it.
#[cfg(test)]
pub(crate) async fn a_test_server(
    directory: &std::path::Path,
    kind: &str,
    roots: Vec<(&str, std::path::PathBuf)>,
) -> (
    melyxar_config::Config,
    melyxar_database::Database,
    melyxar_core::library::Library,
) {
    use melyxar_config::{Config, Directories, LibraryConfig, RootConfig};

    let config = Config {
        directories: Directories {
            data: directory.join("data"),
            cache: directory.join("cache"),
            transcodes: directory.join("cache/transcodes"),
            ..Default::default()
        },
        libraries: vec![LibraryConfig {
            name: "Films".into(),
            kind: kind.into(),
            metadata_language: "fr".into(),
            roots: roots
                .into_iter()
                .map(|(label, path)| RootConfig {
                    label: label.to_string(),
                    path,
                })
                .collect(),
        }],
        ..Config::default()
    };
    startup::prepare_directories(&config).expect("directories prepared");

    let database = melyxar_database::Database::open_in_memory()
        .await
        .expect("database opens");
    startup::reconcile_libraries(&database, &config)
        .await
        .expect("libraries reconciled");
    let library = database
        .library_by_name("Films")
        .await
        .expect("read")
        .expect("the library was declared");
    (config, database, library)
}

/// Somebody to answer for, since what a page shows depends on who is looking
/// at it.
///
/// The one account rather than a new one each time: a server has one until
/// signing in arrives, and asking twice for a second would be asking for
/// somebody who cannot exist.
#[cfg(test)]
pub(crate) async fn a_viewer(state: &AppState) -> melyxar_core::user::User {
    let database = state.database();
    if let Some((already, _)) = database.user_by_name("Viewer").await.expect("read") {
        return already;
    }
    database
        .create_user("Viewer", None, &melyxar_core::user::Permissions::viewer())
        .await
        .expect("account created")
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Streaming(#[from] melyxar_streaming::StreamingError),
    #[error(transparent)]
    Auth(#[from] melyxar_auth::AuthError),
    #[error(transparent)]
    Database(#[from] melyxar_database::DatabaseError),
    #[error(transparent)]
    MediaTools(#[from] melyxar_ffmpeg::FfmpegError),
    #[error(transparent)]
    Config(#[from] melyxar_config::ConfigError),
    #[error("could not prepare a directory: {0}")]
    Directory(#[from] std::io::Error),
    #[error("{0}")]
    Domain(#[from] melyxar_core::Error),
    #[error(transparent)]
    Jobs(#[from] melyxar_jobs::JobError),
}

pub type Result<T> = std::result::Result<T, AppError>;

/// The file behind one copy, refused with a reason when it cannot be opened.
///
/// Every piece of work that reads a film asks the same two questions first:
/// is there a copy of that name, and is the disk it lives on there right now.
/// Four places asked them, each writing out the same two refusals, so a third
/// condition worth refusing on would have had to be remembered four times and
/// would have been remembered once.
///
/// Refused with an explanation rather than opened and failing halfway
/// through, which is what a viewer would otherwise see.
pub async fn playable_file(
    database: &melyxar_database::Database,
    source_id: melyxar_core::id::MediaSourceId,
) -> Result<melyxar_database::playback::PlayableSource> {
    let source = database
        .playable_source(source_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("media source")))?;
    if source.missing {
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::RootUnavailable,
            "the file is not on the disk at the moment",
        )));
    }
    Ok(source)
}
