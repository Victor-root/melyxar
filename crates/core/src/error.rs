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
    /// Asked too often to be answered right now, and told when to come back.
    TooManyAttempts,
    /// This account is already watching as many films at once as it may.
    TooManyStreams,
    Conflict,
    PathNotAllowed,
    RootUnavailable,
    /// Nothing has managed to describe the file, so there is nothing to decide
    /// how to play it with.
    NotDescribed,
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
            Self::TooManyAttempts => "too_many_attempts",
            Self::TooManyStreams => "too_many_streams",
            Self::Conflict => "conflict",
            Self::PathNotAllowed => "path_not_allowed",
            Self::RootUnavailable => "root_unavailable",
            Self::NotDescribed => "not_described",
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

    pub fn not_described(detail: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotDescribed, detail)
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[cfg(test)]
mod tests {
    use super::*;

    /// A client reads the code rather than the sentence: it is what tells a
    /// page to say "nothing there" instead of "something went wrong". Changing
    /// one of these words silently would break every client at once.
    #[test]
    fn every_code_keeps_the_name_a_client_reads() {
        for (code, written) in [
            (ErrorCode::NotFound, "not_found"),
            (ErrorCode::AlreadyExists, "already_exists"),
            (ErrorCode::InvalidInput, "invalid_input"),
            (ErrorCode::Unauthenticated, "unauthenticated"),
            (ErrorCode::Forbidden, "forbidden"),
            (ErrorCode::Conflict, "conflict"),
            (ErrorCode::PathNotAllowed, "path_not_allowed"),
            (ErrorCode::RootUnavailable, "root_unavailable"),
            (ErrorCode::NotDescribed, "not_described"),
            (ErrorCode::DependencyMissing, "dependency_missing"),
            (
                ErrorCode::ExternalServiceUnavailable,
                "external_service_unavailable",
            ),
            (ErrorCode::Internal, "internal"),
        ] {
            assert_eq!(code.as_str(), written);
            assert_eq!(code.to_string(), written, "shown the same way it is sent");
        }
    }

    #[test]
    fn an_error_carries_both_its_code_and_what_went_wrong() {
        let error = Error::not_found("no work with that identifier");
        assert_eq!(error.code, ErrorCode::NotFound);
        assert!(error.to_string().contains("no work with that identifier"));
    }
}
