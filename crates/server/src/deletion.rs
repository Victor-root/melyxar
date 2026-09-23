//! Deleting a work, out of the library and off the disk when asked.

use axum::extract::{Path, Query, State};
use axum::{Json, Router};
use melyxar_app::deletion::FileRole;
use melyxar_app::AppState;
use melyxar_core::id::WorkId;
use serde::{Deserialize, Serialize};

use crate::account::Viewer;
use crate::error::{Result, ServerError};

pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/v1/works/{id}/deletion",
            axum::routing::get(what_deleting_takes),
        )
        .route("/api/v1/works/{id}", axum::routing::delete(delete))
}

#[derive(Debug, Serialize)]
struct DeletionView {
    /// The work and everything under it.
    works: i64,
    /// Every file on the disk it stands for, copies first, by its whole path:
    /// what somebody deleting off the disk says yes to.
    files: Vec<FileView>,
    /// Whether this account may also delete off the disk, so the choice is
    /// only offered to whoever may make it.
    may_delete_from_disk: bool,
}

#[derive(Debug, Serialize)]
struct FileView {
    path: String,
    /// copy, subtitle or extra.
    role: &'static str,
}

#[derive(Debug, Deserialize)]
struct How {
    /// Off the disk as well as out of the library.
    #[serde(default)]
    from_disk: bool,
}

#[derive(Debug, Serialize)]
struct DeletedView {
    works: i64,
    files: i64,
}

async fn what_deleting_takes(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
) -> Result<Json<DeletionView>> {
    let going = melyxar_app::deletion::what_deleting_takes(&state, &who, work_id(&id)?).await?;
    Ok(Json(DeletionView {
        works: going.works,
        files: going
            .files
            .iter()
            .map(|file| FileView {
                path: file.path().to_string_lossy().into_owned(),
                role: match file.role {
                    FileRole::Copy => "copy",
                    FileRole::Subtitle => "subtitle",
                    FileRole::Extra => "extra",
                },
            })
            .collect(),
        may_delete_from_disk: who.permissions.may_delete_from_disk,
    }))
}

async fn delete(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(id): Path<String>,
    Query(how): Query<How>,
) -> Result<Json<DeletedView>> {
    let removed = melyxar_app::deletion::delete(&state, &who, work_id(&id)?, how.from_disk).await?;
    Ok(Json(DeletedView {
        works: removed.works,
        files: removed.files,
    }))
}

fn work_id(id: &str) -> Result<WorkId> {
    id.parse()
        .map_err(|_| ServerError::invalid_input("not a work identifier"))
}

#[cfg(test)]
mod tests {
    /// A refusal the interface has no words for reaches the screen as its
    /// key, in front of somebody who has just said yes to a deletion.
    #[test]
    fn every_reason_a_deletion_is_refused_for_has_words_in_both_languages() {
        let words = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/src/i18n.ts"),
        )
        .expect("the words of the interface");

        for refused in melyxar_app::deletion::Refused::ALL {
            let key = format!("refused.deletion.{}", refused.as_str());
            assert_eq!(
                words.matches(&format!("\"{key}\":")).count(),
                2,
                "{key} needs a sentence in English and one in French"
            );
        }
    }
}
