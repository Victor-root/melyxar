//! Writing a library of music down: its songs, its albums, its artists.
//!
//! A folder at a time, in one transaction, because a folder is what the
//! filing decides for at once: every song of it lands on its album together,
//! or none of them does. On a first scan of a large collection that is also
//! what keeps the writing short, one commit per album rather than a handful
//! per song.
//!
//! Songs, albums and artists are works marked as their own, like what people
//! film themselves: named by what the files say, and waiting on no catalogue
//! of films. See `docs/architecture/06-musique.md`.

use std::collections::{HashMap, HashSet};

use melyxar_core::id::{LibraryId, LibraryRootId, MediaSourceId, WorkId};
use melyxar_core::media::Track;
use melyxar_core::music::{AlbumFiling, Named, SongFiling};
use melyxar_core::time::{Timestamp, now};
use melyxar_core::work::{IdentificationState, WorkKind};
use sqlx::{Sqlite, Transaction};

use crate::analysis::write_analysis;
use crate::catalogue::{Placed, SourceAnalysis, insert_work, recount_children};
use crate::convert::{parse_id, timestamp_to_text};
use crate::libraries::{Swept, sweep_what_nothing_points_at};
use crate::metadata::{GENRES, replace_links};
use crate::{Database, Result};

/// One music file of a folder, filed, with what reading it gave.
#[derive(Debug, Clone)]
pub struct MusicFile {
    /// The file's own identifier: the one it was written down under, or a
    /// new one for a file met for the first time.
    pub source_id: MediaSourceId,
    /// The song it already stands for, when it is written down.
    pub song: Option<WorkId>,
    pub relative_path: std::path::PathBuf,
    pub size_bytes: i64,
    pub modified_at: Timestamp,
    pub filing: SongFiling,
    /// What the file is made of, or why it could not be read.
    pub reading: std::result::Result<(SourceAnalysis, Track), String>,
}

/// What filing a folder changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MusicFiled {
    /// Songs written down for the first time.
    pub added: usize,
    /// Songs already written down that now go on another album.
    pub moved: usize,
}

/// The sort names of an album's artists, joined into what tells it apart.
///
/// Joined by a character no name holds, so two lists of names never read as
/// the same one.
fn artists_key(artists: &[Named]) -> String {
    artists
        .iter()
        .map(|artist| artist.sort_name.as_str())
        .collect::<Vec<_>>()
        .join("\u{1f}")
}

impl Database {
    /// Writes down the songs of one folder where their filing puts them.
    ///
    /// A song already written down keeps its identity, and with it whatever
    /// anybody did with it: it is moved, renamed and renumbered in place. A
    /// file met for the first time is written down with its song.
    ///
    /// What each album and artist is found by is its name as it sorts, in
    /// this library: the same album met in the next folder is the same album.
    pub async fn file_music(
        &self,
        library_id: LibraryId,
        root_id: LibraryRootId,
        files: &[MusicFile],
    ) -> Result<MusicFiled> {
        let mut transaction = self.begin().await?;
        let mut filed = MusicFiled::default();
        let mut artists: HashMap<String, WorkId> = HashMap::new();
        let mut albums: HashMap<(String, String), WorkId> = HashMap::new();
        // Every album that gained or lost a song, counted again at the end.
        let mut touched: HashSet<WorkId> = HashSet::new();

        for file in files {
            let album = match &file.filing.album {
                Some(filing) => Some(
                    album_id(
                        &mut transaction,
                        library_id,
                        filing,
                        &mut albums,
                        &mut artists,
                    )
                    .await?,
                ),
                None => None,
            };
            touched.extend(album);

            let song = match file.song {
                Some(song) => {
                    let was = parent_of(&mut transaction, song).await?;
                    if was != album {
                        filed.moved += 1;
                        touched.extend(was);
                    }
                    place_song(&mut transaction, song, album, &file.filing).await?;
                    song
                }
                None => {
                    let song =
                        write_song(&mut transaction, library_id, album, &file.filing).await?;
                    insert_source(&mut transaction, root_id, song, file).await?;
                    filed.added += 1;
                    song
                }
            };

            sqlx::query(
                "INSERT INTO music_songs (work_id, disc_number) VALUES (?, ?)
                 ON CONFLICT (work_id) DO UPDATE SET disc_number = excluded.disc_number",
            )
            .bind(song.to_db_string())
            .bind(file.filing.disc)
            .execute(&mut *transaction)
            .await?;

            let mut performers = Vec::with_capacity(file.filing.artists.len());
            for artist in &file.filing.artists {
                performers
                    .push(artist_id(&mut transaction, library_id, artist, &mut artists).await?);
            }
            credit(&mut transaction, song, "artist", &performers).await?;
            replace_links(&mut transaction, song, &GENRES, &file.filing.genres).await?;

            match &file.reading {
                Ok((analysis, track)) => {
                    write_analysis(
                        &mut transaction,
                        file.source_id,
                        analysis,
                        std::slice::from_ref(track),
                        &[],
                    )
                    .await?;
                }
                Err(reason) => {
                    crate::analysis::write_analysis_failure(
                        &mut *transaction,
                        file.source_id,
                        reason,
                    )
                    .await?;
                }
            }
        }

        for album in touched {
            settle_album(&mut transaction, album).await?;
        }
        transaction.commit().await?;
        Ok(filed)
    }

