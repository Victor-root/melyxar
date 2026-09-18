//! Use cases: where the modules are assembled.
//!
//! Every action the server can take lives here as a plain function, so the
//! HTTP layer is only a translator and a second entry point such as a command
//! line reuses the same code rather than reimplementing it.
//!
//! Nothing here knows about HTTP, and nothing here writes SQL.

#![forbid(unsafe_code)]

pub mod calibration;
pub mod catalogue;
pub mod detail;
pub mod diagnostics;
pub mod episodes;
pub mod identify;
pub mod images;
pub mod libraries;
pub mod playback;
pub mod preferences;
pub mod scan;
pub mod startup;
pub mod state;
pub mod subtitles;
pub mod thumbnails;
pub mod upkeep;

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
    pub use melyxar_metadata::{MetadataProvider, MovieCandidate};
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Streaming(#[from] melyxar_streaming::StreamingError),
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
