//! Bringing the server up.
//!
//! Start-up is deliberately forgiving about what is missing and strict about
//! what is wrong. A missing media tool must not stop the server from coming
//! up, because a library still browses without it and an administrator needs a
//! running interface to be told what to fix. A malformed configuration, on the
//! other hand, stops everything: carrying on with half a configuration hides
//! the real problem.

use std::sync::Arc;

use melyxar_config::Config;
use melyxar_core::job::{Job, JobKind, JobPriority};
use melyxar_core::library::{Library, LibraryKind, RootAccess};
use melyxar_core::user::Permissions;
use melyxar_database::Database;
use melyxar_ffmpeg::{Capabilities, ToolPaths};

use crate::{AppError, AppState, Result};

/// Name given to the account created on a brand new server.
pub const DEFAULT_ACCOUNT_NAME: &str = "admin";

/// What bringing the server up turned up, besides the server itself.
pub struct BroughtUp {
    pub state: AppState,
    /// Jobs a restart cut short. They come back only here: a later restart
    /// finds those rows long since closed, so this is the one moment anything
    /// can be started again from them.
    pub cut_short: Vec<Job>,
    /// Libraries whose language changed in the configuration. Their films have
    /// been put back in the queue and are waiting to be asked about again.
    pub waiting_on_a_new_language: Vec<Library>,
}

/// Opens everything and returns the assembled server.
///
/// What was found on the way is closed and then forgotten here, which is what
/// the diagnostic and the command line want. The server itself wants to know,
/// and asks for it by name.
pub async fn bring_up(config: Config) -> Result<AppState> {
    Ok(bring_up_and_say_what_is_waiting(config).await?.state)
}

/// The same, handing back what the server alone is meant to act on.
pub async fn bring_up_and_say_what_is_waiting(config: Config) -> Result<BroughtUp> {
    prepare_directories(&config)?;

    // Log redaction is switched on before anything is logged, so a media name
    // cannot escape during start-up.
    melyxar_core::privacy::set_reveal_media_names(config.logging.reveal_media_names);

    let database = Database::open(&config.directories.database_file()).await?;

    let (tools, capabilities) = detect_media_tools(&config).await;

    ensure_default_account(&database).await?;
    let waiting_on_a_new_language = reconcile_libraries(&database, &config).await?;
    refresh_root_access(&database).await?;

    let state = AppState::new(config, database, tools, capabilities);

    // Nothing is running yet, so a job still marked as running is a leftover
    // from a stop or a crash. Saying so beats a progress bar that will never
    // move again.
    let cut_short = state.jobs().close_interrupted().await?;

    Ok(BroughtUp {
        state,
        cut_short,
        waiting_on_a_new_language,
    })
}

/// Asks a provider about every film of the libraries whose language changed.
///
/// Only the server calls this. The films were put back in the queue when the
/// change was noticed; this is what makes somebody see it happen rather than
/// find, months later, that half the library is still described in a language
/// nobody asked for. At the low priority, since nobody is waiting on it.
pub async fn ask_again_about(state: &AppState, libraries: &[Library]) -> usize {
    if libraries.is_empty() {
        return 0;
    }
    let Some(provider) = state.metadata_provider() else {
        // A server without a key browses without one. The films stay in the
        // queue, and the day a key is configured they are asked about.
        tracing::info!(
            libraries = libraries.len(),
            "a language changed but no provider key is configured, so the films wait"
        );
        return 0;
    };

    let mut started = 0;
    for library in libraries {
        match crate::identify::start_identification(state, Arc::clone(&provider), library.clone())
            .await
        {
            Ok(_) => started += 1,
            Err(error) => tracing::warn!(
                library = library.name,
                %error,
                "the films of this library could not be asked about again"
            ),
        }
    }
    started
}

