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

    /// A refusal carrying the values its wording needs, such as how long to
    /// wait before asking again.
    pub fn with_details(
        status: StatusCode,
        code: ErrorCode,
        details: serde_json::Value,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            status,
            body: ApiError::with_details(code, details),
            detail: detail.into(),
        }
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

impl From<melyxar_app::libraries::Trouble> for ServerError {
    /// A refusal carries the word saying which thing to put right, in the
    /// details every client already reads. A failure of the server itself is
    /// the other arm, and stays what it was.
    fn from(trouble: melyxar_app::libraries::Trouble) -> Self {
        match trouble {
            melyxar_app::libraries::Trouble::Refused(refused) => Self {
                status: StatusCode::BAD_REQUEST,
                body: ApiError::with_details(
                    ErrorCode::InvalidInput,
                    serde_json::json!({ "reason": refused.as_str() }),
                ),
                detail: format!("refused: {}", refused.as_str()),
            },
            melyxar_app::libraries::Trouble::Failed(error) => Self::from(error),
        }
    }
}

impl From<melyxar_app::AppError> for ServerError {
    fn from(error: melyxar_app::AppError) -> Self {
        match error {
            // A domain failure already says what kind of thing went wrong, in
            // a word a client keys its own wording on. Flattening it into one
            // generic failure would leave every client saying "something went
            // wrong" for a disk that is merely unplugged.
            melyxar_app::AppError::Domain(domain) => {
                Self::new(status_for(domain.code), domain.code, domain.detail.clone())
            }
            melyxar_app::AppError::Streaming(trouble) => Self::from(trouble),
            other => Self::internal(other.to_string()),
        }
    }
}

/// What a domain failure looks like over HTTP.
fn status_for(code: ErrorCode) -> StatusCode {
    match code {
        ErrorCode::NotFound => StatusCode::NOT_FOUND,
        ErrorCode::AlreadyExists | ErrorCode::Conflict => StatusCode::CONFLICT,
        ErrorCode::InvalidInput => StatusCode::BAD_REQUEST,
        ErrorCode::Unauthenticated => StatusCode::UNAUTHORIZED,
        ErrorCode::Forbidden | ErrorCode::PathNotAllowed => StatusCode::FORBIDDEN,
        ErrorCode::TooManyAttempts => StatusCode::TOO_MANY_REQUESTS,
        // Both will work again once a disk is plugged back in or a tool is
        // installed, which is what tells a client to say so rather than to
        // announce a failure of the server itself.
        ErrorCode::RootUnavailable | ErrorCode::DependencyMissing => {
            StatusCode::SERVICE_UNAVAILABLE
        }
        // The file is there and is the problem: nothing about the server will
        // change that, and no amount of waiting will either.
        ErrorCode::NotDescribed => StatusCode::UNPROCESSABLE_ENTITY,
        ErrorCode::ExternalServiceUnavailable => StatusCode::BAD_GATEWAY,
        ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

impl From<melyxar_app::playback::StreamingError> for ServerError {
    fn from(error: melyxar_app::playback::StreamingError) -> Self {
        use melyxar_app::playback::StreamingError as Failure;
        match error {
            // The session was swept away while nobody was watching. A player
            // that comes back asks for a new one rather than failing outright.
            Failure::NoSuchSession => Self::not_found("that session is over"),
            Failure::NoSuchSegment => Self::not_found("that segment is not part of this film"),
            Failure::TooManyAtOnce => {
                Self::busy("this server is already converting all it can at once")
            }
            other => Self::internal(other.to_string()),
        }
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

    #[test]
    fn a_domain_failure_keeps_the_word_a_client_reads() {
        // Flattening these into one generic failure would leave every client
        // saying "something went wrong" for a disk that is merely unplugged.
        for (code, status) in [
            (ErrorCode::NotFound, StatusCode::NOT_FOUND),
            (ErrorCode::AlreadyExists, StatusCode::CONFLICT),
            (ErrorCode::Conflict, StatusCode::CONFLICT),
            (ErrorCode::InvalidInput, StatusCode::BAD_REQUEST),
            (ErrorCode::Unauthenticated, StatusCode::UNAUTHORIZED),
            (ErrorCode::Forbidden, StatusCode::FORBIDDEN),
            (ErrorCode::PathNotAllowed, StatusCode::FORBIDDEN),
            (ErrorCode::RootUnavailable, StatusCode::SERVICE_UNAVAILABLE),
            (
                ErrorCode::DependencyMissing,
                StatusCode::SERVICE_UNAVAILABLE,
            ),
            (ErrorCode::NotDescribed, StatusCode::UNPROCESSABLE_ENTITY),
            (
                ErrorCode::ExternalServiceUnavailable,
                StatusCode::BAD_GATEWAY,
            ),
            (ErrorCode::Internal, StatusCode::INTERNAL_SERVER_ERROR),
        ] {
            let error = ServerError::from(melyxar_app::AppError::Domain(melyxar_core::Error::new(
                code,
                "what went wrong, for the log",
            )));
            assert_eq!(error.code(), code.as_str());
            assert_eq!(error.status, status, "for {code}");
        }
    }

    #[test]
    fn a_form_to_correct_carries_the_word_saying_which_thing_to_correct() {
        // Every refusal somebody meets while filling a form in has to say
        // which field, in a word the screen words itself: a client cannot
        // translate a sentence, and "the server would not" sends somebody
        // back to try the same thing again.
        let error = ServerError::from(melyxar_app::libraries::Trouble::Refused(
            melyxar_app::libraries::Refused::FolderAlreadyLookedIn,
        ));
        assert_eq!(error.code(), "invalid_input");
        assert_eq!(error.status, StatusCode::BAD_REQUEST);

        let rendered = serde_json::to_string(&error.body).expect("serialises");
        assert!(rendered.contains("folder_already_looked_in"), "{rendered}");
    }

    #[test]
    fn a_machine_converting_all_it_can_says_so_rather_than_failing() {
        let error = ServerError::from(melyxar_app::AppError::Streaming(
            melyxar_app::playback::StreamingError::TooManyAtOnce,
        ));
        assert_eq!(error.code(), "conflict");
        assert_eq!(error.status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn a_session_that_is_over_is_not_reported_as_a_broken_server() {
        // A player that comes back after a pause asks for a new session; it
        // must not be told the server itself is broken.
        let error = ServerError::from(melyxar_app::AppError::Streaming(
            melyxar_app::playback::StreamingError::NoSuchSession,
        ));
        assert_eq!(error.code(), "not_found");
        assert_eq!(error.status, StatusCode::NOT_FOUND);
    }
}
