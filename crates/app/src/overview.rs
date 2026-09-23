//! What the summary at the top of the administration says about the server.
//!
//! One read for the whole of it, cheap enough to be asked for every few
//! seconds while the page is open: counts the database already keeps, and
//! what the server learned about its tools when it started. Nothing here walks
//! a disk or runs a tool, which is what the diagnostic report is for.

use melyxar_core::time::Timestamp;
use serde::Serialize;

use crate::{AppState, Result};

/// How far back a device counts as used today.
const A_DAY: time::Duration = time::Duration::hours(24);

#[derive(Debug, Clone, Serialize)]
pub struct Overview {
    pub server_name: String,
    pub version: &'static str,
    /// When this process started, which is what the time it has been up is
    /// counted from.
    #[serde(with = "time::serde::rfc3339")]
    pub started_at: Timestamp,
    /// Whether the database runs the way it has to: readers never waiting on
    /// the one writer.
    pub database_ready: bool,
    pub database_bytes: i64,
    pub media_tools: MediaTools,
    pub accounts: i64,
    pub devices: i64,
    /// Devices used in the last day.
    pub devices_today: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaTools {
    pub found: bool,
    pub version: Option<String>,
    /// How the graphics card is reached when one was proven to work, by the
    /// name of its way in (`vaapi`, `qsv`).
    pub card: Option<&'static str>,
}

pub async fn collect(state: &AppState) -> Result<Overview> {
    let database = state.database();
    let settings = database.server_settings().await?;
    let journal_mode = database.journal_mode().await?;
    let devices = database
        .device_counts(melyxar_core::time::now() - A_DAY)
        .await?;
    let capabilities = state.capabilities();

    Ok(Overview {
        server_name: settings.server_name,
        version: melyxar_core::BUILD,
        started_at: state.started_at(),
        database_ready: journal_mode.eq_ignore_ascii_case("wal"),
        database_bytes: database.size_bytes().await?,
        media_tools: MediaTools {
            found: state.tools().is_some(),
            version: capabilities.map(|capabilities| capabilities.version.clone()),
            card: capabilities
                .and_then(melyxar_ffmpeg::Capabilities::card)
                .map(|card| card.way.as_str()),
        },
        accounts: database.user_count().await?,
        devices: devices.signed_in,
        devices_today: devices.used_lately,
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
        assert!(overview.started_at <= melyxar_core::time::now());
    }
}
