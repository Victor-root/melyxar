//! A database of the other server, opened from a copy of its own.
//!
//! SQLite cannot be opened to read alone once it keeps a write-ahead journal:
//! it still writes a shared index next to the file. And a server that runs
//! keeps writing, so a copy is also what holds still while it is read.

use std::path::Path;

use sqlx::Connection;
use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};

use crate::Refusal;

/// Copies a database, with what its journal holds and its file does not yet,
/// into `work`, and opens the copy.
pub(crate) async fn opened(database: &Path, work: &Path) -> Result<SqliteConnection, Refusal> {
    let Some(name) = database.file_name() else {
        return Err(Refusal::NotFound(database.display().to_string()));
    };
    tokio::fs::create_dir_all(work).await?;
    let copy = work.join(name);
    tokio::fs::copy(database, &copy).await?;
    let mut journal = name.to_owned();
    journal.push("-wal");
    let beside = database.with_file_name(&journal);
    if tokio::fs::try_exists(&beside).await? {
        tokio::fs::copy(&beside, work.join(&journal)).await?;
    }
    Ok(SqliteConnection::connect_with(&SqliteConnectOptions::new().filename(&copy)).await?)
}

/// Whether a database holds a table.
pub(crate) async fn has_table(connection: &mut SqliteConnection, table: &str) -> Result<bool, Refusal> {
    Ok(sqlx::query("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?")
        .bind(table)
        .fetch_optional(connection)
        .await?
        .is_some())
}
