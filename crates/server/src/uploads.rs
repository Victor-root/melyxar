//! Files sent into a library from the interface, one to a request.
//!
//! The body is the file itself, read as it comes and written to the disk a
//! piece at a time: nothing is held whole in memory, and no size limit of the
//! server's own sits in the way, since what limits a file is the disk and the
//! rule of the use case.

use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Path, Query, State};
use axum::routing::post;
use axum::{Json, Router};
use futures_util::StreamExt;
use melyxar_app::uploads::{Target, begin};
use melyxar_app::AppState;
use serde::{Deserialize, Serialize};

use crate::account::Viewer;
use crate::error::{Result, ServerError};
use crate::identifiers::{parse_library, parse_work};

pub fn router() -> Router<AppState> {
    Router::new().route(
        "/api/v1/libraries/{id}/upload",
        post(upload).layer(DefaultBodyLimit::disable()),
    )
}

#[derive(Debug, Deserialize)]
struct Asked {
    /// The name the file is to have.
    name: String,
    /// The root it is put under, and the folder under it.
    root: Option<String>,
    #[serde(default)]
    folder: String,
    /// Or the album whose folder it is put into.
    album: Option<String>,
}

#[derive(Debug, Serialize)]
struct Sent {
    /// Where it is, under its root.
    path: String,
    bytes: u64,
}

async fn upload(
    State(state): State<AppState>,
    Viewer(who): Viewer,
    Path(library): Path<String>,
    Query(asked): Query<Asked>,
    body: Body,
) -> Result<Json<Sent>> {
    let target = match (&asked.album, &asked.root) {
        (Some(album), _) => Target::Album(parse_work(album)?),
        (None, Some(root)) => Target::Folder {
            root: root
                .parse()
                .map_err(|_| ServerError::invalid_input("the folder identifier is malformed"))?,
            folder: asked.folder.clone(),
        },
        (None, None) => {
            return Err(ServerError::invalid_input(
                "a file is sent to a folder or to an album",
            ));
        }
    };
    let mut arriving =
        begin(&state, &who, parse_library(&library)?, &target, &asked.name).await?;
    let mut pieces = body.into_data_stream();
    while let Some(piece) = pieces.next().await {
        let piece = piece
            .map_err(|_| ServerError::invalid_input("the file stopped arriving before its end"))?;
        arriving.push(&piece).await?;
    }
    let uploaded = arriving.finish().await?;
    Ok(Json(Sent {
        path: uploaded.relative_path.display().to_string(),
        bytes: uploaded.bytes,
    }))
}
