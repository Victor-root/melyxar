//! Notifications: an account's history, what it chose, what the
//! administrator sends and sets for every account, and what deserves an
//! administrator's look.
//!
//! Translation only: what is sent to whom and kept how long is the app's to
//! decide.

use axum::extract::{Path, Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use melyxar_app::notifications::choices::{Channels, NotificationSettings};
use melyxar_app::notifications::attention::Shown;
use melyxar_app::notifications::live::Change;
use melyxar_app::notifications::messages::Written;
use melyxar_app::notifications::{Audience, Kind, Level, Told};
use melyxar_app::AppState;
use melyxar_core::id::NotificationId;
use serde::{Deserialize, Serialize};

use crate::account::{Administrator, Viewer};
use crate::catalogue::{image_view, ImageView};
use crate::error::{Result, ServerError};
use crate::identifiers::{parse_account, parse_library, parse_notification};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/notifications", get(history))
        .route("/api/v1/notifications/read", post(mark_read))
        .route("/api/v1/notifications/unread", post(mark_unread))
        .route(
            "/api/v1/notifications/{id}",
            axum::routing::delete(remove),
        )
        .route("/api/v1/notifications/choices", get(choices))
        .route("/api/v1/notifications/choices/{kind}", put(choose))
        .route("/api/v1/notifications/settings", put(settle))
        .route("/api/v1/notifications/libraries/{id}", put(hear_about))
        .route("/api/v1/system/notifications", get(for_every_account))
        .route("/api/v1/system/notifications/defaults/{kind}", put(set_default))
        .route("/api/v1/system/notifications/libraries/{id}", put(set_announces))
        .route("/api/v1/system/notifications/messages", post(write))
        .route("/api/v1/system/attention", get(attention))
        .route("/api/v1/system/attention/seen", post(mark_seen))
}

/// One notification, as a page draws it.
#[derive(Debug, Serialize)]
pub(crate) struct NotificationView {
    id: String,
    /// message, maintenance, new_content, deletion, request.
    kind: String,
    /// ok, attention, trouble, news.
    level: String,
    /// What it says, translated by the page from its kind.
    data: serde_json::Value,
    /// The work it opens, absent once the work is gone.
    work_id: Option<String>,
    /// Every size of the poster it wears, largest first.
    poster: Vec<ImageView>,
    priority: bool,
    mandatory: bool,
    sticky: bool,
    shown_for_ms: Option<i64>,
    due_at: Option<String>,
    created_at: String,
    read: bool,
}

pub(crate) fn notification_view(told: &Told) -> NotificationView {
    let kept = &told.notification;
    NotificationView {
        id: kept.id.to_string(),
        kind: kept.kind.clone(),
        level: kept.level.clone(),
        // Written by the app from a value it made: a row that does not read
        // is shown with nothing to say rather than failing the whole page.
        data: serde_json::from_str(&kept.data).unwrap_or(serde_json::Value::Null),
        work_id: kept.work_id.map(|work| work.to_string()),
        poster: told.poster.iter().map(image_view).collect(),
        priority: kept.priority,
        mandatory: kept.mandatory,
        sticky: kept.sticky,
        shown_for_ms: kept.shown_for_ms,
        due_at: kept.due_at.map(melyxar_core::time::to_text),
        created_at: melyxar_core::time::to_text(kept.created_at),
        read: kept.read_at.is_some(),
    }
}

/// A change to an account's notifications, as its live line says it: the
/// name of the word, and what it carries.
pub(crate) fn change_word(change: &Change) -> (&'static str, serde_json::Value) {
    let ids = |ids: &[NotificationId]| {
        serde_json::json!({ "ids": ids.iter().map(ToString::to_string).collect::<Vec<_>>() })
    };
    match change {
        Change::Arrived { told, kept, screen } => (
            "notification",
            serde_json::json!({
                "notification": notification_view(told),
                "kept": kept,
                "screen": screen,
            }),
        ),
        Change::Recalled(told) => (
            "notification_recalled",
            serde_json::json!({ "notification": notification_view(told) }),
        ),
        Change::Read(read) => ("notifications_read", ids(read)),
        Change::Unread(unread) => ("notifications_unread", ids(unread)),
        Change::Removed(removed) => ("notifications_removed", ids(removed)),
        Change::ChoicesChanged => ("notification_choices", serde_json::Value::Null),
    }
}

#[derive(Debug, Serialize)]
struct PageView {
    notifications: Vec<NotificationView>,
    unread: i64,
    more: bool,
}

#[derive(Debug, Deserialize)]
struct Before {
    before: Option<String>,
}

async fn history(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Query(asked): Query<Before>,
) -> Result<Json<PageView>> {
    let before = asked.before.as_deref().map(parse_notification).transpose()?;
    let page = melyxar_app::notifications::history::page(&state, who.id, before).await?;
    Ok(Json(PageView {
        notifications: page.told.iter().map(notification_view).collect(),
        unread: page.unread,
        more: page.more,
    }))
}

