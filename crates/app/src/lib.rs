//! Use cases: where the modules are assembled.
//!
//! Every action the server can take lives here as a plain function, so the
//! HTTP layer is only a translator and a second entry point such as a command
//! line reuses the same code rather than reimplementing it.
//!
//! Nothing here knows about HTTP, and nothing here writes SQL.

#![forbid(unsafe_code)]

pub mod diagnostics;
pub mod identify;
pub mod images;
pub mod scan;
pub mod startup;
pub mod state;

pub use state::AppState;

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
