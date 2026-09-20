//! Opening the database and holding its connections.
//!
//! Two pools rather than one: many readers, exactly one writer. That split is
//! the practical form of the rule that browsing must never wait on a scan.
//! With the journal in write-ahead mode, a reader never blocks behind the
//! writer, and with a single write connection there is no lock contention to
//! resolve in the first place.

use std::path::Path;
use std::str::FromStr;
use std::time::Duration;

use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous};
use sqlx::{Sqlite, SqlitePool, Transaction};

#[derive(Debug, thiserror::Error)]
pub enum DatabaseError {
    #[error("database error: {0}")]
    Query(#[from] sqlx::Error),
    #[error("migration failed: {0}")]
    Migration(#[from] sqlx::migrate::MigrateError),
    #[error("the data directory could not be prepared: {0}")]
    Directory(#[from] std::io::Error),
    #[error("stored value is not usable: {0}")]
    Corrupt(String),
}

pub type Result<T> = std::result::Result<T, DatabaseError>;

/// How long a statement waits for a lock before giving up.
///
/// Generous on purpose: a brief wait is far better than an error surfacing in
/// the interface, and with a single writer the wait is rare anyway.
const BUSY_TIMEOUT: Duration = Duration::from_secs(10);

/// Readers in the pool. Enough to serve a page made of several small queries
/// without queuing, well below anything that would stress the file.
const READER_COUNT: u32 = 8;

/// The open database.
#[derive(Clone, Debug)]
pub struct Database {
    readers: SqlitePool,
    writer: SqlitePool,
}

impl Database {
    /// Opens the database at `path`, creating it when absent, and applies any
    /// pending migration.
    ///
    /// The parent directory is created first: on a fresh install nothing
    /// exists yet, and failing on a missing directory would be a poor first
    /// impression.
    pub async fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // The writer comes first: it creates the file, switches the journal to
        // write-ahead mode and applies the migrations. Readers connecting to a
        // file that does not exist yet would simply fail.
        //
        // Exactly one connection, so writes serialise here rather than
        // fighting over the file.
        let writer = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(Self::options(path)?)
            .await?;

        crate::MIGRATOR.run(&writer).await?;

        let readers = SqlitePoolOptions::new()
            .max_connections(READER_COUNT)
            .connect_with(Self::options(path)?)
            .await?;

        Ok(Self { readers, writer })
    }

    /// Opens a throwaway database in memory, for tests.
    ///
    /// A shared cache is used so that both pools see the same data, which an
    /// ordinary in-memory database would not provide.
    pub async fn open_in_memory() -> Result<Self> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?
            .foreign_keys(true)
            .busy_timeout(BUSY_TIMEOUT);

        // A single connection shared by both pools: an in-memory database
        // disappears with its last connection, so readers and writer have to
        // be the same one.
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(options)
            .await?;

        let database = Self {
            readers: pool.clone(),
            writer: pool,
        };
        database.migrate().await?;
        Ok(database)
    }

    /// Connection options shared by both pools.
    ///
    /// The reader pool is deliberately *not* opened read only. Read-only
    /// access to a write-ahead journal still needs to create the shared index
    /// file, so the engine refuses it in exactly the situations that matter.
    /// The single-writer rule is therefore a convention held by this crate,
    /// enforced by every write going through [`Database::writer`], rather than
    /// something the engine is asked to police.
    fn options(path: &Path) -> Result<SqliteConnectOptions> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            // Readers never wait on the writer.
            .journal_mode(SqliteJournalMode::Wal)
            // Durable enough for this workload and far faster than full
            // synchronisation, which is the usual recommendation alongside
            // write-ahead journalling.
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            .busy_timeout(BUSY_TIMEOUT);
        Ok(options)
    }

    async fn migrate(&self) -> Result<()> {
        crate::MIGRATOR.run(&self.writer).await?;
        Ok(())
    }

    /// Pool to read from. Never use it to write: a write here would bypass the
    /// single-writer rule.
    pub fn reader(&self) -> &SqlitePool {
        &self.readers
    }

    /// The single write connection.
    pub fn writer(&self) -> &SqlitePool {
        &self.writer
    }

