//! Reading a library of music back: its albums, its artists, its songs and
//! its genres, and the page of one album or one artist.
//!
//! Read apart from the grids of films, which never show music (see
//! `browse::met_on_its_own`). Every list is a page of a known length, so a
//! collection of a hundred thousand songs is read a screen at a time and
//! never whole.

use std::collections::HashMap;

use melyxar_core::id::{LibraryId, MediaSourceId, WorkId};
use melyxar_core::time::{Millis, Timestamp};
use sqlx::{AssertSqlSafe, Row};

use crate::browse::{escape_for_like, initial_of_a_title};
use crate::convert::{parse_id, parse_timestamp};
use crate::images::StoredImage;
use crate::{Database, Result};

/// An artist named on an album or a song, with where to find them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Credited {
    pub id: WorkId,
    pub name: String,
}

/// An album as a grid shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct AlbumCard {
    pub id: WorkId,
    pub library_id: LibraryId,
    pub title: String,
    /// Whose album it is.
    pub artists: Vec<Credited>,
    pub is_compilation: bool,
    pub year: Option<i32>,
    /// How many songs it holds.
    pub songs: i64,
    pub color: Option<String>,
    /// The letter it is filed under, `#` for a title beginning with none.
    pub initial: String,
    pub added_at: Timestamp,
    /// Every size of its cover, largest first.
    pub cover: Vec<StoredImage>,
}

/// An artist as a list shows them.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtistCard {
    pub id: WorkId,
    pub library_id: LibraryId,
    pub name: String,
    pub initial: String,
    /// Albums that are theirs.
    pub albums: i64,
    /// Songs they play on.
    pub songs: i64,
    pub color: Option<String>,
    /// Their own picture, and failing that the cover of their first album:
    /// an artist nobody photographed is still recognised by their record.
    pub picture: Vec<StoredImage>,
}

/// A song as a list shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct SongRow {
    pub id: WorkId,
    pub title: String,
    pub artists: Vec<Credited>,
    pub album: Option<Credited>,
    pub track: Option<i32>,
    pub disc: Option<i32>,
    pub year: Option<i32>,
    pub duration: Option<Millis>,
    /// The file it is played from, the first there is on a disk.
    pub source_id: Option<MediaSourceId>,
    /// Every size of the cover of its album, largest first: what a player
    /// shows while it plays.
    pub cover: Vec<StoredImage>,
    /// How loud it is, once measured or tagged, in LUFS.
    pub lufs: Option<f64>,
    /// The highest its sound truly reaches, in dBFS.
    pub peak_dbfs: Option<f64>,
    /// How loud its whole album is, for a player that keeps the differences
    /// between the songs of one album.
    pub album_lufs: Option<f64>,
}

/// One genre, how many albums carry it, and a few of them to show it by.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicGenre {
    pub name: String,
    pub albums: i64,
    /// The albums that arrived last, those with a cover first, at most
    /// [`ALBUMS_SHOWING_A_GENRE`].
    pub shown: Vec<GenreAlbum>,
}

/// An album a genre is shown by: its cover and its colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GenreAlbum {
    pub id: WorkId,
    pub color: Option<String>,
    /// Every size of its cover, largest first.
    pub cover: Vec<StoredImage>,
}

/// How many albums a genre is shown by.
pub const ALBUMS_SHOWING_A_GENRE: i64 = 3;

/// A letter of the list of albums or artists, and how many are filed under it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicInitial {
    pub letter: String,
    pub count: i64,
    /// Where the first of them stands in the list read by name, so a letter
    /// takes the list straight there.
    pub offset: i64,
}

/// A page of one list, and how long the whole list is.
#[derive(Debug, Clone, PartialEq)]
pub struct MusicPage<T> {
    pub items: Vec<T>,
    pub total: i64,
}

/// What a few words found in a library of music, a few of each.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MusicFound {
    pub albums: Vec<AlbumCard>,
    pub artists: Vec<ArtistCard>,
    pub songs: Vec<SongRow>,
}

/// How a list of albums is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlbumOrder {
    Title,
    Artist,
    Year,
    Added,
}

impl AlbumOrder {
    /// Written out rather than assembled from what a caller sends.
    fn clause(self, descending: bool) -> &'static str {
        match (self, descending) {
            (Self::Title, false) => "w.sort_title, w.id",
            (Self::Title, true) => "w.sort_title DESC, w.id",
            (Self::Artist, false) => "artist_sort, w.release_year, w.sort_title, w.id",
            (Self::Artist, true) => "artist_sort DESC, w.release_year DESC, w.sort_title, w.id",
            (Self::Year, false) => "w.release_year IS NULL, w.release_year, w.sort_title, w.id",
            (Self::Year, true) => "w.release_year IS NULL, w.release_year DESC, w.sort_title, w.id",
            (Self::Added, false) => "last_arrival DESC, w.id",
            (Self::Added, true) => "last_arrival, w.id",
        }
    }
}

/// How a list of songs is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SongOrder {
    Title,
    Album,
    Added,
}

impl SongOrder {
    fn clause(self, descending: bool) -> &'static str {
        match (self, descending) {
            (Self::Title, false) => "w.sort_title, w.id",
            (Self::Title, true) => "w.sort_title DESC, w.id",
            (Self::Album, false) => "album_sort, ms.disc_number, w.ordinal, w.sort_title, w.id",
            (Self::Album, true) => "album_sort DESC, ms.disc_number, w.ordinal, w.sort_title, w.id",
            (Self::Added, false) => "w.added_at DESC, w.id",
            (Self::Added, true) => "w.added_at, w.id",
        }
    }

    /// What finding a page in this order reads beside the song itself: the
    /// album and the disc only for the order that sorts on them, since joining
    /// them for every song passed over cost a deep page most of its time.
    fn sorted_on(self) -> &'static str {
        match self {
            Self::Album => {
                "w.id, al.sort_title AS album_sort FROM works w
                   LEFT JOIN music_songs ms ON ms.work_id = w.id
                   LEFT JOIN works al ON al.id = w.parent_id"
            }
            Self::Title | Self::Added => "w.id FROM works w",
        }
    }
}

