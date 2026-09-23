//! Deleting works, out of the library and off the disk when asked.

use axum::extract::State;
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
            "/api/v1/deletion/what-it-takes",
            axum::routing::post(what_deleting_takes),
        )
        .route("/api/v1/deletion", axum::routing::post(delete))
}

#[derive(Debug, Deserialize)]
struct Asked {
    /// The works to delete, each with everything under it.
    works: Vec<String>,
    /// Off the disk as well as out of the library.
    #[serde(default)]
    from_disk: bool,
}

#[derive(Debug, Serialize)]
struct DeletionView {
    /// The works and everything under them, each once.
    works: i64,
    /// Every file on the disk they stand for, copies first, by its whole path:
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

#[derive(Debug, Serialize)]
struct DeletedView {
    works: i64,
    files: i64,
}

async fn what_deleting_takes(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Json(asked): Json<Asked>,
) -> Result<Json<DeletionView>> {
    let going =
        melyxar_app::deletion::what_deleting_takes(&state, &who, &work_ids(&asked.works)?).await?;
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
    Json(asked): Json<Asked>,
) -> Result<Json<DeletedView>> {
    let removed =
        melyxar_app::deletion::delete(&state, &who, &work_ids(&asked.works)?, asked.from_disk)
            .await?;
    Ok(Json(DeletedView {
        works: removed.works,
        files: removed.files,
    }))
}

fn work_ids(ids: &[String]) -> Result<Vec<WorkId>> {
    ids.iter()
        .map(|id| {
            id.parse()
                .map_err(|_| ServerError::invalid_input("not a work identifier"))
        })
        .collect()
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
