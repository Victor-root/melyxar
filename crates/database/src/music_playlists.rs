//! Each account's playlists of songs: songs one after the other in an order
//! it chose, the same song as often as it likes, seen by nobody else.
//!
//! Every question names the account it is asked for, and a playlist of
//! another account answers as one that is not there. A song of a library the
//! account was not given is left out of what it reads, never out of the list.

use melyxar_core::id::{LibraryId, PlaylistId, UserId, WorkId};
use melyxar_core::time::now;
use sqlx::{AssertSqlSafe, Row, Sqlite, Transaction};

use crate::browse::kept_inside;
use crate::convert::{parse_id, timestamp_to_text};
use crate::images::StoredImage;
use crate::music_browse::{A_SONG, SongRow};
use crate::{Database, Result};

/// One playlist as the list of them shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct MusicPlaylistSummary {
    pub id: PlaylistId,
    pub name: String,
    /// How many of its songs this account can reach.
    pub songs: i64,
    /// How long they last together, in milliseconds.
    pub duration_ms: i64,
    /// The cover of the album of its first song.
    pub cover: Vec<StoredImage>,
}

impl Database {
    /// Every playlist of songs of this account, by name.
    pub async fn music_playlists(
        &self,
        owner: UserId,
        within: Option<&[LibraryId]>,
    ) -> Result<Vec<MusicPlaylistSummary>> {
        let inside = kept_inside(within, "w.library_id").unwrap_or_else(|| " AND 0".to_string());
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "WITH visible AS (
                SELECT p.playlist_id, p.position, w.parent_id,
                       (SELECT max(s.duration_ms) FROM media_sources s WHERE s.work_id = w.id) AS ms
                  FROM music_playlist_songs p
                  JOIN works w ON w.id = p.song_id
                 WHERE 1 = 1{inside})
             SELECT l.id, l.name,
                    (SELECT count(*) FROM visible v WHERE v.playlist_id = l.id) AS songs,
                    (SELECT coalesce(sum(v.ms), 0) FROM visible v WHERE v.playlist_id = l.id) AS ms,
                    (SELECT v.parent_id FROM visible v WHERE v.playlist_id = l.id
                      ORDER BY v.position LIMIT 1) AS first_album
               FROM music_playlists l
              WHERE l.user_id = ?
              ORDER BY l.name COLLATE NOCASE, l.id"
        )));
        for library in within.unwrap_or_default() {
            query = query.bind(library.to_db_string());
        }
        let rows = query
            .bind(owner.to_db_string())
            .fetch_all(self.reader())
            .await?;

        let mut found = Vec::with_capacity(rows.len());
        let mut albums = Vec::new();
        for row in &rows {
            let album = row
                .try_get::<Option<String>, _>("first_album")?
                .map(|album| parse_id::<WorkId>(&album))
                .transpose()?;
            albums.extend(album);
            found.push((
                MusicPlaylistSummary {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    name: row.try_get("name")?,
                    songs: row.try_get("songs")?,
                    duration_ms: row.try_get("ms")?,
                    cover: Vec::new(),
                },
                album,
            ));
        }
        let covers = self.posters_of(&albums).await?;
        Ok(found
            .into_iter()
            .map(|(mut summary, album)| {
                summary.cover = album
                    .and_then(|album| covers.get(&album).cloned())
                    .unwrap_or_default();
                summary
            })
            .collect())
    }

    /// One playlist of this account, its name and the songs of it the
    /// account can reach, in its order.
    pub async fn music_playlist(
        &self,
        owner: UserId,
        id: PlaylistId,
        within: Option<&[LibraryId]>,
    ) -> Result<Option<(String, Vec<SongRow>)>> {
        let name: Option<String> =
            sqlx::query_scalar("SELECT name FROM music_playlists WHERE id = ? AND user_id = ?")
                .bind(id.to_db_string())
                .bind(owner.to_db_string())
                .fetch_optional(self.reader())
                .await?;
        let Some(name) = name else {
            return Ok(None);
        };
        let Some(inside) = kept_inside(within, "w.library_id") else {
            return Ok(Some((name, Vec::new())));
        };
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {A_SONG}
               FROM music_playlist_songs p
               JOIN works w ON w.id = p.song_id
               LEFT JOIN music_songs ms ON ms.work_id = w.id
               LEFT JOIN works al ON al.id = w.parent_id
              WHERE p.playlist_id = ?{inside}
              ORDER BY p.position"
        )))
        .bind(id.to_db_string());
        for library in within.unwrap_or_default() {
            query = query.bind(library.to_db_string());
        }
        let rows = query.fetch_all(self.reader()).await?;
        Ok(Some((name, self.song_rows(&rows).await?)))
    }

    /// Makes a playlist of songs for this account, empty.
    pub async fn create_music_playlist(&self, owner: UserId, name: &str) -> Result<PlaylistId> {
        let id = PlaylistId::new();
        let at = timestamp_to_text(now());
        sqlx::query(
            "INSERT INTO music_playlists (id, user_id, name, created_at, updated_at)
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

    /// Calls a playlist of this account something else. False for one that
    /// is not there or not its own.
    pub async fn rename_music_playlist(
        &self,
        owner: UserId,
        id: PlaylistId,
        name: &str,
    ) -> Result<bool> {
        let done = sqlx::query(
            "UPDATE music_playlists SET name = ?, updated_at = ? WHERE id = ? AND user_id = ?",
        )
        .bind(name)
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .bind(owner.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Deletes a playlist of this account, and nothing of its songs.
    pub async fn delete_music_playlist(&self, owner: UserId, id: PlaylistId) -> Result<bool> {
        let done = sqlx::query("DELETE FROM music_playlists WHERE id = ? AND user_id = ?")
            .bind(id.to_db_string())
            .bind(owner.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Puts songs at the end of a playlist of this account, in the order
    /// given, even those already in it.
    pub async fn add_to_music_playlist(
        &self,
        owner: UserId,
        id: PlaylistId,
        songs: &[WorkId],
    ) -> Result<bool> {
        let mut transaction = self.begin().await?;
        if !owns(&mut transaction, owner, id).await? {
            return Ok(false);
        }
        for song in songs {
            sqlx::query(
                "INSERT INTO music_playlist_songs (playlist_id, position, song_id)
                 SELECT ?1, coalesce(max(position) + 1, 0), ?2
                   FROM music_playlist_songs WHERE playlist_id = ?1",
            )
            .bind(id.to_db_string())
            .bind(song.to_db_string())
            .execute(&mut *transaction)
            .await?;
        }
        touch(&mut transaction, id).await?;
        transaction.commit().await?;
        Ok(true)
    }

    /// The songs of a playlist of this account, replaced by these in this
    /// order: how a song is moved, and how one is taken out.
    pub async fn set_music_playlist_songs(
        &self,
        owner: UserId,
        id: PlaylistId,
        songs: &[WorkId],
    ) -> Result<bool> {
        let mut transaction = self.begin().await?;
        if !owns(&mut transaction, owner, id).await? {
            return Ok(false);
        }
        sqlx::query("DELETE FROM music_playlist_songs WHERE playlist_id = ?")
            .bind(id.to_db_string())
            .execute(&mut *transaction)
            .await?;
        for (position, song) in songs.iter().enumerate() {
            sqlx::query(
                "INSERT INTO music_playlist_songs (playlist_id, position, song_id) VALUES (?, ?, ?)",
            )
            .bind(id.to_db_string())
            .bind(position as i64)
            .bind(song.to_db_string())
            .execute(&mut *transaction)
            .await?;
        }
        touch(&mut transaction, id).await?;
        transaction.commit().await?;
        Ok(true)
    }
}

/// Whether a playlist of songs is this account's.
async fn owns(
    transaction: &mut Transaction<'_, Sqlite>,
    owner: UserId,
    id: PlaylistId,
) -> Result<bool> {
    let found: Option<i64> =
        sqlx::query_scalar("SELECT 1 FROM music_playlists WHERE id = ? AND user_id = ?")
            .bind(id.to_db_string())
            .bind(owner.to_db_string())
            .fetch_optional(&mut **transaction)
            .await?;
    Ok(found.is_some())
}

/// Writes down that a playlist of songs just changed.
async fn touch(transaction: &mut Transaction<'_, Sqlite>, id: PlaylistId) -> Result<()> {
    sqlx::query("UPDATE music_playlists SET updated_at = ? WHERE id = ?")
        .bind(timestamp_to_text(now()))
        .bind(id.to_db_string())
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use melyxar_core::user::Permissions;

    use super::*;
    use crate::music_browse::SongOrder;
    use crate::music_testing::collection;

    fn titles(songs: &[SongRow]) -> Vec<&str> {
        songs.iter().map(|song| song.title.as_str()).collect()
    }

    #[tokio::test]
    async fn a_playlist_holds_songs_in_its_order_the_same_one_twice_if_asked() {
        let (database, library, _) = collection().await;
        let owner = database
            .create_user("listener", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let songs = database
            .music_songs(library, SongOrder::Title, false, 0, 10)
            .await
            .expect("read")
            .items;
        let id_of = |title: &str| {
            songs
                .iter()
                .find(|song| song.title == title)
                .expect("filed")
                .id
        };
        let playlist = database
            .create_music_playlist(owner, "Morning")
            .await
            .expect("made");
        assert!(
            database
                .add_to_music_playlist(
                    owner,
                    playlist,
                    &[id_of("Tides"), id_of("Zebra"), id_of("Tides")]
                )
                .await
                .expect("added")
        );

        let (name, held) = database
            .music_playlist(owner, playlist, None)
            .await
            .expect("read")
            .expect("there");
        assert_eq!(name, "Morning");
        assert_eq!(titles(&held), vec!["Tides", "Zebra", "Tides"]);

        let every = database.music_playlists(owner, None).await.expect("read");
        assert_eq!(every.len(), 1);
        assert_eq!(every[0].songs, 3);

        assert!(
            database
                .set_music_playlist_songs(owner, playlist, &[id_of("Zebra"), id_of("Tides")])
                .await
                .expect("set")
        );
        let (_, held) = database
            .music_playlist(owner, playlist, None)
            .await
            .expect("read")
            .expect("there");
        assert_eq!(titles(&held), vec!["Zebra", "Tides"]);

        let (_, hidden) = database
            .music_playlist(owner, playlist, Some(&[]))
            .await
            .expect("read")
            .expect("there");
        assert!(hidden.is_empty(), "a library not given is not read");
        assert_eq!(
            database
                .music_playlists(owner, Some(&[]))
                .await
                .expect("read")[0]
                .songs,
            0
        );
    }

    #[tokio::test]
    async fn a_playlist_of_another_account_answers_as_one_that_is_not_there() {
        let (database, _, _) = collection().await;
        let owner = database
            .create_user("listener", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let other = database
            .create_user("other", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let playlist = database
            .create_music_playlist(owner, "Mine")
            .await
            .expect("made");
        assert_eq!(
            database
                .music_playlist(other, playlist, None)
                .await
                .expect("read"),
            None
        );
        assert!(
            !database
                .rename_music_playlist(other, playlist, "Theirs")
                .await
                .expect("tried")
        );
        assert!(
            !database
                .add_to_music_playlist(other, playlist, &[])
                .await
                .expect("tried")
        );
        assert!(
            !database
                .delete_music_playlist(other, playlist)
                .await
                .expect("tried")
        );
        assert!(
            database
                .rename_music_playlist(owner, playlist, "Still mine")
                .await
                .expect("renamed")
        );
        assert!(
            database
                .delete_music_playlist(owner, playlist)
                .await
                .expect("deleted")
        );
        assert!(
            database
                .music_playlists(owner, None)
                .await
                .expect("read")
                .is_empty()
        );
    }
}