    /// Removes the albums no song is on any more, and the artists nobody is
    /// credited as any more, and whatever they alone held.
    ///
    /// What a filing leaves behind when a song moves to another album, or a
    /// file's tags name somebody else. A song whose file is only missing is
    /// still a song, so the album a disk that went away holds stays until the
    /// disk is back.
    pub async fn prune_music(&self, library_id: LibraryId) -> Result<Swept> {
        let mut transaction = self.begin().await?;
        sqlx::query(
            "DELETE FROM works
              WHERE library_id = ? AND kind = 'album'
                AND NOT EXISTS (SELECT 1 FROM works song WHERE song.parent_id = works.id)",
        )
        .bind(library_id.to_db_string())
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            "DELETE FROM works
              WHERE library_id = ? AND kind = 'artist'
                AND NOT EXISTS (SELECT 1 FROM music_credits WHERE artist_id = works.id)",
        )
        .bind(library_id.to_db_string())
        .execute(&mut *transaction)
        .await?;
        let swept = sweep_what_nothing_points_at(&mut transaction).await?;
        transaction.commit().await?;
        Ok(swept)
    }
}

/// The album a filing names, found or written down, with whose album it is.
async fn album_id(
    transaction: &mut Transaction<'_, Sqlite>,
    library_id: LibraryId,
    filing: &AlbumFiling,
    albums: &mut HashMap<(String, String), WorkId>,
    artists: &mut HashMap<String, WorkId>,
) -> Result<WorkId> {
    let key = (filing.title.sort_name.clone(), artists_key(&filing.artists));
    if let Some(album) = albums.get(&key) {
        return Ok(*album);
    }

    let found: Option<String> = sqlx::query_scalar(
        "SELECT w.id FROM works w
           JOIN music_albums a ON a.work_id = w.id
          WHERE w.library_id = ? AND w.kind = 'album' AND w.sort_title = ? AND a.artists_key = ?
          LIMIT 1",
    )
    .bind(library_id.to_db_string())
    .bind(&key.0)
    .bind(&key.1)
    .fetch_optional(&mut **transaction)
    .await?;

    let album = match found {
        Some(id) => parse_id(&id)?,
        None => {
            let album = insert_own_work(
                transaction,
                Placed::in_folder(library_id, None),
                WorkKind::Album,
                &filing.title,
            )
            .await?;
            sqlx::query(
                "INSERT INTO music_albums (work_id, artists_key, is_compilation) VALUES (?, ?, ?)",
            )
            .bind(album.to_db_string())
            .bind(&key.1)
            .bind(i64::from(filing.is_compilation))
            .execute(&mut **transaction)
            .await?;
            let mut owners = Vec::with_capacity(filing.artists.len());
            for artist in &filing.artists {
                owners.push(artist_id(transaction, library_id, artist, artists).await?);
            }
            credit(transaction, album, "album_artist", &owners).await?;
            album
        }
    };
    albums.insert(key, album);
    Ok(album)
}

