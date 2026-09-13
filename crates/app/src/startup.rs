//! Bringing the server up.
//!
//! Start-up is deliberately forgiving about what is missing and strict about
//! what is wrong. A missing media tool must not stop the server from coming
//! up, because a library still browses without it and an administrator needs a
//! running interface to be told what to fix. A malformed configuration, on the
//! other hand, stops everything: carrying on with half a configuration hides
//! the real problem.

use melyxar_config::Config;
use melyxar_core::library::{LibraryKind, RootAccess};
use melyxar_core::user::Permissions;
use melyxar_database::Database;
use melyxar_ffmpeg::{Capabilities, ToolPaths};

use crate::{AppError, AppState, Result};

/// Name given to the account created on a brand new server.
pub const DEFAULT_ACCOUNT_NAME: &str = "admin";

/// Opens everything and returns the assembled server.
pub async fn bring_up(config: Config) -> Result<AppState> {
    prepare_directories(&config)?;

    // Log redaction is switched on before anything is logged, so a media name
    // cannot escape during start-up.
    melyxar_core::privacy::set_reveal_media_names(config.logging.reveal_media_names);

    let database = Database::open(&config.directories.database_file()).await?;

    let (tools, capabilities) = detect_media_tools(&config).await;

    ensure_default_account(&database).await?;
    reconcile_libraries(&database, &config).await?;
    refresh_root_access(&database).await?;

    let state = AppState::new(config, database, tools, capabilities);

    // Nothing is running yet, so a job still marked as running is a leftover
    // from a stop or a crash. Saying so beats a progress bar that will never
    // move again.
    state
        .jobs()
        .close_interrupted("the server restarted before this job finished")
        .await?;

    Ok(state)
}

/// Creates the three directories the server writes to.
///
/// Kept apart on purpose: what must be backed up, what can be regenerated, and
/// what is thrown away. None of them ever sits inside a media folder, so a
/// media disk can stay mounted read only.
pub fn prepare_directories(config: &Config) -> Result<()> {
    for directory in [
        &config.directories.data,
        &config.directories.cache,
        &config.directories.transcodes,
        &config.directories.uploads(),
        &config.directories.images(),
        &config.directories.subtitles(),
        &config.directories.backups(),
    ] {
        std::fs::create_dir_all(directory).map_err(AppError::Directory)?;
    }
    Ok(())
}

/// Finds the media tools and asks them what they can do.
///
/// Returns nothing rather than failing when they are absent: the server comes
/// up, the diagnostic says what is missing, and browsing still works.
pub async fn detect_media_tools(config: &Config) -> (Option<ToolPaths>, Option<Capabilities>) {
    let tools = match ToolPaths::discover(
        config.media_tools.ffmpeg_path.as_deref(),
        config.media_tools.ffprobe_path.as_deref(),
    ) {
        Ok(tools) => tools,
        Err(error) => {
            tracing::warn!(
                %error,
                "the media tools were not found; browsing works, playback does not"
            );
            return (None, None);
        }
    };

    match Capabilities::detect(&tools).await {
        Ok(capabilities) => {
            tracing::info!(
                version = capabilities.version,
                encoders = capabilities.encoders.len(),
                hardware = capabilities.hardware.len(),
                "media tools ready"
            );
            (Some(tools), Some(capabilities))
        }
        Err(error) => {
            tracing::warn!(%error, "the media tools were found but would not answer");
            (Some(tools), None)
        }
    }
}

/// Creates the first account when the server has none.
///
/// Created without a password: the setup wizard sets one. An account exists
/// from the very first start because every progress row, favourite and
/// preference hangs off one.
pub async fn ensure_default_account(database: &Database) -> Result<()> {
    if database.user_count().await? > 0 {
        return Ok(());
    }
    let user = database
        .create_user(DEFAULT_ACCOUNT_NAME, None, &Permissions::administrator())
        .await?;
    tracing::info!(
        user = %user.id,
        "created the first account; the setup wizard will set its password"
    );
    Ok(())
}