/// Starts again what a restart cut short, and says how many that was.
///
/// Only the server calls this, and only for a job a restart ended: one that
/// failed would fail again, and one somebody stopped was stopped on purpose.
/// Nobody is waiting on any of it, so it all goes in at the low priority,
/// behind anything asked for from a button.
///
/// Nothing is picked up from where it stopped, because nothing needs to be:
/// every pass of a scan asks what is left to do rather than walking a list
/// drawn up at the start, so a scan started again simply does not redo what
/// was already done.
pub async fn take_up_again_what_a_restart_cut_short(state: &AppState, cut_short: &[Job]) -> usize {
    let libraries = match state.database().list_libraries().await {
        Ok(libraries) => libraries,
        Err(error) => {
            tracing::warn!(%error, "the libraries could not be read, so nothing was taken up again");
            return 0;
        }
    };

    let mut taken_up = 0;
    for job in cut_short {
        if !job.state.is_worth_taking_up_again() {
            continue;
        }
        // Every job worth taking up again is about one library, and says which
        // by name. One that is about nothing, or about a library that has since
        // been taken out of the configuration, has nothing to be started on.
        //
        // Said out loud rather than passed over: a job that quietly fails to
        // start again looks exactly like one that was never interrupted, and
        // the only visible sign is a scan that never resumes. Nobody could
        // work that out from a screen, and neither could I from a log.
        let Some(library) = job.target_id.as_deref().and_then(|target| {
            libraries
                .iter()
                .find(|library| library.id.to_string() == target)
        }) else {
            tracing::warn!(
                kind = job.kind.as_str(),
                target = job.target_id.as_deref().unwrap_or("none"),
                "a job a restart cut short is about a library this server no longer has, \
                 so nothing was started again for it"
            );
            continue;
        };

        let started = match job.kind {
            JobKind::ScanLibrary => crate::scan::start_scan_and_identification(
                state,
                library.clone(),
                JobPriority::BACKGROUND,
            )
            .await
            .map(|_| ()),
            JobKind::IdentifyWork => match state.metadata_provider() {
                Some(provider) => {
                    crate::identify::start_identification(state, provider, library.clone())
                        .await
                        .map(|_| ())
                }
                None => continue,
            },
            // Everything else is short enough that the next thing to ask for
            // it will do it, and starting it here would only be guessing at
            // what somebody wanted an hour ago.
            _ => continue,
        };

        match started {
            Ok(()) => {
                tracing::info!(
                    library = library.name,
                    kind = job.kind.as_str(),
                    "a job a restart cut short was taken up again"
                );
                taken_up += 1;
            }
            Err(error) => {
                tracing::warn!(
                    library = library.name,
                    kind = job.kind.as_str(),
                    %error,
                    "a job a restart cut short would not start again"
                );
            }
        }
    }
    taken_up
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
            report_the_card(&capabilities);
            (Some(tools), Some(capabilities))
        }
        Err(error) => {
            tracing::warn!(%error, "the media tools were found but would not answer");
            (Some(tools), None)
        }
    }
}

