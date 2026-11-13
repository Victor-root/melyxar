//! The passkeys an account signs in with, besides its password: only what the
//! device made public, the credential's identifier and its public key with
//! its counter, as the passkey library writes them down.

use melyxar_core::id::{PasskeyId, UserId};
use melyxar_core::time::Timestamp;
use sqlx::Row;

use crate::convert::{parse_id, parse_timestamp, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

/// One passkey, as kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredPasskey {
    pub id: PasskeyId,
    pub user_id: UserId,
    /// What the device calls the key, handed back on every use.
    pub credential_id: Vec<u8>,
    /// The name of the site the key was made under.
    pub rp_id: String,
    /// What the account calls it.
    pub name: String,
    /// The public key and its counter, as the library serialises them.
    pub credential: String,
    pub created_at: Timestamp,
    pub used_at: Option<Timestamp>,
}

/// What a new passkey is kept with.
#[derive(Debug, Clone, Copy)]
pub struct NewPasskey<'a> {
    pub user: UserId,
    pub credential_id: &'a [u8],
    pub rp_id: &'a str,
    pub name: &'a str,
    pub credential: &'a str,
    pub at: Timestamp,
}

const WHAT_A_PASSKEY_IS: &str = "id, user_id, credential_id, rp_id, name, credential, created_at, used_at";

fn passkey_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<StoredPasskey> {
    Ok(StoredPasskey {
        id: parse_id(&row.try_get::<String, _>("id")?)?,
        user_id: parse_id(&row.try_get::<String, _>("user_id")?)?,
        credential_id: row.try_get("credential_id")?,
        rp_id: row.try_get("rp_id")?,
        name: row.try_get("name")?,
        credential: row.try_get("credential")?,
        created_at: parse_timestamp(&row.try_get::<String, _>("created_at")?)?,
        used_at: row
            .try_get::<Option<String>, _>("used_at")?
            .as_deref()
            .map(parse_timestamp)
            .transpose()?,
    })
}

