//! What each account makes of its music: the songs, albums and artists it
//! likes, and the songs it listened to, how often and how lately.
//!
//! Liking keeps to the list every work is liked on (see `playback.rs`); the
//! grids of films never read music from it (see `browse::met_on_its_own`).

use melyxar_core::id::{LibraryId, UserId, WorkId};
use melyxar_core::time::Timestamp;
use sqlx::{AssertSqlSafe, Row};

use crate::convert::{parse_id, timestamp_to_text};
use crate::music_browse::{A_SONG, AN_ALBUM, AN_ARTIST, MusicFound, SongRow};
use crate::{Database, Result};

/// Which of the songs listened to come first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Listened {
    /// The last one listened to first.
    Lately,
    /// The one listened to most first, the latest of equals ahead.
    Most,
}

impl Database {
    /// One more listen of a song by this account.
    pub async fn record_listen(&self, user: UserId, song: WorkId, at: Timestamp) -> Result<()> {
        sqlx::query(
            "INSERT INTO music_listens (user_id, song_id, listens, last_listened_at)
             VALUES (?, ?, 1, ?)
             ON CONFLICT (user_id, song_id) DO UPDATE SET
                listens = listens + 1,
                last_listened_at = excluded.last_listened_at",
        )
        .bind(user.to_db_string())
        .bind(song.to_db_string())
        .bind(timestamp_to_text(at))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Every song, album and artist this account likes, in any library: few
    /// enough for a screen to hold them all and light every heart from them.
    pub async fn music_favourite_ids(&self, user: UserId) -> Result<Vec<WorkId>> {
        sqlx::query(
            "SELECT f.work_id FROM favorites f JOIN works w ON w.id = f.work_id
              WHERE f.user_id = ? AND w.kind IN ('song', 'album', 'artist')",
        )
        .bind(user.to_db_string())
        .fetch_all(self.reader())
        .await?
        .iter()
        .map(|row| parse_id(&row.try_get::<String, _>("work_id")?))
        .collect()
    }

    /// What this account likes in a library, the last liked first, up to
    /// `limit` of each.
    pub async fn music_favourites(
        &self,
        user: UserId,
        library_id: LibraryId,
        limit: i64,
    ) -> Result<MusicFound> {
        let read = |sql: String| {
            sqlx::query(AssertSqlSafe(sql))
                .bind(user.to_db_string())
                .bind(library_id.to_db_string())
                .bind(limit)
        };
        let liked = "JOIN favorites f ON f.work_id = w.id AND f.user_id = ?";
        let albums = read(format!(
            "SELECT {AN_ALBUM} FROM works w JOIN music_albums ma ON ma.work_id = w.id {liked}
              WHERE w.library_id = ? AND w.kind = 'album'
              ORDER BY f.created_at DESC, w.id LIMIT ?"
        ))
        .fetch_all(self.reader())
        .await?;
        let artists = read(format!(
            "SELECT {AN_ARTIST} FROM works w {liked}
              WHERE w.library_id = ? AND w.kind = 'artist'
              ORDER BY f.created_at DESC, w.id LIMIT ?"
        ))
        .fetch_all(self.reader())
        .await?;
        let songs = read(format!(
            "SELECT {A_SONG}
               FROM works w {liked}
               LEFT JOIN music_songs ms ON ms.work_id = w.id
               LEFT JOIN works al ON al.id = w.parent_id
              WHERE w.library_id = ? AND w.kind = 'song'
              ORDER BY f.created_at DESC, w.id LIMIT ?"
        ))
        .fetch_all(self.reader())
        .await?;
        Ok(MusicFound {
            albums: self.album_cards(&albums).await?,
            artists: self.artist_cards(&artists).await?,
            songs: self.song_rows(&songs).await?,
        })
    }

    /// The songs of a library this account listened to, up to `limit`.
    pub async fn listened_songs(
        &self,
        user: UserId,
        library_id: LibraryId,
        order: Listened,
        limit: i64,
    ) -> Result<Vec<SongRow>> {
        let order = match order {
            Listened::Lately => "l.last_listened_at DESC, w.id",
            Listened::Most => "l.listens DESC, l.last_listened_at DESC, w.id",
        };
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {A_SONG}
               FROM music_listens l
               JOIN works w ON w.id = l.song_id
               LEFT JOIN music_songs ms ON ms.work_id = w.id
               LEFT JOIN works al ON al.id = w.parent_id
              WHERE l.user_id = ? AND w.library_id = ?
              ORDER BY {order} LIMIT ?"
        )))
        .bind(user.to_db_string())
        .bind(library_id.to_db_string())
        .bind(limit)
        .fetch_all(self.reader())
        .await?;
        self.song_rows(&rows).await
    }
}

