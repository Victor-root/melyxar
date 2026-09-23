//! What the summary at the top of the administration says about the server.
//!
//! One read for the whole of it, cheap enough to be asked for every few
//! seconds while the page is open. Every light it gives is checked when it is
//! asked for rather than copied from what the server found when it started: a
//! light that stays green after what it watches has fallen over is worse than
//! no light. None of the checks reads a disk of films, so a sleeping one is
//! left asleep.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use melyxar_core::time::Timestamp;
use serde::Serialize;

use crate::{AppState, Result};

/// How far back a device counts as used today.
const A_DAY: time::Duration = time::Duration::hours(24);

/// How full a disk may get before it is worth a look.
const NEARLY_FULL: f64 = 0.9;

#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    pub server_name: String,
    pub version: &'static str,
    /// When this process started, which is what the time it has been up is
    /// counted from.
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: Timestamp,
    /// Whether the database runs the way it has to, readers never waiting on
    /// the one writer, and still takes what it is given.
    pub database_ready: bool,
    pub media_tools: MediaTools,
    pub accounts: i64,
    pub devices: i64,
    /// Devices used in the last day.
    pub devices_today: i64,
    /// Everything that failed its check, each by name. Empty is what "all is
    /// well" means, and nothing else does.
    pub worries: Vec<Worry>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaTools {
    /// Whether both tools are still there and may be run.
    pub found: bool,
    pub version: Option<String>,
    /// How the graphics card is reached when one was proven to work, by the
    /// name of its way in (`vaapi`, `qsv`).
    pub card: Option<&'static str>,
    /// Whether that card still opens.
    pub card_opens: bool,
}

/// One thing that failed its check.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Worry {
    /// The database is not in the mode it has to run in.
    DatabaseMode,
    /// The database refused the last thing it was asked to keep.
    DatabaseRefusesWrites,
    /// A media tool is gone, or may no longer be run.
    MediaToolsMissing,
    /// The card proven at start up no longer opens.
    CardUnreachable,
    /// A folder of a library is not where it was.
    FolderMissing { label: String },
    /// A disk the server uses is nearly full.
    DiskNearlyFull { mount: String, used: f64 },
}

/// Whether a tool is still a file this server may run.
fn may_run(tool: &Path) -> bool {
    std::fs::metadata(tool)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}

/// Whether the card still opens, the way the tool opens it to convert a film:
/// for reading and writing.
fn card_opens(device: &Path) -> bool {
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(device)
        .is_ok()
}

pub async fn collect(state: &AppState) -> Result<Overview> {
    let database = state.database();
    let settings = database.server_settings().await?;
    let journal_mode = database.journal_mode().await?;
    let devices = database
        .device_counts(melyxar_core::time::now() - A_DAY)
        .await?;
    let capabilities = state.capabilities();
    let card = capabilities.and_then(melyxar_ffmpeg::Capabilities::card);

    let tools: Vec<PathBuf> = state
        .tools()
        .map(|tools| vec![tools.ffmpeg.clone(), tools.ffprobe.clone()])
        .unwrap_or_default();
    let device = card.map(|card| card.device.clone());
    let (tools_found, card_reachable) = tokio::task::spawn_blocking(move || {
        (
            !tools.is_empty() && tools.iter().all(|tool| may_run(tool)),
            device.as_deref().is_some_and(card_opens),
        )
    })
    .await
    .unwrap_or((false, false));

    let measuring = state.measuring();
    let wal = journal_mode.eq_ignore_ascii_case("wal");
    let writes_refused = measuring.writes_refused();

    let mut worries = Vec::new();
    if !wal {
        worries.push(Worry::DatabaseMode);
    }
    if writes_refused {
        worries.push(Worry::DatabaseRefusesWrites);
    }
    if !tools_found {
        worries.push(Worry::MediaToolsMissing);
    }
    if card.is_some() && !card_reachable {
        worries.push(Worry::CardUnreachable);
    }
    worries.extend(
        measuring
            .missing_folders()
            .into_iter()
            .map(|label| Worry::FolderMissing { label }),
    );
    worries.extend(measuring.disks().into_iter().filter_map(|disk| {
        let used = disk.used();
        (disk.total_bytes > 0 && used >= NEARLY_FULL).then_some(Worry::DiskNearlyFull {
            mount: disk.mount,
            used,
        })
    }));

    Ok(Overview {
        server_name: settings.server_name,
        version: melyxar_core::BUILD,
        started_at: state.started_at(),
        database_ready: wal && !writes_refused,
        media_tools: MediaTools {
            found: tools_found,
            version: capabilities.map(|capabilities| capabilities.version.clone()),
            card: card.map(|card| card.way.as_str()),
            card_opens: card_reachable,
        },
        accounts: database.user_count().await?,
        devices: devices.signed_in,
        devices_today: devices.used_lately,
        worries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::Config;
    use melyxar_core::user::Permissions;
    use melyxar_database::sessions::Remembered;
    use melyxar_database::Database;

    #[tokio::test]
    async fn the_summary_counts_accounts_and_the_devices_used_today() {
        let database = Database::open_in_memory().await.expect("database opens");
        let user = database
            .create_user("somebody", None, &Permissions::administrator())
            .await
            .expect("account created");
        let now = melyxar_core::time::now();
        for (fingerprint, at) in [("today", now), ("last week", now - time::Duration::days(7))] {
            database
                .open_session(user.id, "a browser", fingerprint, Remembered::Yes, at)
                .await
                .expect("session opened");
        }
        let state = AppState::new(Config::default(), database, None, None);

        let overview = collect(&state).await.expect("collected");

        assert_eq!(overview.accounts, 1);
        assert_eq!(overview.devices, 2);
        assert_eq!(overview.devices_today, 1);
        assert!(!overview.media_tools.found);
        assert!(overview.media_tools.card.is_none());
        assert_eq!(
            overview.worries,
            vec![Worry::DatabaseMode, Worry::MediaToolsMissing],
            "a database kept in memory and no tools are said, and nothing else is"
        );
        assert!(overview.started_at <= melyxar_core::time::now());
    }
}

#[cfg(test)]
mod checks {
    use super::*;

    #[test]
    fn a_tool_counts_only_while_it_is_a_file_that_may_be_run() {
        let here = tempfile::tempdir().expect("folder");
        let tool = here.path().join("ffmpeg");
        std::fs::write(&tool, b"#!/bin/sh\n").expect("written");
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o644)).expect("mode");
        assert!(!may_run(&tool), "a file nobody may run is not a tool");
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).expect("mode");
        assert!(may_run(&tool));
        std::fs::remove_file(&tool).expect("removed");
        assert!(!may_run(&tool), "a tool that went away is not found");
        assert!(!may_run(here.path()), "a folder is not a tool");
    }

    #[test]
    fn a_card_that_is_not_there_does_not_open() {
        assert!(!card_opens(Path::new("/nowhere/renderD128")));
    }
}
