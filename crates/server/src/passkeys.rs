//! The passkeys of an account, from its profile, and the door they open.
//!
//! The site a key belongs to is the origin the browser wrote on the request,
//! which a page cannot write for itself: the device signs the same origin
//! into its answer, and an answer whose origin is not this one does not pass.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, header};
use axum::response::Response;
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_app::accounts::SignedInOrNot;
use melyxar_app::passkeys::{
    Challenge, PasskeyView, PublicKeyCredential, RegisterPublicKeyCredential,
};
use serde::{Deserialize, Serialize};

use crate::account::{Viewer, answered_with_a_session, held_back, what_asked, wished_for};
use crate::address::Caller;
use crate::error::{Result, ServerError};
use crate::identifiers::parse_passkey;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/me/passkeys",
            axum::routing::get(my_passkeys).post(finish_making),
        )
        .route(
            "/api/v1/me/passkeys/making",
            axum::routing::post(start_making),
        )
        .route(
            "/api/v1/me/passkeys/{id}",
            axum::routing::put(rename).delete(remove),
        )
        .route(
            "/api/v1/public/passkey/challenge",
            axum::routing::post(start_signing_in),
        )
        .route(
            "/api/v1/public/passkey/session",
            axum::routing::post(finish_signing_in),
        )
        .route(
            "/.well-known/assetlinks.json",
            axum::routing::get(asset_links),
        )
}

/// The origin the browser wrote on the request, which names the site.
fn origin_of(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::ORIGIN)
        .and_then(|value| value.to_str().ok())
}

#[derive(Debug, Serialize)]
struct PasskeyBody {
    id: String,
    name: String,
    site: String,
    created_at: String,
    used_at: Option<String>,
}

impl From<PasskeyView> for PasskeyBody {
    fn from(view: PasskeyView) -> Self {
        Self {
            id: view.id.to_string(),
            name: view.name,
            site: view.site,
            created_at: melyxar_core::time::to_text(view.created_at),
            used_at: view.used_at.map(melyxar_core::time::to_text),
        }
    }
}

/// A challenge, and the word the answer comes back under.
#[derive(Debug, Serialize)]
struct ChallengeBody<T> {
    ceremony: String,
    options: T,
}

impl<T> From<Challenge<T>> for ChallengeBody<T> {
    fn from(challenge: Challenge<T>) -> Self {
        Self {
            ceremony: challenge.ceremony,
            options: challenge.options,
        }
    }
}

async fn my_passkeys(
    State(state): State<AppState>,
    Viewer(user): Viewer,
) -> Result<Json<Vec<PasskeyBody>>> {
    let kept = melyxar_app::passkeys::passkeys_of(&state, &user).await?;
    Ok(Json(kept.into_iter().map(PasskeyBody::from).collect()))
}

#[derive(Debug, Deserialize)]
struct WithThePassword {
    password: String,
}

async fn start_making(
    State(state): State<AppState>,
    headers: HeaderMap,
    Viewer(user): Viewer,
    Caller { address, .. }: Caller,
    Json(asked): Json<WithThePassword>,
) -> Result<Json<ChallengeBody<melyxar_app::passkeys::CreationChallengeResponse>>> {
    let started = melyxar_app::passkeys::start_making(
        &state,
        &user,
        &asked.password,
        origin_of(&headers),
        &what_asked(&headers),
        address,
    )
    .await?;
    Ok(Json(started.into()))
}

#[derive(Debug, Deserialize)]
struct Made {
    ceremony: String,
    name: String,
    credential: RegisterPublicKeyCredential,
}

async fn finish_making(
    State(state): State<AppState>,
    headers: HeaderMap,
    Viewer(user): Viewer,
    Json(asked): Json<Made>,
) -> Result<Json<PasskeyBody>> {
    let made = melyxar_app::passkeys::finish_making(
        &state,
        &user,
        &asked.ceremony,
        &asked.name,
        &asked.credential,
        origin_of(&headers),
    )
    .await?;
    Ok(Json(made.into()))
}

#[derive(Debug, Deserialize)]
struct NameBody {
    name: String,
}

async fn rename(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    Path(id): Path<String>,
    Json(asked): Json<NameBody>,
) -> Result<Json<serde_json::Value>> {
    if !melyxar_app::passkeys::rename(&state, &user, parse_passkey(&id)?, &asked.name).await? {
        return Err(ServerError::not_found("no such passkey"));
    }
    Ok(Json(serde_json::json!({ "renamed": true })))
}

async fn remove(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    if !melyxar_app::passkeys::remove(&state, &user, parse_passkey(&id)?).await? {
        return Err(ServerError::not_found("no such passkey"));
    }
    Ok(Json(serde_json::json!({ "removed": true })))
}

async fn start_signing_in(
    State(state): State<AppState>,
    headers: HeaderMap,
    Caller { address, .. }: Caller,
) -> Result<Json<ChallengeBody<melyxar_app::passkeys::RequestChallengeResponse>>> {
    let started =
        melyxar_app::passkeys::start_signing_in(&state, origin_of(&headers), address).await?;
    Ok(Json(started.into()))
}

#[derive(Debug, Deserialize)]
struct Brought {
    ceremony: String,
    credential: PublicKeyCredential,
    /// As on the door with a password: kept unless said otherwise.
    #[serde(default = "crate::account::kept_unless_said_otherwise")]
    remember: bool,
    #[serde(default)]
    client: Option<String>,
}

async fn finish_signing_in(
    State(state): State<AppState>,
    headers: HeaderMap,
    Caller {
        address, encrypted, ..
    }: Caller,
    Json(asked): Json<Brought>,
) -> Result<Response> {
    let remembered = wished_for(asked.remember);
    match melyxar_app::passkeys::finish_signing_in(
        &state,
        &asked.ceremony,
        &asked.credential,
        origin_of(&headers),
        &what_asked(&headers),
        remembered,
        asked.client.as_deref(),
        address,
    )
    .await?
    {
        SignedInOrNot::Opened(opened) => {
            Ok(answered_with_a_session(*opened, remembered, encrypted))
        }
        SignedInOrNot::NotAPair => Err(ServerError::unauthenticated(
            "that passkey does not open this door",
        )),
        SignedInOrNot::HeldBack { seconds } => Err(held_back(seconds)),
        SignedInOrNot::AwaitingApproval => Err(melyxar_app::accounts::Trouble::Refused(
            melyxar_app::accounts::Refused::AwaitingApproval,
        )
        .into()),
    }
}

/// What lets Melyxar's Android application make and use the passkeys of
/// this site: the fingerprints of the certificates it may be signed with.
///
/// Both relations Android's documentation declares for sharing sign-ins, the
/// one for links included. That one opens no link in the application, which
/// declares none to be verified.
async fn asset_links(State(state): State<AppState>) -> Json<serde_json::Value> {
    let fingerprints = &state.config().passkeys.applications;
    Json(serde_json::json!([{
        "relation": [
            "delegate_permission/common.handle_all_urls",
            "delegate_permission/common.get_login_creds"
        ],
        "target": {
            "namespace": "android_app",
            "package_name": melyxar_config::ANDROID_PACKAGE,
            "sha256_cert_fingerprints": fingerprints,
        }
    }]))
}
