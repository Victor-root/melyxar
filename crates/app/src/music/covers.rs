//! Looking up online the covers the albums of a library lack and the photos
//! its artists lack, each when the library asks for it: MusicBrainz says
//! which album a title and an artist are and the Cover Art Archive gives its
//! front cover, and Deezer gives the photo of an artist it knows by name.
//!
//! A picture beside the songs always comes first, and an album or an artist
//! is asked about once, found or not. A job of its own, started once a scan
//! is over and taken up again after a restart, asking each service no more
//! often than it asks. When one is too busy to answer, what is left of its
//! part waits for the next scan.

use std::future::Future;

use melyxar_core::id::WorkId;
use melyxar_core::job::{JobKind, JobPriority, JobStep};
use melyxar_core::library::Library;
use melyxar_database::music_covers::{AlbumWithoutCover, ArtistWithoutPhoto};
use melyxar_jobs::JobHandle;
use melyxar_metadata::ProviderError;
use melyxar_metadata::deezer::{self, DeezerClient};
use melyxar_metadata::musicbrainz::{self, MusicBrainzClient};

use crate::{AppState, Result};

/// How many albums or artists are asked for at a time.
const IN_ONE_BATCH: i64 = 16;

/// Starts looking pictures up for a library that asks for it and has albums
/// or artists without one, which is most scans of a new collection and few
/// after.
pub async fn start_when_needed(
    state: &AppState,
    library: Library,
    priority: JobPriority,
) -> Result<()> {
    let database = state.database();
    let options = database.music_library_options(library.id).await?;
    let covers =
        options.covers_online && database.count_albums_without_cover(library.id).await? > 0;
    let photos =
        options.artist_photos_online && database.count_artists_without_photo(library.id).await? > 0;
    if !covers && !photos {
        return Ok(());
    }
    start(state, library, priority).await.map(|_| ())
}

pub async fn start(
    state: &AppState,
    library: Library,
    priority: JobPriority,
) -> Result<melyxar_jobs::StartedJob> {
    let owned = state.clone();
    let target = library.id.to_string();
    Ok(state
        .jobs()
        .clone()
        .start(
            JobKind::LookUpAlbumCovers,
            priority,
            Some(target),
            move |handle| async move {
                let found = look_up_the_pictures_of(&owned, &library, &handle)
                    .await
                    .map_err(|error| error.to_string())?;
                tracing::info!(
                    library = library.name,
                    pictures = found,
                    "the covers and artist photos of this library were looked up"
                );
                Ok(())
            },
        )
        .await?)
}

async fn look_up_the_pictures_of(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    let options = state.database().music_library_options(library.id).await?;
    let mut found = 0;
    if options.covers_online {
        found += look_up_covers(state, library, handle).await?;
    }
    if options.artist_photos_online && !handle.is_cancelled() {
        found += look_up_artist_photos(state, library, handle).await?;
    }
    if found > 0 {
        state.database().bump_library_version(library.id).await?;
    }
    Ok(found)
}

async fn look_up_covers(state: &AppState, library: &Library, handle: &JobHandle) -> Result<usize> {
    let database = state.database();
    let waiting = database.count_albums_without_cover(library.id).await?;
    if waiting == 0 {
        return Ok(0);
    }
    let client = match MusicBrainzClient::new() {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(%error, "MusicBrainz could not be prepared");
            return Ok(0);
        }
    };
    handle.at_step(JobStep::LookingUpCovers).await;
    handle.set_total(waiting).await;
    look_up_each(
        state,
        library,
        handle,
        "MusicBrainz",
        || database.albums_without_cover(library.id, IN_ONE_BATCH),
        |album: &AlbumWithoutCover| (album.id, album.title.clone()),
        |album| cover_of(&client, album),
        |album| database.mark_cover_looked_up(album, melyxar_core::time::now()),
    )
    .await
}

async fn look_up_artist_photos(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    let database = state.database();
    let waiting = database.count_artists_without_photo(library.id).await?;
    if waiting == 0 {
        return Ok(0);
    }
    let client = match DeezerClient::new() {
        Ok(client) => client,
        Err(error) => {
            tracing::warn!(%error, "Deezer could not be prepared");
            return Ok(0);
        }
    };
    handle.at_step(JobStep::LookingUpArtistPhotos).await;
    handle.set_total(waiting).await;
    look_up_each(
        state,
        library,
        handle,
        "Deezer",
        || database.artists_without_photo(library.id, IN_ONE_BATCH),
        |artist: &ArtistWithoutPhoto| (artist.id, artist.name.clone()),
        |artist| photo_of(&client, artist),
        |artist| database.mark_artist_photo_looked_up(artist, melyxar_core::time::now()),
    )
    .await
}