/// What narrows a list of albums.
#[derive(Debug, Clone, Default)]
pub struct AlbumsWanted {
    pub genre: Option<String>,
    /// Only the albums that are this artist's.
    pub by: Option<WorkId>,
}

pub(crate) const AN_ALBUM: &str = concat!(
    "w.id, w.library_id, w.title, w.release_year, w.child_count, w.dominant_color, w.added_at,
     ma.is_compilation, ",
    initial_of_a_title!(),
    " AS initial,
     (SELECT a.sort_title FROM music_credits c JOIN works a ON a.id = c.artist_id
       WHERE c.work_id = w.id AND c.role = 'album_artist'
       ORDER BY c.ordinal LIMIT 1) AS artist_sort,
     coalesce((SELECT max(s.added_at) FROM works c JOIN media_sources s ON s.work_id = c.id
                WHERE c.parent_id = w.id), w.added_at) AS last_arrival"
);

pub(crate) const AN_ARTIST: &str = concat!(
    "w.id, w.library_id, w.title, w.dominant_color, ",
    initial_of_a_title!(),
    " AS initial,
     (SELECT count(*) FROM music_credits c
       WHERE c.artist_id = w.id AND c.role = 'album_artist') AS albums,
     (SELECT count(*) FROM music_credits c
       WHERE c.artist_id = w.id AND c.role = 'artist') AS songs"
);

pub(crate) const A_SONG: &str = "w.id, w.title, w.ordinal, w.release_year, ms.disc_number,
     w.parent_id, al.title AS album_title, al.sort_title AS album_sort,
     (SELECT s.id FROM media_sources s WHERE s.work_id = w.id AND s.missing_since IS NULL
       ORDER BY s.added_at LIMIT 1) AS source_id,
     (SELECT max(s.duration_ms) FROM media_sources s WHERE s.work_id = w.id) AS duration_ms,
     (SELECT t.loudness_integrated_lufs FROM media_sources s
        JOIN tracks t ON t.source_id = s.id AND t.kind = 'audio'
       WHERE s.work_id = w.id AND s.missing_since IS NULL
       ORDER BY s.added_at LIMIT 1) AS lufs,
     (SELECT t.loudness_true_peak_dbfs FROM media_sources s
        JOIN tracks t ON t.source_id = s.id AND t.kind = 'audio'
       WHERE s.work_id = w.id AND s.missing_since IS NULL
       ORDER BY s.added_at LIMIT 1) AS peak_dbfs";

