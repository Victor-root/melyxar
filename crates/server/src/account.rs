//! The gate: who is asking, and whether they may be here at all.
//!
//! Everything this server answers goes past here first. What arrives is a
//! cookie; what comes out the other side is an account, with its rights and
//! its granted libraries, put on the request for whoever handles it.
//!
//! **Closed unless named open.** The addresses that answer to anybody are
//! written out in one short list below, and everything else needs somebody
//! signed in. A route added tomorrow and not thought about is therefore a
//! route nobody can reach, which is the right way round: the other way, a
//! route added in a hurry is a route left open, and nothing says so.
//!
//! A cookie rather than a token the interface holds on to. A token kept in the
//! browser's own store is readable by any piece of script that ends up on the
//! page, a personal stylesheet or an extension included, and it has to be
//! attached by hand to every request; a cookie marked as closed to scripts is
//! neither, and it travels on its own with the pictures and the segments of a
//! film, which is most of what this server serves. It is marked as not
//! travelling to other sites, which is also what stops another site making
//! this one act in somebody's name behind their back.

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderMap, HeaderValue};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::{Json, Router};
use melyxar_app::accounts::{
    OpenedSession, PasswordChange, Remembered, SessionToken, SignedIn, SignedInOrNot,
};
use melyxar_app::AppState;
use melyxar_core::user::User;
use serde::{Deserialize, Serialize};

use crate::error::{Result, ServerError};

/// The name the session travels under.
const COOKIE: &str = "melyxar_session";

/// The furthest ahead it is worth dating a cookie, in seconds.
///
/// Four hundred days, and this server did not pick the number: browsers cap
/// what they honour at that, and anything further ahead is quietly cut back
/// to it. So it is not a lifetime this server is granting, it is the longest
/// a browser will hold on to anything, which is what "stay signed in" means
/// in practice. A browser used at least once a year never reaches it, because
/// every sign in writes the date out afresh.
const THE_LONGEST_A_BROWSER_KEEPS_ONE: i64 = 400 * 24 * 60 * 60;

/// The longest a device name may be before it is cut.
///
/// What a browser says about itself, which is what tells two devices apart in
/// the list somebody revokes from. Some of them are very long and none of them
/// says anything after the first line of it.
const LONGEST_DEVICE_NAME: usize = 200;

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/session",
            axum::routing::post(sign_in).delete(sign_out),
        )
        .route("/api/v1/me", axum::routing::get(me))
        .route("/api/v1/me/password", axum::routing::put(change_password))
        .route("/api/v1/me/name", axum::routing::put(rename))
        .route("/api/v1/me/browser", axum::routing::put(name_the_browser))
        .route(
            "/api/v1/me/avatar",
            axum::routing::put(choose_avatar)
                .delete(remove_avatar)
                // A photo straight off a phone is larger than what any other
                // request is allowed to carry. Past this, the answer is the
                // plain "too large" of the protocol, which the interface words.
                .layer(axum::extract::DefaultBodyLimit::max(
                    melyxar_app::avatars::LARGEST,
                )),
        )
        .route("/api/v1/setup", axum::routing::post(set_this_server_up))
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

/// The addresses that answer without anybody signed in.
///
/// Five of them, each for a reason that cannot be got round.
///
/// A watchdog or a reverse proxy has to be able to ask whether this server is
/// alive without holding an account. The sign in screen has to draw the name
/// and the mark of the server before anybody has signed in, which is what the
/// branding address is for and why it says nothing else, and it offers the
/// names on this server, which is the one deliberate disclosure here and is
/// turned off from two places. Signing in cannot itself require being signed
/// in. And a server nobody has set up yet has no account to sign in as, so the
/// one that makes the first account answers to whoever reaches a brand new
/// server first, and shuts behind itself.
///
/// The interface's own files are open too, and are not in the list: they are
/// the page that draws the sign in screen, and a page nobody can load is a
/// server nobody can sign into. They hold no library and no name: the
/// interface is the same for every installation of this server.
fn answers_to_anybody(path: &str) -> bool {
    // Read without regard to case, so that an address spelt `/API/...` is
    // still an address of the interface's own surface rather than a page of
    // it. The router would not match it either way, but a door has to be shut
    // for a reason rather than by luck.
    const THE_SURFACE: &[u8] = b"/api";
    let bytes = path.as_bytes();
    if bytes.len() < THE_SURFACE.len()
        || !bytes[..THE_SURFACE.len()].eq_ignore_ascii_case(THE_SURFACE)
    {
        return true;
    }
    // The pictures of the accounts, which the sign in screen shows beside
    // their names.
    path.starts_with("/api/v1/public/faces/")
        || matches!(
            path,
            "/api/v1/system/health"
                | "/api/v1/public/branding"
                | "/api/v1/public/names"
                | "/api/v1/session"
                | "/api/v1/setup"
        )
}

