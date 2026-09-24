//! The activity journal: what people and the server did, for the
//! administrator. See the decision on the activity journal.
//!
//! Written as it happens and read by the administration only. A line carries
//! the name of the account and the title it is about, so it still reads once
//! either is gone. Writing one never makes what it tells of fail: a line that
//! could not be written is said in the technical journal and nothing else.

use std::time::Duration;

use melyxar_core::id::{ActivityId, DeviceId, UserId, WorkId};
use melyxar_core::job::{JobKind, JobState};
use melyxar_core::time::{Millis, Timestamp};
use melyxar_core::work::WorkKind;
use melyxar_database::activity::NewActivity;
use melyxar_database::Database;
use serde_json::{json, Value};

use crate::{AppState, Result};

/// How many titles a line about a deletion names; past that, it counts.
const TITLES_NAMED: usize = 5;

/// Something worth a line.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    ServerStarted,
    ServerStopped,
    /// The device is named here so its line can be told later which browser
    /// it really is: the page says so only once it is signed in.
    SignedIn {
        user: UserId,
        user_name: String,
        device: String,
        device_id: DeviceId,
    },
    /// A name and a password that were not a pair. The name is what was
    /// typed, which may be nobody's.
    SignInRefused { name: String, device: String },
    /// An account held back after too many wrong passwords.
    SignInHeldBack { user: UserId, user_name: String, device: String },
    SignedOut {
        user: UserId,
        user_name: String,
        device: String,
        browser: Option<String>,
    },
    PasswordChanged { user: UserId, user_name: String },
    AccountCreated { user: UserId, user_name: String },
    AccountRemoved { user_name: String },
    AccountRenamed { user: UserId, from: String, to: String },
    Watched(Viewing),
    TaskEnded(TaskEnded),
    WorksDeleted {
        user: UserId,
        user_name: String,
        /// The titles asked for, the first few of them.
        titles: Vec<String>,
        /// Every work that went, those under a series included.
        works: i64,
        from_the_disk: bool,
    },
}

/// One film watched on one device, from its start to its end.
#[derive(Debug, Clone, PartialEq)]
pub struct Viewing {
    pub user: UserId,
    pub user_name: String,
    /// What the browser said it was, and the browser its page found.
    pub device: String,
    pub browser: Option<String>,
    pub work: WorkId,
    /// How it reached the device, when what was decided for it was heard.
    pub method: Option<&'static str>,
    /// How long it really played, pauses left out.
    pub played: Duration,
    /// Where it stopped, and how long the film is when that is known.
    pub reached: Millis,
    pub length: Option<Millis>,
    pub started_at: Timestamp,
    pub stopped_by_administrator: bool,
}

/// One task of the server, over.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskEnded {
    pub kind: JobKind,
    pub state: JobState,
    pub reason: Option<String>,
    /// What it was about: a library or a work, by identifier.
    pub target: Option<String>,
    pub took: Duration,
}

/// The families the journal is read by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Category {
    Access,
    Playback,
    Library,
    Server,
}

impl Category {
    pub fn from_word(word: &str) -> Option<Self> {
        match word {
            "access" => Some(Self::Access),
            "playback" => Some(Self::Playback),
            "library" => Some(Self::Library),
            "server" => Some(Self::Server),
            _ => None,
        }
    }

    fn kinds(self) -> &'static [&'static str] {
        match self {
            Self::Access => &[
                SIGNED_IN,
                SIGN_IN_REFUSED,
                SIGN_IN_HELD_BACK,
                SIGNED_OUT,
                PASSWORD_CHANGED,
                ACCOUNT_CREATED,
                ACCOUNT_REMOVED,
                ACCOUNT_RENAMED,
            ],
            Self::Playback => &[WATCHED],
            Self::Library => &[TASK_FINISHED, TASK_FAILED, TASK_STOPPED, WORKS_DELETED],
            Self::Server => &[SERVER_STARTED, SERVER_STOPPED],
        }
    }
}

