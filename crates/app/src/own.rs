//! Filing what people filmed and photographed themselves.
//!
//! No name is read the way a release is read: a file is named by its file,
//! and it sits in the folders it sits in on the disk. See the decisions on
//! libraries of home media for why.

use std::path::Path;

use melyxar_core::id::WorkId;
use melyxar_core::library::Library;
use melyxar_core::work::WorkKind;

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
