//! Turning a failure into a response.
//!
//! A response carries a stable code and the values that go with it, never a
//! finished sentence. That is what lets every client show the message in its
//! own language, and it is why a server that answers with prose cannot be
//! translated by the clients it serves.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use melyxar_core::error::ErrorCode;
use serde::Serialize;

/// The shape every failure takes on the wire.
#[derive(Debug, Clone, Serialize)]
pub struct ApiError {
    /// Stable identifier the client keys its wording on.
    pub code: &'static str,
    /// Values the wording needs, such as a name or a limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(code: ErrorCode) -> Self {
        Self {
            code: code.as_str(),
            details: None,
        }
    }

    pub fn with_details(code: ErrorCode, details: serde_json::Value) -> Self {
        Self {
            code: code.as_str(),
            details: Some(details),
        }
    }
}

/// The failure type handlers return.
pub struct ServerError {
    status: StatusCode,
    body: ApiError,
    /// Kept for the log, never sent: it is written for the maintainer, not for
    /// a viewer, and it can name paths.
    detail: String,
}

impl ServerError {
    pub fn new(status: StatusCode, code: ErrorCode, detail: impl Into<String>) -> Self {
        Self {
            status,
            body: ApiError::new(code),
            detail: detail.into(),
        }
    }

    pub fn not_found(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, ErrorCode::NotFound, detail)
    }

    pub fn internal(detail: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::Internal,
            detail,
        )
    }

    /// Something the caller sent cannot be used. Says which thing, never why
    /// in prose: the wording belongs to the client, in its own language.
    pub fn invalid_input(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ErrorCode::InvalidInput, detail)
    }

    /// The server cannot do this right now, though the request is sound.
    ///
    /// Told apart from a refusal: a client that waits and asks again will get
    /// an answer, and one that is told plainly can say so rather than showing
    /// a spinner that never resolves.
    pub fn busy(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Conflict, detail)
    }

    pub fn unauthenticated(detail: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, ErrorCode::Unauthenticated, detail)
    }
}

impl ServerError {
    /// The stable code this failure carries, which is what a test and a client
    /// key on rather than on the wording.
    pub fn code(&self) -> &'static str {
        self.body.code
    }
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        // Server side failures are worth a log line; a client mistake is not.
        if self.status.is_server_error() {
            tracing::error!(
                code = self.body.code,
                detail = self.detail,
                "request failed"
            );
        } else {
            tracing::debug!(
                code = self.body.code,
                detail = self.detail,
                "request refused"
            );
        }
        (self.status, Json(self.body)).into_response()
    }
}

impl From<melyxar_app::AppError> for ServerError {
    fn from(error: melyxar_app::AppError) -> Self {
        Self::internal(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, ServerError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_carries_a_code_and_never_a_sentence() {
        let error = ApiError::new(ErrorCode::NotFound);
        let rendered = serde_json::to_string(&error).expect("serialises");
        assert!(rendered.contains("not_found"));
        assert!(
            !rendered.contains(' ') || !rendered.contains("was not"),
            "a finished sentence cannot be translated by a client: {rendered}"
        );
    }

    #[test]
    fn details_travel_as_values_the_wording_can_use() {
        let error = ApiError::with_details(
            ErrorCode::InvalidInput,
            serde_json::json!({ "field": "name", "max_length": 64 }),
        );
        let rendered = serde_json::to_string(&error).expect("serialises");
        assert!(rendered.contains("invalid_input"));
        assert!(rendered.contains("max_length"));
    }

    #[test]
    fn a_failure_without_details_leaves_the_field_out_entirely() {
        let rendered =
            serde_json::to_string(&ApiError::new(ErrorCode::Internal)).expect("serialises");
        assert!(!rendered.contains("details"));
    }
}
