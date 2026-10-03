//! What the tests of notifications are set up with.

use melyxar_database::Database;

use crate::AppState;

/// A server with an empty database and its folders in a temporary place,
/// kept for as long as the folder is held.
pub(super) async fn a_server() -> (tempfile::TempDir, AppState) {
    let directory = tempfile::tempdir().expect("temporary directory");
    let config = melyxar_config::Config {
        directories: melyxar_config::Directories {
            data: directory.path().join("data"),
            cache: directory.path().join("cache"),
            transcodes: directory.path().join("cache/transcodes"),
            ..Default::default()
        },
        ..melyxar_config::Config::default()
    };
    crate::startup::prepare_directories(&config).expect("directories prepared");
    let database = Database::open_in_memory().await.expect("database opens");
    (directory, AppState::new(config, database, None, None))
}
