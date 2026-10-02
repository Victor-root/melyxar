//! The routes that read a library of music: its albums, artists, songs and
//! genres, the page of one album or one artist, and what each account makes
//! of it, liked and listened to.
//!
//! Apart from the routes of films, as music is everywhere in this server.

use axum::body::{Body, Bytes};
use axum::extract::{Path, Query, State};
use axum::http::{HeaderName, Request, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_app::music::browse::{
    AlbumCard, AlbumOrder, AlbumsWanted, ArtistCard, Credited, MusicFound, MusicGenre,
    MusicInitial, MusicPage, Paging, SongOrder, SongRow,
};
use melyxar_core::music::Spectrum;
use serde::{Deserialize, Serialize};

use melyxar_app::music::marks::Listened;

use crate::account::Viewer;
use crate::catalogue::{ImageView, image_view};
use crate::error::{Result, ServerError};
use crate::identifiers::{parse_library, parse_work};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/music/{library}/albums", get(albums))
        .route("/api/v1/music/{library}/artists", get(artists))
        .route("/api/v1/music/{library}/songs", get(songs))
        .route("/api/v1/music/{library}/queue", get(queue))
        .route("/api/v1/music/{library}/genres", get(genres))
        .route("/api/v1/music/{library}/initials", get(initials))
        .route("/api/v1/music/search", get(search))
        .route("/api/v1/music/favourites", get(favourite_ids))
        .route("/api/v1/music/{library}/favourites", get(favourites))
        .route("/api/v1/music/{library}/listened", get(listened))
        .route("/api/v1/music/songs/{id}/listened", post(record_listen))
        .route("/api/v1/music/songs/{id}/lyrics", get(lyrics))
        .route("/api/v1/music/albums/{id}", get(album))
        .route("/api/v1/music/artists/{id}", get(artist))
        .route("/api/v1/music/artists/{id}/songs", get(artist_songs))
        .route("/api/v1/music/songs/{id}/sound", get(sound))
        .route("/api/v1/music/songs/{id}/spectrum", get(spectrum))
}

/// What a page holds when nothing is said: a few screens of a grid.
const DEFAULT_PAGE: i64 = 100;

#[derive(Debug, Serialize)]
struct CreditedView {
    id: String,
    name: String,
}

fn credited(credit: &Credited) -> CreditedView {
    CreditedView {
        id: credit.id.to_string(),
        name: credit.name.clone(),
    }
}

#[derive(Debug, Serialize)]
struct AlbumView {
    id: String,
    library: String,
    title: String,
    artists: Vec<CreditedView>,
    compilation: bool,
    year: Option<i32>,
    songs: i64,
    color: Option<String>,
    initial: String,
    cover: Vec<ImageView>,
}

fn album_view(album: &AlbumCard) -> AlbumView {
    AlbumView {
        id: album.id.to_string(),
        library: album.library_id.to_string(),
        title: album.title.clone(),
        artists: album.artists.iter().map(credited).collect(),
        compilation: album.is_compilation,
        year: album.year,
        songs: album.songs,
        color: album.color.clone(),
        initial: album.initial.clone(),
        cover: album.cover.iter().map(image_view).collect(),
    }
}

#[derive(Debug, Serialize)]
struct ArtistView {
    id: String,
    library: String,
    name: String,
    initial: String,
    albums: i64,
    songs: i64,
    color: Option<String>,
    picture: Vec<ImageView>,
}

