//! What taking up the accounts of another server writes (see the crate
//! `melyxar-import`): the works its items are looked for among, and what each
//! account did there, merged into what it already did here.
//!
//! Nothing done in Melyxar is lost to it: the latest of the two wins for where
//! a work was left and whether it was watched, the greater for how often it
//! was played, and a favourite only adds.

use std::collections::HashMap;

use melyxar_core::id::{LibraryId, UserId, WorkId};
use melyxar_core::time::{Millis, Timestamp, now};
use melyxar_core::work::{PlaybackState, WorkKind};
use sqlx::Row;

use crate::convert::{parse_id, parse_optional_timestamp, timestamp_to_text};
use crate::{Database, Result};

/// A work as an item of another server is looked for: what it is, where its
/// files are, and what it can be known by.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkToMatch {
    pub id: WorkId,
    pub kind: WorkKind,
    pub library: LibraryId,
    pub title: String,
    pub year: Option<i32>,
    /// Where its files are, whole, the ones a scan no longer finds left out.
    pub paths: Vec<String>,
    pub external_ids: Vec<(String, String)>,
    /// The number of a season, of an episode within its season, of a song.
    pub number: Option<i32>,
    /// The season of an episode, the series of a season, the album of a song.
    pub parent: Option<WorkId>,
    /// The artists an album is credited to.
    pub artists: Vec<String>,
}

/// What an account did with a film or an episode on the other server.
#[derive(Debug, Clone, PartialEq)]
pub struct TakenProgress {
    pub work: WorkId,
    pub watched: bool,
    pub play_count: u32,
    pub position: Millis,
    pub last_played: Option<Timestamp>,
}

/// How often an account listened to a song on the other server, and when last.
#[derive(Debug, Clone, PartialEq)]
pub struct TakenListen {
    pub song: WorkId,
    pub listens: u32,
    pub last_listened: Option<Timestamp>,
}

impl Database {
    /// Every film, series, season, episode, video, song, album and artist,
    /// with what they can be found by.
    pub async fn works_to_match_against(&self) -> Result<Vec<WorkToMatch>> {
        let rows = sqlx::query(
            "SELECT id, kind, library_id, title, release_year, ordinal, parent_id FROM works
              WHERE kind IN ('movie', 'series', 'season', 'episode', 'video', 'song', 'album', 'artist')",
        )
        .fetch_all(self.reader())
        .await?;
        let mut paths: HashMap<String, Vec<String>> = HashMap::new();
        for row in sqlx::query(
            "SELECT s.work_id, r.path AS root, s.relative_path
               FROM media_sources s JOIN library_roots r ON r.id = s.root_id
              WHERE s.missing_since IS NULL",
        )
        .fetch_all(self.reader())
        .await?
        {
            let root: String = row.try_get("root")?;
            let relative: String = row.try_get("relative_path")?;
            paths
                .entry(row.try_get("work_id")?)
                .or_default()
                .push(format!("{}/{}", root.trim_end_matches('/'), relative));
        }
        let mut ids: HashMap<String, Vec<(String, String)>> = HashMap::new();
        for row in sqlx::query("SELECT work_id, provider, external_id FROM work_external_ids")
            .fetch_all(self.reader())
            .await?
        {
            ids.entry(row.try_get("work_id")?)
                .or_default()
                .push((row.try_get("provider")?, row.try_get("external_id")?));
        }
        let mut artists: HashMap<String, Vec<String>> = HashMap::new();
        for row in sqlx::query(
            "SELECT c.work_id, a.title FROM music_credits c JOIN works a ON a.id = c.artist_id
              WHERE c.role = 'album_artist' ORDER BY c.work_id, c.ordinal",
        )
        .fetch_all(self.reader())
        .await?
        {
            artists.entry(row.try_get("work_id")?).or_default().push(row.try_get("title")?);
        }
        rows.iter()
            .filter_map(|row| {
                let kind = WorkKind::parse(row.try_get::<String, _>("kind").ok()?.as_str())?;
                Some((row, kind))
            })
            .map(|(row, kind)| {
                let id: String = row.try_get("id")?;
                Ok(WorkToMatch {
                    id: parse_id(&id)?,
                    kind,
                    library: parse_id(&row.try_get::<String, _>("library_id")?)?,
                    title: row.try_get("title")?,
                    year: row.try_get("release_year")?,
                    paths: paths.remove(&id).unwrap_or_default(),
                    external_ids: ids.remove(&id).unwrap_or_default(),
                    number: row.try_get("ordinal")?,
                    parent: row
                        .try_get::<Option<String>, _>("parent_id")?
                        .map(|parent| parse_id(&parent))
                        .transpose()?,
                    artists: artists.remove(&id).unwrap_or_default(),
                })
            })
            .collect()
    }