impl Database {
    /// Keeps a passkey an account has just made, and answers it as kept.
    pub async fn add_passkey(&self, new: NewPasskey<'_>) -> Result<StoredPasskey> {
        let id = PasskeyId::new();
        sqlx::query(
            "INSERT INTO passkeys (id, user_id, credential_id, rp_id, name, credential, created_at)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(new.user.to_db_string())
        .bind(new.credential_id)
        .bind(new.rp_id)
        .bind(new.name)
        .bind(new.credential)
        .bind(timestamp_to_text(new.at))
        .execute(self.writer())
        .await?;
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {WHAT_A_PASSKEY_IS} FROM passkeys WHERE id = ?"
        )))
        .bind(id.to_db_string())
        .fetch_optional(self.writer())
        .await?
        .ok_or_else(|| DatabaseError::Corrupt("a passkey just written is missing".to_string()))?;
        passkey_from_row(&row)
    }

    /// The passkeys of one account, the oldest first.
    pub async fn passkeys_of(&self, user: UserId) -> Result<Vec<StoredPasskey>> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {WHAT_A_PASSKEY_IS} FROM passkeys WHERE user_id = ? ORDER BY created_at, id"
        )))
        .bind(user.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(passkey_from_row).collect()
    }

    /// The passkey a device names by its credential, whoever it belongs to.
    pub async fn passkey_by_credential(&self, credential_id: &[u8]) -> Result<Option<StoredPasskey>> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            "SELECT {WHAT_A_PASSKEY_IS} FROM passkeys WHERE credential_id = ?"
        )))
        .bind(credential_id)
        .fetch_optional(self.reader())
        .await?;
        row.as_ref().map(passkey_from_row).transpose()
    }

    /// Writes down that a passkey has just been used, with its counter as it
    /// now stands.
    pub async fn passkey_used(&self, id: PasskeyId, credential: &str, at: Timestamp) -> Result<()> {
        sqlx::query("UPDATE passkeys SET credential = ?, used_at = ? WHERE id = ?")
            .bind(credential)
            .bind(timestamp_to_text(at))
            .bind(id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Renames one of an account's passkeys. Whether it was one of its own.
    pub async fn rename_passkey(&self, user: UserId, id: PasskeyId, name: &str) -> Result<bool> {
        let done = sqlx::query("UPDATE passkeys SET name = ? WHERE id = ? AND user_id = ?")
            .bind(name)
            .bind(id.to_db_string())
            .bind(user.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Takes away every passkey of an account, and says how many there were.
    pub async fn remove_passkeys_of(&self, user: UserId) -> Result<u64> {
        let done = sqlx::query("DELETE FROM passkeys WHERE user_id = ?")
            .bind(user.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected())
    }

    /// Takes away one of an account's passkeys, and answers its name when it
    /// was one of its own.
    pub async fn remove_passkey(&self, user: UserId, id: PasskeyId) -> Result<Option<String>> {
        let row = sqlx::query("DELETE FROM passkeys WHERE id = ? AND user_id = ? RETURNING name")
            .bind(id.to_db_string())
            .bind(user.to_db_string())
            .fetch_optional(self.writer())
            .await?;
        row.map(|row| row.try_get::<String, _>("name")).transpose().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn an_account(database: &Database, name: &str) -> UserId {
        database
            .create_user(name, Some("hash"), &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created")
            .id
    }

    fn a_key<'a>(user: UserId, credential_id: &'a [u8], name: &'a str) -> NewPasskey<'a> {
        NewPasskey {
            user,
            credential_id,
            rp_id: "melyxar.example",
            name,
            credential: "{\"cred\":1}",
            at: melyxar_core::time::now(),
        }
    }

    #[tokio::test]
    async fn a_passkey_is_kept_listed_and_found_by_its_credential() {
        let database = Database::open_in_memory().await.expect("database opens");
        let alice = an_account(&database, "Alice").await;
        let kept = database.add_passkey(a_key(alice, b"one", "Phone")).await.expect("kept");
        database.add_passkey(a_key(alice, b"two", "Laptop")).await.expect("kept");

        let listed = database.passkeys_of(alice).await.expect("listed");
        assert_eq!(listed.iter().map(|key| key.name.as_str()).collect::<Vec<_>>(), ["Phone", "Laptop"]);
        let found = database.passkey_by_credential(b"one").await.expect("read").expect("found");
        assert_eq!(found, kept);
        assert!(database.passkey_by_credential(b"three").await.expect("read").is_none());
    }

    #[tokio::test]
    async fn the_same_credential_is_never_kept_twice() {
        let database = Database::open_in_memory().await.expect("database opens");
        let alice = an_account(&database, "Alice").await;
        database.add_passkey(a_key(alice, b"one", "Phone")).await.expect("kept");
        assert!(database.add_passkey(a_key(alice, b"one", "Again")).await.is_err());
    }

    #[tokio::test]
    async fn a_use_writes_the_counter_and_the_day() {
        let database = Database::open_in_memory().await.expect("database opens");
        let alice = an_account(&database, "Alice").await;
        let kept = database.add_passkey(a_key(alice, b"one", "Phone")).await.expect("kept");
        let at = melyxar_core::time::now();
        database.passkey_used(kept.id, "{\"cred\":2}", at).await.expect("written");
        let found = database.passkey_by_credential(b"one").await.expect("read").expect("found");
        assert_eq!(found.credential, "{\"cred\":2}");
        assert!(found.used_at.is_some());
    }

    #[tokio::test]
    async fn only_its_own_account_renames_or_removes_a_passkey() {
        let database = Database::open_in_memory().await.expect("database opens");
        let alice = an_account(&database, "Alice").await;
        let bob = an_account(&database, "Bob").await;
        let kept = database.add_passkey(a_key(alice, b"one", "Phone")).await.expect("kept");

        assert!(!database.rename_passkey(bob, kept.id, "Mine").await.expect("ran"));
        assert!(database.remove_passkey(bob, kept.id).await.expect("ran").is_none());
        assert!(database.rename_passkey(alice, kept.id, "Pocket").await.expect("ran"));
        assert_eq!(
            database.remove_passkey(alice, kept.id).await.expect("ran").as_deref(),
            Some("Pocket")
        );
        assert!(database.passkeys_of(alice).await.expect("listed").is_empty());
    }

    #[tokio::test]
    async fn every_passkey_of_one_account_is_taken_away_and_no_other() {
        let database = Database::open_in_memory().await.expect("database opens");
        let alice = an_account(&database, "Alice").await;
        let bob = an_account(&database, "Bob").await;
        database.add_passkey(a_key(alice, b"one", "Phone")).await.expect("kept");
        database.add_passkey(a_key(alice, b"two", "Laptop")).await.expect("kept");
        database.add_passkey(a_key(bob, b"three", "Phone")).await.expect("kept");
        assert_eq!(database.remove_passkeys_of(alice).await.expect("ran"), 2);
        assert!(database.passkeys_of(alice).await.expect("listed").is_empty());
        assert_eq!(database.passkeys_of(bob).await.expect("listed").len(), 1);
    }

    #[tokio::test]
    async fn an_account_removed_takes_its_passkeys_with_it() {
        let database = Database::open_in_memory().await.expect("database opens");
        let alice = an_account(&database, "Alice").await;
        database.add_passkey(a_key(alice, b"one", "Phone")).await.expect("kept");
        database.delete_user(alice).await.expect("removed");
        assert!(database.passkey_by_credential(b"one").await.expect("read").is_none());
    }
}