impl Database {
    /// A page of the albums of a library.
    pub async fn music_albums(
        &self,
        library_id: LibraryId,
        wanted: &AlbumsWanted,
        order: AlbumOrder,
        descending: bool,
        offset: i64,
        limit: i64,
    ) -> Result<MusicPage<AlbumCard>> {
        let mut narrowing = String::new();
        if wanted.genre.is_some() {
            narrowing.push_str(
                " AND EXISTS (SELECT 1 FROM work_genres wg JOIN genres g ON g.id = wg.genre_id
                               WHERE wg.work_id = w.id AND g.name = ? COLLATE NOCASE)",
            );
        }
        if wanted.by.is_some() {
            narrowing.push_str(
                " AND EXISTS (SELECT 1 FROM music_credits c
                               WHERE c.work_id = w.id AND c.role = 'album_artist'
                                 AND c.artist_id = ?)",
            );
        }
        let from = format!(
            "FROM works w JOIN music_albums ma ON ma.work_id = w.id
             WHERE w.library_id = ? AND w.kind = 'album'{narrowing}"
        );
        let bind = |sql: String| {
            let mut query = sqlx::query(AssertSqlSafe(sql)).bind(library_id.to_db_string());
            if let Some(genre) = &wanted.genre {
                query = query.bind(genre.clone());
            }
            if let Some(artist) = wanted.by {
                query = query.bind(artist.to_db_string());
            }
            query
        };

        let total: i64 = bind(format!("SELECT count(*) AS total {from}"))
            .fetch_one(self.reader())
            .await?
            .try_get("total")?;
        let rows = bind(format!(
            "SELECT {AN_ALBUM} {from} ORDER BY {} LIMIT ? OFFSET ?",
            order.clause(descending)
        ))
        .bind(limit)
        .bind(offset)
        .fetch_all(self.reader())
        .await?;

        let items = self.album_cards(&rows).await?;
        Ok(MusicPage { items, total })
    }

    /// A page of the artists of a library: every one of them, or only those
    /// with an album of their own.
    pub async fn music_artists(
        &self,
        library_id: LibraryId,
        album_artists_only: bool,
        offset: i64,
        limit: i64,
    ) -> Result<MusicPage<ArtistCard>> {
        let from = format!(
            "FROM works w WHERE w.library_id = ? AND w.kind = 'artist'{}",
            if album_artists_only {
                " AND EXISTS (SELECT 1 FROM music_credits c
                               WHERE c.artist_id = w.id AND c.role = 'album_artist')"
            } else {
                ""
            }
        );
        let total: i64 = sqlx::query(AssertSqlSafe(format!("SELECT count(*) AS total {from}")))
            .bind(library_id.to_db_string())
            .fetch_one(self.reader())
            .await?
            .try_get("total")?;
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {AN_ARTIST} {from} ORDER BY w.sort_title, w.id LIMIT ? OFFSET ?"
        )))
        .bind(library_id.to_db_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(self.reader())
        .await?;

        let items = self.artist_cards(&rows).await?;
        Ok(MusicPage { items, total })
    }

    /// A page of the songs of a library.
    ///
    /// The songs of the page are found first and only they are then described:
    /// described while being sorted, every song passed over on the way to a
    /// deep page had its sources and loudness read for nothing.
    pub async fn music_songs(
        &self,
        library_id: LibraryId,
        order: SongOrder,
        descending: bool,
        offset: i64,
        limit: i64,
    ) -> Result<MusicPage<SongRow>> {
        let total: i64 =
            sqlx::query_scalar("SELECT count(*) FROM works WHERE library_id = ? AND kind = 'song'")
                .bind(library_id.to_db_string())
                .fetch_one(self.reader())
                .await?;
        let sorted = order.clause(descending);
        let rows = sqlx::query(AssertSqlSafe(format!(
            "WITH page AS (
               SELECT {} WHERE w.library_id = ? AND w.kind = 'song'
                ORDER BY {sorted} LIMIT ? OFFSET ?)
             SELECT {A_SONG}
               FROM page p
               JOIN works w ON w.id = p.id
               LEFT JOIN music_songs ms ON ms.work_id = w.id
               LEFT JOIN works al ON al.id = w.parent_id
              ORDER BY {sorted}",
            order.sorted_on()
        )))
        .bind(library_id.to_db_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(self.reader())
        .await?;
        let items = self.song_rows(&rows).await?;
        Ok(MusicPage { items, total })
    }

    /// The albums, artists and songs of a library whose name holds these
    /// words, the first few of each in the order their lists are read in.
    ///
    /// Both the name and its sort form, for the same reason as the grids of
    /// films: the sort form finds "ete" in "Été", the name finds a search
    /// that opens with the article the sort form dropped.
    pub async fn music_search(
        &self,
        library_id: LibraryId,
        words: &str,
        limit: i64,
    ) -> Result<MusicFound> {
        let pattern = format!("%{}%", escape_for_like(words));
        let named = "(w.title LIKE ? ESCAPE '\\' OR w.sort_title LIKE ? ESCAPE '\\')";
        let read = |sql: String| {
            sqlx::query(AssertSqlSafe(sql))
                .bind(library_id.to_db_string())
                .bind(pattern.clone())
                .bind(pattern.clone())
                .bind(limit)
        };

        let albums = read(format!(
            "SELECT {AN_ALBUM} FROM works w JOIN music_albums ma ON ma.work_id = w.id
              WHERE w.library_id = ? AND w.kind = 'album' AND {named}
              ORDER BY {} LIMIT ?",
            AlbumOrder::Title.clause(false)
        ))
        .fetch_all(self.reader())
        .await?;
        let artists = read(format!(
            "SELECT {AN_ARTIST} FROM works w
              WHERE w.library_id = ? AND w.kind = 'artist' AND {named}
              ORDER BY w.sort_title, w.id LIMIT ?"
        ))
        .fetch_all(self.reader())
        .await?;
        let songs = read(format!(
            "SELECT {A_SONG}
               FROM works w
               LEFT JOIN music_songs ms ON ms.work_id = w.id
               LEFT JOIN works al ON al.id = w.parent_id
              WHERE w.library_id = ? AND w.kind = 'song' AND {named}
              ORDER BY {} LIMIT ?",
            SongOrder::Title.clause(false)
        ))
        .fetch_all(self.reader())
        .await?;

        Ok(MusicFound {
            albums: self.album_cards(&albums).await?,
            artists: self.artist_cards(&artists).await?,
            songs: self.song_rows(&songs).await?,
        })
    }

    /// Every genre the albums of a library carry, by name, each with the
    /// albums it is shown by.
    pub async fn music_genres(&self, library_id: LibraryId) -> Result<Vec<MusicGenre>> {
        let rows = sqlx::query(
            "SELECT g.id, g.name, count(*) AS albums
               FROM genres g
               JOIN work_genres wg ON wg.genre_id = g.id
               JOIN works w ON w.id = wg.work_id
              WHERE w.library_id = ? AND w.kind = 'album'
              GROUP BY g.id
              ORDER BY g.name COLLATE NOCASE",
        )
        .bind(library_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        // The last ones to arrive of each genre, read in one go: a cover
        // first, since a genre is shown by its pictures.
        let shown_rows = sqlx::query(
            "WITH ranked AS (
               SELECT wg.genre_id, w.id, w.dominant_color,
                      row_number() OVER (
                        PARTITION BY wg.genre_id
                        ORDER BY EXISTS (SELECT 1 FROM images i
                                          WHERE i.owner_kind = 'work' AND i.owner_id = w.id
                                            AND i.image_kind = 'poster') DESC,
                                 w.added_at DESC, w.id
                      ) AS place
                 FROM work_genres wg
                 JOIN works w ON w.id = wg.work_id
                WHERE w.library_id = ? AND w.kind = 'album')
             SELECT genre_id, id, dominant_color FROM ranked
              WHERE place <= ?
              ORDER BY genre_id, place",
        )
        .bind(library_id.to_db_string())
        .bind(ALBUMS_SHOWING_A_GENRE)
        .fetch_all(self.reader())
        .await?;
        let mut shown: HashMap<String, Vec<(WorkId, Option<String>)>> = HashMap::new();
        for row in &shown_rows {
            shown
                .entry(row.try_get("genre_id")?)
                .or_default()
                .push((parse_id(&row.try_get::<String, _>("id")?)?, row.try_get("dominant_color")?));
        }
        let albums: Vec<WorkId> = shown.values().flatten().map(|(id, _)| *id).collect();
        let covers = self.posters_of(&albums).await?;

        rows.iter()
            .map(|row| {
                let genre: String = row.try_get("id")?;
                Ok(MusicGenre {
                    name: row.try_get("name")?,
                    albums: row.try_get("albums")?,
                    shown: shown
                        .remove(&genre)
                        .unwrap_or_default()
                        .into_iter()
                        .map(|(id, color)| GenreAlbum {
                            id,
                            color,
                            cover: covers.get(&id).cloned().unwrap_or_default(),
                        })
                        .collect(),
                })
            })
            .collect()
    }

    /// The letters the albums, or the artists, of a library are filed under,
    /// with where each begins in the list read by name.
    pub async fn music_initials(
        &self,
        library_id: LibraryId,
        artists: bool,
        album_artists_only: bool,
    ) -> Result<Vec<MusicInitial>> {
        let narrowing = match (artists, album_artists_only) {
            (false, _) => "w.kind = 'album'",
            (true, false) => "w.kind = 'artist'",
            (true, true) => {
                "w.kind = 'artist' AND EXISTS (SELECT 1 FROM music_credits c
                                                WHERE c.artist_id = w.id
                                                  AND c.role = 'album_artist')"
            }
        };
        // Where a letter begins is how many sort before its first title:
        // titles beginning with no letter sit at both ends of the list, and
        // lead to the first of them. Read off one walk down the list in
        // order, each title knowing its place in it: counted again for every
        // letter instead, a library of a thousand albums took half a second.
        let rows = sqlx::query(AssertSqlSafe(format!(
            "WITH placed AS (
                 SELECT {} AS letter,
                        row_number() OVER (ORDER BY w.sort_title) - 1 AS place
                   FROM works w
                  WHERE w.library_id = ? AND {narrowing}
             )
             SELECT letter, count(*) AS count, min(place) AS offset
               FROM placed
              GROUP BY letter
              ORDER BY letter",
            initial_of_a_title!()
        )))
        .bind(library_id.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter()
            .map(|row| {
                Ok(MusicInitial {
                    letter: row.try_get("letter")?,
                    count: row.try_get("count")?,
                    offset: row.try_get("offset")?,
                })
            })
            .collect()
    }

    /// One album, with its songs in their order on it.
    pub async fn music_album(&self, album: WorkId) -> Result<Option<(AlbumCard, Vec<SongRow>)>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {AN_ALBUM} FROM works w JOIN music_albums ma ON ma.work_id = w.id
              WHERE w.id = ?"
        )))
        .bind(album.to_db_string())
        .fetch_all(self.reader())
        .await?;
        let Some(card) = self.album_cards(&rows).await?.pop() else {
            return Ok(None);
        };
        let songs = sqlx::query(AssertSqlSafe(format!(
            "SELECT {A_SONG}
               FROM works w
               LEFT JOIN music_songs ms ON ms.work_id = w.id
               LEFT JOIN works al ON al.id = w.parent_id
              WHERE w.parent_id = ?
              ORDER BY ms.disc_number IS NULL, ms.disc_number, w.ordinal IS NULL, w.ordinal,
                       w.sort_title"
        )))
        .bind(album.to_db_string())
        .fetch_all(self.reader())
        .await?;
        Ok(Some((card, self.song_rows(&songs).await?)))
    }

    /// One artist, with the albums that are theirs and the ones they only
    /// play on, the earliest first.
    ///
    /// Both found from the artist's own credits: asked of every album of the
    /// library in turn, the albums they play on took a second on a large one.
    pub async fn music_artist(
        &self,
        artist: WorkId,
    ) -> Result<Option<(ArtistCard, Vec<AlbumCard>, Vec<AlbumCard>)>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {AN_ARTIST} FROM works w WHERE w.id = ? AND w.kind = 'artist'"
        )))
        .bind(artist.to_db_string())
        .fetch_all(self.reader())
        .await?;
        let Some(card) = self.artist_cards(&rows).await?.pop() else {
            return Ok(None);
        };
        let order = AlbumOrder::Year.clause(false);
        let theirs = sqlx::query(AssertSqlSafe(format!(
            "SELECT {AN_ALBUM} FROM works w JOIN music_albums ma ON ma.work_id = w.id
              WHERE w.id IN (SELECT c.work_id FROM music_credits c
                              WHERE c.artist_id = ?1 AND c.role = 'album_artist')
              ORDER BY {order}"
        )))
        .bind(artist.to_db_string())
        .fetch_all(self.reader())
        .await?;
        let played_on = sqlx::query(AssertSqlSafe(format!(
            "SELECT {AN_ALBUM} FROM works w JOIN music_albums ma ON ma.work_id = w.id
              WHERE w.id IN (SELECT song.parent_id FROM music_credits c
                               JOIN works song ON song.id = c.work_id
                              WHERE c.artist_id = ?1 AND c.role = 'artist')
                AND NOT EXISTS (SELECT 1 FROM music_credits c WHERE c.work_id = w.id
                                 AND c.role = 'album_artist' AND c.artist_id = ?1)
              ORDER BY {order}"
        )))
        .bind(artist.to_db_string())
        .fetch_all(self.reader())
        .await?;
        Ok(Some((
            card,
            self.album_cards(&theirs).await?,
            self.album_cards(&played_on).await?,
        )))
    }

    /// One song, as a list of them would give it.
    pub async fn music_song(&self, song: WorkId) -> Result<Option<SongRow>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {A_SONG}
               FROM works w
               LEFT JOIN music_songs ms ON ms.work_id = w.id
               LEFT JOIN works al ON al.id = w.parent_id
              WHERE w.id = ? AND w.kind = 'song'"
        )))
        .bind(song.to_db_string())
        .fetch_all(self.reader())
        .await?;
        Ok(self.song_rows(&rows).await?.into_iter().next())
    }

    /// Every song an artist plays on, album by album from the earliest,
    /// each in its order on its album.
    pub async fn music_artist_songs(&self, artist: WorkId) -> Result<Vec<SongRow>> {
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {A_SONG}
               FROM works w
               JOIN music_credits c ON c.work_id = w.id AND c.role = 'artist' AND c.artist_id = ?
               LEFT JOIN music_songs ms ON ms.work_id = w.id
               LEFT JOIN works al ON al.id = w.parent_id
              WHERE w.kind = 'song'
              ORDER BY al.release_year, al.sort_title, al.id, ms.disc_number, w.ordinal,
                       w.sort_title, w.id"
        )))
        .bind(artist.to_db_string())
        .fetch_all(self.reader())
        .await?;
        self.song_rows(&rows).await
    }

    /// How loud each of these albums is, from its songs measured so far.
    async fn album_loudness_of(&self, albums: &[WorkId]) -> Result<HashMap<WorkId, f64>> {
        if albums.is_empty() {
            return Ok(HashMap::new());
        }
        let mut distinct = albums.to_vec();
        distinct.sort();
        distinct.dedup();
        let marks = vec!["?"; distinct.len()].join(", ");
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT w.parent_id, t.loudness_integrated_lufs AS lufs, s.duration_ms
               FROM works w
               JOIN media_sources s ON s.work_id = w.id AND s.missing_since IS NULL
               JOIN tracks t ON t.source_id = s.id AND t.kind = 'audio'
              WHERE w.kind = 'song' AND w.parent_id IN ({marks})
                AND t.loudness_integrated_lufs IS NOT NULL AND s.duration_ms IS NOT NULL"
        )));
        for album in &distinct {
            query = query.bind(album.to_db_string());
        }
        let mut songs: HashMap<WorkId, Vec<(f64, i64)>> = HashMap::new();
        for row in query.fetch_all(self.reader()).await? {
            songs
                .entry(parse_id(&row.try_get::<String, _>("parent_id")?)?)
                .or_default()
                .push((row.try_get("lufs")?, row.try_get("duration_ms")?));
        }
        Ok(songs
            .into_iter()
            .filter_map(|(album, songs)| {
                melyxar_core::music::album_loudness(&songs).map(|lufs| (album, lufs))
            })
            .collect())
    }

    /// Albums read from rows, with their artists and their covers.
    pub(crate) async fn album_cards(
        &self,
        rows: &[sqlx::sqlite::SqliteRow],
    ) -> Result<Vec<AlbumCard>> {
        let ids: Vec<WorkId> = rows
            .iter()
            .map(|row| parse_id(&row.try_get::<String, _>("id")?))
            .collect::<Result<_>>()?;
        let mut artists = self.credited_on(&ids, "album_artist").await?;
        let mut covers = self.posters_of(&ids).await?;
        rows.iter()
            .zip(ids)
            .map(|(row, id)| {
                Ok(AlbumCard {
                    id,
                    library_id: parse_id(&row.try_get::<String, _>("library_id")?)?,
                    title: row.try_get("title")?,
                    artists: artists.remove(&id).unwrap_or_default(),
                    is_compilation: row.try_get::<i64, _>("is_compilation")? != 0,
                    year: row.try_get("release_year")?,
                    songs: row.try_get("child_count")?,
                    color: row.try_get("dominant_color")?,
                    initial: row.try_get("initial")?,
                    added_at: parse_timestamp(&row.try_get::<String, _>("added_at")?)?,
                    cover: covers.remove(&id).unwrap_or_default(),
                })
            })
            .collect()
    }

    /// Artists read from rows, each with their picture or the cover of their
    /// first album.
    pub(crate) async fn artist_cards(
        &self,
        rows: &[sqlx::sqlite::SqliteRow],
    ) -> Result<Vec<ArtistCard>> {
        let ids: Vec<WorkId> = rows
            .iter()
            .map(|row| parse_id(&row.try_get::<String, _>("id")?))
            .collect::<Result<_>>()?;
        let mut pictures = self.posters_of(&ids).await?;

        let lacking: Vec<WorkId> = ids
            .iter()
            .filter(|id| !pictures.contains_key(id))
            .copied()
            .collect();
        let lent = self.first_albums_of(&lacking).await?;
        let albums: Vec<WorkId> = lent.values().copied().collect();
        let mut covers = self.posters_of(&albums).await?;
        for (artist, album) in lent {
            if let Some(cover) = covers.remove(&album) {
                pictures.insert(artist, cover);
            }
        }

        rows.iter()
            .zip(ids)
            .map(|(row, id)| {
                Ok(ArtistCard {
                    id,
                    library_id: parse_id(&row.try_get::<String, _>("library_id")?)?,
                    name: row.try_get("title")?,
                    initial: row.try_get("initial")?,
                    albums: row.try_get("albums")?,
                    songs: row.try_get("songs")?,
                    color: row.try_get("dominant_color")?,
                    picture: pictures.remove(&id).unwrap_or_default(),
                })
            })
            .collect()
    }

    /// Songs read from rows, with their artists.
    pub(crate) async fn song_rows(&self, rows: &[sqlx::sqlite::SqliteRow]) -> Result<Vec<SongRow>> {
        let ids: Vec<WorkId> = rows
            .iter()
            .map(|row| parse_id(&row.try_get::<String, _>("id")?))
            .collect::<Result<_>>()?;
        let artists = self.credited_on(&ids, "artist").await?;
        let albums: Vec<WorkId> = rows
            .iter()
            .filter_map(|row| row.try_get::<Option<String>, _>("parent_id").ok().flatten())
            .map(|album| parse_id(&album))
            .collect::<Result<_>>()?;
        let covers = self.posters_of(&albums).await?;
        let album_loudness = self.album_loudness_of(&albums).await?;
        rows.iter()
            .zip(ids)
            .map(|(row, id)| {
                let album = row
                    .try_get::<Option<String>, _>("parent_id")?
                    .map(|album| -> Result<Credited> {
                        Ok(Credited {
                            id: parse_id(&album)?,
                            name: row.try_get("album_title")?,
                        })
                    })
                    .transpose()?;
                Ok(SongRow {
                    id,
                    title: row.try_get("title")?,
                    artists: artists.get(&id).cloned().unwrap_or_default(),
                    track: row.try_get("ordinal")?,
                    disc: row.try_get("disc_number")?,
                    year: row.try_get("release_year")?,
                    duration: row
                        .try_get::<Option<i64>, _>("duration_ms")?
                        .map(Millis::new),
                    source_id: row
                        .try_get::<Option<String>, _>("source_id")?
                        .map(|source| parse_id(&source))
                        .transpose()?,
                    cover: album
                        .as_ref()
                        .and_then(|album| covers.get(&album.id))
                        .cloned()
                        .unwrap_or_default(),
                    lufs: row.try_get("lufs")?,
                    peak_dbfs: row.try_get("peak_dbfs")?,
                    album_lufs: album
                        .as_ref()
                        .and_then(|album| album_loudness.get(&album.id))
                        .copied(),
                    album,
                })
            })
            .collect()
    }

    /// Who is credited in one role on each of these works, in order. One
    /// query for the whole page.
    async fn credited_on(
        &self,
        works: &[WorkId],
        role: &str,
    ) -> Result<HashMap<WorkId, Vec<Credited>>> {
        let mut found: HashMap<WorkId, Vec<Credited>> = HashMap::new();
        if works.is_empty() {
            return Ok(found);
        }
        // Only question marks are assembled, one per identifier this crate
        // read back itself.
        let places = vec!["?"; works.len()].join(", ");
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT c.work_id, a.id, a.title FROM music_credits c
               JOIN works a ON a.id = c.artist_id
              WHERE c.role = ? AND c.work_id IN ({places})
              ORDER BY c.work_id, c.ordinal"
        )))
        .bind(role);
        for work in works {
            query = query.bind(work.to_db_string());
        }
        for row in query.fetch_all(self.reader()).await? {
            found
                .entry(parse_id(&row.try_get::<String, _>("work_id")?)?)
                .or_default()
                .push(Credited {
                    id: parse_id(&row.try_get::<String, _>("id")?)?,
                    name: row.try_get("title")?,
                });
        }
        Ok(found)
    }

    /// The earliest album of each of these artists that wears a cover.
    async fn first_albums_of(&self, artists: &[WorkId]) -> Result<HashMap<WorkId, WorkId>> {
        let mut found = HashMap::new();
        if artists.is_empty() {
            return Ok(found);
        }
        let places = vec!["?"; artists.len()].join(", ");
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT c.artist_id, w.id
               FROM music_credits c
               JOIN works w ON w.id = c.work_id
              WHERE c.role = 'album_artist' AND c.artist_id IN ({places})
                AND EXISTS (SELECT 1 FROM images i WHERE i.owner_kind = 'work'
                             AND i.owner_id = w.id AND i.image_kind = 'poster')
              ORDER BY c.artist_id, w.release_year IS NULL, w.release_year, w.sort_title"
        )));
        for artist in artists {
            query = query.bind(artist.to_db_string());
        }
        for row in query.fetch_all(self.reader()).await? {
            let artist: WorkId = parse_id(&row.try_get::<String, _>("artist_id")?)?;
            found
                .entry(artist)
                .or_insert(parse_id(&row.try_get::<String, _>("id")?)?);
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music_testing::{collection, empty_music_library, named};

    fn titles(albums: &[AlbumCard]) -> Vec<&str> {
        albums.iter().map(|album| album.title.as_str()).collect()
    }

    #[tokio::test]
    async fn albums_are_read_a_page_at_a_time_in_every_order() {
        let (database, library, _) = collection().await;
        let wanted = AlbumsWanted::default();

        let by_title = database
            .music_albums(library, &wanted, AlbumOrder::Title, false, 0, 2)
            .await
            .expect("read");
        assert_eq!(by_title.total, 4);
        assert_eq!(
            titles(&by_title.items),
            vec!["Early Days", "Northern Lights"]
        );
        let next = database
            .music_albums(library, &wanted, AlbumOrder::Title, false, 2, 2)
            .await
            .expect("read");
        assert_eq!(titles(&next.items), vec!["Road", "Summer Hits"]);

        let by_year = database
            .music_albums(library, &wanted, AlbumOrder::Year, true, 0, 10)
            .await
            .expect("read");
        assert_eq!(
            titles(&by_year.items),
            vec!["Summer Hits", "Northern Lights", "Road", "Early Days"]
        );

        let by_artist = database
            .music_albums(library, &wanted, AlbumOrder::Artist, false, 0, 10)
            .await
            .expect("read");
        assert_eq!(
            titles(&by_artist.items),
            vec!["Early Days", "Northern Lights", "Road", "Summer Hits"],
            "Amber Field's by year, then The Lanterns', then everybody's"
        );

        let lights = &by_title.items[1];
        assert_eq!(lights.songs, 2);
        assert_eq!(lights.year, Some(2019));
        assert_eq!(lights.initial, "n");
        assert_eq!(
            lights
                .artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Amber Field"]
        );
        assert!(next.items[1].is_compilation);
    }

    #[tokio::test]
    async fn an_album_a_song_has_just_reached_comes_first_among_the_newest() {
        let (database, library, _) = collection().await;
        let wanted = AlbumsWanted::default();
        let first = || async {
            database
                .music_albums(library, &wanted, AlbumOrder::Added, false, 0, 1)
                .await
                .expect("read")
                .items
                .remove(0)
                .id
        };
        let before = first().await;
        let oldest: String = sqlx::query_scalar(
            "SELECT w.id FROM works w WHERE w.library_id = ? AND w.kind = 'album' AND w.id <> ?
              ORDER BY w.added_at LIMIT 1",
        )
        .bind(library.to_db_string())
        .bind(before.to_db_string())
        .fetch_one(database.reader())
        .await
        .expect("another album");
        sqlx::query(
            "UPDATE media_sources SET added_at = '2999-01-01T00:00:00Z'
              WHERE work_id = (SELECT id FROM works WHERE parent_id = ? LIMIT 1)",
        )
        .bind(&oldest)
        .execute(database.writer())
        .await
        .expect("a song arrives");
        assert_eq!(first().await.to_db_string(), oldest);
    }

    #[tokio::test]
    async fn albums_are_narrowed_to_a_genre_or_to_an_artist() {
        let (database, library, _) = collection().await;
        let pop = database
            .music_albums(
                library,
                &AlbumsWanted {
                    genre: Some("pop".to_string()),
                    by: None,
                },
                AlbumOrder::Title,
                false,
                0,
                10,
            )
            .await
            .expect("read");
        assert_eq!(titles(&pop.items), vec!["Early Days", "Summer Hits"]);

        let amber = database
            .music_artists(library, true, 0, 10)
            .await
            .expect("read")
            .items
            .into_iter()
            .find(|artist| artist.name == "Amber Field")
            .expect("an album artist");
        let theirs = database
            .music_albums(
                library,
                &AlbumsWanted {
                    genre: None,
                    by: Some(amber.id),
                },
                AlbumOrder::Title,
                false,
                0,
                10,
            )
            .await
            .expect("read");
        assert_eq!(titles(&theirs.items), vec!["Early Days", "Northern Lights"]);
    }

    #[tokio::test]
    async fn artists_are_every_one_who_plays_or_only_those_with_an_album() {
        let (database, library, _) = collection().await;
        let everybody = database
            .music_artists(library, false, 0, 10)
            .await
            .expect("read");
        assert_eq!(
            everybody
                .items
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Amber Field", "The Lanterns", "Quartz", "Various Artists"]
        );
        let with_albums = database
            .music_artists(library, true, 0, 10)
            .await
            .expect("read");
        assert_eq!(
            with_albums
                .items
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Amber Field", "The Lanterns", "Various Artists"],
            "a guest on a compilation has no album of their own"
        );
        let amber = &everybody.items[0];
        assert_eq!((amber.albums, amber.songs), (2, 4));
    }

    #[tokio::test]
    async fn songs_are_read_a_page_at_a_time_with_their_album_and_artists() {
        let (database, library, _) = collection().await;
        let page = database
            .music_songs(library, SongOrder::Album, false, 0, 10)
            .await
            .expect("read");
        assert_eq!(page.total, 5);
        let first: Vec<&str> = page.items.iter().map(|song| song.title.as_str()).collect();
        assert_eq!(
            first,
            vec![
                "Beginnings",
                "Quiet Harbour",
                "Tides",
                "The Long Road",
                "Zebra"
            ]
        );
        let road = &page.items[3];
        assert_eq!(road.album.as_ref().map(|a| a.name.as_str()), Some("Road"));
        assert_eq!(
            road.artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>(),
            vec!["The Lanterns", "Amber Field"]
        );
        assert!(road.source_id.is_some(), "a song is played from its file");
    }

    #[tokio::test]
    async fn songs_read_page_by_page_come_in_the_order_of_the_whole_list() {
        let (database, library, _) = collection().await;
        for order in [SongOrder::Title, SongOrder::Album, SongOrder::Added] {
            for descending in [false, true] {
                let whole = database
                    .music_songs(library, order, descending, 0, 10)
                    .await
                    .expect("read");
                let mut paged = Vec::new();
                for offset in (0..whole.total).step_by(2) {
                    let page = database
                        .music_songs(library, order, descending, offset, 2)
                        .await
                        .expect("read");
                    paged.extend(page.items.into_iter().map(|song| song.id));
                }
                let ids: Vec<_> = whole.items.into_iter().map(|song| song.id).collect();
                assert_eq!(paged, ids, "{order:?} descending {descending}");
            }
        }
    }

    #[tokio::test]
    async fn an_album_opens_on_its_songs_in_their_order() {
        let (database, library, _) = collection().await;
        let lights = database
            .music_albums(
                library,
                &AlbumsWanted::default(),
                AlbumOrder::Title,
                false,
                1,
                1,
            )
            .await
            .expect("read")
            .items
            .remove(0);
        let (card, songs) = database
            .music_album(lights.id)
            .await
            .expect("read")
            .expect("there");
        assert_eq!(card.title, "Northern Lights");
        assert_eq!(
            songs
                .iter()
                .map(|s| (s.track, s.title.as_str()))
                .collect::<Vec<_>>(),
            vec![(Some(1), "Quiet Harbour"), (Some(2), "Tides")]
        );
        assert!(
            database
                .music_album(WorkId::new())
                .await
                .expect("read")
                .is_none()
        );
    }

    #[tokio::test]
    async fn an_artist_opens_on_their_albums_and_the_ones_they_play_on() {
        let (database, library, _) = collection().await;
        let amber = database
            .music_artists(library, false, 0, 1)
            .await
            .expect("read")
            .items
            .remove(0);
        let (card, theirs, played_on) = database
            .music_artist(amber.id)
            .await
            .expect("read")
            .expect("there");
        assert_eq!(card.name, "Amber Field");
        assert_eq!(
            titles(&theirs),
            vec!["Early Days", "Northern Lights"],
            "earliest first"
        );
        assert_eq!(titles(&played_on), vec!["Road"]);
    }

    #[tokio::test]
    async fn a_search_finds_albums_artists_and_songs_by_a_piece_of_their_name() {
        let (database, library, _) = collection().await;
        let found = database
            .music_search(library, "road", 10)
            .await
            .expect("searched");
        assert_eq!(titles(&found.albums), vec!["Road"]);
        assert!(found.artists.is_empty());
        assert_eq!(
            found
                .songs
                .iter()
                .map(|s| s.title.as_str())
                .collect::<Vec<_>>(),
            vec!["The Long Road"]
        );

        let amber = database
            .music_search(library, "amber", 10)
            .await
            .expect("searched");
        assert_eq!(
            amber
                .artists
                .iter()
                .map(|a| a.name.as_str())
                .collect::<Vec<_>>(),
            vec!["Amber Field"]
        );
        assert!(amber.albums.is_empty() && amber.songs.is_empty());

        let few = database
            .music_search(library, "e", 1)
            .await
            .expect("searched");
        assert_eq!(few.albums.len(), 1);
        assert_eq!(few.songs.len(), 1);

        let nothing = database
            .music_search(library, "100%", 10)
            .await
            .expect("searched");
        assert_eq!(nothing, MusicFound::default());
    }

    #[tokio::test]
    async fn every_song_an_artist_plays_on_comes_in_the_order_of_their_albums() {
        let (database, library, _) = collection().await;
        let amber = database
            .music_artists(library, false, 0, 10)
            .await
            .expect("read")
            .items
            .into_iter()
            .find(|artist| artist.name == "Amber Field")
            .expect("in the collection")
            .id;
        let songs = database.music_artist_songs(amber).await.expect("read");
        assert_eq!(
            songs
                .iter()
                .map(|song| song.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Beginnings", "The Long Road", "Quiet Harbour", "Tides"],
            "2011, then the song they sing on in 2015, then 2019 in its order"
        );
    }

    #[tokio::test]
    async fn one_song_is_read_with_its_artists_and_its_album() {
        let (database, library, _) = collection().await;
        let listed = database
            .music_songs(library, SongOrder::Title, false, 0, 50)
            .await
            .expect("read")
            .items;
        let wanted = listed
            .iter()
            .find(|song| song.title == "Tides")
            .expect("in the collection");
        let found = database
            .music_song(wanted.id)
            .await
            .expect("read")
            .expect("there");
        assert_eq!(&found, wanted);
        let album = database
            .music_albums(library, &AlbumsWanted::default(), AlbumOrder::Title, false, 0, 1)
            .await
            .expect("read")
            .items
            .remove(0);
        assert_eq!(
            database.music_song(album.id).await.expect("read"),
            None,
            "an album is not a song"
        );
    }

    #[tokio::test]
    async fn genres_and_letters_say_what_is_there_and_where_it_begins() {
        let (database, library, _) = collection().await;
        let genres = database.music_genres(library).await.expect("read");
        assert_eq!(
            genres
                .iter()
                .map(|g| (g.name.as_str(), g.albums, g.shown.len()))
                .collect::<Vec<_>>(),
            vec![("Folk", 1, 1), ("Pop", 2, 2), ("Rock", 1, 1)]
        );

        let letters = database
            .music_initials(library, false, false)
            .await
            .expect("read");
        assert_eq!(
            letters
                .iter()
                .map(|l| (l.letter.as_str(), l.count, l.offset))
                .collect::<Vec<_>>(),
            vec![("e", 1, 0), ("n", 1, 1), ("r", 1, 2), ("s", 1, 3)]
        );
    }

    /// A genre is shown by its albums with a cover before those without, and
    /// by no more of them than a tile holds.
    #[tokio::test]
    async fn a_genre_is_shown_by_its_albums_with_a_cover_first() {
        use crate::images::StoredImage;
        use crate::music::MusicFile;
        use melyxar_core::id::MediaSourceId;
        use melyxar_core::music::{AlbumFiling, SongFiling};
        use melyxar_core::time::now;
        use std::path::PathBuf;

        let (database, library, root) = collection().await;
        let pop = database
            .music_albums(
                library,
                &AlbumsWanted {
                    genre: Some("Pop".to_string()),
                    by: None,
                },
                AlbumOrder::Title,
                false,
                0,
                10,
            )
            .await
            .expect("read")
            .items;
        assert_eq!(pop.len(), 2);
        // Whichever would come first otherwise, the one given a cover leads.
        for pictured in &pop {
            for album in &pop {
                database
                    .replace_images("work", &album.id.to_db_string(), "poster", &[])
                    .await
                    .expect("cleared");
            }
            let poster = StoredImage {
                owner_kind: "work".to_string(),
                owner_id: pictured.id.to_db_string(),
                image_kind: "poster".to_string(),
                relative_path: format!("works/{}/poster-400.webp", pictured.id),
                width: Some(400),
                height: Some(400),
                fingerprint: "a cover".to_string(),
                dominant_color: Some("#335577".to_string()),
            };
            database
                .replace_images("work", &pictured.id.to_db_string(), "poster", &[poster])
                .await
                .expect("stored");
            let genres = database.music_genres(library).await.expect("read");
            let shown = &genres.iter().find(|g| g.name == "Pop").expect("pop").shown;
            assert_eq!(shown[0].id, pictured.id);
            assert_eq!(shown[0].cover.len(), 1);
            assert!(shown[1].cover.is_empty());
        }

        // A genre carried by more albums than a tile holds shows that many.
        let more: Vec<_> = (0..5)
            .map(|at| MusicFile {
                source_id: MediaSourceId::new(),
                song: None,
                relative_path: PathBuf::from(format!("More/{at}/01.flac")),
                size_bytes: 10,
                modified_at: now(),
                filing: SongFiling {
                    title: named("A song"),
                    artists: vec![named("Somebody")],
                    album: Some(AlbumFiling {
                        title: named(&format!("Album {at}")),
                        artists: vec![named("Somebody")],
                        is_compilation: false,
                    }),
                    track: Some(1),
                    disc: Some(1),
                    year: Some(2020),
                    genres: vec!["Pop".to_string()],
                },
                reading: Err("not read in this test".to_string()),
            })
            .collect();
        database.file_music(library, root, &more).await.expect("filed");
        let genres = database.music_genres(library).await.expect("read");
        let pop = genres.iter().find(|g| g.name == "Pop").expect("pop");
        assert_eq!(pop.albums, 7);
        assert_eq!(pop.shown.len() as i64, ALBUMS_SHOWING_A_GENRE);
    }

    /// Titles that begin with no letter sit at both ends of the list, and two
    /// albums can share a title: each letter still leads to where its first
    /// title stands.
    #[tokio::test]
    async fn a_letter_leads_to_its_first_title_wherever_the_others_sort() {
        use melyxar_core::id::MediaSourceId;
        use melyxar_core::music::{AlbumFiling, SongFiling};
        use melyxar_core::time::now;
        use std::path::PathBuf;

        let (database, library, root) = empty_music_library().await;
        let albums = [
            ("Zoo", "X"),
            ("~Tilde", "X"),
            ("Arrival", "X"),
            ("10 Years", "X"),
            ("Arrival", "Y"),
        ];
        let files: Vec<crate::music::MusicFile> = albums
            .iter()
            .enumerate()
            .map(|(place, (album, artist))| crate::music::MusicFile {
                source_id: MediaSourceId::new(),
                song: None,
                relative_path: PathBuf::from(format!("{place}.flac")),
                size_bytes: 10,
                modified_at: now(),
                filing: SongFiling {
                    title: named("Song"),
                    artists: vec![named(artist)],
                    album: Some(AlbumFiling {
                        title: named(album),
                        artists: vec![named(artist)],
                        is_compilation: false,
                    }),
                    track: Some(1),
                    disc: Some(1),
                    year: None,
                    genres: Vec::new(),
                },
                reading: Err("not read in this test".to_string()),
            })
            .collect();
        database.file_music(library, root, &files).await.expect("filed");

        let letters = database
            .music_initials(library, false, false)
            .await
            .expect("read");
        assert_eq!(
            letters
                .iter()
                .map(|l| (l.letter.as_str(), l.count, l.offset))
                .collect::<Vec<_>>(),
            vec![("#", 2, 0), ("a", 2, 1), ("z", 1, 3)]
        );
    }

    /// A list read by title comes out of its index already in order, ties
    /// included, so a page of it reads that page and no more.
    #[tokio::test]
    async fn a_list_read_by_title_is_never_sorted_by_hand() {
        let (database, library, _) = empty_music_library().await;
        for kind in ["album", "artist", "song"] {
            let plan: Vec<String> = sqlx::query(AssertSqlSafe(format!(
                "EXPLAIN QUERY PLAN
                 SELECT w.id FROM works w
                  WHERE w.library_id = ? AND w.kind = '{kind}'
                  ORDER BY {} LIMIT 1",
                SongOrder::Title.clause(false)
            )))
            .bind(library.to_db_string())
            .fetch_all(database.reader())
            .await
            .expect("planned")
            .iter()
            .map(|step| step.try_get("detail").expect("a step says what it does"))
            .collect();
            assert!(
                plan.iter().all(|step| !step.contains("TEMP B-TREE")),
                "{kind}: {plan:?}"
            );
        }
    }
}