/// Turns the cookie on a request into the account behind it.
///
/// Refuses on the spot for anything that is not open, so that a handler cannot
/// forget to ask. Handlers still say what they need: one that takes a
/// [`Viewer`] wants the account, one that takes an [`Administrator`] wants the
/// rights checked too.
pub async fn at_the_gate(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let open = answers_to_anybody(request.uri().path());

    match who_is_asking(&state, request.headers()).await {
        Ok(Some(holder)) => {
            request.extensions_mut().insert(holder);
        }
        Ok(None) if open => {}
        Ok(None) => {
            return ServerError::unauthenticated("nobody is signed in").into_response();
        }
        // The database could not be asked. Letting the request through would
        // be treating a server that is unwell as a server with no door.
        Err(error) => return ServerError::into_response(error),
    }

    next.run(request).await
}

async fn who_is_asking(state: &AppState, headers: &HeaderMap) -> Result<Option<SignedIn>> {
    let Some(token) = token_in(headers) else {
        return Ok(None);
    };
    Ok(melyxar_app::accounts::who_holds(state, &token).await?)
}

/// The session token carried by a request, when it carries one.
fn token_in(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .filter_map(|one| one.split_once('='))
        .find(|(name, _)| name.trim() == COOKIE)
        .map(|(_, value)| value.trim().to_string())
}

/// Whoever is signed in.
///
/// A handler that takes one of these cannot be reached by anybody who is not:
/// the gate above has already refused them.
pub(crate) struct Viewer(pub User);

/// The same, and holding the rights of an administrator.
///
/// Carries nothing on purpose: a handler that takes one has had the rights
/// checked, and one that also needs to know who it is takes a [`Viewer`]
/// beside it.
pub(crate) struct Administrator;

/// Whoever is signed in, and the device they are on: who is watching, for
/// the list of what is being watched.
pub(crate) struct Watcher(pub melyxar_app::watching::Viewer);

/// What the browser behind this request was told about keeping its session.
///
/// For the one handler that hands the same browser a new token: it has to be
/// kept for as long as the one it replaces, and nobody but the row of the
/// device it came from knows how long that was.
pub(crate) struct ThisBrowser(pub Remembered);

