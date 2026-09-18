//! The little pictures shown while somebody drags along the playback bar.
//!
//! Making them means reading a film through from end to end, which is why it
//! happens in a background pass of a scan and never while somebody is
//! watching. One reading writes every sheet a film needs.
//!
//! The sheets live in the cache, named after the file they came out of. They
//! are handed over as they are: a page fetches the sheet holding the moment
//! under the cursor and cuts the thumbnail out of it itself.

use std::path::{Path, PathBuf};

use melyxar_core::id::MediaSourceId;
use melyxar_core::media::TrackKind;
use melyxar_core::privacy::MediaName;
use melyxar_core::thumbnails::{Layout, Thumbnails};
use melyxar_core::time::Millis;

use serde::{Deserialize, Serialize};

use crate::{AppError, AppState, Result};

/// What a folder of sheets says about itself.
///
/// Written beside the sheets, and the reason it exists is the cost of what it
/// describes: reading three hundred films takes a night, and without this the
/// only record of that night is a row in a database. Lose the row, by a
/// mistake of mine or a database started again, and every one of those films
/// is read through again for pictures already sitting on the disk.
///
/// So the sheets carry their own description and the table is an index in
/// front of them. A folder that matches the shape asked for is taken up as it
/// stands, and the film is not touched.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
struct WhatIsOnDisk {
    every_ms: i64,
    width: u32,
    height: u32,
    columns: u32,
    rows: u32,
    counted: u32,
    sheets: u32,
}

impl WhatIsOnDisk {
    fn of(made: &Thumbnails) -> Self {
        Self {
            every_ms: made.every.get(),
            width: made.width,
            height: made.height,
            columns: made.columns,
            rows: made.rows,
            counted: made.counted,
            sheets: made.sheets,
        }
    }

    fn read_back(self) -> Thumbnails {
        Thumbnails {
            every: Millis::new(self.every_ms),
            width: self.width,
            height: self.height,
            columns: self.columns,
            rows: self.rows,
            counted: self.counted,
            sheets: self.sheets,
        }
    }
}

/// The name of that description inside a folder of sheets.
const WHAT_IT_IS: &str = "made.json";

