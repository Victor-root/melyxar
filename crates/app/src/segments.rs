//! Where a person says the opening and closing titles of a file really are.
//!
//! The file's own chapters and the listening of its season are right most of
//! the time, and a person watching is right every time. What is said here
//! stands in front of both, kind by kind, for the one file it was said about:
//! two copies of the same episode are not cut the same, and a correction made
//! on one would skip the wrong stretch of the other.

use melyxar_core::id::MediaSourceId;
use melyxar_core::segments::{what_is_skipped, MediaSegment, SegmentKind};
use melyxar_core::time::Millis;

use crate::{AppError, AppState, Result};

/// What a person says about one kind of stretch in one file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Correction {
    /// It runs from here to there.
    Stretch { start: Millis, end: Millis },
    /// There is none of it in this file.
    None,
}

/// The stretches a player offers to skip in this file.
pub async fn skipped_in(state: &AppState, source_id: MediaSourceId) -> Result<Vec<MediaSegment>> {
    let said = state.database().segments_of_source(source_id).await?;
    Ok(what_is_skipped(&said))
}

/// Says where one kind of stretch is in one file, or that there is none, and
/// answers what a player now offers to skip in it.
///
/// A stretch runs forwards and ends inside the file: one that ends before it
/// starts, or after the file does, is a slip of the hand rather than anything
/// anybody meant, and is refused rather than kept.
pub async fn correct(
    state: &AppState,
    source_id: MediaSourceId,
    kind: SegmentKind,
    correction: Correction,
) -> Result<Vec<MediaSegment>> {
    let database = state.database();
    let source = database
        .playable_source(source_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("media source")))?;
    let (start, end) = match correction {
        Correction::Stretch { start, end } => {
            let past_the_end = source.duration.is_some_and(|length| end > length);
            if start.get() < 0 || end <= start || past_the_end {
                return Err(AppError::Domain(melyxar_core::Error::invalid_input(
                    "a stretch starts before it ends, and ends inside the file",
                )));
            }
            (start, end)
        }
        Correction::None => (Millis::ZERO, Millis::ZERO),
    };
    database.correct_segment(source_id, kind, start, end).await?;
    #[cfg(debug_assertions)]
    tracing::debug!(
        file = %source.path.display(),
        kind = kind.as_str(),
        start_ms = start.get(),
        end_ms = end.get(),
        "a stretch was corrected by hand"
    );
    skipped_in(state, source_id).await
}

/// Takes back what a person said about one kind of stretch in one file, and
/// answers what a player now offers to skip in it.
pub async fn take_back(
    state: &AppState,
    source_id: MediaSourceId,
    kind: SegmentKind,
) -> Result<Vec<MediaSegment>> {
    let database = state.database();
    database
        .playable_source(source_id)
        .await?
        .ok_or_else(|| AppError::Domain(melyxar_core::Error::not_found("media source")))?;
    database.forget_segment_correction(source_id, kind).await?;
    skipped_in(state, source_id).await
}