fn artist_view(artist: &ArtistCard) -> ArtistView {
    ArtistView {
        id: artist.id.to_string(),
        library: artist.library_id.to_string(),
        name: artist.name.clone(),
        initial: artist.initial.clone(),
        albums: artist.albums,
        songs: artist.songs,
        color: artist.color.clone(),
        picture: artist.picture.iter().map(image_view).collect(),
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct SongView {
    id: String,
    title: String,
    artists: Vec<CreditedView>,
    album: Option<CreditedView>,
    track: Option<i32>,
    disc: Option<i32>,
    year: Option<i32>,
    /// Whole seconds, which is what a list of songs shows.
    seconds: Option<i64>,
    /// The file it plays from.
    source: Option<String>,
    /// The cover of its album.
    cover: Vec<ImageView>,
    /// How loud it is, in LUFS, and how high its sound reaches, in dBFS,
    /// once measured: what the player levels it by.
    lufs: Option<f64>,
    peak_dbfs: Option<f64>,
    /// How loud its whole album is, for levelling album by album.
    album_lufs: Option<f64>,
}

pub(crate) fn song_view(song: &SongRow) -> SongView {
    SongView {
        id: song.id.to_string(),
        title: song.title.clone(),
        artists: song.artists.iter().map(credited).collect(),
        album: song.album.as_ref().map(credited),
        track: song.track,
        disc: song.disc,
        year: song.year,
        seconds: song.duration.map(|length| (length.get() + 500) / 1_000),
        source: song.source_id.map(|source| source.to_string()),
        cover: song.cover.iter().map(image_view).collect(),
        lufs: song.lufs,
        peak_dbfs: song.peak_dbfs,
        album_lufs: song.album_lufs,
    }
}

#[derive(Debug, Serialize)]
struct PageView<T> {
    items: Vec<T>,
    total: i64,
}

fn page_view<T, V>(page: &MusicPage<T>, view: impl Fn(&T) -> V) -> PageView<V> {
    PageView {
        items: page.items.iter().map(view).collect(),
        total: page.total,
    }
}

#[derive(Debug, Deserialize)]
struct AlbumsQuery {
    order: Option<AlbumOrder>,
    #[serde(default)]
    descending: bool,
    offset: Option<i64>,
    limit: Option<i64>,
    genre: Option<String>,
    /// Only the albums of this artist.
    artist: Option<String>,
}

fn paging(offset: Option<i64>, limit: Option<i64>) -> Paging {
    Paging {
        offset: offset.unwrap_or(0),
        limit: limit.unwrap_or(DEFAULT_PAGE),
    }
}

async fn albums(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(library): Path<String>,
    Query(query): Query<AlbumsQuery>,
) -> Result<Json<PageView<AlbumView>>> {
    let wanted = AlbumsWanted {
        genre: query.genre.filter(|genre| !genre.trim().is_empty()),
        by: query.artist.as_deref().map(parse_work).transpose()?,
    };
    let page = melyxar_app::music::browse::albums(
        &state,
        &who,
        parse_library(&library)?,
        &wanted,
        query.order.unwrap_or(AlbumOrder::Title),
        query.descending,
        paging(query.offset, query.limit),
    )
    .await?;
    Ok(Json(page_view(&page, album_view)))
}

#[derive(Debug, Deserialize)]
struct ArtistsQuery {
    /// Only the artists with an album of their own.
    #[serde(default)]
    album_artists: bool,
    offset: Option<i64>,
    limit: Option<i64>,
}

async fn artists(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(library): Path<String>,
    Query(query): Query<ArtistsQuery>,
) -> Result<Json<PageView<ArtistView>>> {
    let page = melyxar_app::music::browse::artists(
        &state,
        &who,
        parse_library(&library)?,
        query.album_artists,
        paging(query.offset, query.limit),
    )
    .await?;
    Ok(Json(page_view(&page, artist_view)))
}

#[derive(Debug, Deserialize)]
struct SongsQuery {
    order: Option<SongOrder>,
    #[serde(default)]
    descending: bool,
    offset: Option<i64>,
    limit: Option<i64>,
}

async fn songs(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(library): Path<String>,
    Query(query): Query<SongsQuery>,
) -> Result<Json<PageView<SongView>>> {
    let page = melyxar_app::music::browse::songs(
        &state,
        &who,
        parse_library(&library)?,
        query.order.unwrap_or(SongOrder::Title),
        query.descending,
        paging(query.offset, query.limit),
    )
    .await?;
    Ok(Json(page_view(&page, song_view)))
}

#[derive(Debug, Deserialize)]
struct QueueQuery {
    offset: Option<i64>,
}

/// The songs a whole library is played or shuffled from.
async fn queue(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(library): Path<String>,
    Query(query): Query<QueueQuery>,
) -> Result<Json<PageView<SongView>>> {
    let page = melyxar_app::music::browse::queue(
        &state,
        &who,
        parse_library(&library)?,
        query.offset.unwrap_or(0),
    )
    .await?;
    Ok(Json(page_view(&page, song_view)))
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    #[serde(default)]
    words: String,
    /// One library of music, or every one this account may read.
    library: Option<String>,
}

#[derive(Debug, Serialize)]
struct FoundView {
    albums: Vec<AlbumView>,
    artists: Vec<ArtistView>,
    songs: Vec<SongView>,
}

async fn search(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Query(query): Query<SearchQuery>,
) -> Result<Json<FoundView>> {
    let library = query.library.as_deref().map(parse_library).transpose()?;
    let found = melyxar_app::music::browse::search(&state, &who, library, &query.words).await?;
    Ok(Json(found_view(&found)))
}

fn found_view(found: &MusicFound) -> FoundView {
    FoundView {
        albums: found.albums.iter().map(album_view).collect(),
        artists: found.artists.iter().map(artist_view).collect(),
        songs: found.songs.iter().map(song_view).collect(),
    }
}

async fn favourite_ids(
    State(state): State<AppState>,
    Viewer(who): Viewer,
) -> Result<Json<Vec<String>>> {
    let ids = melyxar_app::music::marks::favourite_ids(&state, &who).await?;
    Ok(Json(ids.iter().map(|id| id.to_string()).collect()))
}

async fn favourites(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(library): Path<String>,
) -> Result<Json<FoundView>> {
    let found =
        melyxar_app::music::marks::favourites(&state, &who, parse_library(&library)?).await?;
    Ok(Json(found_view(&found)))
}

#[derive(Debug, Deserialize)]
struct ListenedQuery {
    order: Listened,
}

async fn listened(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(library): Path<String>,
    Query(query): Query<ListenedQuery>,
) -> Result<Json<Vec<SongView>>> {
    let songs =
        melyxar_app::music::marks::listened(&state, &who, parse_library(&library)?, query.order)
            .await?;
    Ok(Json(songs.iter().map(song_view).collect()))
}

async fn record_listen(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>> {
    melyxar_app::music::marks::record_listen(&state, &who, parse_work(&id)?).await?;
    Ok(Json(serde_json::json!({ "listened": true })))
}

#[derive(Debug, Serialize)]
struct GenreView {
    name: String,
    albums: i64,
}

async fn genres(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(library): Path<String>,
) -> Result<Json<Vec<GenreView>>> {
    let found = melyxar_app::music::browse::genres(&state, &who, parse_library(&library)?).await?;
    Ok(Json(
        found
            .iter()
            .map(|genre: &MusicGenre| GenreView {
                name: genre.name.clone(),
                albums: genre.albums,
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
struct InitialsQuery {
    /// albums, artists or album_artists.
    of: Option<String>,
}

#[derive(Debug, Serialize)]
struct InitialView {
    letter: String,
    count: i64,
    offset: i64,
}

async fn initials(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(library): Path<String>,
    Query(query): Query<InitialsQuery>,
) -> Result<Json<Vec<InitialView>>> {
    let (artists, album_artists_only) = match query.of.as_deref() {
        None | Some("albums") => (false, false),
        Some("artists") => (true, false),
        Some("album_artists") => (true, true),
        Some(_) => {
            return Err(ServerError::invalid_input(
                "no such list to read letters of",
            ));
        }
    };
    let found = melyxar_app::music::browse::initials(
        &state,
        &who,
        parse_library(&library)?,
        artists,
        album_artists_only,
    )
    .await?;
    Ok(Json(
        found
            .iter()
            .map(|initial: &MusicInitial| InitialView {
                letter: initial.letter.clone(),
                count: initial.count,
                offset: initial.offset,
            })
            .collect(),
    ))
}

#[derive(Debug, Serialize)]
struct AlbumPageView {
    #[serde(flatten)]
    album: AlbumView,
    /// Its songs in their order on it. Named apart from how many there are.
    tracks: Vec<SongView>,
}

async fn album(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<AlbumPageView>> {
    let (album, songs) = melyxar_app::music::browse::album(&state, &who, parse_work(&id)?)
        .await?
        .ok_or_else(|| ServerError::not_found("no such album"))?;
    Ok(Json(AlbumPageView {
        album: album_view(&album),
        tracks: songs.iter().map(song_view).collect(),
    }))
}

#[derive(Debug, Serialize)]
struct ArtistPageView {
    #[serde(flatten)]
    artist: ArtistView,
    /// The albums that are theirs. Named apart from how many there are.
    their_albums: Vec<AlbumView>,
    appears_on: Vec<AlbumView>,
}

async fn artist(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<ArtistPageView>> {
    let (artist, theirs, played_on) =
        melyxar_app::music::browse::artist(&state, &who, parse_work(&id)?)
            .await?
            .ok_or_else(|| ServerError::not_found("no such artist"))?;
    Ok(Json(ArtistPageView {
        artist: artist_view(&artist),
        their_albums: theirs.iter().map(album_view).collect(),
        appears_on: played_on.iter().map(album_view).collect(),
    }))
}

async fn artist_songs(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<Vec<SongView>>> {
    let songs = melyxar_app::music::browse::artist_songs(&state, &who, parse_work(&id)?).await?;
    Ok(Json(songs.iter().map(song_view).collect()))
}

/// One line sung at a known moment.
#[derive(Debug, Serialize)]
struct LineView {
    at_ms: i64,
    text: String,
}

#[derive(Debug, Serialize)]
struct LyricsView {
    /// song, beside or online.
    source: &'static str,
    plain: String,
    /// Empty when the words carry no moments.
    lines: Vec<LineView>,
    instrumental: bool,
}

/// The lyrics of a song, or nothing when none were found anywhere.
async fn lyrics(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<Option<LyricsView>>> {
    let found = melyxar_app::music::lyrics::lyrics_of(&state, &who, parse_work(&id)?).await?;
    Ok(Json(found.map(|found| {
        LyricsView {
            source: found.source.as_str(),
            plain: found.lyrics.plain,
            lines: found
                .lyrics
                .synced
                .into_iter()
                .map(|line| LineView {
                    at_ms: line.at.get(),
                    text: line.text,
                })
                .collect(),
            instrumental: found.instrumental,
        }
    })))
}

#[derive(Debug, Deserialize)]
struct SoundQuery {
    /// The forms the browser says it plays, separated by commas.
    #[serde(default)]
    plays: String,
    /// Where to start, in seconds, for a song that has to be converted. A
    /// song sent as it is starts wherever the browser asks it to.
    start: Option<f64>,
}

/// How much of a converted song is handed on at a time.
const CHUNK: usize = 64 * 1024;

/// The sound of one song: its file, or the file converted on the way.
async fn sound(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Query(query): Query<SoundQuery>,
    request: Request<Body>,
) -> Result<Response> {
    let plays: Vec<&str> = query
        .plays
        .split(',')
        .map(str::trim)
        .filter(|form| !form.is_empty())
        .collect();
    let start = melyxar_core::time::Millis::from_seconds_f64(query.start.unwrap_or(0.0).max(0.0));
    let sound = melyxar_app::music::listen::sound_of(&state, &who, parse_work(&id)?, &plays, start)
        .await?
        .ok_or_else(|| ServerError::not_found("no such song on a disk"))?;

    match sound {
        melyxar_app::music::listen::Sound::AsItIs(path) => {
            crate::serve_the_file(&path, request).await
        }
        melyxar_app::music::listen::Sound::Converted { song, into } => {
            use tokio::io::AsyncReadExt;
            // The tool is held by the stream, and stops when the stream is
            // dropped: a listener who moves on or goes away takes it with
            // them.
            let stream = futures_util::stream::unfold(Some(song), |held| async move {
                let mut song = held?;
                let mut chunk = vec![0u8; CHUNK];
                match song.output.read(&mut chunk).await {
                    Ok(0) => None,
                    Ok(read) => {
                        chunk.truncate(read);
                        Some((Ok::<Bytes, std::io::Error>(Bytes::from(chunk)), Some(song)))
                    }
                    Err(error) => Some((Err(error), None)),
                }
            });
            Ok((
                [
                    (header::CONTENT_TYPE, into.content_type()),
                    (header::CACHE_CONTROL, "no-store"),
                ],
                Body::from_stream(stream),
            )
                .into_response())
        }
    }
}

/// How the sound of one song is spread: its levels, a byte each, with how
/// many bands each reading has and how many readings a second in the headers.
/// Nothing for a song that has none, which is no fault: the interface draws
/// no wave for it.
async fn spectrum(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Response> {
    let spectrum =
        melyxar_app::music::listen::spectrum_of_song(&state, &who, parse_work(&id)?).await?;
    Ok(spectrum.map_or_else(|| StatusCode::NO_CONTENT.into_response(), spectrum_reply))
}

/// What does not change until the song's file does is kept an hour by the
/// browser, which asks again for each song it plays.
fn spectrum_reply(spectrum: Spectrum) -> Response {
    (
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_string()),
            (header::CACHE_CONTROL, "private, max-age=3600".to_string()),
            (
                HeaderName::from_static("x-spectrum-bands"),
                spectrum.bands.to_string(),
            ),
            (
                HeaderName::from_static("x-spectrum-frames-a-second"),
                spectrum.frames_a_second.to_string(),
            ),
        ],
        spectrum.levels,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use melyxar_core::id::{LibraryId, WorkId};

    use super::*;

    #[tokio::test]
    async fn a_spectrum_goes_out_as_its_levels_with_its_shape_in_the_headers() {
        let reply = spectrum_reply(Spectrum {
            bands: 3,
            frames_a_second: 4,
            levels: vec![9, 8, 7, 6, 5, 4],
        });
        assert_eq!(reply.headers()["x-spectrum-bands"], "3");
        assert_eq!(reply.headers()["x-spectrum-frames-a-second"], "4");
        let body = axum::body::to_bytes(reply.into_body(), usize::MAX)
            .await
            .expect("a body");
        assert_eq!(body.as_ref(), [9, 8, 7, 6, 5, 4]);
    }

    #[test]
    fn the_page_of_an_album_says_how_many_songs_and_which_under_two_names() {
        let album = AlbumCard {
            id: WorkId::new(),
            library_id: LibraryId::new(),
            title: "Northern Lights".to_string(),
            artists: Vec::new(),
            is_compilation: false,
            year: Some(2019),
            songs: 1,
            color: None,
            initial: "n".to_string(),
            added_at: melyxar_core::time::now(),
            cover: Vec::new(),
        };
        let song = SongRow {
            id: WorkId::new(),
            title: "Quiet Harbour".to_string(),
            artists: Vec::new(),
            album: None,
            track: Some(1),
            disc: None,
            year: None,
            duration: None,
            source_id: None,
            cover: Vec::new(),
            lufs: None,
            peak_dbfs: None,
            album_lufs: None,
        };
        // Written as it is sent, where two keys of one name would both go
        // out and a browser would keep only the last.
        let written = serde_json::to_string(&AlbumPageView {
            album: album_view(&album),
            tracks: vec![song_view(&song)],
        })
        .expect("written");
        assert_eq!(
            written.matches("\"songs\"").count(),
            1,
            "one key of that name"
        );
        let page: serde_json::Value = serde_json::from_str(&written).expect("read back");
        assert_eq!(page["songs"], 1, "how many");
        assert_eq!(page["tracks"][0]["title"], "Quiet Harbour", "which");
    }
}