/// The artist of that name in this library, found or written down.
async fn artist_id(
    transaction: &mut Transaction<'_, Sqlite>,
    library_id: LibraryId,
    artist: &Named,
    artists: &mut HashMap<String, WorkId>,
) -> Result<WorkId> {
    if let Some(found) = artists.get(&artist.sort_name) {
        return Ok(*found);
    }
    let found: Option<String> = sqlx::query_scalar(
        "SELECT id FROM works
          WHERE library_id = ? AND kind = 'artist' AND sort_title = ?
          LIMIT 1",
    )
    .bind(library_id.to_db_string())
    .bind(&artist.sort_name)
    .fetch_optional(&mut **transaction)
    .await?;
    let id = match found {
        Some(id) => parse_id(&id)?,
        None => {
            insert_own_work(
                transaction,
                Placed::in_folder(library_id, None),
                WorkKind::Artist,
                artist,
            )
            .await?
        }
    };
    artists.insert(artist.sort_name.clone(), id);
    Ok(id)
}

/// Writes a work named by its own files, waiting on no catalogue.
async fn insert_own_work(
    transaction: &mut Transaction<'_, Sqlite>,
    placed: Placed,
    kind: WorkKind,
    named: &Named,
) -> Result<WorkId> {
    let work = insert_work(
        &mut **transaction,
        placed,
        kind,
        &named.name,
        &named.sort_name,
        None,
    )
    .await?;
    sqlx::query("UPDATE works SET identification = ? WHERE id = ?")
        .bind(IdentificationState::Own.as_str())
        .bind(work.id.to_db_string())
        .execute(&mut **transaction)
        .await?;
    Ok(work.id)
}

/// Writes a song met for the first time, on its album at its track.
async fn write_song(
    transaction: &mut Transaction<'_, Sqlite>,
    library_id: LibraryId,
    album: Option<WorkId>,
    filing: &SongFiling,
) -> Result<WorkId> {
    let track = filing.track.and_then(|track| i32::try_from(track).ok());
    let placed = match (album, track) {
        (Some(album), Some(track)) => Placed::under(library_id, album, track),
        (album, _) => Placed::in_folder(library_id, album),
    };
    let song = insert_own_work(transaction, placed, WorkKind::Song, &filing.title).await?;
    sqlx::query("UPDATE works SET release_year = ? WHERE id = ?")
        .bind(filing.year)
        .bind(song.to_db_string())
        .execute(&mut **transaction)
        .await?;
    Ok(song)
}