    /// Starts a write transaction.
    ///
    /// Keep it short and never hold it across an external call such as reading
    /// a file or querying a provider: that is exactly what makes a scan block
    /// someone's playback position from being saved.
    pub async fn begin(&self) -> Result<Transaction<'static, Sqlite>> {
        Ok(self.writer.begin().await?)
    }

    /// Reports the journal mode actually in force.
    ///
    /// Used by the diagnostic command: the write-ahead mode is what keeps
    /// browsing responsive during a scan, so it is worth showing rather than
    /// assuming.
    pub async fn journal_mode(&self) -> Result<String> {
        let mode: (String,) = sqlx::query_as("PRAGMA journal_mode")
            .fetch_one(self.readers.acquire().await?.as_mut())
            .await?;
        Ok(mode.0)
    }

    /// Size of the database file in bytes, for the diagnostic screen.
    pub async fn size_bytes(&self) -> Result<i64> {
        let row: (i64, i64) =
            sqlx::query_as("SELECT (SELECT page_count FROM pragma_page_count()), (SELECT page_size FROM pragma_page_size())")
                .fetch_one(self.readers.acquire().await?.as_mut())
                .await?;
        Ok(row.0 * row.1)
    }

    /// Gives the disk back the room a large removal left behind.
    ///
    /// The file never shrinks on its own: the pages of what was removed stay
    /// in it, ready to be written into again. That is the right behaviour for
    /// a library losing a film and the wrong one after an invented library of
    /// a hundred thousand works, which leaves half a gigabyte behind for a
    /// collection of fifty.
    ///
    /// Wants the file to itself for a moment, so it is asked for rather than
    /// insisted on: whoever calls this has already done what it was asked to
    /// do, and a removal reported as a failure because a page was being read
    /// at that moment would be a lie.
    pub async fn reclaim_space(&self) -> Result<()> {
        sqlx::query("VACUUM").execute(self.writer()).await?;
        // The rebuilt database lands in the journal, and the file itself is
        // only cut back when the journal is folded into it. Without this the
        // room is given back at the next restart and, until then, the server
        // holds both the old file and a journal the size of it.
        sqlx::query("PRAGMA wal_checkpoint(TRUNCATE)")
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Closes both pools, waiting for in-flight statements.
    pub async fn close(&self) {
        self.writer.close().await;
        self.readers.close().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_in_memory_database_comes_up_migrated() {
        let database = Database::open_in_memory().await.expect("database opens");
        let tables: Vec<(String,)> =
            sqlx::query_as("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
                .fetch_all(database.reader())
                .await
                .expect("tables can be listed");
        let names: Vec<&str> = tables.iter().map(|row| row.0.as_str()).collect();
        for expected in [
            "users",
            "libraries",
            "works",
            "media_sources",
            "tracks",
            "jobs",
        ] {
            assert!(names.contains(&expected), "table {expected} is missing");
        }
    }

    #[tokio::test]
    async fn a_file_database_uses_the_write_ahead_journal() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let database = Database::open(&directory.path().join("melyxar.db"))
            .await
            .expect("database opens");
        let mode = database
            .journal_mode()
            .await
            .expect("journal mode readable");
        assert_eq!(
            mode.to_lowercase(),
            "wal",
            "readers must never wait on the writer"
        );

        // The diagnostic screen shows this number, and a number that says a
        // migrated database weighs nothing helps nobody. It counts the pages
        // the database holds rather than the bytes of the one file, since in
        // this journal mode the newest pages are still in the journal.
        let size = database.size_bytes().await.expect("size readable");
        assert!(
            size >= 16_384,
            "a database carrying every table is bigger than that: {size}"
        );

        database.close().await;
        assert!(
            database.journal_mode().await.is_err(),
            "a closed database refuses work rather than answering from a pool nobody stopped"
        );
    }

    #[tokio::test]
    async fn opening_creates_the_missing_parent_directory() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let nested = directory
            .path()
            .join("deep")
            .join("data")
            .join("melyxar.db");
        let database = Database::open(&nested).await.expect("database opens");
        assert!(nested.exists(), "the database file was created");
        database.close().await;
    }

    #[tokio::test]
    async fn running_migrations_twice_changes_nothing() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("melyxar.db");
        let first = Database::open(&path).await.expect("first open");
        first.close().await;
        let second = Database::open(&path).await.expect("second open");
        second.close().await;
    }

    #[tokio::test]
    async fn foreign_keys_are_enforced_rather_than_merely_declared() {
        let database = Database::open_in_memory().await.expect("database opens");
        let outcome = sqlx::query(
            "INSERT INTO library_roots (id, library_id, label, path) VALUES ('a', 'nowhere', 'label', '/tmp')",
        )
        .execute(database.writer())
        .await;
        assert!(
            outcome.is_err(),
            "a root pointing at no library must be refused"
        );
    }

    #[tokio::test]
    async fn the_settings_row_exists_from_the_start() {
        let database = Database::open_in_memory().await.expect("database opens");
        let count: (i64,) = sqlx::query_as("SELECT count(*) FROM server_settings")
            .fetch_one(database.reader())
            .await
            .expect("settings are readable");
        assert_eq!(count.0, 1, "exactly one settings row must exist");
    }
}
