//! The error type shared across the whole server.
//!
//! Variants describe *what kind of thing* went wrong, never how it should be
//! rendered. The HTTP layer maps each variant to a status code and a stable
//! machine-readable code; the client decides the wording. That is what lets
//! every client translate messages on its own.

use std::fmt;

/// A stable, machine-readable error code sent to clients.
///
/// Clients key their translated messages on this value, so a variant must
/// never be renamed once released.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    AlreadyExists,
    InvalidInput,
    Unauthenticated,
    Forbidden,
    Conflict,
    PathNotAllowed,
    RootUnavailable,
    DependencyMissing,
    ExternalServiceUnavailable,
    Internal,
}

impl ErrorCode {
    /// Short stable identifier, also used as the serialised form.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotFound => "not_found",
            Self::AlreadyExists => "already_exists",
            Self::InvalidInput => "invalid_input",
            Self::Unauthenticated => "unauthenticated",
            Self::Forbidden => "forbidden",
            Self::Conflict => "conflict",
            Self::PathNotAllowed => "path_not_allowed",
            Self::RootUnavailable => "root_unavailable",
            Self::DependencyMissing => "dependency_missing",
            Self::ExternalServiceUnavailable => "external_service_unavailable",
            Self::Internal => "internal",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A domain error: a stable code plus a developer-facing detail.
///
/// The detail is for logs and for the maintainer, never for end users.
#[derive(Debug, thiserror::Error)]
#[error("{code}: {detail}")]
pub struct Error {
    pub code: ErrorCode,
    pub detail: String,
}

impl Error {
    pub fn new(code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, detail)
    }

    pub fn invalid_input(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, detail)
    }

    pub fn forbidden(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Forbidden, detail)
    }

    pub fn unauthenticated(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unauthenticated, detail)
    }

    pub fn conflict(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, detail)
    }

    pub fn dependency_missing(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::DependencyMissing, detail)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