const SERVER_STARTED: &str = "server_started";
const SERVER_STOPPED: &str = "server_stopped";
const SIGNED_IN: &str = "signed_in";
pub(crate) const SIGN_IN_REFUSED: &str = "sign_in_refused";
pub(crate) const SIGN_IN_HELD_BACK: &str = "sign_in_held_back";
const SIGNED_OUT: &str = "signed_out";
const PASSWORD_CHANGED: &str = "password_changed";
const ACCOUNT_CREATED: &str = "account_created";
const ACCOUNT_REMOVED: &str = "account_removed";
const ACCOUNT_RENAMED: &str = "account_renamed";
const WATCHED: &str = "watched";
const TASK_FINISHED: &str = "task_finished";
pub(crate) const TASK_FAILED: &str = "task_failed";
const TASK_STOPPED: &str = "task_stopped";
const WORKS_DELETED: &str = "works_deleted";

/// How much a line asks of whoever reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Information,
    Attention,
    Trouble,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Information => "information",
            Self::Attention => "attention",
            Self::Trouble => "trouble",
        }
    }

    fn of(kind: &str) -> Self {
        match kind {
            SIGN_IN_REFUSED => Self::Attention,
            SIGN_IN_HELD_BACK | TASK_FAILED => Self::Trouble,
            _ => Self::Information,
        }
    }
}

/// A line as it is about to be written: its columns, and what goes in its
/// details.
struct Line {
    kind: &'static str,
    user: Option<UserId>,
    work: Option<WorkId>,
    device: Option<String>,
    details: Value,
}

fn task_kind(state: JobState) -> &'static str {
    match state {
        JobState::Failed => TASK_FAILED,
        JobState::Cancelled => TASK_STOPPED,
        _ => TASK_FINISHED,
    }
}

/// What the line about a viewing says of the film, looked up now so it
/// reads the same once the film is gone.
async fn what_was_watched(database: &Database, work: WorkId) -> Result<Value> {
    let Some(watched) = database.work(work).await? else {
        return Ok(json!({}));
    };
    if watched.kind != WorkKind::Episode {
        return Ok(json!({ "title": watched.title, "year": watched.release_year }));
    }
    let ancestry = database.ancestry_of(work).await?;
    let season = ancestry.iter().find(|up| up.kind == WorkKind::Season);
    let series = ancestry.iter().find(|up| up.kind == WorkKind::Series);
    Ok(json!({
        "title": watched.title,
        "series": series.map(|up| up.title.clone()),
        "season": season.and_then(|up| up.ordinal),
        "episode": watched.ordinal,
    }))
}

/// What a task was about, by name, looked up now for the same reason.
async fn what_the_task_was_on(database: &Database, target: Option<&str>) -> Result<Value> {
    let Some(target) = target else {
        return Ok(Value::Null);
    };
    if let Ok(library) = target.parse() {
        if let Some(found) = database
            .list_libraries()
            .await?
            .into_iter()
            .find(|library_found| library_found.id == library)
        {
            return Ok(json!({ "library": found.name }));
        }
    }
    if let Ok(work) = target.parse() {
        if let Some(found) = database.work(work).await? {
            return Ok(json!({ "title": found.title }));
        }
    }
    Ok(Value::Null)
}

