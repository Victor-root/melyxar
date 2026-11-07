//! What the libraries still taking in keep out of sight, and when they let go.
//!
//! A library that asked for it shows what arrives in it only once it is ready
//! to be played. It holds anything back only while it is still taking in:
//! from the moment a scan of it starts until the last reading that follows an
//! arrival there is over. What is ready and what is new is the database's to
//! say; this says which libraries are asking, request by request, and writes
//! down when one has settled.
//!
//! Asked again for every request rather than kept: it is nothing at all
//! almost always, and what it costs then is one look at what is running.

use std::collections::HashMap;
use std::sync::Mutex;

use melyxar_core::id::LibraryId;
use melyxar_core::library::Library;
use melyxar_database::held_back::HeldBack;

use crate::{AppState, Result};

/// The libraries a scan and what follows it are under way in, however many
/// times over.
///
/// What is running is otherwise read off the jobs, and a scan, its look up and
/// its readings are three jobs started one after the other: between two of
/// them none is running, and a library read at that instant would let go of
/// everything it holds back before it is ready.
#[derive(Default)]
pub struct Intakes {
    under_way: Mutex<HashMap<LibraryId, usize>>,
}

impl Intakes {
    fn under_way(&self) -> std::sync::MutexGuard<'_, HashMap<LibraryId, usize>> {
        self.under_way
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub(crate) fn holds(&self, library: LibraryId) -> bool {
        self.under_way().contains_key(&library)
    }
}

/// One scan and what follows it, under way in a library until dropped.
pub(crate) struct Intake {
    state: AppState,
    library: LibraryId,
}

impl Intake {
    pub(crate) fn begins(state: &AppState, library: LibraryId) -> Self {
        *state.intakes().under_way().entry(library).or_insert(0) += 1;
        Self {
            state: state.clone(),
            library,
        }
    }
}

impl Drop for Intake {
    fn drop(&mut self) {
        let mut under_way = self.state.intakes().under_way();
        if let Some(count) = under_way.get_mut(&self.library) {
            *count -= 1;
            if *count == 0 {
                under_way.remove(&self.library);
            }
        }
    }
}

/// What is held back from a viewer right now.
///
/// A library holds anything back only when it asked to and is still taking in
/// what arrived. A work also waits for its name only where a provider can be
/// asked for one: without a key, nothing would ever be named.
pub async fn now(state: &AppState) -> Result<HeldBack> {
    let database = state.database();
    let unfinished = database.unfinished_jobs().await?;
    if unfinished.is_empty() && state.intakes().under_way().is_empty() {
        return Ok(HeldBack::nothing());
    }
    let holding = database
        .list_libraries()
        .await?
        .into_iter()
        .filter(|library| {
            library.holds_back_until_ready()
                && crate::upkeep::takes_in(state, library, &unfinished)
        })
        .map(|library| library.id)
        .collect();
    Ok(HeldBack::by(holding, state.metadata_provider().is_some()))
}

/// Refuses a copy held back from being played, the way a page would refuse
/// to show it: it is not there yet.
pub(crate) async fn may_play(
    state: &AppState,
    source_id: melyxar_core::id::MediaSourceId,
) -> Result<()> {
    let held = now(state).await?;
    match state.database().is_held_back(source_id, &held).await? {
        true => Err(crate::AppError::Domain(melyxar_core::Error::new(
            melyxar_core::error::ErrorCode::NotReady,
            "this copy has just arrived and is not ready to be played yet",
        ))),
        false => Ok(()),
    }
}

/// Writes down that a library has settled, once it has nothing left to take
/// in and something arrived since it last did.
///
/// What arrived before this instant has been shown, and is never held back
/// again. The pages are told when anything held back comes into sight.
///
/// The instant is read before asking what is running: a scan starting just
/// after the question has already said so, and one starting just before it
/// keeps the library from settling at all.
async fn settle_now(state: &AppState, library: &Library) -> Result<()> {
    let at = melyxar_core::time::now();
    let database = state.database();
    if !library.kind.holds_videos()
        || crate::upkeep::is_still_taking_in(state, library).await?
        || !database.arrived_since_settled(library.id).await?
    {
        return Ok(());
    }
    database.settle_library(library.id, at).await?;
    if library.holds_back_until_ready() {
        database.bump_library_version(library.id).await?;
    }
    Ok(())
}

/// Settles a library once what followed an arrival in it is over.
pub(crate) async fn settle(state: &AppState, library: &Library) {
    if let Err(error) = settle_now(state, library).await {
        tracing::warn!(library = library.name, %error, "the library could not be settled");
    }
}

