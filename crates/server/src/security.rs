//! How the way in is protected, for the administration.
//!
//! Translation only: the bounds are the app's to decide.

use axum::extract::State;
use axum::{Json, Router};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::account::Administrator;
use melyxar_app::access::{Mode, Status};

use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/system/security/tries",
            axum::routing::get(tries).put(set_tries),
        )
        .route(
            "/api/v1/system/security/access",
            axum::routing::get(access).put(choose_access),
        )
}

#[derive(Debug, Serialize, Deserialize)]
struct TriesView {
    tries: i64,
}

async fn tries(_: Administrator, State(state): State<AppState>) -> Result<Json<TriesView>> {
    Ok(Json(TriesView {
        tries: melyxar_app::accounts::sign_in_tries(&state).await?,
    }))
}

async fn set_tries(
    _: Administrator,
    State(state): State<AppState>,
    Json(asked): Json<TriesView>,
) -> Result<Json<TriesView>> {
    Ok(Json(TriesView {
        tries: melyxar_app::accounts::set_sign_in_tries(&state, asked.tries).await?,
    }))
}

#[derive(Debug, Serialize)]
struct CertificateView {
    names: Vec<String>,
    issuer: String,
    not_before: String,
    not_after: String,
}

#[derive(Debug, Serialize)]
struct AccessView {
    /// proxy, self_signed, provided or automatic.
    mode: &'static str,
    certificate_path: Option<String>,
    private_key_path: Option<String>,
    /// The certificate in use, when connections are encrypted.
    certificate: Option<CertificateView>,
    /// Why they are not, when they should be, as a word.
    problem: Option<&'static str>,
}

impl From<Status> for AccessView {
    fn from(status: Status) -> Self {
        Self {
            mode: status.mode.as_str(),
            certificate_path: status.certificate_path,
            private_key_path: status.private_key_path,
            certificate: status.certificate.map(|certificate| CertificateView {
                names: certificate.names,
                issuer: certificate.issuer,
                not_before: melyxar_core::time::to_text(certificate.not_before),
                not_after: melyxar_core::time::to_text(certificate.not_after),
            }),
            problem: status.problem.map(|problem| problem.as_str()),
        }
    }
}

#[derive(Debug, Deserialize)]
struct AccessAsked {
    mode: String,
    #[serde(default)]
    certificate_path: Option<String>,
    #[serde(default)]
    private_key_path: Option<String>,
}

async fn access(_: Administrator, State(state): State<AppState>) -> Json<AccessView> {
    Json(melyxar_app::access::status(&state).into())
}

async fn choose_access(
    _: Administrator,
    State(state): State<AppState>,
    Json(asked): Json<AccessAsked>,
) -> Result<Json<AccessView>> {
    let mode = Mode::from_word(&asked.mode)
        .ok_or_else(|| ServerError::invalid_input("no such way of reaching the server"))?;
    let status = melyxar_app::access::choose(
        &state,
        mode,
        asked.certificate_path.as_deref(),
        asked.private_key_path.as_deref(),
    )
    .await?;
    Ok(Json(status.into()))
}
