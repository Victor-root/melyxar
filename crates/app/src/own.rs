//! Filing what people filmed and photographed themselves.
//!
//! No name is read the way a release is read: a file is named by its file,
//! and it sits in the folders it sits in on the disk. See the decisions on
//! libraries of home media for why.

use std::path::Path;

use melyxar_core::id::WorkId;
use melyxar_core::job::JobStep;
use melyxar_core::library::Library;
use melyxar_core::orientation::Orientation;
use melyxar_core::time::Millis;
use melyxar_core::work::WorkKind;
use melyxar_database::own::OwnFileToPicture;
use melyxar_jobs::JobHandle;

use crate::{AppState, Result};

/// The work a file of a library of home media stands for, with every folder
/// above it written down on the way.
///
/// Always a new one: two files are two works whatever they are called, and a
/// file already written down is never asked about again.
pub(crate) async fn work_for(
    state: &AppState,
    library: &Library,
    relative_path: &Path,
) -> Result<WorkId> {
    let database = state.database();

    let mut folder = None;
    if let Some(above) = relative_path.parent() {
        for name in above.iter().filter_map(|part| part.to_str()) {
            let found = database
                .own_folder(library.id, folder, name, &melyxar_library::sort_title(name))
                .await?;
            folder = Some(found.id);
        }
    }

    let file_name = relative_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    let kind = match melyxar_library::naming::is_photo_file(file_name) {
        true => WorkKind::Photo,
        false => WorkKind::Video,
    };
    let title = relative_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(file_name);

    let work = database
        .create_own_work(
            library.id,
            folder,
            kind,
            title,
            &melyxar_library::sort_title(title),
        )
        .await?;
    Ok(work.id)
}

/// Takes the picture of every video and photo of a library of home media
/// that has none made the way pictures are made today, out of the file
/// itself. Answers how many were made.
///
/// Bounded like the analysis before it, for the same reason: the machine is
/// there for somebody watching something first.
pub(crate) async fn picture_what_has_none(
    state: &AppState,
    library: &Library,
    handle: &JobHandle,
) -> Result<usize> {
    let waiting = state
        .database()
        .own_files_to_picture(library.id, crate::images::RECIPE)
        .await?;
    if waiting.is_empty() {
        return Ok(0);
    }

    handle.at_step(JobStep::PicturingOwnFiles).await;
    handle.set_total(waiting.len() as i64).await;
    let limit = state.config().limits.concurrent_probes;
    let owned_state = state.clone();
    let owned_handle = handle.clone();

    let made = melyxar_jobs::for_each_bounded(waiting, limit, move |file| {
        let state = owned_state.clone();
        let handle = owned_handle.clone();
        async move {
            if handle.is_cancelled() {
                return false;
            }
            let shown = file.path.display().to_string();
            handle.now_working_on(Some(&shown)).await;
            let made = picture_one(&state, &file).await.unwrap_or_else(|error| {
                tracing::warn!(
                    file = %shown,
                    error = %error,
                    "no picture could be taken out of this file; it will be tried again"
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

/// Where the picture of a video is taken, as a share of how long it runs.
///
/// A tenth of the way in: the very first picture of a video somebody filmed is
/// as often as not black, blurred, or the ground while the phone came up.
const INTO_A_VIDEO: i64 = 10;

async fn picture_one(state: &AppState, file: &OwnFileToPicture) -> Result<bool> {
    let (at, orientation) = match file.kind {
        WorkKind::Photo => {
            let path = file.path.clone();
            let orientation = tokio::task::spawn_blocking(move || {
                melyxar_library::orientation::orientation_of(&path)
            })
            .await
            .unwrap_or_default();
            (None, orientation)
        }
        _ => {
            let at = file
                .duration
                .map(|length| Millis::new(length.get() / INTO_A_VIDEO))
                .unwrap_or(Millis::ZERO);
            (Some(at), Orientation::AsStored)
        }
    };
    crate::images::store_own_picture(
        state,
        file.work_id,
        &file.path,
        at,
        orientation,
        &file.made_from,
    )
    .await
}