    /// Merges what an account did on the other server into what it did here,
    /// in one transaction. The latest of the two says whether a work was
    /// watched and where it was left, a work without a moment counting as
    /// older than any; the play count is the greater of the two.
    pub async fn take_up_progress(&self, user: UserId, taken: &[TakenProgress]) -> Result<()> {
        let mut transaction = self.begin().await?;
        for one in taken {
            let kept: Option<(Option<String>, i64)> = sqlx::query_as(
                "SELECT last_played_at, play_count FROM playback_progress WHERE user_id = ? AND work_id = ?",
            )
            .bind(user.to_db_string())
            .bind(one.work.to_db_string())
            .fetch_optional(&mut *transaction)
            .await?;
            let wins = match &kept {
                None => true,
                Some((kept_at, _)) => match (parse_optional_timestamp(kept_at.as_deref())?, one.last_played) {
                    (_, None) => false,
                    (None, Some(_)) => true,
                    (Some(kept_at), Some(at)) => at > kept_at,
                },
            };
            let play_count = i64::from(one.play_count).max(kept.as_ref().map_or(0, |(_, count)| *count));
            if !wins {
                sqlx::query("UPDATE playback_progress SET play_count = ? WHERE user_id = ? AND work_id = ?")
                    .bind(play_count)
                    .bind(user.to_db_string())
                    .bind(one.work.to_db_string())
                    .execute(&mut *transaction)
                    .await?;
                continue;
            }
            let (state, position) = match (one.watched, one.position.get()) {
                (true, _) => (PlaybackState::Watched, 0),
                (false, position) if position > 0 => (PlaybackState::InProgress, position),
                (false, _) => (PlaybackState::NotStarted, 0),
            };
            let at = one.last_played.map(timestamp_to_text);
            sqlx::query(
                "INSERT INTO playback_progress
                    (user_id, work_id, position_ms, state, play_count, reported_at, last_played_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
                 ON CONFLICT (user_id, work_id) DO UPDATE SET
                    position_ms = excluded.position_ms,
                    state = excluded.state,
                    play_count = excluded.play_count,
                    reported_at = excluded.reported_at,
                    last_played_at = excluded.last_played_at",
            )
            .bind(user.to_db_string())
            .bind(one.work.to_db_string())
            .bind(position)
            .bind(state.as_str())
            .bind(play_count)
            .bind(at.as_deref())
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Adds the works an account liked on the other server to the ones it
    /// likes here.
    pub async fn take_up_favourites(&self, user: UserId, works: &[WorkId]) -> Result<()> {
        let mut transaction = self.begin().await?;
        let at = timestamp_to_text(now());
        for work in works {
            sqlx::query(
                "INSERT INTO favorites (user_id, work_id, created_at) VALUES (?, ?, ?)
                 ON CONFLICT (user_id, work_id) DO NOTHING",
            )
            .bind(user.to_db_string())
            .bind(work.to_db_string())
            .bind(&at)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    /// Merges how often an account listened to songs on the other server into
    /// what it listened to here: the greater count, and the latest moment.
    pub async fn take_up_listens(&self, user: UserId, taken: &[TakenListen]) -> Result<()> {
        let mut transaction = self.begin().await?;
        for one in taken {
            sqlx::query(
                "INSERT INTO music_listens (user_id, song_id, listens, last_listened_at) VALUES (?, ?, ?, ?)
                 ON CONFLICT (user_id, song_id) DO UPDATE SET
                    listens = max(music_listens.listens, excluded.listens),
                    last_listened_at = CASE
                        WHEN music_listens.last_listened_at IS NULL THEN excluded.last_listened_at
                        WHEN excluded.last_listened_at > music_listens.last_listened_at THEN excluded.last_listened_at
                        ELSE music_listens.last_listened_at END",
            )
            .bind(user.to_db_string())
            .bind(one.song.to_db_string())
            .bind(i64::from(one.listens))
            .bind(one.last_listened.map(timestamp_to_text))
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use melyxar_core::library::LibraryKind;
    use melyxar_core::user::Permissions;
    use time::macros::datetime;

    use super::*;
    use crate::music_testing::collection;

    async fn films(database: &Database) -> (LibraryId, melyxar_core::id::LibraryRootId) {
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-two".to_string(), PathBuf::from("/mnt/films/"))],
            )
            .await
            .expect("library");
        (library.id, library.roots[0].id)
    }

    async fn somebody(database: &Database) -> UserId {
        database.create_user("ada", None, &Permissions::administrator()).await.expect("account").id
    }

    #[tokio::test]
    async fn every_work_is_offered_with_its_files_its_identifiers_and_its_artists() {
        let (database, _, _) = collection().await;
        let (films, root) = films(&database).await;
        let film = database.create_work(films, WorkKind::Movie, "Harbour Lights", "harbour lights", Some(2011)).await.unwrap();
        database.insert_source(film.id, root, Path::new("Harbour Lights.mkv"), 10, now()).await.unwrap();
        database.set_work_external_id(film.id, "tmdb", "1001").await.unwrap();
        let series = database.create_work(films, WorkKind::Series, "Quiet Valley", "quiet valley", None).await.unwrap();
        let season =
            database.create_child_work(films, series.id, 2, WorkKind::Season, "Season 2", "season 2").await.unwrap();
        let episode =
            database.create_child_work(films, season.id, 5, WorkKind::Episode, "Snowfall", "snowfall").await.unwrap();

        let works = database.works_to_match_against().await.unwrap();
        let of = |id: WorkId| works.iter().find(|work| work.id == id).unwrap();
        let found = of(film.id);
        assert_eq!((found.kind, found.library, found.year), (WorkKind::Movie, films, Some(2011)));
        assert_eq!(found.paths, vec!["/mnt/films/Harbour Lights.mkv"]);
        assert_eq!(found.external_ids, vec![("tmdb".to_string(), "1001".to_string())]);
        assert_eq!((of(season.id).number, of(season.id).parent), (Some(2), Some(series.id)));
        assert_eq!((of(episode.id).number, of(episode.id).parent), (Some(5), Some(season.id)));

        let album = works.iter().find(|work| work.kind == WorkKind::Album && work.title == "Northern Lights").unwrap();
        assert_eq!(album.artists, vec!["Amber Field"]);
        let song = works.iter().find(|work| work.kind == WorkKind::Song && work.title == "Tides").unwrap();
        assert_eq!((song.parent, song.number), (Some(album.id), Some(2)));
        assert_eq!(song.paths, vec!["/mnt/one/A/Lights/02.flac"]);
        assert!(works.iter().any(|work| work.kind == WorkKind::Artist && work.title == "Amber Field"));
    }

    async fn kept(database: &Database, user: UserId, work: WorkId) -> (String, i64, i64, Option<String>) {
        sqlx::query_as(
            "SELECT state, position_ms, play_count, last_played_at FROM playback_progress
              WHERE user_id = ? AND work_id = ?",
        )
        .bind(user.to_db_string())
        .bind(work.to_db_string())
        .fetch_one(database.reader())
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn the_latest_of_the_two_says_where_a_work_was_left_and_the_greater_how_often() {
        let database = Database::open_in_memory().await.unwrap();
        let (films, _) = films(&database).await;
        let user = somebody(&database).await;
        let mut works = Vec::new();
        for title in ["one", "two", "three", "four"] {
            works.push(database.create_work(films, WorkKind::Movie, title, title, None).await.unwrap().id);
        }
        let [never_here, watched_here, older_here, dated_here] = works[..] else { unreachable!() };
        database
            .record_playback_progress(user, watched_here, Millis::new(60_000), PlaybackState::InProgress, now())
            .await
            .unwrap();
        let early = datetime!(2020-01-01 10:00 UTC);
        let late = datetime!(2024-06-01 10:00 UTC);
        database
            .take_up_progress(
                user,
                &[TakenProgress { work: older_here, watched: false, play_count: 1, position: Millis::new(5_000), last_played: Some(early) }],
            )
            .await
            .unwrap();
        database
            .take_up_progress(
                user,
                &[TakenProgress { work: dated_here, watched: true, play_count: 2, position: Millis::ZERO, last_played: Some(early) }],
            )
            .await
            .unwrap();

        let taken = |work, watched, play_count, position, last_played| TakenProgress {
            work,
            watched,
            play_count,
            position: Millis::new(position),
            last_played,
        };
        database
            .take_up_progress(
                user,
                &[
                    taken(never_here, true, 3, 0, Some(late)),
                    taken(watched_here, true, 5, 0, Some(early)),
                    taken(older_here, false, 0, 90_000, Some(late)),
                    taken(dated_here, false, 0, 30_000, None),
                ],
            )
            .await
            .unwrap();

        assert_eq!(kept(&database, user, never_here).await, ("watched".into(), 0, 3, Some(timestamp_to_text(late))));
        let (state, position, count, _) = kept(&database, user, watched_here).await;
        assert_eq!((state.as_str(), position, count), ("in_progress", 60_000, 5));
        assert_eq!(kept(&database, user, older_here).await, ("in_progress".into(), 90_000, 1, Some(timestamp_to_text(late))));
        assert_eq!(kept(&database, user, dated_here).await, ("watched".into(), 0, 2, Some(timestamp_to_text(early))));
    }

    #[tokio::test]
    async fn favourites_only_add_and_listens_keep_the_most_and_the_latest() {
        let (database, _, _) = collection().await;
        let user = somebody(&database).await;
        let songs: Vec<WorkId> = database
            .works_to_match_against()
            .await
            .unwrap()
            .into_iter()
            .filter(|work| work.kind == WorkKind::Song)
            .map(|work| work.id)
            .collect();
        database.take_up_favourites(user, &songs[..1]).await.unwrap();
        database.take_up_favourites(user, &songs[..2]).await.unwrap();
        assert_eq!(database.music_favourite_ids(user).await.unwrap().len(), 2);

        let late = datetime!(2024-06-01 10:00 UTC);
        database.record_listen(user, songs[0], late).await.unwrap();
        database
            .take_up_listens(
                user,
                &[
                    TakenListen { song: songs[0], listens: 7, last_listened: None },
                    TakenListen { song: songs[1], listens: 3, last_listened: None },
                ],
            )
            .await
            .unwrap();
        database
            .take_up_listens(user, &[TakenListen { song: songs[1], listens: 2, last_listened: Some(late) }])
            .await
            .unwrap();
        let listens: Vec<(String, i64, Option<String>)> =
            sqlx::query_as("SELECT song_id, listens, last_listened_at FROM music_listens WHERE user_id = ?")
                .bind(user.to_db_string())
                .fetch_all(database.reader())
                .await
                .unwrap();
        let of = |song: WorkId| listens.iter().find(|row| row.0 == song.to_db_string()).cloned().unwrap();
        assert_eq!((of(songs[0]).1, of(songs[0]).2), (7, Some(timestamp_to_text(late))));
        assert_eq!((of(songs[1]).1, of(songs[1]).2), (3, Some(timestamp_to_text(late))));
    }
}