/// The shape this server is set to make them in, when it makes them at all.
///
/// Read from the settings rather than from the configuration file, so that
/// changing it is a switch on a screen rather than a terminal, a text editor
/// and a restart. Read each time rather than kept: it changes rarely and it
/// decides what a reading of a whole film produces, so an answer a minute old
/// is an answer that makes the wrong thing.
pub async fn wanted(state: &AppState) -> Option<Layout> {
    let asked = state.database().library_work().await.ok()?;
    if !asked.thumbnails_enabled {
        return None;
    }
    // Every value is brought into a range that can work on its way into the
    // settings, so nothing here has to guard against a nought. Guarded anyway,
    // because a database somebody has edited by hand is still a database.
    Some(Layout {
        every: Millis::new(asked.thumbnails_every_seconds.max(1) * 1_000),
        height: asked.thumbnails_height.max(1) as u32,
        columns: asked.thumbnails_columns.max(1) as u32,
        rows: asked.thumbnails_rows.max(1) as u32,
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

/// What is already in the cache for one film, when it is whole and of the
/// shape asked for.
///
/// Every sheet is looked for, not merely the description: a folder half
/// emptied by hand is worse than an empty one, because it answers every
/// question until somebody drags the cursor into the part that is gone.
async fn already_on_disk(folder: &Path, layout: Layout) -> Option<Thumbnails> {
    let written = tokio::fs::read(folder.join(WHAT_IT_IS)).await.ok()?;
    let said: WhatIsOnDisk = serde_json::from_slice(&written).ok()?;
    let made = said.read_back();
    if made.layout() != layout {
        return None;
    }
    for number in 0..made.sheets {
        let sheet = melyxar_ffmpeg::thumbnails::sheet_at(folder, number);
        if !tokio::fs::try_exists(&sheet).await.unwrap_or(false) {
            return None;
        }
    }
    Some(made)
}

/// Reads one film through and writes down what it gave.
///
/// A film whose sheets are already in the cache, whole and of the shape asked
/// for, is taken up as it stands rather than read again. Reading three hundred
/// films takes a night, and a row lost from the table must never cost that
/// night twice: the sheets carry their own description, and the table is an
/// index in front of them.
///
/// Answers what came out. Nothing counted is an answer too: there are files in
/// a film folder that hold no picture, and it is written down so the file is
/// never read through again for the same nothing.
pub async fn make_for(state: &AppState, source_id: MediaSourceId) -> Result<Thumbnails> {
    let Some(layout) = wanted(state).await else {
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
    let kept = kept_at(state, source_id);
    if let Some(found) = already_on_disk(&kept, layout).await {
        database.store_thumbnails(source_id, &found).await?;
        tracing::info!(
            thumbnails = found.counted,
            sheets = found.sheets,
            "the thumbnails of this film were already in the cache, so it is not read again"
        );
        return Ok(found);
    }

    let source = crate::playable_file(database, source_id).await?;

    // An unconverted frame of a wide gamut film is the washed out thumbnail
    // seen on other servers, so the film is asked what it is.
    let tracks = database.tracks_of_source(source_id).await?;
    let tone_map = tracks.iter().any(|track| match &track.kind {
        TrackKind::Video(details) => details.needs_tone_mapping(),
        _ => false,
    });

    let name = MediaName::of_file(&source.path);
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

    // The sheets describe themselves, so that losing the row in the table
    // costs a moment rather than another night of reading.
    let said = serde_json::to_vec_pretty(&WhatIsOnDisk::of(&made))
        .map_err(|error| AppError::Directory(std::io::Error::other(error)))?;
    tokio::fs::write(aside.join(WHAT_IT_IS), said)
        .await
        .map_err(AppError::Directory)?;

    // Moved into place in one step, once the reading is over.
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
        // cache emptied by hand looks like. The row is what keeps this film out
        // of the pass that makes them, so it goes: without that, the bar of
        // this film stays bare for ever and nothing anywhere says why.
        tracing::warn!(
            sheet = number,
            "a sheet of thumbnails is written down and is not in the cache, so this film \
             will be read for them again"
        );
        state.database().forget_thumbnails(source_id).await?;
        return Err(AppError::Domain(melyxar_core::Error::not_found(
            "that sheet of thumbnails",
        )));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A server whose settings say this about the thumbnails of the bar.
    ///
    /// Written through the settings the way a screen writes them, so what is
    /// read back below is what somebody would really have left behind.
    async fn state_with(asked: melyxar_database::settings::LibraryWork) -> AppState {
        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        database
            .save_library_work(asked)
            .await
            .expect("the settings are written");
        AppState::new(melyxar_config::Config::default(), database, None, None)
    }

    /// What a server nobody has configured asks for.
    async fn as_it_comes() -> melyxar_database::settings::LibraryWork {
        let database = melyxar_database::Database::open_in_memory()
            .await
            .expect("database opens");
        database.library_work().await.expect("read")
    }

    #[tokio::test]
    async fn the_shape_asked_for_is_the_one_the_settings_say() {
        let state = state_with(as_it_comes().await).await;
        assert_eq!(
            wanted(&state).await,
            Some(Layout {
                every: Millis::new(10_000),
                height: 180,
                columns: 10,
                rows: 10,
            })
        );
    }

    /// A folder of sheets as one really sits in the cache.
    async fn a_folder_of_sheets(sheets: u32) -> (tempfile::TempDir, PathBuf, Thumbnails) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let folder = directory.path().join("one");
        std::fs::create_dir_all(&folder).expect("the folder");
        let made = Thumbnails {
            every: Millis::new(10_000),
            width: 320,
            height: 180,
            columns: 10,
            rows: 10,
            counted: sheets * 100,
            sheets,
        };
        for number in 0..sheets {
            std::fs::write(
                melyxar_ffmpeg::thumbnails::sheet_at(&folder, number),
                b"a sheet",
            )
            .expect("a sheet");
        }
        std::fs::write(
            folder.join(WHAT_IT_IS),
            serde_json::to_vec(&WhatIsOnDisk::of(&made)).expect("written"),
        )
        .expect("what it is");
        (directory, folder, made)
    }

    #[tokio::test]
    async fn sheets_already_in_the_cache_are_taken_up_rather_than_read_again() {
        // Reading three hundred films takes a night. A row lost from the table
        // must never cost that night twice, so the sheets describe themselves
        // and the table is an index in front of them.
        let (_directory, folder, made) = a_folder_of_sheets(2).await;
        assert_eq!(already_on_disk(&folder, made.layout()).await, Some(made));
    }

    #[tokio::test]
    async fn sheets_made_to_another_shape_are_not_taken_up() {
        let (_directory, folder, made) = a_folder_of_sheets(2).await;
        let closer = Layout {
            every: Millis::new(5_000),
            ..made.layout()
        };
        assert_eq!(already_on_disk(&folder, closer).await, None);
    }

    #[tokio::test]
    async fn a_folder_missing_a_sheet_is_not_taken_up_at_all() {
        // Worse than an empty one: it answers every question until somebody
        // drags the cursor into the part that is gone.
        let (_directory, folder, made) = a_folder_of_sheets(3).await;
        std::fs::remove_file(melyxar_ffmpeg::thumbnails::sheet_at(&folder, 2))
            .expect("emptied by hand");
        assert_eq!(already_on_disk(&folder, made.layout()).await, None);
    }

    #[tokio::test]
    async fn a_folder_that_says_nothing_about_itself_is_not_taken_up() {
        let (_directory, folder, made) = a_folder_of_sheets(1).await;
        std::fs::remove_file(folder.join(WHAT_IT_IS)).expect("removed");
        assert_eq!(already_on_disk(&folder, made.layout()).await, None);
    }

    #[tokio::test]
    async fn a_server_told_not_to_make_them_makes_none() {
        let state = state_with(melyxar_database::settings::LibraryWork {
            thumbnails_enabled: false,
            ..as_it_comes().await
        })
        .await;
        assert_eq!(wanted(&state).await, None);
    }

    #[tokio::test]
    async fn a_shape_that_cannot_hold_a_thumbnail_never_reaches_a_reading() {
        // Nought seconds apart is a film's worth of pictures at one moment, and
        // a sheet with no rows holds nothing. Neither is worth reading three
        // hundred films for. The settings bring such a number back into range
        // as it is written, so what comes out here is the nearest shape that
        // works rather than nothing at all: the screen that sent it has a
        // defect, and a server that stops making thumbnails until somebody
        // notices is a worse answer than one that makes them a little wrong.
        let state = state_with(melyxar_database::settings::LibraryWork {
            thumbnails_every_seconds: 0,
            thumbnails_columns: 0,
            thumbnails_rows: 0,
            thumbnails_height: 0,
            ..as_it_comes().await
        })
        .await;

        assert_eq!(
            wanted(&state).await,
            Some(Layout {
                every: Millis::new(1_000),
                height: 1,
                columns: 1,
                rows: 1,
            })
        );
    }
}
