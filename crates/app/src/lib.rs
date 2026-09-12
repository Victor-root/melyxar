//! Use cases: where the modules are assembled.
//!
//! Every action the server can take lives here as a plain function, so the
//! HTTP layer is only a translator and a second entry point such as a command
//! line reuses the same code rather than reimplementing it.
//!
//! Nothing here knows about HTTP, and nothing here writes SQL.

#![forbid(unsafe_code)]

pub mod catalogue;
pub mod detail;
pub mod diagnostics;
pub mod identify;
pub mod images;
pub mod scan;
pub mod startup;
pub mod state;

pub use state::AppState;

/// What browsing a library looks like, for whoever asks.
///
/// Re-exported here so the layer above speaks to the use cases and never to
/// the storage, which is what keeps the dependencies pointing one way.
pub mod browse {
    pub use melyxar_database::browse::{
        BrowseRequest, WorkCard, WorkOrder, WorkPage, DEFAULT_PAGE, LARGEST_PAGE,
    };
}

/// Pictures the server has prepared, as they are stored.
pub mod picture {
    pub use melyxar_database::images::StoredImage;
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
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