async fn line_of(database: &Database, event: Event) -> Result<Line> {
    let line = |kind, user, device: Option<String>, details| Line {
        kind,
        user,
        work: None,
        device,
        details,
    };
    Ok(match event {
        Event::ServerStarted => line(
            SERVER_STARTED,
            None,
            None,
            json!({ "version": melyxar_core::BUILD }),
        ),
        Event::ServerStopped => line(SERVER_STOPPED, None, None, json!({})),
        Event::SignedIn {
            user,
            user_name,
            device,
            device_id,
        } => line(
            SIGNED_IN,
            Some(user),
            Some(device),
            json!({ "user_name": user_name, "device_id": device_id.to_db_string() }),
        ),
        Event::SignInRefused { name, device } => {
            line(SIGN_IN_REFUSED, None, Some(device), json!({ "user_name": name }))
        }
        Event::SignInHeldBack { user, user_name, device } => line(
            SIGN_IN_HELD_BACK,
            Some(user),
            Some(device),
            json!({ "user_name": user_name }),
        ),
        Event::SignedOut {
            user,
            user_name,
            device,
            browser,
        } => line(
            SIGNED_OUT,
            Some(user),
            Some(device),
            json!({ "user_name": user_name, "browser": browser }),
        ),
        Event::PasswordChanged { user, user_name } => {
            line(PASSWORD_CHANGED, Some(user), None, json!({ "user_name": user_name }))
        }
        Event::AccountCreated { user, user_name } => {
            line(ACCOUNT_CREATED, Some(user), None, json!({ "user_name": user_name }))
        }
        Event::AccountRemoved { user_name } => {
            line(ACCOUNT_REMOVED, None, None, json!({ "user_name": user_name }))
        }
        Event::AccountRenamed { user, from, to } => line(
            ACCOUNT_RENAMED,
            Some(user),
            None,
            json!({ "user_name": to, "previous_name": from }),
        ),
        Event::Watched(viewing) => {
            let mut details = what_was_watched(database, viewing.work).await?;
            details["user_name"] = json!(viewing.user_name);
            details["browser"] = json!(viewing.browser);
            details["method"] = json!(viewing.method);
            details["played_seconds"] = json!(viewing.played.as_secs());
            details["reached_seconds"] = json!(viewing.reached.as_seconds_f64().round());
            details["length_seconds"] =
                json!(viewing.length.map(|length| length.as_seconds_f64().round()));
            details["started_at"] = json!(melyxar_core::time::to_text(viewing.started_at));
            details["stopped_by_administrator"] = json!(viewing.stopped_by_administrator);
            Line {
                kind: WATCHED,
                user: Some(viewing.user),
                work: Some(viewing.work),
                device: Some(viewing.device),
                details,
            }
        }
        Event::TaskEnded(task) => line(
            task_kind(task.state),
            None,
            None,
            json!({
                "task": task.kind.as_str(),
                "on": what_the_task_was_on(database, task.target.as_deref()).await?,
                "reason": task.reason,
                "took_seconds": task.took.as_secs(),
            }),
        ),
        Event::WorksDeleted {
            user,
            user_name,
            titles,
            works,
            from_the_disk,
        } => line(
            WORKS_DELETED,
            Some(user),
            None,
            json!({
                "user_name": user_name,
                "titles": titles,
                "works": works,
                "from_the_disk": from_the_disk,
            }),
        ),
    })
}

/// Where lines are written, and what is told each time one is: the
/// administration's live line, so a page shows it the moment it is written.
///
/// Its own handle rather than the whole server, for what writes lines and is
/// built before the server is, such as the runner of the tasks.
#[derive(Clone)]
pub struct Journal {
    database: Database,
    written: std::sync::Arc<tokio::sync::watch::Sender<u64>>,
}

impl Journal {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            written: std::sync::Arc::new(tokio::sync::watch::channel(0).0),
        }
    }

    /// Writes one line, and tells whoever follows the journal.
    pub async fn write(&self, event: Event) {
        match write_line(&self.database, event).await {
            Ok(()) => self
                .written
                .send_modify(|count| *count = count.wrapping_add(1)),
            Err(error) => {
                tracing::warn!(%error, "a line of the activity journal could not be written");
            }
        }
    }

    /// Gives the line of a device's sign in the browser its page found, and
    /// tells whoever follows the journal when a line changed.
    pub async fn browser_found(&self, device: DeviceId, browser: &str) {
        match self
            .database
            .name_the_browser_of_a_sign_in(SIGNED_IN, device, browser)
            .await
        {
            Ok(0) => {}
            Ok(_) => self
                .written
                .send_modify(|count| *count = count.wrapping_add(1)),
            Err(error) => {
                tracing::warn!(%error, "the line of a sign in could not be given its browser");
            }
        }
    }

    /// Moved on every time a line is written.
    pub fn written(&self) -> tokio::sync::watch::Receiver<u64> {
        self.written.subscribe()
    }
}

async fn write_line(database: &Database, event: Event) -> Result<()> {
    let line = line_of(database, event).await?;
    let details = line.details.to_string();
    database
        .record_activity(&NewActivity {
            at: melyxar_core::time::now(),
            kind: line.kind,
            user_id: line.user,
            work_id: line.work,
            device_name: line.device.as_deref(),
            details: Some(&details),
        })
        .await?;
    Ok(())
}