/// Puts a song already written down where its filing now says it goes.
async fn place_song(
    transaction: &mut Transaction<'_, Sqlite>,
    song: WorkId,
    album: Option<WorkId>,
    filing: &SongFiling,
) -> Result<()> {
    sqlx::query(
        "UPDATE works
            SET parent_id = ?, ordinal = ?, title = ?, sort_title = ?, release_year = ?,
                updated_at = ?
          WHERE id = ?",
    )
    .bind(album.map(|album| album.to_db_string()))
    .bind(filing.track.and_then(|track| i32::try_from(track).ok()))
    .bind(&filing.title.name)
    .bind(&filing.title.sort_name)
    .bind(filing.year)
    .bind(timestamp_to_text(now()))
    .bind(song.to_db_string())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn parent_of(
    transaction: &mut Transaction<'_, Sqlite>,
    work: WorkId,
) -> Result<Option<WorkId>> {
    let parent: Option<String> = sqlx::query_scalar("SELECT parent_id FROM works WHERE id = ?")
        .bind(work.to_db_string())
        .fetch_one(&mut **transaction)
        .await?;
    parent.map(|parent| parse_id(&parent)).transpose()
}

async fn insert_source(
    transaction: &mut Transaction<'_, Sqlite>,
    root_id: LibraryRootId,
    song: WorkId,
    file: &MusicFile,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO media_sources
            (id, work_id, root_id, relative_path, size_bytes, modified_at, added_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(file.source_id.to_db_string())
    .bind(song.to_db_string())
    .bind(root_id.to_db_string())
    .bind(file.relative_path.to_string_lossy().as_ref())
    .bind(file.size_bytes)
    .bind(timestamp_to_text(file.modified_at))
    .bind(timestamp_to_text(now()))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Replaces who is credited on a work in one role, in order.
async fn credit(
    transaction: &mut Transaction<'_, Sqlite>,
    work: WorkId,
    role: &str,
    artists: &[WorkId],
) -> Result<()> {
    sqlx::query("DELETE FROM music_credits WHERE work_id = ? AND role = ?")
        .bind(work.to_db_string())
        .bind(role)
        .execute(&mut **transaction)
        .await?;
    for (ordinal, artist) in artists.iter().enumerate() {
        sqlx::query(
            "INSERT OR IGNORE INTO music_credits (work_id, artist_id, role, ordinal)
             VALUES (?, ?, ?, ?)",
        )
        .bind(work.to_db_string())
        .bind(artist.to_db_string())
        .bind(role)
        .bind(ordinal as i64)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

/// Brings what an album shows of its songs up to date: how many there are,
/// the year of the earliest, and every genre any of them carries.
///
/// Worked out from the songs rather than from one folder's filing, since an
/// album spread over two disc folders is filed one folder at a time.
async fn settle_album(transaction: &mut Transaction<'_, Sqlite>, album: WorkId) -> Result<()> {
    recount_children(&mut **transaction, album).await?;
    sqlx::query(
        "UPDATE works
            SET release_year = (SELECT min(song.release_year) FROM works song
                                 WHERE song.parent_id = works.id)
          WHERE id = ?",
    )
    .bind(album.to_db_string())
    .execute(&mut **transaction)
    .await?;
    sqlx::query("DELETE FROM work_genres WHERE work_id = ?")
        .bind(album.to_db_string())
        .execute(&mut **transaction)
        .await?;
    sqlx::query(
        "INSERT OR IGNORE INTO work_genres (work_id, genre_id)
         SELECT ?1, g.genre_id FROM work_genres g
           JOIN works song ON song.id = g.work_id
          WHERE song.parent_id = ?1",
    )
    .bind(album.to_db_string())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use melyxar_core::id::TrackId;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::media::{AudioDetails, Loudness, TrackKind};
    use melyxar_core::time::Millis;
    use melyxar_core::user::Permissions;

    use super::*;

    async fn library() -> (Database, LibraryId, LibraryRootId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Music",
                LibraryKind::Music,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Music"))],
            )
            .await
            .expect("library created");
        let root = library.roots[0].id;
        (database, library.id, root)
    }

    fn named(name: &str) -> Named {
        Named {
            name: name.to_string(),
            sort_name: name.to_lowercase(),
        }
    }

    fn album(title: &str, artists: &[&str]) -> AlbumFiling {
        AlbumFiling {
            title: named(title),
            artists: artists.iter().map(|artist| named(artist)).collect(),
            is_compilation: artists == ["Various Artists"],
        }
    }

    fn song(title: &str, artists: &[&str], on: Option<AlbumFiling>, track: u32) -> SongFiling {
        SongFiling {
            title: named(title),
            artists: artists.iter().map(|artist| named(artist)).collect(),
            album: on,
            track: Some(track),
            disc: Some(1),
            year: Some(2019),
            genres: vec!["Folk".to_string()],
        }
    }

    fn read_well(source_id: MediaSourceId) -> std::result::Result<(SourceAnalysis, Track), String> {
        Ok((
            SourceAnalysis {
                container: Some("flac".to_string()),
                duration: Some(Millis::new(240_000)),
                overall_bitrate: Some(900_000),
            },
            Track {
                id: TrackId::new(),
                source_id,
                stream_index: 0,
                language: None,
                title: None,
                is_default: true,
                is_forced: false,
                kind: TrackKind::Audio(AudioDetails {
                    codec: "flac".to_string(),
                    profile: None,
                    channels: 2,
                    channel_layout: None,
                    sample_rate: Some(44_100),
                    bit_depth: Some(16),
                    bitrate: Some(900_000),
                    loudness: Loudness::default(),
                }),
            },
        ))
    }

    fn new_file(path: &str, filing: SongFiling) -> MusicFile {
        let source_id = MediaSourceId::new();
        MusicFile {
            source_id,
            song: None,
            relative_path: PathBuf::from(path),
            size_bytes: 1_000,
            modified_at: now(),
            filing,
            reading: read_well(source_id),
        }
    }

    async fn credited(database: &Database, work: WorkId, role: &str) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT a.title FROM music_credits c JOIN works a ON a.id = c.artist_id
              WHERE c.work_id = ? AND c.role = ? ORDER BY c.ordinal",
        )
        .bind(work.to_db_string())
        .bind(role)
        .fetch_all(database.reader())
        .await
        .expect("read")
    }

    async fn works_of_kind(database: &Database, library: LibraryId, kind: &str) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT title FROM works WHERE library_id = ? AND kind = ? ORDER BY sort_title",
        )
        .bind(library.to_db_string())
        .bind(kind)
        .fetch_all(database.reader())
        .await
        .expect("read")
    }

    #[tokio::test]
    async fn a_folder_is_written_down_as_its_album_its_artist_and_its_songs() {
        let (database, library, root) = library().await;
        let lights = album("Northern Lights", &["Amber Field"]);
        let files = [
            new_file(
                "Amber Field/Northern Lights/01.flac",
                song("Quiet Harbour", &["Amber Field"], Some(lights.clone()), 1),
            ),
            new_file(
                "Amber Field/Northern Lights/02.flac",
                song("Tides", &["Amber Field"], Some(lights), 2),
            ),
        ];
        let filed = database
            .file_music(library, root, &files)
            .await
            .expect("filed");
        assert_eq!(filed, MusicFiled { added: 2, moved: 0 });

        assert_eq!(
            works_of_kind(&database, library, "artist").await,
            vec!["Amber Field"]
        );
        assert_eq!(
            works_of_kind(&database, library, "album").await,
            vec!["Northern Lights"]
        );
        assert_eq!(
            works_of_kind(&database, library, "song").await,
            vec!["Quiet Harbour", "Tides"]
        );

        let source = database
            .sources_of_root(root)
            .await
            .expect("read")
            .into_iter()
            .find(|source| source.relative_path == Path::new("Amber Field/Northern Lights/01.flac"))
            .expect("the file is written down");
        let song = database
            .work(source.work_id)
            .await
            .expect("read")
            .expect("a song");
        assert_eq!(song.kind, WorkKind::Song);
        assert_eq!(song.ordinal, Some(1));
        assert_eq!(song.release_year, Some(2019));
        assert_eq!(
            song.identification,
            IdentificationState::Own,
            "waiting on no catalogue"
        );
        assert_eq!(
            credited(&database, song.id, "artist").await,
            vec!["Amber Field"]
        );
        assert_eq!(
            database.work_genres(song.id).await.expect("read"),
            vec!["Folk"]
        );

        let album = database
            .work(song.parent_id.expect("on its album"))
            .await
            .expect("read")
            .expect("an album");
        assert_eq!(album.kind, WorkKind::Album);
        assert_eq!(album.release_year, Some(2019));
        assert_eq!(
            credited(&database, album.id, "album_artist").await,
            vec!["Amber Field"]
        );
        assert_eq!(
            database.work_genres(album.id).await.expect("read"),
            vec!["Folk"]
        );
        let count: i64 = sqlx::query_scalar("SELECT child_count FROM works WHERE id = ?")
            .bind(album.id.to_db_string())
            .fetch_one(database.reader())
            .await
            .expect("read");
        assert_eq!(count, 2);

        let tracks = database.tracks_of_source(source.id).await.expect("read");
        assert_eq!(tracks.len(), 1, "the one stream of sound the reading found");
        let (analysis, analysed_at) = database
            .source_details(source.id)
            .await
            .expect("read")
            .expect("the file");
        assert_eq!(analysis.duration, Some(Millis::new(240_000)));
        assert!(
            analysed_at.is_some(),
            "read, so not read again by the analyser of the films"
        );
    }

    #[tokio::test]
    async fn the_same_album_met_in_another_folder_is_the_same_album() {
        let (database, library, root) = library().await;
        let lights = album("Northern Lights", &["Amber Field"]);
        let mut second = song("Tides", &["Amber Field"], Some(lights.clone()), 1);
        second.disc = Some(2);
        database
            .file_music(
                library,
                root,
                &[new_file(
                    "A/Disc 1/01.flac",
                    song("Quiet Harbour", &["Amber Field"], Some(lights), 1),
                )],
            )
            .await
            .expect("filed");
        database
            .file_music(library, root, &[new_file("A/Disc 2/01.flac", second)])
            .await
            .expect("filed");

        assert_eq!(
            works_of_kind(&database, library, "album").await,
            vec!["Northern Lights"]
        );
        assert_eq!(
            works_of_kind(&database, library, "artist").await,
            vec!["Amber Field"]
        );
        let discs: Vec<(String, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT w.title, w.ordinal, s.disc_number FROM works w
               JOIN music_songs s ON s.work_id = w.id ORDER BY s.disc_number",
        )
        .fetch_all(database.reader())
        .await
        .expect("read");
        assert_eq!(
            discs,
            vec![
                ("Quiet Harbour".to_string(), Some(1), Some(1)),
                ("Tides".to_string(), Some(1), Some(2)),
            ],
            "two first tracks, one on each disc"
        );
    }

    #[tokio::test]
    async fn two_albums_of_one_name_by_two_artists_stay_two() {
        let (database, library, root) = library().await;
        let files = [
            new_file(
                "A/1.flac",
                song(
                    "One",
                    &["Amber Field"],
                    Some(album("Greatest Hits", &["Amber Field"])),
                    1,
                ),
            ),
            new_file(
                "B/1.flac",
                song(
                    "Two",
                    &["The Lanterns"],
                    Some(album("Greatest Hits", &["The Lanterns"])),
                    1,
                ),
            ),
        ];
        database
            .file_music(library, root, &files)
            .await
            .expect("filed");
        assert_eq!(works_of_kind(&database, library, "album").await.len(), 2);
    }

    #[tokio::test]
    async fn a_compilation_is_various_artists_album_and_each_song_keeps_its_own() {
        let (database, library, root) = library().await;
        let hits = album("Summer Hits", &["Various Artists"]);
        let files = [
            new_file(
                "Hits/1.mp3",
                song("One", &["Amber Field"], Some(hits.clone()), 1),
            ),
            new_file(
                "Hits/2.mp3",
                song("Two", &["The Lanterns", "Amber Field"], Some(hits), 2),
            ),
        ];
        database
            .file_music(library, root, &files)
            .await
            .expect("filed");

        assert_eq!(
            works_of_kind(&database, library, "artist").await,
            vec!["Amber Field", "The Lanterns", "Various Artists"]
        );
        let two = database
            .sources_of_root(root)
            .await
            .expect("read")
            .into_iter()
            .find(|source| source.relative_path == Path::new("Hits/2.mp3"))
            .expect("written down")
            .work_id;
        assert_eq!(
            credited(&database, two, "artist").await,
            vec!["The Lanterns", "Amber Field"]
        );
        let is_compilation: i64 = sqlx::query_scalar("SELECT is_compilation FROM music_albums")
            .fetch_one(database.reader())
            .await
            .expect("read");
        assert_eq!(is_compilation, 1);
    }

    #[tokio::test]
    async fn a_song_filed_again_moves_with_what_people_did_with_it_and_its_old_album_goes() {
        let (database, library, root) = library().await;
        let first = new_file(
            "A/1.flac",
            song(
                "Quiet Harbour",
                &["Amber Field"],
                Some(album("Demo", &["Amber Field"])),
                1,
            ),
        );
        let source_id = first.source_id;
        database
            .file_music(library, root, &[first])
            .await
            .expect("filed");
        let song_id = database.sources_of_root(root).await.expect("read")[0].work_id;

        let viewer = database
            .create_user("Listener", None, &Permissions::viewer())
            .await
            .expect("account")
            .id;
        database
            .set_favourite(viewer, song_id, true)
            .await
            .expect("liked");

        // The file was tagged again: another album, by another artist.
        let again = MusicFile {
            song: Some(song_id),
            ..new_file(
                "A/1.flac",
                song(
                    "Quiet Harbour",
                    &["The Lanterns"],
                    Some(album("Northern Lights", &["The Lanterns"])),
                    4,
                ),
            )
        };
        let again = MusicFile {
            source_id,
            reading: read_well(source_id),
            ..again
        };
        let filed = database
            .file_music(library, root, &[again])
            .await
            .expect("filed");
        assert_eq!(filed, MusicFiled { added: 0, moved: 1 });

        let song = database
            .work(song_id)
            .await
            .expect("read")
            .expect("the same song");
        assert_eq!(song.ordinal, Some(4));
        assert_eq!(
            credited(&database, song_id, "artist").await,
            vec!["The Lanterns"]
        );
        let liked: i64 = sqlx::query_scalar("SELECT count(*) FROM favorites WHERE work_id = ?")
            .bind(song_id.to_db_string())
            .fetch_one(database.reader())
            .await
            .expect("read");
        assert_eq!(liked, 1, "still liked: it is the same song");

        assert_eq!(
            works_of_kind(&database, library, "album").await,
            vec!["Demo", "Northern Lights"]
        );
        database.prune_music(library).await.expect("pruned");
        assert_eq!(
            works_of_kind(&database, library, "album").await,
            vec!["Northern Lights"]
        );
        assert_eq!(
            works_of_kind(&database, library, "artist").await,
            vec!["The Lanterns"]
        );
        assert_eq!(
            database.sources_of_root(root).await.expect("read").len(),
            1,
            "one file, still one"
        );
    }

    #[tokio::test]
    async fn an_album_whose_disk_went_away_is_kept_until_the_disk_is_back() {
        let (database, library, root) = library().await;
        let file = new_file(
            "A/1.flac",
            song(
                "Quiet Harbour",
                &["Amber Field"],
                Some(album("Demo", &["Amber Field"])),
                1,
            ),
        );
        let source = file.source_id;
        database
            .file_music(library, root, &[file])
            .await
            .expect("filed");
        database.mark_source_missing(source).await.expect("missing");

        database.prune_music(library).await.expect("pruned");
        assert_eq!(
            works_of_kind(&database, library, "album").await,
            vec!["Demo"]
        );
        assert_eq!(
            works_of_kind(&database, library, "artist").await,
            vec!["Amber Field"]
        );
    }

    #[tokio::test]
    async fn a_file_that_could_not_be_read_is_still_written_down_with_why() {
        let (database, library, root) = library().await;
        let file = MusicFile {
            reading: Err("this kind of file is not one the tag reader knows".to_string()),
            ..new_file("Loose.wma", song("Loose", &[], None, 1))
        };
        let source = file.source_id;
        database
            .file_music(library, root, &[file])
            .await
            .expect("filed");

        assert_eq!(
            works_of_kind(&database, library, "song").await,
            vec!["Loose"]
        );
        assert!(works_of_kind(&database, library, "album").await.is_empty());
        let failure: Option<String> =
            sqlx::query_scalar("SELECT analysis_failure FROM media_sources WHERE id = ?")
                .bind(source.to_db_string())
                .fetch_one(database.reader())
                .await
                .expect("read");
        assert!(failure.is_some_and(|reason| reason.contains("tag reader")));
    }
}
