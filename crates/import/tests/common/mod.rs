//! What the tests of every server make up their databases with.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqliteConnection};
use sqlx::{Connection, Executor};

/// A new database at `path`, open to be filled.
pub async fn opened(path: &Path) -> SqliteConnection {
    let options = SqliteConnectOptions::new().filename(path).create_if_missing(true);
    SqliteConnection::connect_with(&options).await.unwrap()
}

/// A database made at `path` by running `sql`.
pub async fn database(path: &Path, sql: &str) {
    let mut connection = opened(path).await;
    connection.execute(sqlx::raw_sql(sqlx::AssertSqlSafe(sql))).await.unwrap();
    connection.close().await.unwrap();
}