/// Writes one line.
pub async fn record(state: &AppState, event: Event) {
    state.journal().write(event).await;
}

/// Writes one line without waiting for it, from somewhere that cannot wait.
pub fn record_later(state: &AppState, event: Event) {
    let journal = state.journal().clone();
    tokio::spawn(async move { journal.write(event).await });
}

/// Moved on every time a line is written, for the administration's live
/// line.
pub fn written(state: &AppState) -> tokio::sync::watch::Receiver<u64> {
    state.journal().written()
}

/// The words of the titles about to be deleted, for the line that says so.
pub async fn titles_of(state: &AppState, works: &[WorkId]) -> Result<Vec<String>> {
    let mut titles = Vec::new();
    for &work in works.iter().take(TITLES_NAMED) {
        if let Some(found) = state.database().work(work).await? {
            titles.push(found.title);
        }
    }
    Ok(titles)
}

/// One line as the administration reads it.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: ActivityId,
    pub at: Timestamp,
    pub kind: String,
    pub level: Level,
    pub user: Option<UserId>,
    pub work: Option<WorkId>,
    pub device: Option<String>,
    /// Everything else the line says, as it was written.
    pub details: Value,
}

/// The latest lines of these families, or of all of them when none is
/// asked for, older than the last line already shown when one is given.
pub async fn page(
    state: &AppState,
    categories: &[Category],
    before: Option<ActivityId>,
    most: i64,
) -> Result<Vec<Entry>> {
    let kinds: Vec<&str> = categories
        .iter()
        .flat_map(|category| category.kinds().iter().copied())
        .collect();
    let lines = state.database().activity_page(&kinds, before, most).await?;
    Ok(lines
        .into_iter()
        .map(|line| Entry {
            level: Level::of(&line.kind),
            details: line
                .details
                .as_deref()
                .and_then(|text| serde_json::from_str(text).ok())
                .unwrap_or(Value::Null),
            id: line.id,
            at: line.at,
            kind: line.kind,
            user: line.user_id,
            work: line.work_id,
            device: line.device_name,
        })
        .collect())
}

/// The fewest and the most days the journal may be set to keep.
pub const KEPT_DAYS: std::ops::RangeInclusive<i64> = 7..=3650;

/// How many days the journal keeps.
pub async fn kept_days(state: &AppState) -> Result<i64> {
    Ok(state.database().server_settings().await?.activity_retention_days)
}

/// Sets how many days the journal keeps, refusing what is out of bounds.
pub async fn keep_days(state: &AppState, days: i64) -> Result<i64> {
    if !KEPT_DAYS.contains(&days) {
        return Err(crate::AppError::Domain(melyxar_core::Error::invalid_input(
            "the journal keeps between a week and ten years",
        )));
    }
    state.database().set_activity_retention_days(days).await?;
    Ok(days)
}