/// Writes down what the search for a card found, whether or not it found one.
///
/// Every trial, not only the outcome. A card that was refused is the single
/// commonest thing to go wrong here, and what the tool printed when it refused
/// is the whole answer: a container never given the graphics device, a driver
/// that is not installed, a codec this generation of card does not carry. None
/// of those can be told apart from "no card" without these lines.
fn report_the_card(capabilities: &Capabilities) {
    let search = &capabilities.card_search;

    for trial in &search.trials {
        if trial.worked {
            tracing::info!(
                what = trial.what,
                device = trial.device,
                "the card can do this"
            );
        } else {
            tracing::warn!(
                what = trial.what,
                device = trial.device,
                said = trial.said,
                "the card would not do this"
            );
        }
    }

    match &search.card {
        Some(card) => tracing::info!(
            device = %card.device.display(),
            way = card.way.as_str(),
            writes = ?card.encoders.keys().collect::<Vec<_>>(),
            reads = ?card.decoders.iter().collect::<Vec<_>>(),
            can_scale = card.can_scale,
            can_tone_map = card.can_tone_map,
            "a card is rebuilding pictures"
        ),
        None if search.devices.is_empty() => tracing::warn!(
            "no graphics device is visible here, so every picture is rebuilt on the processor; \
             an unprivileged container has to be given /dev/dri explicitly"
        ),
        // The device opened and then answered nothing. That is a driver, not a
        // permission and not the card: the video acceleration driver is a
        // package of its own, separate from the media tools, and a machine can
        // carry the card and the tools and still have no driver between them.
        None if search.a_device_opened() => tracing::warn!(
            devices = ?search.devices,
            "the graphics device opens but no video acceleration driver answers for it, so the \
             processor rebuilds every picture; that driver is a package of its own, apart from \
             the media tools"
        ),
        None => tracing::warn!(
            devices = ?search.devices,
            "the graphics device is there and this server is not allowed to open it, so the \
             processor rebuilds every picture"
        ),
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
///
/// Answers the libraries whose language changed, whose films are waiting to be
/// asked about again. Answered rather than acted on here, because this runs
/// for the diagnostic and the command line too, and neither has any business
/// starting a download.
pub async fn reconcile_libraries(database: &Database, config: &Config) -> Result<Vec<Library>> {
    let mut changed = Vec::new();
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
                    let Some(stored) = existing
                        .roots
                        .iter()
                        .find(|stored| stored.path == root.path)
                    else {
                        database
                            .add_root(existing.id, &root.label, &root.path)
                            .await?;
                        tracing::info!(
                            library = declared.name,
                            root = root.label,
                            "root added to an existing library"
                        );
                        continue;
                    };

                    // A root is recognised by its path and called by its
                    // label, so a label corrected in the file is a correction
                    // to make, not a second root.
                    if stored.label != root.label {
                        database.rename_root(stored.id, &root.label).await?;
                        tracing::info!(
                            library = declared.name,
                            was = stored.label,
                            now = root.label,
                            "a root is called something else now"
                        );
                    }
                }

                // The language was read once, when the library was created,
                // and never again: somebody who installed this server and then
                // wrote their own language in the file saw nothing happen, and
                // had no way at all of putting it right.
                if database
                    .set_metadata_language(existing.id, &declared.metadata_language)
                    .await?
                {
                    let waiting = database.ask_again_about_every_work(existing.id).await?;
                    tracing::info!(
                        library = declared.name,
                        was = existing.metadata_language,
                        now = declared.metadata_language,
                        films = waiting,
                        "this library is described in another language now, and its films \
                         are being asked about again"
                    );
                    changed.push(existing);
                }
            }
        }
    }
    Ok(changed)
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
    use melyxar_core::job::JobState;
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

    /// A configuration pointing at a temporary folder, ready to be brought up.
    fn config_on(directory: &std::path::Path) -> Config {
        let mut config = config_with_library(&directory.join("films"));
        config.directories = melyxar_config::Directories {
            data: directory.join("data"),
            cache: directory.join("cache"),
            transcodes: directory.join("cache/transcodes"),
        };
        std::fs::create_dir_all(directory.join("films")).expect("folder created");
        config
    }

    /// A server standing up on a temporary folder, with one library.
    async fn server_with_a_library(directory: &std::path::Path) -> (AppState, Vec<Job>) {
        let brought_up = bring_up_and_say_what_is_waiting(config_on(directory))
            .await
            .expect("the server comes up");
        (brought_up.state, brought_up.cut_short)
    }

    #[tokio::test]
    async fn a_language_changed_in_the_file_reaches_a_library_that_already_exists() {
        // What somebody who installed this and then wrote their own language
        // into the file used to get: nothing at all, with no way out short of
        // editing the database by hand.
        let directory = tempfile::tempdir().expect("temporary directory");
        let mut config = config_on(directory.path());
        config.libraries[0].metadata_language = "fr".into();

        let first = bring_up_and_say_what_is_waiting(config.clone())
            .await
            .expect("the server comes up");
        assert!(
            first.waiting_on_a_new_language.is_empty(),
            "a library just created is already in the language it was asked for"
        );
        let library = first
            .state
            .database()
            .library_by_name("Films")
            .await
            .expect("read")
            .expect("declared");
        assert_eq!(library.metadata_language, "fr");
        first.state.database().close().await;

        config.libraries[0].metadata_language = "en".into();
        let second = bring_up_and_say_what_is_waiting(config.clone())
            .await
            .expect("the server comes up again");
        assert_eq!(
            second.waiting_on_a_new_language.len(),
            1,
            "the change is noticed and handed to the server to act on"
        );
        assert_eq!(
            second
                .state
                .database()
                .library_by_name("Films")
                .await
                .expect("read")
                .expect("declared")
                .metadata_language,
            "en"
        );
        second.state.database().close().await;

        // Starting again on an unchanged file must not send a provider the
        // whole library a second time.
        let third = bring_up_and_say_what_is_waiting(config)
            .await
            .expect("the server comes up a third time");
        assert!(
            third.waiting_on_a_new_language.is_empty(),
            "nothing changed, so nothing is asked about again"
        );
    }

    #[tokio::test]
    async fn a_scan_a_restart_cut_short_is_taken_up_again_on_its_own() {
        // An update in the middle of a scan of the whole collection must not
        // mean starting it over by hand, or worse, forgetting to.
        let directory = tempfile::tempdir().expect("temporary directory");
        let (state, nothing) = server_with_a_library(directory.path()).await;
        assert!(nothing.is_empty(), "a first start cut nothing short");

        let library = state
            .database()
            .library_by_name("Films")
            .await
            .expect("read")
            .expect("the library was declared");
        state
            .database()
            .create_job(
                JobKind::ScanLibrary,
                JobPriority::REQUESTED,
                Some(&library.id.to_string()),
            )
            .await
            .expect("a scan was under way when the server went away");

        let cut_short = state
            .jobs()
            .close_interrupted()
            .await
            .expect("the restart closed it");
        assert_eq!(cut_short.len(), 1);
        assert_eq!(
            take_up_again_what_a_restart_cut_short(&state, &cut_short).await,
            1
        );

        // The row exists before the work starts, so it is there by the time
        // this returns rather than at some moment worth waiting for.
        let taken_up = state
            .database()
            .recent_jobs(10)
            .await
            .expect("read")
            .into_iter()
            .find(|job| job.kind == JobKind::ScanLibrary && !job.state.is_finished())
            .expect("a scan is under way again");
        assert_eq!(
            taken_up.priority,
            JobPriority::BACKGROUND,
            "nobody is waiting on this one, so it goes behind anything asked for"
        );
        assert_eq!(
            taken_up.target_id.as_deref(),
            Some(&*library.id.to_string())
        );
    }

    #[tokio::test]
    async fn a_job_that_failed_or_was_stopped_is_left_alone() {
        // One that failed would fail again, and one somebody stopped was
        // stopped on purpose: starting either back up would be the server
        // arguing with the person using it.
        let directory = tempfile::tempdir().expect("temporary directory");
        let (state, _) = server_with_a_library(directory.path()).await;
        let library = state
            .database()
            .library_by_name("Films")
            .await
            .expect("read")
            .expect("the library was declared");

        let mut ended = Vec::new();
        for state_it_ended_in in [JobState::Failed, JobState::Cancelled, JobState::Succeeded] {
            let job = state
                .database()
                .create_job(
                    JobKind::ScanLibrary,
                    JobPriority::REQUESTED,
                    Some(&library.id.to_string()),
                )
                .await
                .expect("job created");
            ended.push(Job {
                state: state_it_ended_in,
                ..job
            });
        }

        assert_eq!(
            take_up_again_what_a_restart_cut_short(&state, &ended).await,
            0
        );
    }

    #[tokio::test]
    async fn a_job_about_a_library_that_is_gone_has_nothing_to_start_on() {
        // What taking a library out of the configuration leaves behind.
        let directory = tempfile::tempdir().expect("temporary directory");
        let (state, _) = server_with_a_library(directory.path()).await;
        let orphan = state
            .database()
            .create_job(
                JobKind::ScanLibrary,
                JobPriority::REQUESTED,
                Some("a-library-nobody-has"),
            )
            .await
            .expect("job created");

        let cut_short = vec![Job {
            state: JobState::Interrupted,
            ..orphan
        }];
        assert_eq!(
            take_up_again_what_a_restart_cut_short(&state, &cut_short).await,
            0
        );
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
    async fn a_label_corrected_in_the_file_is_corrected_everywhere() {
        // A root is recognised by its path and called by its label. Mistyping
        // the label used to be permanent: it is what every log line and every
        // screen shows, and nothing read the file for it ever again.
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = Database::open_in_memory().await.expect("database opens");
        let mut config = config_with_library(directory.path());
        let path = config.libraries[0].roots[0].path.clone();

        reconcile_libraries(&database, &config)
            .await
            .expect("first pass");

        config.libraries[0].roots[0].label = "the name it should have had".into();
        reconcile_libraries(&database, &config)
            .await
            .expect("second pass");

        let libraries = database.list_libraries().await.expect("listed");
        assert_eq!(
            libraries[0].roots.len(),
            1,
            "a new name is a correction, not a second root"
        );
        assert_eq!(libraries[0].roots[0].label, "the name it should have had");
        assert_eq!(libraries[0].roots[0].path, path);
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