impl<S: Send + Sync> FromRequestParts<S> for Viewer {
    type Rejection = ServerError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self> {
        parts
            .extensions
            .get::<SignedIn>()
            .map(|holder| Self(holder.user.clone()))
            .ok_or_else(|| ServerError::unauthenticated("nobody is signed in"))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Watcher {
    type Rejection = ServerError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self> {
        parts
            .extensions
            .get::<SignedIn>()
            .map(|holder| {
                Self(melyxar_app::watching::Viewer {
                    user: holder.user.id,
                    user_name: holder.user.name.clone(),
                    device: holder.device,
                    device_name: holder.device_name.clone(),
                    browser: holder.device_browser.clone(),
                })
            })
            .ok_or_else(|| ServerError::unauthenticated("nobody is signed in"))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for ThisBrowser {
    type Rejection = ServerError;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self> {
        parts
            .extensions
            .get::<SignedIn>()
            .map(|holder| Self(holder.remembered))
            .ok_or_else(|| ServerError::unauthenticated("nobody is signed in"))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Administrator {
    type Rejection = ServerError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self> {
        let Viewer(user) = Viewer::from_request_parts(parts, state).await?;
        if !user.permissions.is_administrator {
            return Err(ServerError::new(
                axum::http::StatusCode::FORBIDDEN,
                melyxar_core::error::ErrorCode::Forbidden,
                "this is an administrator's to do",
            ));
        }
        Ok(Self)
    }
}

// ---------------------------------------------------------------------------
// The cookie
// ---------------------------------------------------------------------------

/// The cookie that carries a session, written for the way this server is
/// reached.
///
/// Closed to scripts, so nothing running on the page can read it. Kept to this
/// site, so another site cannot make this one act in somebody's name behind
/// their back. And marked as needing an encrypted connection only when this
/// server is serving one: a server reached in the clear on a home network,
/// which is how most of them start, would otherwise hand out a cookie the
/// browser refuses to send back, and nobody would ever sign in.
///
/// How long the browser keeps it is the one thing somebody chooses at the
/// door. Remembered, it is dated as far out as a browser will take, which is
/// to say indefinitely: this server refuses no token for its age, so nothing
/// signs that browser out until somebody does it, changes their password, or
/// the upkeep sweeps a session nobody has used in a year. Not remembered, it
/// is given no date at all, and a cookie with no date is one the browser
/// drops the moment it closes: that is the whole of what the box does, and it
/// is the browser rather than this server that honours it, which is what
/// makes it true even for somebody who walks away from a machine that stays
/// on.
fn cookie_carrying(
    token: &SessionToken,
    state: &AppState,
    remembered: Remembered,
) -> HeaderValue {
    let encrypted = matches!(
        state.config().access,
        melyxar_config::AccessMode::Encrypted { .. }
    );
    let written = cookie_written(token.as_text(), encrypted, remembered);
    // Every piece of it is a constant here or a token this server just drew,
    // so there is nothing in it a header cannot hold.
    HeaderValue::from_str(&written).unwrap_or_else(|_| HeaderValue::from_static(""))
}

/// The line itself, apart from where its two answers come from.
///
/// Its own function so that what a browser is actually told can be read back
/// in a test: whether this server is serving an encrypted connection and
/// whether somebody ticked the box are both awkward to stand up, and neither
/// is what could go wrong here.
fn cookie_written(token: &str, encrypted: bool, remembered: Remembered) -> String {
    let how_long = match remembered {
        Remembered::Yes => format!("; Max-Age={THE_LONGEST_A_BROWSER_KEEPS_ONE}"),
        Remembered::UntilTheBrowserCloses => String::new(),
    };
    format!(
        "{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax{how_long}{}",
        if encrypted { "; Secure" } else { "" }
    )
}

/// The same cookie, emptied and dated in the past, which is how a browser is
/// told to forget one.
fn cookie_taken_back() -> HeaderValue {
    HeaderValue::from_static(
        "melyxar_session=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0",
    )
}

/// What to call the device this request came from.
///
/// What the browser says about itself, cut to a length a list can show. It is
/// only ever shown, never decided from.
fn what_asked(headers: &HeaderMap) -> String {
    let said = headers
        .get(header::USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .trim();
    match said.is_empty() {
        true => "an unnamed device".to_string(),
        false => said.chars().take(LONGEST_DEVICE_NAME).collect(),
    }
}

// ---------------------------------------------------------------------------
// The doors
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct WhoAndWhat {
    name: String,
    password: String,
    /// Whether the browser is to keep this session once it is closed. Left
    /// out, it is kept: that is what every client did before the box on the
    /// sign in screen existed, and it is what the box is ticked to.
    #[serde(default = "kept_unless_said_otherwise")]
    remember: bool,
}

fn kept_unless_said_otherwise() -> bool {
    true
}

/// What the person ticked, as the session layer says it.
fn wished_for(remember: bool) -> Remembered {
    match remember {
        true => Remembered::Yes,
        false => Remembered::UntilTheBrowserCloses,
    }
}

/// An account as the interface is told about it.
///
/// Its rights and nothing else about it: they are what the interface hides
/// things by. What somebody has chosen for themselves travels separately,
/// under the preferences, because a screen that changes one does not touch the
/// other.
#[derive(Debug, Serialize)]
struct AccountView {
    id: String,
    name: String,
    is_administrator: bool,
    may_download: bool,
    may_delete: bool,
    may_delete_from_disk: bool,
    /// Where its picture is served, when it has one.
    avatar: Option<String>,
}

impl From<&User> for AccountView {
    fn from(user: &User) -> Self {
        Self {
            id: user.id.to_string(),
            name: user.name.clone(),
            is_administrator: user.permissions.is_administrator,
            may_download: user.permissions.may_download,
            may_delete: user.permissions.may_delete,
            may_delete_from_disk: user.permissions.may_delete_from_disk,
            avatar: user.avatar_path.as_deref().map(crate::images::face_url),
        }
    }
}

async fn sign_in(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(asked): Json<WhoAndWhat>,
) -> Result<Response> {
    match melyxar_app::accounts::sign_in(
        &state,
        &asked.name,
        &asked.password,
        &what_asked(&headers),
        wished_for(asked.remember),
    )
    .await?
    {
        SignedInOrNot::Opened(opened) => Ok(answered_with_a_session(
            &state,
            *opened,
            wished_for(asked.remember),
        )),
        SignedInOrNot::NotAPair => Err(ServerError::unauthenticated(
            "no account answers to that pair",
        )),
        // Told how long to wait, because somebody who has mistyped their own
        // password ten times has to know to come back rather than to try
        // harder.
        SignedInOrNot::HeldBack { seconds } => Err(ServerError::with_details(
            axum::http::StatusCode::TOO_MANY_REQUESTS,
            melyxar_core::error::ErrorCode::TooManyAttempts,
            serde_json::json!({ "seconds": seconds }),
            "held back after too many wrong passwords",
        )),
    }
}

async fn sign_out(State(state): State<AppState>, headers: HeaderMap) -> Result<Response> {
    if let Some(token) = token_in(&headers) {
        melyxar_app::accounts::sign_out(&state, &token).await?;
    }
    // The same answer either way: somebody pressing the button is done, and a
    // session that was already over is not something to tell them about.
    Ok((
        [(header::SET_COOKIE, cookie_taken_back())],
        Json(serde_json::json!({ "signed_out": true })),
    )
        .into_response())
}

async fn me(Viewer(user): Viewer) -> Json<AccountView> {
    Json(AccountView::from(&user))
}

/// Makes the image sent the picture of this account, and answers the account
/// wearing it.
async fn choose_avatar(
    State(state): State<AppState>,
    Viewer(mut user): Viewer,
    image: axum::body::Bytes,
) -> Result<Json<AccountView>> {
    user.avatar_path = Some(melyxar_app::avatars::set(&state, user.id, &image).await?);
    Ok(Json(AccountView::from(&user)))
}

async fn remove_avatar(
    State(state): State<AppState>,
    Viewer(mut user): Viewer,
) -> Result<Json<AccountView>> {
    melyxar_app::avatars::remove(&state, user.id).await?;
    user.avatar_path = None;
    Ok(Json(AccountView::from(&user)))
}

#[derive(Debug, Deserialize)]
struct TheOldAndTheNew {
    current: String,
    wanted: String,
}

async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Viewer(user): Viewer,
    ThisBrowser(remembered): ThisBrowser,
    Json(asked): Json<TheOldAndTheNew>,
) -> Result<Response> {
    let changed = melyxar_app::accounts::change_password(
        &state,
        &user,
        &asked.current,
        &asked.wanted,
        &what_asked(&headers),
        remembered,
    )
    .await?;

    match changed {
        PasswordChange::Changed(token) => Ok(answered_with_a_session(
            &state,
            OpenedSession { token, user },
            remembered,
        )),
        PasswordChange::NotTheCurrentOne => Err(ServerError::unauthenticated(
            "that is not the current password",
        )),
    }
}

#[derive(Debug, Deserialize)]
struct NameBody {
    name: String,
}

/// Gives this account the name asked for, and answers the account wearing it.
async fn rename(
    State(state): State<AppState>,
    Viewer(user): Viewer,
    Json(asked): Json<NameBody>,
) -> Result<Json<AccountView>> {
    let renamed = melyxar_app::accounts::rename(&state, &user, &asked.name).await?;
    Ok(Json(AccountView::from(&renamed)))
}

#[derive(Debug, Deserialize)]
struct BrowserBody {
    /// Absent when the page could tell nothing the line of its browser does
    /// not already say.
    browser: Option<String>,
}

/// Which browser this device is, as its own page found: the line a browser
/// sends about itself cannot tell every one apart.
async fn name_the_browser(
    State(state): State<AppState>,
    Watcher(watcher): Watcher,
    Json(body): Json<BrowserBody>,
) -> Result<Json<serde_json::Value>> {
    melyxar_app::accounts::name_the_browser(
        &state,
        watcher.device,
        watcher.browser.as_deref(),
        body.browser.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "named": true })))
}

/// Makes the first account of a brand new server, and signs it in.
///
/// Signs it in straight away because the alternative is to hand somebody a
/// sign in screen for an account they made three seconds ago, which is asking
/// them to prove they are the person they have just finished being.
async fn set_this_server_up(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(asked): Json<WhoAndWhat>,
) -> Result<Response> {
    let user =
        melyxar_app::accounts::create_the_first_account(&state, &asked.name, &asked.password)
            .await?;

    let SignedInOrNot::Opened(opened) = melyxar_app::accounts::sign_in(
        &state,
        &user.name,
        &asked.password,
        &what_asked(&headers),
        wished_for(asked.remember),
    )
    .await?
    else {
        return Err(ServerError::internal(
            "the account just made would not sign in",
        ));
    };

    Ok(answered_with_a_session(
        &state,
        *opened,
        wished_for(asked.remember),
    ))
}

/// The answer to every door that opens a session: the cookie, and who it is.
fn answered_with_a_session(
    state: &AppState,
    opened: OpenedSession,
    remembered: Remembered,
) -> Response {
    (
        [(
            header::SET_COOKIE,
            cookie_carrying(&opened.token, state, remembered),
        )],
        Json(AccountView::from(&opened.user)),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::id::{DeviceId, UserId};
    use melyxar_core::user::{Permissions, Preferences};

    /// A request somebody is behind, as the gate would have left it.
    fn a_request_from(permissions: Permissions) -> Parts {
        let user = User {
            id: UserId::new(),
            name: "victor".to_string(),
            avatar_path: None,
            permissions,
            preferences: Preferences::default(),
            created_at: melyxar_core::time::now(),
        };
        let mut request = axum::http::Request::new(());
        request.extensions_mut().insert(SignedIn {
            user,
            device: DeviceId::new(),
            device_name: "a browser".to_string(),
            device_browser: None,
            remembered: Remembered::Yes,
        });
        request.into_parts().0
    }

    #[tokio::test]
    async fn a_request_nobody_is_behind_reaches_neither() {
        let mut parts = axum::http::Request::new(()).into_parts().0;
        let refused = Viewer::from_request_parts(&mut parts, &())
            .await
            .err()
            .expect("nobody is signed in");
        assert_eq!(refused.code(), "unauthenticated");

        let refused = Administrator::from_request_parts(&mut parts, &())
            .await
            .err()
            .expect("nobody is signed in");
        assert_eq!(refused.code(), "unauthenticated");
    }

    #[tokio::test]
    async fn an_ordinary_viewer_is_refused_where_an_administrator_is_wanted() {
        // The journal names every film by its own name and its whole path,
        // and the folder picker walks the server's disk. Neither is anybody's
        // but whoever runs this server.
        let mut parts = a_request_from(Permissions::viewer());
        assert!(
            Viewer::from_request_parts(&mut parts, &()).await.is_ok(),
            "an ordinary account is still somebody"
        );

        let refused = Administrator::from_request_parts(&mut parts, &())
            .await
            .err()
            .expect("an ordinary account is not an administrator");
        assert_eq!(refused.code(), "forbidden");
    }

    #[tokio::test]
    async fn an_administrator_reaches_both() {
        let mut parts = a_request_from(Permissions::administrator());
        assert!(Viewer::from_request_parts(&mut parts, &()).await.is_ok());
        assert!(Administrator::from_request_parts(&mut parts, &())
            .await
            .is_ok());
    }

    #[test]
    fn every_route_this_module_declares_is_one_a_router_accepts() {
        let _ = router();
    }

    /// A refusal the interface has no words for reaches the screen as its
    /// key, in front of somebody who has just chosen a photo.
    #[test]
    fn every_reason_a_picture_is_refused_for_has_words_in_both_languages() {
        let words = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/i18n.ts"),
        )
        .expect("the words of the interface");

        for refused in melyxar_app::avatars::Refused::ALL {
            let key = format!("refused.avatar.{}", refused.as_str());
            assert_eq!(
                words.matches(&format!("\"{key}\":")).count(),
                2,
                "{key} needs a sentence in English and one in French"
            );
        }
    }

    #[test]
    fn the_token_is_picked_out_of_whatever_else_the_cookie_carries() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("something=else; melyxar_session=a token; more=still"),
        );
        assert_eq!(token_in(&headers).as_deref(), Some("a token"));
    }

    /// The whole of what the box on the sign in screen does.
    ///
    /// A cookie with a date on it is one the browser writes down and has again
    /// the next morning; a cookie with no date at all is one it drops as it
    /// closes. Nothing else about the two differs, and nothing else may: the
    /// session itself lives exactly as long either way.
    #[test]
    fn a_browser_told_to_remember_is_given_a_date_and_one_told_not_to_is_not() {
        let remembered = cookie_written("a token", false, Remembered::Yes);
        assert!(
            remembered.contains(&format!("Max-Age={THE_LONGEST_A_BROWSER_KEEPS_ONE}")),
            "a remembered browser is asked to keep it as long as it will: {remembered}"
        );

        let for_now = cookie_written("a token", false, Remembered::UntilTheBrowserCloses);
        assert!(
            !for_now.contains("Max-Age"),
            "a browser that was not to remember is given no date at all: {for_now}"
        );

        // What the two share, which is everything else. Closed to scripts and
        // kept to this site whichever was chosen: the box says how long, never
        // how safe.
        for written in [&remembered, &for_now] {
            assert!(written.starts_with("melyxar_session=a token; Path=/"), "{written}");
            assert!(written.contains("HttpOnly"), "{written}");
            assert!(written.contains("SameSite=Lax"), "{written}");
        }
    }

    /// Only a server serving an encrypted connection may ask for the cookie
    /// back over one. Asked for on a server reached in the clear, which is how
    /// most of them start, the browser would keep the cookie and never send it.
    #[test]
    fn the_cookie_is_held_to_an_encrypted_connection_only_where_there_is_one() {
        assert!(!cookie_written("a token", false, Remembered::Yes).contains("Secure"));
        assert!(cookie_written("a token", true, Remembered::Yes).contains("Secure"));
        assert!(
            cookie_written("a token", true, Remembered::UntilTheBrowserCloses).contains("Secure"),
            "the two choices are about how long, never about how safe"
        );
    }

    #[test]
    fn a_request_carrying_no_session_carries_no_token() {
        assert_eq!(token_in(&HeaderMap::new()), None);

        let mut headers = HeaderMap::new();
        headers.insert(header::COOKIE, HeaderValue::from_static("something=else"));
        assert_eq!(token_in(&headers), None);

        // A name that merely ends with ours is not ours.
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            HeaderValue::from_static("not_melyxar_session=a token"),
        );
        assert_eq!(token_in(&headers), None);
    }

