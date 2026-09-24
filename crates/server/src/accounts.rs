//! The accounts of this server, as the administration manages them.
//!
//! Every route here is an administrator's and says so by asking for one. What
//! may be done to which account, and what is refused, is the use cases' to
//! decide: this only reads what was sent and says what came of it.

use axum::extract::{Path, State};
use axum::routing::{delete, get, put};
use axum::{Json, Router};
use melyxar_app::accounts::{Listed, Rights};
use melyxar_app::AppState;
use melyxar_core::id::UserId;
use melyxar_core::user::User;
use serde::{Deserialize, Serialize};

use crate::account::{Administrator, Viewer};
use crate::error::Result;
use crate::identifiers::{parse_account, parse_library};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/accounts", get(every_account).post(create))
        .route("/api/v1/accounts/{id}", delete(remove))
        .route("/api/v1/accounts/{id}/rights", put(set_rights))
        .route("/api/v1/accounts/{id}/name", put(rename))
        .route("/api/v1/accounts/{id}/password", put(put_a_password))
        .route(
            "/api/v1/accounts/{id}/sessions",
            delete(sign_out_everywhere),
        )
}

/// What an account may do and see, as the administration shows and sends it.
#[derive(Debug, Serialize, Deserialize)]
struct RightsView {
    is_administrator: bool,
    sees_every_library: bool,
    /// The libraries granted, read only when it does not see every one.
    libraries: Vec<String>,
    may_delete: bool,
    may_delete_from_disk: bool,
    /// How many films it may watch at once. Absent for no limit.
    most_streams: Option<i32>,
}

impl RightsView {
    fn of(user: &User) -> Self {
        let held = &user.permissions;
        Self {
            is_administrator: held.is_administrator,
            sees_every_library: held.sees_every_library,
            libraries: held
                .allowed_libraries
                .iter()
                .map(ToString::to_string)
                .collect(),
            may_delete: held.may_delete,
            may_delete_from_disk: held.may_delete_from_disk,
            most_streams: held.max_sessions,
        }
    }

    fn read(self) -> Result<Rights> {
        Ok(Rights {
            is_administrator: self.is_administrator,
            sees_every_library: self.sees_every_library,
            libraries: self
                .libraries
                .iter()
                .map(|library| parse_library(library))
                .collect::<Result<_>>()?,
            may_delete: self.may_delete,
            may_delete_from_disk: self.may_delete_from_disk,
            most_streams: self.most_streams,
        })
    }
}

/// One account as the page of accounts lists it.
#[derive(Debug, Serialize)]
struct ManagedAccountView {
    id: String,
    name: String,
    /// Where its picture is served, when it has one.
    avatar: Option<String>,
    /// Whether this is the administrator looking at the page, which is what
    /// the page greys out the gestures refused on one's own account by.
    is_you: bool,
    rights: RightsView,
    created_at: String,
    /// How many devices it is signed in on.
    devices: i64,
    /// When the most recent of them was last used. Absent when it is signed
    /// in nowhere.
    last_seen_at: Option<String>,
}

fn managed_view(listed: &Listed, you: UserId) -> ManagedAccountView {
    let user = &listed.user;
    ManagedAccountView {
        id: user.id.to_string(),
        name: user.name.clone(),
        avatar: user.avatar_path.as_deref().map(crate::images::face_url),
        is_you: user.id == you,
        rights: RightsView::of(user),
        created_at: melyxar_core::time::to_text(user.created_at),
        devices: listed.devices.map_or(0, |devices| devices.signed_in),
        last_seen_at: listed
            .devices
            .map(|devices| melyxar_core::time::to_text(devices.last_seen_at)),
    }
}

/// The same for an account just made or changed, which no list was read for.
fn one_view(user: User, you: UserId) -> ManagedAccountView {
    managed_view(
        &Listed {
            user,
            devices: None,
        },
        you,
    )
}

async fn every_account(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
) -> Result<Json<Vec<ManagedAccountView>>> {
    let listed = melyxar_app::accounts::every_account(&state).await?;
    Ok(Json(
        listed.iter().map(|one| managed_view(one, who.id)).collect(),
    ))
}

#[derive(Debug, Deserialize)]
struct NewAccountBody {
    name: String,
    password: String,
    rights: RightsView,
}

async fn create(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Json(body): Json<NewAccountBody>,
) -> Result<Json<ManagedAccountView>> {
    let rights = body.rights.read()?;
    let permissions = rights.over(&melyxar_core::user::Permissions::viewer());
    let made =
        melyxar_app::accounts::create_account(&state, &body.name, &body.password, &permissions)
            .await?;
    Ok(Json(one_view(made, who.id)))
}

async fn set_rights(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<RightsView>,
) -> Result<Json<ManagedAccountView>> {
    let target = parse_account(&id)?;
    let changed = melyxar_app::accounts::set_rights(&state, &who, target, &body.read()?).await?;
    Ok(Json(one_view(changed, who.id)))
}

#[derive(Debug, Deserialize)]
struct NameBody {
    name: String,
}

async fn rename(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<NameBody>,
) -> Result<Json<ManagedAccountView>> {
    let target = parse_account(&id)?;
    let renamed = melyxar_app::accounts::rename_account(&state, target, &body.name).await?;
    Ok(Json(one_view(renamed, who.id)))
}

#[derive(Debug, Deserialize)]
struct PasswordBody {
    password: String,
}

async fn put_a_password(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(body): Json<PasswordBody>,
) -> Result<Json<serde_json::Value>> {
    let target = parse_account(&id)?;
    melyxar_app::accounts::put_a_password(&state, &who, target, &body.password).await?;
    Ok(Json(serde_json::json!({ "changed": true })))
}

async fn sign_out_everywhere(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let target = parse_account(&id)?;
    let closed = melyxar_app::accounts::sign_out_everywhere(&state, &who, target).await?;
    Ok(Json(serde_json::json!({ "signed_out": closed })))
}

async fn remove(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    let target = parse_account(&id)?;
    melyxar_app::accounts::remove_account_by_id(&state, &who, target).await?;
    Ok(Json(serde_json::json!({ "removed": true })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }
}
