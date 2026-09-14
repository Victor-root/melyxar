//! The little pictures shown while somebody drags along the playback bar.
//!
//! Making them means reading a film through from end to end, which is why it
//! happens in a background pass of a scan and never while somebody is
//! watching. One reading writes every sheet a film needs.
//!
//! The sheets live in the cache, named after the file they came out of. They
//! are handed over as they are: a page fetches the sheet holding the moment
//! under the cursor and cuts the thumbnail out of it itself.

use std::path::PathBuf;

use melyxar_core::id::MediaSourceId;
use melyxar_core::media::TrackKind;
use melyxar_core::privacy::MediaName;
use melyxar_core::thumbnails::{Layout, Thumbnails};
use melyxar_core::time::Millis;

use crate::{AppError, AppState, Result};

/// The shape this server is set to make them in, when it makes them at all.
pub fn wanted(state: &AppState) -> Option<Layout> {
    let asked = &state.config().thumbnails;
    if !asked.enabled || asked.every_seconds == 0 || asked.columns == 0 || asked.rows == 0 {
        return None;
    }
    Some(Layout {
        every: Millis::new(i64::from(asked.every_seconds) * 1_000),
        height: asked.height.max(1),
        columns: asked.columns,
        rows: asked.rows,
    })
}

/// Where the sheets of one film are kept.
fn kept_at(state: &AppState, source_id: MediaSourceId) -> PathBuf {
    state
        .config()
        .directories
        .thumbnails()
        .join(source_id.to_string())
}

/// Where they are written while the film is still being read.
///
/// The same rule as everywhere else in the cache: what is under the name the
/// server reads is whole, or it is not there. The tool writes sheet after
/// sheet as it goes, and a sheet handed over half written is a row of grey
/// squares on somebody's bar.
fn while_it_is_read(state: &AppState, source_id: MediaSourceId) -> PathBuf {
    state
        .config()
        .directories
        .thumbnails()
        .join(format!("{source_id}.making"))
}

/// Reads one film through and writes down what it gave.
///
/// Answers what came out. Nothing counted is an answer too: there are files in
/// a film folder that hold no picture, and it is written down so the file is
/// never read through again for the same nothing.
pub async fn make_for(state: &AppState, source_id: MediaSourceId) -> Result<Thumbnails> {
    let Some(layout) = wanted(state) else {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "this server is not making thumbnails for the playback bar",
        )));
    };
    let tools = state.tools().ok_or_else(|| {
        AppError::Domain(melyxar_core::Error::dependency_missing(
            "this server has no media tools, so no thumbnail can be made",
        ))
    })?;

    let database = state.database();
    let source = database
        .playable_source(source_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("media source")))?;
    if source.missing {
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::RootUnavailable,
            "the file is not on the disk at the moment",
        )));
    }

    // An unconverted frame of a wide gamut film is the washed out thumbnail
    // seen on other servers, so the film is asked what it is.
    let tracks = database.tracks_of_source(source_id).await?;
    let tone_map = tracks.iter().any(|track| match &track.kind {
        TrackKind::Video(details) => details.needs_tone_mapping(),
        _ => false,
    });

    let name = MediaName::new(
        source
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default(),
    );
    let aside = while_it_is_read(state, source_id);
    let _ = tokio::fs::remove_dir_all(&aside).await;
    tokio::fs::create_dir_all(&aside)
        .await
        .map_err(AppError::Directory)?;

    tracing::debug!(file = %name, tone_map, "reading a film for the thumbnails of its bar");
    // Only the pictures that stand on their own, which is four times faster
    // and invisible at this size. A film whose pictures cannot be read that
    // way gives nothing at all, and is read again in full rather than written
    // down as having none: one rung down the ladder, once.
    let mut made =
        melyxar_ffmpeg::thumbnails::make(tools, &source.path, &aside, layout, tone_map, true).await;
    if made.as_ref().is_ok_and(|made| made.counted == 0) {
        tracing::debug!(
            file = %name,
            "no picture of this film stands on its own, so it is read again in full"
        );
        made =
            melyxar_ffmpeg::thumbnails::make(tools, &source.path, &aside, layout, tone_map, false)
                .await;
    }

    let made = match made {
        Ok(made) => made,
        Err(error) => {
            let _ = tokio::fs::remove_dir_all(&aside).await;
            tracing::warn!(
                file = %name,
                %error,
                "this film could not be read for the thumbnails of its bar"
            );
            return Err(error.into());
        }
    };

    // Moved into place in one step, once the reading is over.
    let kept = kept_at(state, source_id);
    let _ = tokio::fs::remove_dir_all(&kept).await;
    if let Err(error) = tokio::fs::rename(&aside, &kept).await {
        let _ = tokio::fs::remove_dir_all(&aside).await;
        tracing::warn!(
            file = %name,
            %error,
            "the thumbnails of this film could not be put in the cache"
        );
        return Err(AppError::Directory(error));
    }

    database.store_thumbnails(source_id, &made).await?;
    tracing::info!(
        file = %name,
        thumbnails = made.counted,
        sheets = made.sheets,
        "a film has the thumbnails of its bar"
    );
    Ok(made)
}

