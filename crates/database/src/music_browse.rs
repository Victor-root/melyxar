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

use crate::browse::initial_of_a_title;
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
}

/// One genre, and how many albums carry it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MusicGenre {
    pub name: String,
    pub albums: i64,
}

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
            (Self::Added, false) => "w.added_at DESC, w.id",
            (Self::Added, true) => "w.added_at, w.id",
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
}

/// What narrows a list of albums.
#[derive(Debug, Clone, Default)]
pub struct AlbumsWanted {
    pub genre: Option<String>,
    /// Only the albums that are this artist's.
    pub by: Option<WorkId>,
}

const AN_ALBUM: &str = concat!(
    "w.id, w.library_id, w.title, w.release_year, w.child_count, w.dominant_color, w.added_at,
     ma.is_compilation, ",
    initial_of_a_title!(),
    " AS initial,
     (SELECT a.sort_title FROM music_credits c JOIN works a ON a.id = c.artist_id
       WHERE c.work_id = w.id AND c.role = 'album_artist'
       ORDER BY c.ordinal LIMIT 1) AS artist_sort"
);

const AN_ARTIST: &str = concat!(
    "w.id, w.library_id, w.title, w.dominant_color, ",
    initial_of_a_title!(),
    " AS initial,
     (SELECT count(*) FROM music_credits c
       WHERE c.artist_id = w.id AND c.role = 'album_artist') AS albums,
     (SELECT count(*) FROM music_credits c
       WHERE c.artist_id = w.id AND c.role = 'artist') AS songs"
);

