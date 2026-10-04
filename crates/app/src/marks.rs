//! What each account makes of a work: liked, put aside for later, watched,
//! and where it got to.
//!
//! The rules of who may mark which work, and of what a library keeps of a
//! play, live here, so the routes that set a mark and the player reporting
//! where it is answer to the same ones.

use melyxar_core::id::WorkId;
use melyxar_core::library::LibraryOptions;
use melyxar_core::time::{Millis, Timestamp};
use melyxar_core::work::{progress_after, PlaybackState};

use crate::{AppError, AppState, Result};

/// Marks a film as one this viewer likes, or takes the mark off.
///
/// Answers the state it is in now rather than what was asked for, so a button
/// pressed twice in a second cannot end up saying one thing while the server
/// says another.
///
/// Here rather than in the layer above, which used to reach past these use
/// cases straight into the storage: a rule about who may touch which film
/// belongs where every other one is.
pub async fn set_favourite(
    state: &AppState,
    who: &melyxar_core::user::User,
    work_id: WorkId,
    favourite: bool,
) -> Result<bool> {
    crate::reach::may_read_the_work(state, who, work_id).await?;
    Ok(state
        .database()
        .set_favourite(who.id, work_id, favourite)
        .await?)
}

/// Puts a work on this viewer's list of what to watch later, or takes it off.
pub async fn set_watch_later(
    state: &AppState,
    who: &melyxar_core::user::User,
    work_id: WorkId,
    later: bool,
) -> Result<bool> {
    crate::reach::may_read_the_work(state, who, work_id).await?;
    Ok(state
        .database()
        .set_watch_later(who.id, work_id, later)
        .await?)
}

/// Marks a work watched by hand, or puts it back to unwatched.
///
/// A season and a series answer for their episodes, which is the storage's
/// affair; what belongs here is that a work this account cannot reach is a
/// work it cannot mark, and that a name meaning nothing is a name meaning
/// nothing rather than a silent success.
pub async fn mark_watched(
    state: &AppState,
    who: &melyxar_core::user::User,
    work_id: WorkId,
    watched: bool,
) -> Result<bool> {
    crate::reach::may_read_the_work(state, who, work_id).await?;
    let keeps_watched_marks = state
        .database()
        .library_of_work(work_id)
        .await?
        .is_none_or(|(_, options)| options.keeps_watched_marks);
    if !keeps_watched_marks {
        return Err(AppError::Domain(melyxar_core::Error::invalid_input(
            "the library of this work keeps no watched marks",
        )));
    }
    let database = state.database();
    let written = database.mark_watched(who.id, work_id, watched).await?;
    if written == 0 {
        return Err(AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::NotFound,
            "no work with that identifier",
        )));
    }
    // Watched is what later was waiting for.
    if watched {
        database.set_watch_later(who.id, work_id, false).await?;
    }
    Ok(watched)
}

/// Records where a viewer got to.
///
/// Answers whether the report was kept: one arriving after a fresher one is
/// refused, so a client that reconnects cannot make the resume point go
/// backwards.
pub async fn record_position(
    state: &AppState,
    who: &melyxar_core::user::User,
    work_id: WorkId,
    position: Millis,
    reported_at: Timestamp,
) -> Result<bool> {
    crate::reach::may_read_the_work(state, who, work_id).await?;
    let user_id = who.id;
    let database = state.database();
    let duration = longest_version(state, work_id).await?;

    let marked_manually = database
        .playback_progress(user_id, work_id)
        .await?
        .is_some_and(|progress| progress.marked_manually);

    // Held to the rules of this viewer for this kind of library, which say
    // when a film only glanced at starts again from the beginning and when
    // one nearly finished counts as watched, and then to what that library
    // keeps of a play at all.
    let (rules, options) = match database.library_of_work(work_id).await? {
        Some((kind, options)) => (who.preferences.resume_rules_for(kind), options),
        None => (who.preferences.resume_rules, LibraryOptions::default()),
    };
    // Played through takes it off the list of what to watch later, as the
    // play says and whatever a mark by hand says: a film marked watched and
    // put back on the list to see again stays there until it is seen again.
    let (played, _) = progress_after(position, duration, rules, false);
    if played == PlaybackState::Watched {
        database.set_watch_later(user_id, work_id, false).await?;
    }
    let progress = progress_after(position, duration, rules, marked_manually);
    let Some((state_now, kept)) = options.kept_of(progress) else {
        return Ok(false);
    };

    Ok(database
        .record_playback_progress(user_id, work_id, kept, state_now, reported_at)
        .await?)
}

/// How long the work runs, from the longest copy of it that was analysed.
///
/// The runtime a provider gave describes the film; what decides whether
/// someone reached the end is the file they are actually watching.
pub(crate) async fn longest_version(state: &AppState, work_id: WorkId) -> Result<Option<Millis>> {
    let database = state.database();
    let mut longest = None;
    for source in database.sources_of_work(work_id).await? {
        if let Some((analysis, _)) = database.source_details(source.id).await? {
            longest = match (longest, analysis.duration) {
                (Some(known), Some(found)) if found > known => Some(found),
                (None, found) => found,
                (known, _) => known,
            };
        }
    }
    Ok(longest)
}