/// Settles every library that has stopped taking in, whatever started what it
/// took in: a reading taken up again after a restart is not followed by
/// anything that would settle it.
pub async fn settle_every_library(state: &AppState) -> Result<()> {
    for library in state.database().list_libraries().await? {
        settle_now(state, &library).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::error::ErrorCode;
    use melyxar_core::id::{MediaSourceId, WorkId};
    use melyxar_core::job::{JobKind, JobPriority};
    use melyxar_core::library::{LibraryKind, LibraryOptions};
    use melyxar_core::work::WorkKind;
    use melyxar_database::browse::BrowseRequest;
    use std::path::{Path, PathBuf};

    async fn a_library(state: &AppState, kind: LibraryKind) -> Library {
        state
            .database()
            .create_library(
                kind.as_str(),
                kind,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created")
    }

    /// A film with one copy on the disk, nothing read out of it yet.
    async fn a_film(state: &AppState, library: &Library, title: &str) -> (WorkId, MediaSourceId) {
        let database = state.database();
        let work = database
            .create_work(library.id, WorkKind::Movie, title, &title.to_lowercase(), Some(2020))
            .await
            .expect("work created");
        let copy = database
            .insert_source(work.id, library.roots[0].id, Path::new(title), 1_000, melyxar_core::time::now())
            .await
            .expect("copy recorded");
        (work.id, copy)
    }

    async fn found(state: &AppState, who: &melyxar_core::user::User, title: &str) -> usize {
        crate::catalogue::browse(
            state,
            &BrowseRequest {
                search: Some(title.to_string()),
                ..Default::default()
            },
            who,
        )
        .await
        .expect("grid read")
        .cards
        .len()
    }

    #[tokio::test]
    async fn a_library_holds_back_only_while_it_takes_in_and_only_if_it_asked() {
        let (_held, state) = crate::an_empty_server().await;
        let films = a_library(&state, LibraryKind::Movies).await;
        let music = a_library(&state, LibraryKind::Music).await;
        assert!(!now(&state).await.expect("read").holds_anything(), "nothing is taking anything in");

        let database = state.database();
        let elsewhere = database
            .create_job(JobKind::FetchRatings, JobPriority::BACKGROUND, Some(&films.id.to_string()))
            .await
            .expect("job written");
        assert!(
            !now(&state).await.expect("read").holds_anything(),
            "ratings are not what a film waits for"
        );
        database
            .finish_job(elsewhere.id, melyxar_core::job::JobState::Succeeded, None)
            .await
            .expect("job over");

        let scan = database
            .create_job(JobKind::ScanLibrary, JobPriority::BACKGROUND, Some(&films.id.to_string()))
            .await
            .expect("job written");
        assert_eq!(now(&state).await.expect("read").libraries(), [films.id]);
        database
            .finish_job(scan.id, melyxar_core::job::JobState::Succeeded, None)
            .await
            .expect("job over");
        assert!(!now(&state).await.expect("read").holds_anything());

        let between_two_jobs = Intake::begins(&state, films.id);
        let of_music = Intake::begins(&state, music.id);
        assert_eq!(
            now(&state).await.expect("read").libraries(),
            [films.id],
            "a scan and what follows it count until the end, and music waits for nothing"
        );
        database
            .set_library_options(
                films.id,
                LibraryOptions {
                    holds_back_until_ready: false,
                    ..LibraryOptions::default()
                },
            )
            .await
            .expect("switched off");
        assert!(!now(&state).await.expect("read").holds_anything(), "switched off, nothing waits");
        drop((between_two_jobs, of_music));
        assert!(!state.intakes().holds(films.id));
    }

    #[tokio::test]
    async fn a_new_film_comes_into_sight_once_its_library_has_taken_it_in() {
        let (_held, state) = crate::an_empty_server().await;
        let who = crate::a_viewer(&state).await;
        let library = a_library(&state, LibraryKind::Movies).await;
        let database = state.database();
        database
            .settle_library(library.id, melyxar_core::time::now())
            .await
            .expect("settled");

        let intake = Intake::begins(&state, library.id);
        // Never read, never named: the worst a file can do.
        let (film, copy) = a_film(&state, &library, "Quiet Shore").await;
        database.bump_library_version(library.id).await.expect("moved");

        assert_eq!(found(&state, &who, "shore").await, 0);
        let home = crate::catalogue::home(&state, None, &who).await.expect("home read");
        assert!(home.recently_added.cards.is_empty());
        assert_eq!(home.works, 0, "not counted either");
        let refused = crate::playback::file_of(&state, &who, copy).await.expect_err("refused");
        assert!(
            matches!(&refused, crate::AppError::Domain(error) if error.code == ErrorCode::NotReady),
            "{refused:?}"
        );
        assert!(
            crate::detail::work_detail(&state, &who, film).await.expect("read").is_none(),
            "its page is not there yet"
        );

        let version = database.library_version(library.id).await.expect("read");
        drop(intake);
        settle(&state, &library).await;
        assert!(
            database.library_version(library.id).await.expect("read") > version,
            "the pages are told"
        );
        assert!(!database.arrived_since_settled(library.id).await.expect("read"));
        assert_eq!(found(&state, &who, "shore").await, 1, "shown once the library is done, ready or not");
        let home = crate::catalogue::home(&state, None, &who).await.expect("home read");
        assert_eq!(home.recently_added.cards.len(), 1);
        assert_eq!(home.works, 1);
        assert!(crate::playback::file_of(&state, &who, copy).await.is_ok());

        let settled_version = database.library_version(library.id).await.expect("read");
        settle_every_library(&state).await.expect("settled");
        assert_eq!(
            database.library_version(library.id).await.expect("read"),
            settled_version,
            "nothing arrived since, so nothing is said again"
        );
    }
}