const A_SONG: &str = "w.id, w.title, w.ordinal, w.release_year, ms.disc_number,
     w.parent_id, al.title AS album_title, al.sort_title AS album_sort,
     (SELECT s.id FROM media_sources s WHERE s.work_id = w.id AND s.missing_since IS NULL
       ORDER BY s.added_at LIMIT 1) AS source_id,
     (SELECT max(s.duration_ms) FROM media_sources s WHERE s.work_id = w.id) AS duration_ms";

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
        let rows = sqlx::query(AssertSqlSafe(format!(
            "SELECT {A_SONG}
               FROM works w
               LEFT JOIN music_songs ms ON ms.work_id = w.id
               LEFT JOIN works al ON al.id = w.parent_id
              WHERE w.library_id = ? AND w.kind = 'song'
              ORDER BY {} LIMIT ? OFFSET ?",
            order.clause(descending)
        )))
        .bind(library_id.to_db_string())
        .bind(limit)
        .bind(offset)
        .fetch_all(self.reader())
        .await?;
        let items = self.song_rows(&rows).await?;
        Ok(MusicPage { items, total })
    }

    /// Every genre the albums of a library carry, by name.
    pub async fn music_genres(&self, library_id: LibraryId) -> Result<Vec<MusicGenre>> {
        let rows = sqlx::query(
            "SELECT g.name, count(*) AS albums
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
        rows.iter()
            .map(|row| {
                Ok(MusicGenre {
                    name: row.try_get("name")?,
                    albums: row.try_get("albums")?,
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
        // lead to the first of them.
        let rows = sqlx::query(AssertSqlSafe(format!(
            "WITH letters AS (
                 SELECT {} AS letter, count(*) AS count, min(w.sort_title) AS first
                   FROM works w
                  WHERE w.library_id = ?1 AND {narrowing}
                  GROUP BY letter
             )
             SELECT letter, count,
                    (SELECT count(*) FROM works w
                      WHERE w.library_id = ?1 AND {narrowing}
                        AND w.sort_title < letters.first) AS offset
               FROM letters
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
              WHERE EXISTS (SELECT 1 FROM music_credits c WHERE c.work_id = w.id
                             AND c.role = 'album_artist' AND c.artist_id = ?1)
              ORDER BY {order}"
        )))
        .bind(artist.to_db_string())
        .fetch_all(self.reader())
        .await?;
        let played_on = sqlx::query(AssertSqlSafe(format!(
            "SELECT {AN_ALBUM} FROM works w JOIN music_albums ma ON ma.work_id = w.id
              WHERE EXISTS (SELECT 1 FROM works song
                              JOIN music_credits c ON c.work_id = song.id AND c.role = 'artist'
                             WHERE song.parent_id = w.id AND c.artist_id = ?1)
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

    /// Albums read from rows, with their artists and their covers.
    async fn album_cards(&self, rows: &[sqlx::sqlite::SqliteRow]) -> Result<Vec<AlbumCard>> {
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
    async fn artist_cards(&self, rows: &[sqlx::sqlite::SqliteRow]) -> Result<Vec<ArtistCard>> {
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
    async fn song_rows(&self, rows: &[sqlx::sqlite::SqliteRow]) -> Result<Vec<SongRow>> {
        let ids: Vec<WorkId> = rows
            .iter()
            .map(|row| parse_id(&row.try_get::<String, _>("id")?))
            .collect::<Result<_>>()?;
        let mut artists = self.credited_on(&ids, "artist").await?;
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
                    artists: artists.remove(&id).unwrap_or_default(),
                    album,
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
    use std::path::PathBuf;

    use melyxar_core::id::LibraryRootId;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::music::{AlbumFiling, Named, SongFiling};
    use melyxar_core::time::now;

    use super::*;
    use crate::music::MusicFile;

    fn named(name: &str) -> Named {
        Named {
            name: name.to_string(),
            // Sorted the way the filing sorts them: without capitals and
            // without a leading article.
            sort_name: name.to_lowercase().trim_start_matches("the ").to_string(),
        }
    }

    struct Song<'a> {
        path: &'a str,
        title: &'a str,
        artists: &'a [&'a str],
        album: &'a str,
        album_artists: &'a [&'a str],
        track: u32,
        year: i32,
        genre: &'a str,
    }

    fn file(song: Song<'_>) -> MusicFile {
        MusicFile {
            source_id: MediaSourceId::new(),
            song: None,
            relative_path: PathBuf::from(song.path),
            size_bytes: 10,
            modified_at: now(),
            filing: SongFiling {
                title: named(song.title),
                artists: song.artists.iter().map(|a| named(a)).collect(),
                album: Some(AlbumFiling {
                    title: named(song.album),
                    artists: song.album_artists.iter().map(|a| named(a)).collect(),
                    is_compilation: song.album_artists == ["Various Artists"],
                }),
                track: Some(song.track),
                disc: Some(1),
                year: Some(song.year),
                genres: vec![song.genre.to_string()],
            },
            reading: Err("not read in this test".to_string()),
        }
    }

    /// Two albums of Amber Field, one of The Lanterns on which Amber Field
    /// sings once, and a compilation.
    async fn collection() -> (Database, LibraryId, LibraryRootId) {
        let database = Database::open_in_memory().await.expect("opens");
        let library = database
            .create_library(
                "Music",
                LibraryKind::Music,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one"))],
            )
            .await
            .expect("library");
        let root = library.roots[0].id;
        let songs = [
            Song {
                path: "A/Lights/02.flac",
                title: "Tides",
                artists: &["Amber Field"],
                album: "Northern Lights",
                album_artists: &["Amber Field"],
                track: 2,
                year: 2019,
                genre: "Folk",
            },
            Song {
                path: "A/Lights/01.flac",
                title: "Quiet Harbour",
                artists: &["Amber Field"],
                album: "Northern Lights",
                album_artists: &["Amber Field"],
                track: 1,
                year: 2019,
                genre: "Folk",
            },
            Song {
                path: "A/Early/01.flac",
                title: "Beginnings",
                artists: &["Amber Field"],
                album: "Early Days",
                album_artists: &["Amber Field"],
                track: 1,
                year: 2011,
                genre: "Pop",
            },
            Song {
                path: "L/Road/01.flac",
                title: "The Long Road",
                artists: &["The Lanterns", "Amber Field"],
                album: "Road",
                album_artists: &["The Lanterns"],
                track: 1,
                year: 2015,
                genre: "Rock",
            },
            Song {
                path: "V/Hits/01.flac",
                title: "Zebra",
                artists: &["Quartz"],
                album: "Summer Hits",
                album_artists: &["Various Artists"],
                track: 1,
                year: 2020,
                genre: "Pop",
            },
        ];
        database
            .file_music(library.id, root, &songs.map(file))
            .await
            .expect("filed");
        (database, library.id, root)
    }

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
    async fn genres_and_letters_say_what_is_there_and_where_it_begins() {
        let (database, library, _) = collection().await;
        let genres = database.music_genres(library).await.expect("read");
        assert_eq!(
            genres
                .iter()
                .map(|g| (g.name.as_str(), g.albums))
                .collect::<Vec<_>>(),
            vec![("Folk", 1), ("Pop", 2), ("Rock", 1)]
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
}
