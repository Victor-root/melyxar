//! The covers of albums and the pictures of artists, taken from beside the
//! songs.
//!
//! An album wears the picture its folder holds under a name such as
//! `cover.jpg` or `folder.jpg`, and failing that the cover its first song
//! carries inside it. An artist wears the picture the folder above their
//! album holds under a name such as `artist.jpg`. Nothing is asked of any
//! service: whoever keeps a collection of music keeps its covers with it, and
//! the servers people come from read them from there too.
//!
//! The pictures beside the songs are found by the walk on its way past, so
//! nothing here looks at a folder again. A picture already made from the same
//! file, unchanged, is not made again.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use melyxar_core::id::LibraryRootId;
use melyxar_core::job::JobStep;
use melyxar_core::library::Library;
use melyxar_core::work::WorkKind;
use melyxar_database::music_pictures::MusicToPicture;
use melyxar_jobs::JobHandle;
use melyxar_library::FoundFile;
use melyxar_library::music::{album_folder, album_picture, artist_folder, artist_picture};

use crate::{AppState, Result};

/// The pictures one walk found beside the songs, by the root it walked.
pub(crate) type PicturesFound = HashMap<LibraryRootId, Vec<FoundFile>>;

/// Where one picture comes from.
enum Source {
    /// A picture file beside the songs.
    File(PathBuf),
    /// The cover a song carries inside it.
    Song(PathBuf),
}

/// One picture to make.
struct ToMake {
    subject: MusicToPicture,
    source: Source,
    made_from: String,
}

/// Makes the pictures of every album and artist whose picture is missing or
/// was made from something that has changed. Answers how many were made.
///
/// Only the roots walked this time are looked at: a disk that could not be
/// walked says nothing about the pictures it holds, and what its albums wear
/// stays as it is.
pub(crate) async fn picture(
    state: &AppState,
    library: &Library,
    found: &PicturesFound,
    handle: &JobHandle,
) -> Result<usize> {
    let to_make: Vec<ToMake> = state
        .database()
        .music_to_picture(library.id)
        .await?
        .into_iter()
        .filter_map(|subject| {
            let pictures = found.get(&subject.root_id)?;
            let (source, made_from) = source_of(&subject, pictures)?;
            let wanted = crate::images::fingerprint_of(&made_from);
            (subject.fingerprint.as_deref() != Some(wanted.as_str())).then_some(ToMake {
                subject,
                source,
                made_from,
            })
        })
        .collect();
    if to_make.is_empty() {
        return Ok(0);
    }

    handle.at_step(JobStep::PicturingOwnFiles).await;
    handle.set_total(to_make.len() as i64).await;
    let limit = state.config().limits.concurrent_probes;
    let owned_state = state.clone();
    let owned_handle = handle.clone();

    let made = melyxar_jobs::for_each_bounded(to_make, limit, move |one| {
        let state = owned_state.clone();
        let handle = owned_handle.clone();
        async move {
            if handle.is_cancelled() {
                return false;
            }
            let shown = one.subject.song.display().to_string();
            handle.now_working_on(Some(&shown)).await;
            let made = make(&state, &one).await.unwrap_or_else(|error| {
                tracing::warn!(
                    file = %shown,
                    error = %error,
                    "no picture could be made for this album or artist; it will be tried again"
                );
                false
            });
            handle.advance(1).await;
            made
        }
    })
    .await;
    Ok(made.into_iter().filter(|made| *made).count())
}

/// Which picture a subject is to wear, and what says which state of it the
/// picture is made from. Nothing for an artist whose folder holds no picture
/// of them.
fn source_of(subject: &MusicToPicture, pictures: &[FoundFile]) -> Option<(Source, String)> {
    let album = album_folder(subject.song.parent().unwrap_or(Path::new("")));
    let folder = match subject.kind {
        WorkKind::Artist => artist_folder(album)?,
        _ => album,
    };
    let beside: Vec<&FoundFile> = pictures
        .iter()
        .filter(|picture| picture.relative_path.parent() == Some(folder))
        .collect();
    let names = beside
        .iter()
        .filter_map(|picture| picture.relative_path.file_name()?.to_str());
    let chosen = match subject.kind {
        WorkKind::Artist => artist_picture(names),
        _ => album_picture(names),
    };

    match chosen {
        Some(name) => {
            let picture = beside.iter().find(|picture| {
                picture.relative_path.file_name().and_then(|n| n.to_str()) == Some(name)
            })?;
            Some((
                Source::File(subject.root_path.join(&picture.relative_path)),
                format!(
                    "file|{}|{}|{}",
                    picture.relative_path.display(),
                    picture.size_bytes,
                    melyxar_database::timestamp_to_text(picture.modified_at)
                ),
            ))
        }
        None if subject.kind == WorkKind::Album => Some((
            Source::Song(subject.root_path.join(&subject.song)),
            format!("song|{}", subject.song_made_from),
        )),
        None => None,
    }
}

async fn make(state: &AppState, one: &ToMake) -> Result<bool> {
    let work = one.subject.work_id;
    match &one.source {
        Source::File(path) => {
            crate::images::store_poster_from_file(state, work, path, &one.made_from).await
        }
        Source::Song(path) => {
            let song = path.clone();
            let cover = tokio::task::spawn_blocking(move || melyxar_tags::front_cover(&song))
                .await
                .unwrap_or_else(|failure| std::panic::resume_unwind(failure.into_panic()));
            match cover {
                Ok(Some(cover)) => {
                    crate::images::store_poster_from_bytes(
                        state,
                        work,
                        &cover.data,
                        cover.extension,
                        &one.made_from,
                    )
                    .await
                }
                // An album with no picture anywhere: its card shows its
                // colour and its name, as a film without a poster does.
                Ok(None) | Err(melyxar_tags::ReadError::Unsupported) => Ok(false),
                Err(melyxar_tags::ReadError::Unreadable(reason)) => {
                    tracing::debug!(file = %path.display(), %reason, "the cover of this song could not be read");
                    Ok(false)
                }
            }
        }
    }
}