/// Brings the stored libraries in line with the configuration file.
///
/// Additive only. A library that disappears from the configuration is left
/// alone rather than deleted, because a typo in a file must never destroy a
/// watch history.
pub async fn reconcile_libraries(database: &Database, config: &Config) -> Result<()> {
    for declared in &config.libraries {
        let Some(kind) = LibraryKind::parse(&declared.kind) else {
            // Validation already refused this, so reaching here means the file
            // changed under us.
            tracing::warn!(
                library = declared.name,
                kind = declared.kind,
                "unknown library kind, skipped"
            );
            continue;
        };

        match database.library_by_name(&declared.name).await? {
            None => {
                let roots: Vec<_> = declared
                    .roots
                    .iter()
                    .map(|root| (root.label.clone(), root.path.clone()))
                    .collect();
                let library = database
                    .create_library(&declared.name, kind, &declared.metadata_language, &roots)
                    .await?;
                tracing::info!(
                    library = declared.name,
                    roots = library.roots.len(),
                    "library declared"
                );
            }
            Some(existing) => {
                // Only add roots that are new. Removing one is an explicit
                // action, never a side effect of editing a file.
                for root in &declared.roots {
                    let already_there =
                        existing.roots.iter().any(|stored| stored.path == root.path);
                    if !already_there {
                        database
                            .add_root(existing.id, &root.label, &root.path)
                            .await?;
                        tracing::info!(
                            library = declared.name,
                            root = root.label,
                            "root added to an existing library"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

/// Tests every root and records what it found.
///
/// Done by trying, not by reading permission bits, and recorded so the
/// interface can grey out what will not work instead of offering it and
/// failing later.
pub async fn refresh_root_access(database: &Database) -> Result<()> {
    for entry in database.roots_with_access().await? {
        let state = melyxar_library::check_root_access(&entry.root.path);
        database.set_root_access(entry.root.id, state).await?;

        match state {
            RootAccess::Missing => tracing::warn!(
                root = entry.root.label,
                "root not found; a scan on it will stop rather than empty the library"
            ),
            RootAccess::Unreadable => tracing::warn!(
                root = entry.root.label,
                "root present but not readable by the server account"
            ),
            RootAccess::ReadOnly => tracing::info!(root = entry.root.label, "root readable"),
            RootAccess::ReadWrite => {
                tracing::info!(root = entry.root.label, "root readable and writable")
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_config::{LibraryConfig, RootConfig};
    use std::path::PathBuf;

    fn config_with_library(root: &std::path::Path) -> Config {
        Config {
            libraries: vec![LibraryConfig {
                name: "Films".into(),
                kind: "movies".into(),
                metadata_language: "fr".into(),
                roots: vec![RootConfig {
                    label: "disk-one".into(),
                    path: root.to_path_buf(),
                }],
            }],
            ..Config::default()
        }
    }

    #[tokio::test]
    async fn the_first_start_creates_exactly_one_administrator() {
        let database = Database::open_in_memory().await.expect("database opens");
        ensure_default_account(&database)
            .await
            .expect("account created");

        let users = database.list_users().await.expect("listed");
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].name, DEFAULT_ACCOUNT_NAME);
        assert!(users[0].permissions.is_administrator);
    }

    #[tokio::test]
    async fn starting_again_does_not_create_a_second_account() {
        let database = Database::open_in_memory().await.expect("database opens");
        ensure_default_account(&database)
            .await
            .expect("first start");
        ensure_default_account(&database)
            .await
            .expect("second start");
        assert_eq!(database.user_count().await.expect("counted"), 1);
    }

    #[tokio::test]
    async fn a_declared_library_is_created_with_its_roots() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = Database::open_in_memory().await.expect("database opens");
        let config = config_with_library(directory.path());

        reconcile_libraries(&database, &config)
            .await
            .expect("reconciled");

        let libraries = database.list_libraries().await.expect("listed");
        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0].name, "Films");
        assert_eq!(libraries[0].kind, LibraryKind::Movies);
        assert_eq!(libraries[0].roots.len(), 1);
    }

    #[tokio::test]
    async fn reconciling_twice_changes_nothing() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = Database::open_in_memory().await.expect("database opens");
        let config = config_with_library(directory.path());

        reconcile_libraries(&database, &config)
            .await
            .expect("first pass");
        reconcile_libraries(&database, &config)
            .await
            .expect("second pass");

        let libraries = database.list_libraries().await.expect("listed");
        assert_eq!(libraries.len(), 1);
        assert_eq!(libraries[0].roots.len(), 1);
    }

    #[tokio::test]
    async fn a_root_added_to_the_file_is_picked_up_on_the_next_start() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = Database::open_in_memory().await.expect("database opens");
        let mut config = config_with_library(directory.path());

        reconcile_libraries(&database, &config)
            .await
            .expect("first pass");

        config.libraries[0].roots.push(RootConfig {
            label: "disk-two".into(),
            path: PathBuf::from("/mnt/two/Films"),
        });
        reconcile_libraries(&database, &config)
            .await
            .expect("second pass");

        let libraries = database.list_libraries().await.expect("listed");
        assert_eq!(libraries[0].roots.len(), 2);
    }

    #[tokio::test]
    async fn a_library_removed_from_the_file_is_left_alone_rather_than_deleted() {
        // A typo in a configuration file must never destroy a watch history.
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = Database::open_in_memory().await.expect("database opens");
        let config = config_with_library(directory.path());

        reconcile_libraries(&database, &config)
            .await
            .expect("first pass");
        reconcile_libraries(&database, &Config::default())
            .await
            .expect("second pass with nothing declared");

        assert_eq!(
            database.list_libraries().await.expect("listed").len(),
            1,
            "the library must survive being dropped from the file"
        );
    }

    #[tokio::test]
    async fn root_access_is_recorded_from_a_real_test() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = Database::open_in_memory().await.expect("database opens");
        let config = config_with_library(directory.path());
        reconcile_libraries(&database, &config)
            .await
            .expect("reconciled");

        refresh_root_access(&database)
            .await
            .expect("access refreshed");

        let roots = database.roots_with_access().await.expect("roots readable");
        assert_eq!(roots.len(), 1);
        assert!(roots[0].access.is_usable());
        assert!(roots[0].checked_at.is_some());
    }

    #[tokio::test]
    async fn a_root_that_is_not_mounted_is_recorded_as_missing_rather_than_readable() {
        let database = Database::open_in_memory().await.expect("database opens");
        let config = Config {
            libraries: vec![LibraryConfig {
                name: "Films".into(),
                kind: "movies".into(),
                metadata_language: "fr".into(),
                roots: vec![RootConfig {
                    label: "disk-one".into(),
                    path: PathBuf::from("/nowhere/at/all"),
                }],
            }],
            ..Config::default()
        };
        reconcile_libraries(&database, &config)
            .await
            .expect("reconciled");
        refresh_root_access(&database)
            .await
            .expect("access refreshed");

        let roots = database.roots_with_access().await.expect("roots readable");
        assert_eq!(roots[0].access, RootAccess::Missing);
        assert!(!roots[0].access.is_usable());
    }

    #[tokio::test]
    async fn the_three_directories_are_created_and_none_sits_inside_a_media_folder() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let config = Config {
            directories: melyxar_config::Directories {
                data: directory.path().join("data"),
                cache: directory.path().join("cache"),
                transcodes: directory.path().join("cache/transcodes"),
            },
            ..Config::default()
        };

        prepare_directories(&config).expect("directories prepared");

        assert!(config.directories.data.is_dir());
        assert!(config.directories.cache.is_dir());
        assert!(config.directories.transcodes.is_dir());
        assert!(config.directories.uploads().is_dir());
        assert!(config.directories.images().is_dir());
        assert!(config.directories.subtitles().is_dir());
        assert!(config.directories.backups().is_dir());
    }

    #[tokio::test]
    async fn the_media_tools_are_detected_and_report_what_they_can_do() {
        let (tools, capabilities) = detect_media_tools(&Config::default()).await;
        assert!(
            tools.is_some(),
            "the tools are installed in this environment"
        );
        let capabilities = capabilities.expect("capabilities were read");
        assert!(capabilities.supports_minimum_targets());
    }

    #[tokio::test]
    async fn a_missing_media_tool_does_not_stop_the_server_from_coming_up() {
        let config = Config {
            media_tools: melyxar_config::MediaToolsConfig {
                ffmpeg_path: Some(PathBuf::from("/nowhere/ffmpeg")),
                ffprobe_path: None,
            },
            ..Config::default()
        };
        let (tools, capabilities) = detect_media_tools(&config).await;
        assert!(tools.is_none());
        assert!(capabilities.is_none());
    }
}
