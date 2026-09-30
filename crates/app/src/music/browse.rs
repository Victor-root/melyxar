//! What an account reads of a library of music.
//!
//! Every read is checked against the libraries the account was granted, the
//! same way the grids of films are: a library or an album somebody was not
//! given answers as one that is not there.

use melyxar_core::id::{LibraryId, WorkId};
use melyxar_core::library::LibraryKind;
use melyxar_core::user::User;
pub use melyxar_database::music_browse::{
    AlbumCard, AlbumOrder, AlbumsWanted, ArtistCard, Credited, MusicFound, MusicGenre,
    MusicInitial, MusicPage, SongOrder, SongRow,
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
        .music_albums(
            library,
            &AlbumsWanted::default(),
            AlbumOrder::Title,
            false,
            0,
            1,
        )
        .await?
        .total)
}

/// The most of each list a search answers with: it is a glance, and the
/// library itself is where the rest is read.
pub const FOUND_OF_EACH: i64 = 24;

/// What a few words found among the music an account may read: in one
/// library, or in every library of music it was granted, taken in turn from
/// each so that none pushes the others out.
pub async fn search(
    state: &AppState,
    who: &User,
    library: Option<LibraryId>,
    words: &str,
) -> Result<MusicFound> {
    let words = words.trim();
    if words.is_empty() {
        return Ok(MusicFound::default());
    }
    let database = state.database();
    let mut libraries = Vec::new();
    for held in database.list_libraries().await? {
        if held.kind == LibraryKind::Music
            && who.permissions.may_access_library(held.id)
            && library.is_none_or(|wanted| wanted == held.id)
        {
            libraries.push(held.id);
        }
    }
    let mut each = Vec::with_capacity(libraries.len());
    for library in libraries {
        each.push(database.music_search(library, words, FOUND_OF_EACH).await?);
    }
    let room = FOUND_OF_EACH as usize;
    Ok(MusicFound {
        albums: in_turn(
            each.iter_mut()
                .map(|f| std::mem::take(&mut f.albums))
                .collect(),
            room,
        ),
        artists: in_turn(
            each.iter_mut()
                .map(|f| std::mem::take(&mut f.artists))
                .collect(),
            room,
        ),
        songs: in_turn(
            each.iter_mut()
                .map(|f| std::mem::take(&mut f.songs))
                .collect(),
            room,
        ),
    })
}

/// Several lists as one, one from each in turn, up to `room`.
fn in_turn<T>(lists: Vec<Vec<T>>, room: usize) -> Vec<T> {
    let mut iterators: Vec<_> = lists.into_iter().map(Vec::into_iter).collect();
    let mut taken = Vec::new();
    while taken.len() < room {
        let before = taken.len();
        for iterator in &mut iterators {
            if taken.len() < room
                && let Some(next) = iterator.next()
            {
                taken.push(next);
            }
        }
        if taken.len() == before {
            break;
        }
    }
    taken
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

/// The most a queue started from a whole library holds: far more than is
/// listened to in a sitting, and few enough to be sent at once.
pub const LONGEST_QUEUE: i64 = 2000;

/// The songs a whole library is played from, in the order of its albums,
/// starting at this one. The total says how many there are beyond the queue.
pub async fn queue(
    state: &AppState,
    who: &User,
    library: LibraryId,
    offset: i64,
) -> Result<MusicPage<SongRow>> {
    may_read(who, library)?;
    Ok(state
        .database()
        .music_songs(library, SongOrder::Album, false, offset.max(0), LONGEST_QUEUE)
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

/// Every song an artist plays on, for playing them all.
pub async fn artist_songs(state: &AppState, who: &User, artist: WorkId) -> Result<Vec<SongRow>> {
    may_read_the_work(state, who, artist).await?;
    Ok(state.database().music_artist_songs(artist).await?)
}

#[cfg(test)]
mod tests {
    use super::in_turn;

    #[test]
    fn several_lists_are_taken_from_in_turn_up_to_the_room() {
        assert_eq!(
            in_turn(vec![vec![1, 2, 3], vec![10]], 10),
            vec![1, 10, 2, 3]
        );
        assert_eq!(in_turn(vec![vec![1, 2], vec![10, 20]], 3), vec![1, 10, 2]);
        assert_eq!(in_turn::<i32>(Vec::new(), 3), Vec::<i32>::new());
    }
}