/// Where one sheet of a film is, for the route that hands it over.
///
/// Only sheets this server wrote, and only by the number it gave them: nothing
/// a client sends is ever treated as a path.
pub async fn sheet_of(state: &AppState, source_id: MediaSourceId, number: u32) -> Result<PathBuf> {
    let made = state
        .database()
        .thumbnails_of(source_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("thumbnails")))?;
    if number >= made.sheets {
        return Err(AppError::Domain(melyxar_core::Error::not_found(
            "that sheet of thumbnails",
        )));
    }
    let path = melyxar_ffmpeg::thumbnails::sheet_at(&kept_at(state, source_id), number);
    if !tokio::fs::try_exists(&path).await.unwrap_or(false) {
        // The row says it was made and the sheet is not there, which is what a
        // cache emptied by hand looks like. Said out loud: the film is read
        // again by the next scan, and until then a bar with no pictures on it
        // is the only sign.
        tracing::warn!(
            sheet = number,
            "a sheet of thumbnails is written down and is not in the cache"
        );
        return Err(AppError::Domain(melyxar_core::Error::not_found(
            "that sheet of thumbnails",
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Nothing below touches the disk or the database: only the shape the
    /// settings ask for is being read.
    async fn state_with(asked: melyxar_config::ThumbnailsConfig) -> AppState {
        let config = melyxar_config::Config {
            thumbnails: asked,
            ..melyxar_config::Config::default()
        };
        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        AppState::new(config, database, None, None)
    }

    #[tokio::test]
    async fn the_shape_asked_for_is_the_one_the_settings_say() {
        let state = state_with(melyxar_config::ThumbnailsConfig::default()).await;
        assert_eq!(
            wanted(&state),
            Some(Layout {
                every: Millis::new(10_000),
                height: 180,
                columns: 10,
                rows: 10,
            })
        );
    }

    #[tokio::test]
    async fn a_server_told_not_to_make_them_makes_none() {
        let state = state_with(melyxar_config::ThumbnailsConfig {
            enabled: false,
            ..Default::default()
        })
        .await;
        assert_eq!(wanted(&state), None);
    }

    #[tokio::test]
    async fn a_shape_that_cannot_hold_a_thumbnail_is_refused_rather_than_made() {
        // Nought seconds apart is a film's worth of pictures at one moment, and
        // a sheet with no rows holds nothing. Both are a settings file somebody
        // typed into, and neither is worth reading three hundred films for.
        for wrong in [
            melyxar_config::ThumbnailsConfig {
                every_seconds: 0,
                ..Default::default()
            },
            melyxar_config::ThumbnailsConfig {
                columns: 0,
                ..Default::default()
            },
            melyxar_config::ThumbnailsConfig {
                rows: 0,
                ..Default::default()
            },
        ] {
            assert_eq!(wanted(&state_with(wrong).await), None);
        }
    }
}
