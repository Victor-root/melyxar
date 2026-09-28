//! Each account's playlists: works it put one after the other, in an order it
//! chooses, seen by nobody else.
//!
//! Every question names the account it is asked for, and a playlist of
//! another account answers as one that is not there.

use melyxar_core::id::{LibraryId, PlaylistId, UserId, WorkId};
use melyxar_core::time::now;
use sqlx::{AssertSqlSafe, Row, Sqlite, Transaction};

use crate::browse::{kept_inside, WorkCard, WHAT_A_CARD_IS};
use crate::convert::{parse_id, timestamp_to_text};
use crate::{Database, Result};

/// One playlist as the list of them shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistSummary {
    pub id: PlaylistId,
    pub name: String,
    /// How many of its works this account can still reach.
    pub held: i64,
    /// Its first work this account can reach, whose poster it wears.
    pub cover: Option<WorkCard>,
}

/// One playlist with its works, in its order.
#[derive(Debug, Clone, PartialEq)]
pub struct PlaylistHeld {
    pub id: PlaylistId,
    pub name: String,
    pub cards: Vec<WorkCard>,
}

impl Database {
    /// Every playlist of this account, by name.
    pub async fn playlists(
        &self,
        owner: UserId,
        within: Option<&[LibraryId]>,
    ) -> Result<Vec<PlaylistSummary>> {
        // A library taken away leaves the playlist there, empty of what it
        // held from it.
        let inside = kept_inside(within, "w.library_id").unwrap_or_else(|| " AND 0".to_string());
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "WITH visible AS (
                SELECT pi.playlist_id, pi.work_id, pi.ordinal
                  FROM playlist_items pi
                  JOIN works w ON w.id = pi.work_id
                 WHERE 1 = 1{inside})
             SELECT p.id, p.name,
                    (SELECT count(*) FROM visible v WHERE v.playlist_id = p.id) AS held,
                    (SELECT v.work_id FROM visible v WHERE v.playlist_id = p.id
                      ORDER BY v.ordinal LIMIT 1) AS cover
               FROM playlists p
              WHERE p.user_id = ?
              ORDER BY p.name COLLATE NOCASE"
        )));
        for library in within.unwrap_or_default() {
            query = query.bind(library.to_db_string());
        }
        let rows = query
            .bind(owner.to_db_string())
            .fetch_all(self.reader())
            .await?;

        let mut found = Vec::new();
        for row in &rows {
            let cover: Option<String> = row.try_get("cover")?;
            found.push((
                PlaylistSummary {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    name: row.try_get("name")?,
                    held: row.try_get("held")?,
                    cover: None,
                },
                cover.map(|id| parse_id::<WorkId>(&id)).transpose()?,
            ));
        }
        let covers: Vec<WorkId> = found.iter().filter_map(|(_, cover)| *cover).collect();
        let cards = self.cards_in_order(owner, &covers).await?;
        Ok(found
            .into_iter()
            .map(|(mut summary, cover)| {
                summary.cover = cover.and_then(|id| cards.iter().find(|card| card.id == id).cloned());
                summary
            })
            .collect())
    }

    /// One playlist of this account and the works of it it can reach, in its
    /// order.
    pub async fn playlist(
        &self,
        owner: UserId,
        id: PlaylistId,
        within: Option<&[LibraryId]>,
    ) -> Result<Option<PlaylistHeld>> {
        let name: Option<String> =
            sqlx::query_scalar("SELECT name FROM playlists WHERE id = ? AND user_id = ?")
                .bind(id.to_db_string())
                .bind(owner.to_db_string())
                .fetch_optional(self.reader())
                .await?;
        let Some(name) = name else {
            return Ok(None);
        };
        let mut cards = Vec::new();
        if let Some(inside) = kept_inside(within, "w.library_id") {
            let mut query = sqlx::query(AssertSqlSafe(format!(
                "SELECT {WHAT_A_CARD_IS}
                   FROM works w
                   JOIN playlist_items pi ON pi.work_id = w.id
                  WHERE pi.playlist_id = ?{inside}
                  ORDER BY pi.ordinal"
            )))
            .bind(id.to_db_string());
            for library in within.unwrap_or_default() {
                query = query.bind(library.to_db_string());
            }
            cards = query
                .fetch_all(self.reader())
                .await?
                .iter()
                .map(crate::browse::card_from_row)
                .collect::<Result<Vec<_>>>()?;
            self.attach_posters(&mut cards).await?;
            self.attach_viewer_state(owner, &mut cards).await?;
        }
        Ok(Some(PlaylistHeld { id, name, cards }))
    }

    /// Makes a playlist for this account, empty.
    pub async fn create_playlist(&self, owner: UserId, name: &str) -> Result<PlaylistId> {
        let id = PlaylistId::new();
        let at = timestamp_to_text(now());
        sqlx::query(
            "INSERT INTO playlists (id, user_id, name, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id.to_db_string())
        .bind(owner.to_db_string())
        .bind(name)
        .bind(&at)
        .bind(&at)
        .execute(self.writer())
        .await?;
        Ok(id)
    }

    /// Calls a playlist of this account something else. Answers false for
    /// one that is not there or not its own.
    pub async fn rename_playlist(&self, owner: UserId, id: PlaylistId, name: &str) -> Result<bool> {
        let done = sqlx::query(
            "UPDATE playlists SET name = ?, updated_at = ? WHERE id = ? AND user_id = ?",
        )
        .bind(name)
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .bind(owner.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Deletes a playlist of this account, and nothing of the works in it.
    pub async fn delete_playlist(&self, owner: UserId, id: PlaylistId) -> Result<bool> {
        let done = sqlx::query("DELETE FROM playlists WHERE id = ? AND user_id = ?")
            .bind(id.to_db_string())
            .bind(owner.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Puts works at the end of a playlist of this account, in the order
    /// given, leaving where they are the ones already in it.
    pub async fn add_to_playlist(&self, owner: UserId, id: PlaylistId, works: &[WorkId]) -> Result<bool> {
        let mut transaction = self.begin().await?;
        if !owns(&mut transaction, owner, id).await? {
            return Ok(false);
        }
        for work in works {
            sqlx::query(
                "INSERT INTO playlist_items (playlist_id, work_id, ordinal)
                 SELECT ?1, ?2, coalesce(max(ordinal) + 1, 0)
                   FROM playlist_items WHERE playlist_id = ?1
                 ON CONFLICT (playlist_id, work_id) DO NOTHING",
            )
            .bind(id.to_db_string())
            .bind(work.to_db_string())
            .execute(&mut *transaction)
            .await?;
        }
        touch(&mut transaction, id).await?;
        transaction.commit().await?;
        Ok(true)
    }

    /// Takes works out of a playlist of this account.
    pub async fn remove_from_playlist(
        &self,
        owner: UserId,
        id: PlaylistId,
        works: &[WorkId],
    ) -> Result<bool> {
        let mut transaction = self.begin().await?;
        if !owns(&mut transaction, owner, id).await? {
            return Ok(false);
        }
        for work in works {
            sqlx::query("DELETE FROM playlist_items WHERE playlist_id = ? AND work_id = ?")
                .bind(id.to_db_string())
                .bind(work.to_db_string())
                .execute(&mut *transaction)
                .await?;
        }
        touch(&mut transaction, id).await?;
        transaction.commit().await?;
        Ok(true)
    }

    /// Puts the works of a playlist of this account in the order given. A
    /// work it holds and the order leaves out keeps its place after them, so
    /// a list sent by a screen that could not see all of it loses nothing.
    pub async fn reorder_playlist(
        &self,
        owner: UserId,
        id: PlaylistId,
        order: &[WorkId],
    ) -> Result<bool> {
        let mut transaction = self.begin().await?;
        if !owns(&mut transaction, owner, id).await? {
            return Ok(false);
        }
        let held: Vec<String> = sqlx::query_scalar(
            "SELECT work_id FROM playlist_items WHERE playlist_id = ? ORDER BY ordinal",
        )
        .bind(id.to_db_string())
        .fetch_all(&mut *transaction)
        .await?;
        let given: Vec<String> = order.iter().map(WorkId::to_db_string).collect();
        let placed = given
            .iter()
            .filter(|work| held.contains(work))
            .chain(held.iter().filter(|work| !given.contains(work)));
        for (ordinal, work) in placed.enumerate() {
            sqlx::query("UPDATE playlist_items SET ordinal = ? WHERE playlist_id = ? AND work_id = ?")
                .bind(ordinal as i64)
                .bind(id.to_db_string())
                .bind(work)
                .execute(&mut *transaction)
                .await?;
        }
        touch(&mut transaction, id).await?;
        transaction.commit().await?;
        Ok(true)
    }

    /// The playlists of this account a work is in.
    pub async fn playlists_holding(&self, owner: UserId, work_id: WorkId) -> Result<Vec<PlaylistId>> {
        let rows: Vec<String> = sqlx::query_scalar(
            "SELECT p.id FROM playlists p
               JOIN playlist_items pi ON pi.playlist_id = p.id
              WHERE pi.work_id = ? AND p.user_id = ?",
        )
        .bind(work_id.to_db_string())
        .bind(owner.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter().map(|id| parse_id(id)).collect()
    }
}

/// Whether this playlist is there and belongs to this account.
async fn owns(transaction: &mut Transaction<'_, Sqlite>, owner: UserId, id: PlaylistId) -> Result<bool> {
    let found: Option<i64> = sqlx::query_scalar("SELECT 1 FROM playlists WHERE id = ? AND user_id = ?")
        .bind(id.to_db_string())
        .bind(owner.to_db_string())
        .fetch_optional(&mut **transaction)
        .await?;
    Ok(found.is_some())
}

/// Writes down that a playlist just changed.
async fn touch(transaction: &mut Transaction<'_, Sqlite>, id: PlaylistId) -> Result<()> {
    sqlx::query("UPDATE playlists SET updated_at = ? WHERE id = ?")
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::WorkKind;
    use std::path::PathBuf;

    #[tokio::test]
    async fn a_playlist_is_its_owners_alone_and_keeps_the_order_it_is_given() {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library("Films", LibraryKind::Movies, "fr", &[("one".to_string(), PathBuf::from("/mnt/one/Films"))])
            .await
            .expect("library")
            .id;
        let me = database.create_user("me", None, &Permissions::viewer()).await.expect("account").id;
        let other = database.create_user("other", None, &Permissions::viewer()).await.expect("account").id;
        let mut works = Vec::new();
        for title in ["Quiet Harbour", "Lantern Row", "Amber Field"] {
            works.push(
                database
                    .create_work(library, WorkKind::Movie, title, &title.to_lowercase(), None)
                    .await
                    .expect("work")
                    .id,
            );
        }
        let [a, b, c] = [works[0], works[1], works[2]];

        let night = database.create_playlist(me, "Friday night").await.expect("made");
        assert!(database.add_to_playlist(me, night, &[a, b, c, a]).await.expect("added"));
        assert!(!database.add_to_playlist(other, night, &[a]).await.expect("refused"));
        assert!(database.playlist(other, night, None).await.expect("read").is_none());
        assert!(database.playlists(other, None).await.expect("read").is_empty());

        assert!(database.reorder_playlist(me, night, &[c, a]).await.expect("reordered"));
        let held = database.playlist(me, night, None).await.expect("read").expect("there");
        assert_eq!(
            held.cards.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![c, a, b],
            "what the order left out keeps its place after it"
        );

        let listed = database.playlists(me, None).await.expect("read");
        assert_eq!((listed.len(), listed[0].held), (1, 3));
        assert_eq!(listed[0].cover.as_ref().map(|card| card.id), Some(c));
        assert_eq!(database.playlists(me, Some(&[])).await.expect("read")[0].held, 0);
        assert_eq!(database.playlists_holding(me, a).await.expect("read"), vec![night]);
        assert!(database.playlists_holding(other, a).await.expect("read").is_empty());

        assert!(database.remove_from_playlist(me, night, &[c]).await.expect("taken out"));
        assert!(database.rename_playlist(me, night, "Saturday").await.expect("renamed"));
        assert!(!database.rename_playlist(other, night, "Mine").await.expect("refused"));
        let held = database.playlist(me, night, None).await.expect("read").expect("there");
        assert_eq!((held.name.as_str(), held.cards.len()), ("Saturday", 2));
        assert!(!database.delete_playlist(other, night).await.expect("refused"));
        assert!(database.delete_playlist(me, night).await.expect("deleted"));
        assert!(database.playlists(me, None).await.expect("read").is_empty());
    }
}
