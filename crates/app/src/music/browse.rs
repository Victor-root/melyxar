//! What an account reads of a library of music.
//!
//! Every read is checked against the libraries the account was granted, the
//! same way the grids of films are: a library or an album somebody was not
//! given answers as one that is not there.

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::user::User;
pub use melyxar_database::music_browse::{
    AlbumCard, AlbumOrder, AlbumsWanted, ArtistCard, Credited, MusicGenre, MusicInitial, MusicPage,
    SongOrder, SongRow,
};

use crate::reach::{may_read, may_read_the_work};
use crate::{AppState, Result};

/// The most any one page holds, whatever is asked: a screen shows far fewer,
/// and a page of a whole collection would be the collection.
pub const LONGEST_PAGE: i64 = 200;

/// Where a page starts and how long it is, brought within bounds.
#[derive(Debug, Clone, Copy)]
pub struct Paging {
    pub offset: i64,
    pub limit: i64,
}

impl Paging {
    fn bounded(self) -> (i64, i64) {
        (self.offset.max(0), self.limit.clamp(1, LONGEST_PAGE))
    }
}

pub async fn albums(
    state: &AppState,
    who: &User,
    library: LibraryId,
    wanted: &AlbumsWanted,
    order: AlbumOrder,
    descending: bool,
    paging: Paging,
) -> Result<MusicPage<AlbumCard>> {
    may_read(who, library)?;
    let (offset, limit) = paging.bounded();
    Ok(state
        .database()
        .music_albums(library, wanted, order, descending, offset, limit)
        .await?)
}

/// How many albums a library holds, which is what a library of music counts
/// in: its songs are met inside them.
pub async fn albums_held(state: &AppState, library: LibraryId) -> Result<i64> {
    Ok(state
        .database()
        .music_albums(library, &AlbumsWanted::default(), AlbumOrder::Title, false, 0, 1)
        .await?
        .total)
}

pub async fn artists(
    state: &AppState,
    who: &User,
    library: LibraryId,
    album_artists_only: bool,
    paging: Paging,
) -> Result<MusicPage<ArtistCard>> {
    may_read(who, library)?;
    let (offset, limit) = paging.bounded();
    Ok(state
        .database()
        .music_artists(library, album_artists_only, offset, limit)
        .await?)
}

pub async fn songs(
    state: &AppState,
    who: &User,
    library: LibraryId,
    order: SongOrder,
    descending: bool,
    paging: Paging,
) -> Result<MusicPage<SongRow>> {
    may_read(who, library)?;
    let (offset, limit) = paging.bounded();
    Ok(state
        .database()
        .music_songs(library, order, descending, offset, limit)
        .await?)
}

pub async fn genres(state: &AppState, who: &User, library: LibraryId) -> Result<Vec<MusicGenre>> {
    may_read(who, library)?;
    Ok(state.database().music_genres(library).await?)
}

/// The letters of the albums, or of the artists, of a library.
pub async fn initials(
    state: &AppState,
    who: &User,
    library: LibraryId,
    artists: bool,
    album_artists_only: bool,
) -> Result<Vec<MusicInitial>> {
    may_read(who, library)?;
    Ok(state
        .database()
        .music_initials(library, artists, album_artists_only)
        .await?)
}

pub async fn album(
    state: &AppState,
    who: &User,
    album: WorkId,
) -> Result<Option<(AlbumCard, Vec<SongRow>)>> {
    may_read_the_work(state, who, album).await?;
    Ok(state.database().music_album(album).await?)
}

pub async fn artist(
    state: &AppState,
    who: &User,
    artist: WorkId,
) -> Result<Option<(ArtistCard, Vec<AlbumCard>, Vec<AlbumCard>)>> {
    may_read_the_work(state, who, artist).await?;
    Ok(state.database().music_artist(artist).await?)
}
