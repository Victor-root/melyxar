//! Looking up online the covers the albums of a library lack, when the
//! library asks for it: MusicBrainz says which album a title and an artist
//! are, and the Cover Art Archive gives its front cover.
//!
//! A cover beside the songs always comes first, and an album is asked about
//! once, found or not. A job of its own, started once a scan is over and
//! taken up again after a restart, asking MusicBrainz no more than once a
//! second, as it asks. When it is too busy to answer, the job stops and the
//! albums left wait for the next scan.

use melyxar_core::job::{JobKind, JobPriority, JobStep};
use melyxar_core::library::Library;
use melyxar_database::music_covers::AlbumWithoutCover;
use melyxar_jobs::JobHandle;
use melyxar_metadata::musicbrainz::{BETWEEN_QUESTIONS, MusicBrainzClient};

use crate::{AppState, Result};

/// How many albums are asked for at a time.
const IN_ONE_BATCH: i64 = 16;

/// Starts looking covers up for a library that asks for it and has albums
/// without one, which is most scans of a new collection and few after.
pub async fn start_when_needed(
    state: &AppState,
    library: Library,
    priority: JobPriority,
) -> Result<()> {
    let database = state.database();
    if !database
        .music_library_options(library.id)
        .await?
        .covers_online
        || database.count_albums_without_cover(library.id).await? == 0
    {
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
                let found = look_up_the_covers_of(&owned, &library, &handle)
                    .await
                    .map_err(|error| error.to_string())?;
                tracing::info!(
                    library = library.name,
                    covers = found,
                    "the covers of this library were looked up"
                );
                Ok(())
            },
        )
        .await?)
}

async fn look_up_the_covers_of(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    let database = state.database();
    if !database
        .music_library_options(library.id)
        .await?
        .covers_online
    {
        return Ok(0);
    }
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

    let mut found = 0;
    'batches: loop {
        let batch = database
            .albums_without_cover(library.id, IN_ONE_BATCH)
            .await?;
        if batch.is_empty() {
            break;
        }
        for album in batch {
            if handle.is_cancelled() {
                break 'batches;
            }
            handle.now_working_on(Some(&album.title)).await;
            match cover_of(&client, &album).await {
                Asked::Found(bytes) => {
                    if crate::images::store_poster_from_bytes(
                        state,
                        album.id,
                        &bytes,
                        "jpg",
                        &format!("musicbrainz:{}", album.id),
                    )
                    .await?
                    {
                        found += 1;
                    }
                }
                Asked::Nothing => {}
                Asked::Later(reason) => {
                    tracing::warn!(
                        library = library.name,
                        reason,
                        "MusicBrainz is busy, the covers left wait for the next scan"
                    );
                    break 'batches;
                }
            }
            database
                .mark_cover_looked_up(album.id, melyxar_core::time::now())
                .await?;
            handle.advance(1).await;
        }
    }
    if found > 0 {
        database.bump_library_version(library.id).await?;
    }
    Ok(found)
}

/// What asking about one album came to.
enum Asked {
    Found(Vec<u8>),
    /// Nothing found, which is an answer: the album is not asked about again.
    Nothing,
    /// No answer for now: the album is asked about again another time.
    Later(String),
}

/// The cover of one album, asked of MusicBrainz and then of the Cover Art
/// Archive, each question followed by the pause MusicBrainz asks for.
async fn cover_of(client: &MusicBrainzClient, album: &AlbumWithoutCover) -> Asked {
    let release_group = client.album(&album.title, album.artist.as_deref()).await;
    tokio::time::sleep(BETWEEN_QUESTIONS).await;
    let release_group = match release_group {
        Ok(Some(release_group)) => release_group,
        Ok(None) => return Asked::Nothing,
        Err(error) if error.is_worth_retrying() => return Asked::Later(error.to_string()),
        Err(error) => {
            tracing::debug!(album = album.title, %error, "MusicBrainz gave no usable answer");
            return Asked::Nothing;
        }
    };
    match client.front_cover(&release_group).await {
        Ok(Some(bytes)) => Asked::Found(bytes),
        Ok(None) => Asked::Nothing,
        Err(error) if error.is_worth_retrying() => Asked::Later(error.to_string()),
        Err(error) => {
            tracing::debug!(album = album.title, %error, "the Cover Art Archive gave no usable answer");
            Asked::Nothing
        }
    }
}