#[cfg(test)]
mod tests {
    use melyxar_core::user::Permissions;

    use super::*;
    use crate::music_browse::{AlbumOrder, AlbumsWanted, SongOrder};
    use crate::music_testing::collection;

    fn titles(songs: &[SongRow]) -> Vec<&str> {
        songs.iter().map(|song| song.title.as_str()).collect()
    }

    #[tokio::test]
    async fn songs_listened_to_come_back_the_latest_or_the_most_first() {
        let (database, library, _) = collection().await;
        let user = database
            .create_user("listener", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let songs = database
            .music_songs(library, SongOrder::Title, false, 0, 10)
            .await
            .expect("read")
            .items;
        let named = |title: &str| {
            songs
                .iter()
                .find(|song| song.title == title)
                .expect("in the collection")
                .id
        };
        let start = melyxar_core::time::now();
        let at = |seconds: i64| start + time::Duration::seconds(seconds);
        for (title, when) in [("Tides", 1), ("Zebra", 2), ("Tides", 3), ("Beginnings", 4)] {
            database
                .record_listen(user, named(title), at(when))
                .await
                .expect("recorded");
        }

        let lately = database
            .listened_songs(user, library, Listened::Lately, 10)
            .await
            .expect("read");
        assert_eq!(titles(&lately), vec!["Beginnings", "Tides", "Zebra"]);
        let most = database
            .listened_songs(user, library, Listened::Most, 2)
            .await
            .expect("read");
        assert_eq!(titles(&most), vec!["Tides", "Beginnings"]);

        let somebody_else = database
            .create_user("other", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account")
            .id;
        assert!(
            database
                .listened_songs(somebody_else, library, Listened::Lately, 10)
                .await
                .expect("read")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn what_is_liked_is_read_back_by_kind_the_last_liked_first() {
        let (database, library, _) = collection().await;
        let user = database
            .create_user("listener", Some("a stored form"), &Permissions::viewer())
            .await
            .expect("account")
            .id;
        let albums = database
            .music_albums(
                library,
                &AlbumsWanted::default(),
                AlbumOrder::Title,
                false,
                0,
                10,
            )
            .await
            .expect("read")
            .items;
        let song = database
            .music_songs(library, SongOrder::Title, false, 0, 1)
            .await
            .expect("read")
            .items[0]
            .id;
        let artist = albums[0].artists[0].id;
        for work in [albums[1].id, song, albums[0].id, artist] {
            database
                .set_favourite(user, work, true)
                .await
                .expect("liked");
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }

        let liked = database
            .music_favourites(user, library, 10)
            .await
            .expect("read");
        assert_eq!(
            liked.albums.iter().map(|a| a.id).collect::<Vec<_>>(),
            vec![albums[0].id, albums[1].id]
        );
        assert_eq!(liked.artists.len(), 1);
        assert_eq!(liked.songs.len(), 1);

        let mut ids = database.music_favourite_ids(user).await.expect("read");
        ids.sort();
        let mut expected = vec![albums[0].id, albums[1].id, song, artist];
        expected.sort();
        assert_eq!(ids, expected);
    }
}