/// Throws away what is older than the server keeps.
pub async fn forget_the_old(state: &AppState) {
    let forgotten = async {
        let days = state.database().server_settings().await?.activity_retention_days;
        let before = melyxar_core::time::now() - time::Duration::days(days.max(1));
        Ok::<_, crate::AppError>(state.database().forget_activity_before(before).await?)
    };
    match forgotten.await {
        Ok(0) => tracing::debug!("nothing in the activity journal was old enough to go"),
        Ok(forgotten) => tracing::info!(forgotten, "old lines of the activity journal were forgotten"),
        Err(error) => tracing::warn!(%error, "the activity journal could not be purged"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{Config, Directories, LibraryConfig, RootConfig};

    /// A server holding one library, with the folder it lives in kept as long
    /// as the test does.
    async fn a_server() -> (tempfile::TempDir, AppState) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = Config {
            directories: Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
                ..Default::default()
            },
            libraries: vec![LibraryConfig {
                name: "Films".into(),
                kind: "movies".into(),
                metadata_language: "fr".into(),
                roots: vec![RootConfig {
                    label: "disk-one".into(),
                    path: directory.path().join("films"),
                }],
            }],
            ..Config::default()
        };
        crate::startup::prepare_directories(&config).expect("directories prepared");
        let database = Database::open_in_memory().await.expect("database opens");
        crate::startup::reconcile_libraries(&database, &config)
            .await
            .expect("libraries reconciled");
        (directory, AppState::new(config, database, None, None))
    }

    async fn a_film(state: &AppState, title: &str) -> WorkId {
        let database = state.database();
        let library = database
            .library_by_name("Films")
            .await
            .expect("read")
            .expect("declared");
        database
            .create_work(library.id, WorkKind::Movie, title, title, Some(2019))
            .await
            .expect("work created")
            .id
    }

    #[tokio::test]
    async fn a_viewing_is_written_with_what_was_watched_and_read_back_in_its_family() {
        let (_held, state) = a_server().await;
        let work = a_film(&state, "Quiet Harbour").await;
        let user = state
            .database()
            .create_user("somebody", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created")
            .id;
        record(
            &state,
            Event::Watched(Viewing {
                user,
                user_name: "somebody".to_string(),
                device: "a browser".to_string(),
                browser: Some("Brave".to_string()),
                work,
                method: Some("direct_play"),
                played: Duration::from_secs(1500),
                reached: Millis::from_seconds_f64(1620.4),
                length: Some(Millis::from_seconds_f64(6000.0)),
                started_at: melyxar_core::time::now(),
                stopped_by_administrator: false,
            }),
        )
        .await;
        record(&state, Event::ServerStarted).await;
        record(
            &state,
            Event::TaskEnded(TaskEnded {
                kind: JobKind::ScanLibrary,
                state: JobState::Succeeded,
                reason: None,
                target: Some(
                    state
                        .database()
                        .library_by_name("Films")
                        .await
                        .expect("read")
                        .expect("declared")
                        .id
                        .to_string(),
                ),
                took: Duration::from_secs(40),
            }),
        )
        .await;
        let library = page(&state, &[Category::Library], None, 10).await.expect("read");
        assert_eq!(library[0].kind, "task_finished");
        assert_eq!(library[0].details["on"]["library"], "Films", "named as it was then");

        let watched = page(&state, &[Category::Playback], None, 10).await.expect("read");
        assert_eq!(watched.len(), 1, "one family only");
        let line = &watched[0];
        assert_eq!(line.kind, "watched");
        assert_eq!(line.level, Level::Information);
        assert_eq!(line.work, Some(work));
        assert_eq!(line.device.as_deref(), Some("a browser"));
        assert_eq!(line.details["title"], "Quiet Harbour");
        assert_eq!(line.details["user_name"], "somebody");
        assert_eq!(line.details["played_seconds"], 1500);
        assert_eq!(line.details["reached_seconds"], 1620.0);
        assert_eq!(line.details["method"], "direct_play");

        assert_eq!(page(&state, &[], None, 10).await.expect("read").len(), 3);
    }

    #[tokio::test]
    async fn every_line_written_is_told_to_whoever_follows_the_journal() {
        let (_held, state) = a_server().await;
        let followed = written(&state);
        assert!(!followed.has_changed().expect("open"));
        record(&state, Event::ServerStarted).await;
        assert!(followed.has_changed().expect("open"), "told at once");
    }

    #[tokio::test]
    async fn a_refusal_and_a_failed_task_ask_more_of_their_reader() {
        let (_held, state) = a_server().await;
        record(
            &state,
            Event::SignInRefused {
                name: "nobody".to_string(),
                device: "a browser".to_string(),
            },
        )
        .await;
        record(
            &state,
            Event::TaskEnded(TaskEnded {
                kind: JobKind::ScanLibrary,
                state: JobState::Failed,
                reason: Some("a folder could not be read".to_string()),
                target: None,
                took: Duration::from_secs(3),
            }),
        )
        .await;

        let lines = page(&state, &[Category::Access, Category::Library], None, 10)
            .await
            .expect("read");
        assert_eq!(lines[0].kind, "task_failed");
        assert_eq!(lines[0].level, Level::Trouble);
        assert_eq!(lines[0].details["task"], "scan_library");
        assert_eq!(lines[1].kind, "sign_in_refused");
        assert_eq!(lines[1].level, Level::Attention);
        assert_eq!(lines[1].details["user_name"], "nobody");
    }
}