    #[test]
    fn everything_is_closed_unless_it_is_one_of_the_five() {
        for open in [
            "/api/v1/system/health",
            "/api/v1/public/branding",
            "/api/v1/public/names",
            "/api/v1/public/faces/an-account/avatar-abc.webp",
            "/api/v1/session",
            "/api/v1/setup",
        ] {
            assert!(answers_to_anybody(open), "{open} has to answer to anybody");
        }

        // A route nobody thought about is a route nobody can reach.
        for closed in [
            "/api/v1/works",
            "/api/v1/home",
            "/api/v1/libraries",
            "/api/v1/images/works/w/poster.webp",
            "/api/v1/system/journal",
            "/api/v1/system/diagnostics",
            "/api/v1/preferences",
            "/api/v1/public",
            "/api/v1/public/accounts",
            "/api/v1/stream/a-session/playlist.m3u8",
            "/api/v1/something/invented/tomorrow",
        ] {
            assert!(!answers_to_anybody(closed), "{closed} must need an account");
        }

        // Shouted, which the router would not match either way. A door has
        // to be shut for a reason rather than by luck.
        for shouted in ["/API/v1/home", "/Api/V1/Works", "/APi/anything"] {
            assert!(
                !answers_to_anybody(shouted),
                "{shouted} must not pass for a page of the interface"
            );
        }

        // The interface itself, which is the page that draws the sign in
        // screen.
        for page in ["/", "/index.html", "/assets/interface.js", "/work/abcdef"] {
            assert!(answers_to_anybody(page));
        }
    }

    #[test]
    fn a_device_is_named_after_what_the_browser_says_and_never_runs_long() {
        let mut headers = HeaderMap::new();
        assert_eq!(what_asked(&headers), "an unnamed device");

        headers.insert(header::USER_AGENT, HeaderValue::from_static("a browser"));
        assert_eq!(what_asked(&headers), "a browser");

        let very_long = "b".repeat(LONGEST_DEVICE_NAME * 3);
        headers.insert(
            header::USER_AGENT,
            HeaderValue::from_str(&very_long).expect("a header"),
        );
        assert_eq!(what_asked(&headers).chars().count(), LONGEST_DEVICE_NAME);
    }
}