#[derive(Debug, Deserialize)]
struct Named {
    /// Absent: every one.
    ids: Option<Vec<String>>,
}

fn parse_notifications(ids: &[String]) -> Result<Vec<NotificationId>> {
    ids.iter().map(|id| parse_notification(id)).collect()
}

async fn mark_read(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Json(named): Json<Named>,
) -> Result<Json<()>> {
    let ids = named.ids.as_deref().map(parse_notifications).transpose()?;
    melyxar_app::notifications::history::mark_read(&state, who.id, ids.as_deref()).await?;
    Ok(Json(()))
}

async fn mark_unread(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Json(named): Json<Named>,
) -> Result<Json<()>> {
    let ids = parse_notifications(named.ids.as_deref().unwrap_or_default())?;
    melyxar_app::notifications::history::mark_unread(&state, who.id, &ids).await?;
    Ok(Json(()))
}

async fn remove(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<()>> {
    melyxar_app::notifications::history::remove(&state, who.id, parse_notification(&id)?).await?;
    Ok(Json(()))
}

fn parse_kind(word: &str) -> Result<Kind> {
    Kind::parse(word).ok_or_else(|| ServerError::invalid_input("no such kind of notification"))
}

#[derive(Debug, Serialize, Deserialize)]
struct ChannelsView {
    bell: bool,
    screen: bool,
}

impl From<ChannelsView> for Channels {
    fn from(view: ChannelsView) -> Self {
        Self {
            bell: view.bell,
            screen: view.screen,
        }
    }
}

#[derive(Debug, Serialize)]
struct KindView {
    kind: &'static str,
    bell: bool,
    screen: bool,
    /// Whether the account chose, rather than following the default.
    chosen: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct SettingsView {
    /// Minutes after midnight, on the clock of the device.
    quiet_from: Option<i32>,
    quiet_until: Option<i32>,
    priority_while_playing: bool,
    priority_while_quiet: bool,
}

#[derive(Debug, Serialize)]
struct LibraryHeardView {
    id: String,
    name: String,
    announced: bool,
}

#[derive(Debug, Serialize)]
struct ChoicesView {
    kinds: Vec<KindView>,
    settings: SettingsView,
    libraries: Vec<LibraryHeardView>,
}

async fn choices(Viewer(who): Viewer, State(state): State<AppState>) -> Result<Json<ChoicesView>> {
    let chosen = melyxar_app::notifications::choices::of(&state, &who).await?;
    let settings = chosen.settings;
    Ok(Json(ChoicesView {
        kinds: chosen
            .kinds
            .iter()
            .map(|one| KindView {
                kind: one.kind.as_str(),
                bell: one.channels.bell,
                screen: one.channels.screen,
                chosen: one.chosen,
            })
            .collect(),
        settings: SettingsView {
            quiet_from: settings.quiet_from,
            quiet_until: settings.quiet_until,
            priority_while_playing: settings.priority_while_playing,
            priority_while_quiet: settings.priority_while_quiet,
        },
        libraries: chosen
            .libraries
            .into_iter()
            .map(|library| LibraryHeardView {
                id: library.id.to_string(),
                name: library.name,
                announced: library.announced,
            })
            .collect(),
    }))
}

async fn choose(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(kind): Path<String>,
    Json(channels): Json<ChannelsView>,
) -> Result<Json<()>> {
    melyxar_app::notifications::choices::choose(&state, who.id, parse_kind(&kind)?, channels.into())
        .await?;
    Ok(Json(()))
}

async fn settle(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Json(view): Json<SettingsView>,
) -> Result<Json<()>> {
    let settings = NotificationSettings {
        quiet_from: view.quiet_from,
        quiet_until: view.quiet_until,
        priority_while_playing: view.priority_while_playing,
        priority_while_quiet: view.priority_while_quiet,
    };
    melyxar_app::notifications::choices::settle(&state, who.id, &settings).await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct Heard {
    announced: bool,
}

async fn hear_about(
    Viewer(who): Viewer,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(heard): Json<Heard>,
) -> Result<Json<()>> {
    melyxar_app::notifications::choices::hear_about(&state, &who, parse_library(&id)?, heard.announced)
        .await?;
    Ok(Json(()))
}

#[derive(Debug, Serialize)]
struct DefaultView {
    kind: &'static str,
    bell: bool,
    screen: bool,
}

#[derive(Debug, Serialize)]
struct LibraryAnnouncingView {
    id: String,
    name: String,
    announces: bool,
}

#[derive(Debug, Serialize)]
struct ForEveryAccountView {
    defaults: Vec<DefaultView>,
    libraries: Vec<LibraryAnnouncingView>,
}

async fn for_every_account(
    _: Administrator,
    State(state): State<AppState>,
) -> Result<Json<ForEveryAccountView>> {
    let defaults = melyxar_app::notifications::choices::defaults(&state).await?;
    let libraries = melyxar_app::notifications::news::announcing(&state).await?;
    Ok(Json(ForEveryAccountView {
        defaults: defaults
            .iter()
            .map(|(kind, channels)| DefaultView {
                kind: kind.as_str(),
                bell: channels.bell,
                screen: channels.screen,
            })
            .collect(),
        libraries: libraries
            .into_iter()
            .map(|library| LibraryAnnouncingView {
                id: library.id.to_string(),
                name: library.name,
                announces: library.announces.unwrap_or(true),
            })
            .collect(),
    }))
}

async fn set_default(
    _: Administrator,
    State(state): State<AppState>,
    Path(kind): Path<String>,
    Json(channels): Json<ChannelsView>,
) -> Result<Json<()>> {
    melyxar_app::notifications::choices::set_default(&state, parse_kind(&kind)?, channels.into())
        .await?;
    Ok(Json(()))
}

#[derive(Debug, Deserialize)]
struct Announces {
    announces: bool,
}

async fn set_announces(
    _: Administrator,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(asked): Json<Announces>,
) -> Result<Json<()>> {
    melyxar_app::notifications::news::set_announces(&state, parse_library(&id)?, asked.announces)
        .await?;
    Ok(Json(()))
}

/// Who a message goes to, as the form says it.
#[derive(Debug, Deserialize)]
#[serde(tag = "to", rename_all = "snake_case")]
enum AudienceAsked {
    Administrators,
    Everyone,
    Accounts { accounts: Vec<String> },
}

#[derive(Debug, Deserialize)]
struct WrittenAsked {
    level: Level,
    title: String,
    #[serde(default)]
    text: String,
    audience: AudienceAsked,
    shown_for_seconds: Option<u32>,
    #[serde(default)]
    sticky: bool,
    #[serde(default)]
    priority: bool,
    #[serde(default)]
    mandatory: bool,
    /// An instant, for a maintenance.
    due_at: Option<String>,
}

async fn write(
    _: Administrator,
    State(state): State<AppState>,
    Json(asked): Json<WrittenAsked>,
) -> Result<Json<()>> {
    let audience = match asked.audience {
        AudienceAsked::Administrators => Audience::Administrators,
        AudienceAsked::Everyone => Audience::Everyone,
        AudienceAsked::Accounts { accounts } => Audience::Accounts(
            accounts
                .iter()
                .map(|id| parse_account(id))
                .collect::<Result<_>>()?,
        ),
    };
    let due_at = asked
        .due_at
        .as_deref()
        .map(|text| {
            melyxar_core::time::Timestamp::parse(
                text,
                &time::format_description::well_known::Rfc3339,
            )
            .map_err(|_| ServerError::invalid_input("the time of the maintenance is malformed"))
        })
        .transpose()?;
    melyxar_app::notifications::messages::write(
        &state,
        Written {
            level: asked.level,
            title: asked.title,
            text: asked.text,
            audience,
            shown_for_seconds: asked.shown_for_seconds,
            sticky: asked.sticky,
            priority: asked.priority,
            mandatory: asked.mandatory,
            due_at,
        },
    )
    .await?;
    Ok(Json(()))
}

#[derive(Debug, Serialize)]
struct AttentionView {
    points: Vec<Shown>,
}

async fn attention(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
) -> Result<Json<AttentionView>> {
    Ok(Json(AttentionView {
        points: melyxar_app::notifications::attention::points(&state, who.id).await?,
    }))
}

async fn mark_seen(
    _: Administrator,
    Viewer(who): Viewer,
    State(state): State<AppState>,
) -> Result<Json<AttentionView>> {
    melyxar_app::notifications::attention::mark_seen(&state, who.id).await?;
    Ok(Json(AttentionView {
        points: melyxar_app::notifications::attention::points(&state, who.id).await?,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    #[test]
    fn a_message_asked_for_reads_whoever_it_goes_to() {
        let asked: WrittenAsked = serde_json::from_value(serde_json::json!({
            "level": "news",
            "title": "Hello",
            "audience": { "to": "accounts", "accounts": ["0191f1c0-0000-7000-8000-000000000000"] },
            "shown_for_seconds": 8,
            "due_at": "2026-10-04T20:00:00Z",
        }))
        .expect("read");
        assert_eq!(asked.level, Level::News);
        assert!(matches!(asked.audience, AudienceAsked::Accounts { ref accounts } if accounts.len() == 1));
        assert!(!asked.sticky && !asked.priority && !asked.mandatory);
        assert_eq!(asked.text, "");
    }
}