/// Asks about each album or artist left, batch after batch, keeping each
/// picture found and marking each one asked about, until none is left, the
/// job is stopped, or the service is too busy.
#[allow(clippy::too_many_arguments)]
async fn look_up_each<T, Batch, Ask, Asking, Mark, Marking>(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
    service: &str,
    batch: impl Fn() -> Batch,
    who: impl Fn(&T) -> (WorkId, String),
    ask: Ask,
    mark: Mark,
) -> Result<usize>
where
    Batch: Future<Output = melyxar_database::Result<Vec<T>>>,
    Ask: Fn(T) -> Asking,
    Asking: Future<Output = Asked>,
    Mark: Fn(WorkId) -> Marking,
    Marking: Future<Output = melyxar_database::Result<()>>,
{
    let mut found = 0;
    loop {
        let left = batch().await?;
        if left.is_empty() {
            return Ok(found);
        }
        for one in left {
            if handle.is_cancelled() {
                return Ok(found);
            }
            let (id, name) = who(&one);
            handle.now_working_on(Some(&name)).await;
            match ask(one).await {
                Asked::Found { bytes, made_from } => {
                    if crate::images::store_poster_from_bytes(state, id, &bytes, "jpg", &made_from)
                        .await?
                    {
                        found += 1;
                    }
                }
                Asked::Nothing => {}
                Asked::Later(reason) => {
                    tracing::warn!(
                        library = library.name,
                        service,
                        reason,
                        "the service is busy, the pictures left wait for the next scan"
                    );
                    return Ok(found);
                }
            }
            mark(id).await?;
            handle.advance(1).await;
        }
    }
}

/// What asking about one album or artist came to.
enum Asked {
    Found {
        bytes: Vec<u8>,
        made_from: String,
    },
    /// Nothing found, which is an answer: it is not asked about again.
    Nothing,
    /// No answer for now: it is asked about again another time.
    Later(String),
}

impl Asked {
    /// What an error of a service comes to: worth another try later, or an
    /// answer of nothing.
    fn from_error(error: ProviderError, name: &str, service: &str) -> Self {
        if error.is_worth_retrying() {
            return Self::Later(error.to_string());
        }
        tracing::debug!(name, service, %error, "the service gave no usable answer");
        Self::Nothing
    }
}

/// The cover of one album, asked of MusicBrainz and then of the Cover Art
/// Archive, each question followed by the pause MusicBrainz asks for.
async fn cover_of(client: &MusicBrainzClient, album: AlbumWithoutCover) -> Asked {
    let release_group = client.album(&album.title, album.artist.as_deref()).await;
    tokio::time::sleep(musicbrainz::BETWEEN_QUESTIONS).await;
    let release_group = match release_group {
        Ok(Some(release_group)) => release_group,
        Ok(None) => return Asked::Nothing,
        Err(error) => return Asked::from_error(error, &album.title, "MusicBrainz"),
    };
    match client.front_cover(&release_group).await {
        Ok(Some(bytes)) => Asked::Found {
            bytes,
            made_from: format!("musicbrainz:{}", album.id),
        },
        Ok(None) => Asked::Nothing,
        Err(error) => Asked::from_error(error, &album.title, "the Cover Art Archive"),
    }
}

/// The photo of one artist, asked of Deezer, each question followed by the
/// pause that keeps under what Deezer allows.
async fn photo_of(client: &DeezerClient, artist: ArtistWithoutPhoto) -> Asked {
    let url = client.artist_photo(&artist.name).await;
    tokio::time::sleep(deezer::BETWEEN_QUESTIONS).await;
    let url = match url {
        Ok(Some(url)) => url,
        Ok(None) => return Asked::Nothing,
        Err(error) => return Asked::from_error(error, &artist.name, "Deezer"),
    };
    match client.photo(&url).await {
        Ok(bytes) => Asked::Found {
            bytes,
            made_from: format!("deezer:{url}"),
        },
        Err(error) => Asked::from_error(error, &artist.name, "Deezer"),
    }
}
