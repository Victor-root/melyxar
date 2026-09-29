//! The routes that read a library of music: its albums, artists, songs and
//! genres, and the page of one album or one artist.
//!
//! Apart from the routes of films, as music is everywhere in this server.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use melyxar_app::AppState;
use melyxar_app::music::browse::{
    AlbumCard, AlbumOrder, AlbumsWanted, ArtistCard, Credited, MusicGenre, MusicInitial, MusicPage,
    Paging, SongOrder, SongRow,
};
use serde::{Deserialize, Serialize};

use crate::account::Viewer;
use crate::catalogue::{ImageView, image_view};
use crate::error::{Result, ServerError};
use crate::identifiers::{parse_library, parse_work};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/v1/music/{library}/albums", get(albums))
        .route("/api/v1/music/{library}/artists", get(artists))
        .route("/api/v1/music/{library}/songs", get(songs))
        .route("/api/v1/music/{library}/genres", get(genres))
        .route("/api/v1/music/{library}/initials", get(initials))
        .route("/api/v1/music/albums/{id}", get(album))
        .route("/api/v1/music/artists/{id}", get(artist))
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
struct SongView {
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
}

fn song_view(song: &SongRow) -> SongView {
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
    songs: Vec<SongView>,
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
        songs: songs.iter().map(song_view).collect(),
    }))
}

#[derive(Debug, Serialize)]
struct ArtistPageView {
    #[serde(flatten)]
    artist: ArtistView,
    albums: Vec<AlbumView>,
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
        albums: theirs.iter().map(album_view).collect(),
        appears_on: played_on.iter().map(album_view).collect(),
    }))
}
