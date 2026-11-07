//! What a library still taking in keeps out of sight.
//!
//! A library can ask for what arrives in it to be shown only once it is ready
//! to be played: read by the analyser, read for where a jump can land, and
//! named by the provider when one can be asked. Played any earlier, a film
//! plays badly in ways that look like the server going wrong.
//!
//! Which libraries are holding anything back right now is the application's
//! to say, request by request, and it says so only while one is still taking
//! in what arrived: what can never be made ready is shown once that is over,
//! rather than kept out of sight for ever. What is ready, and what is new, is
//! said here once, for every listing and every play button to agree on.
//!
//! New is what arrived after the library last settled, which is the moment it
//! last had nothing left to take in. Everything before it has been shown
//! already and is never held back again: a second copy of a film somebody has
//! been watching for a year does not take the film away.

use melyxar_core::id::{LibraryId, MediaSourceId};
use melyxar_core::time::Timestamp;

use crate::convert::timestamp_to_text;
use crate::{Database, Result};

/// The libraries holding back what is not ready, as one request reads them.
///
/// Nothing at all almost always: a library only holds anything back while it
/// is taking in what arrived, and a request asked while none is costs no more
/// than it did before this existed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeldBack {
    libraries: Vec<LibraryId>,
    until_named: bool,
}

impl HeldBack {
    /// What these libraries hold back. A work also waits for its name when
    /// `until_named`, which is only worth asking where a provider can be asked:
    /// without one, nothing would ever be named.
    pub fn by(libraries: Vec<LibraryId>, until_named: bool) -> Self {
        Self {
            libraries,
            until_named,
        }
    }

    /// Nothing held back anywhere, which is what everything working on the
    /// files rather than for a viewer reads with.
    pub fn nothing() -> Self {
        Self::default()
    }

    pub fn holds_anything(&self) -> bool {
        !self.libraries.is_empty()
    }

    pub fn libraries(&self) -> &[LibraryId] {
        &self.libraries
    }

    /// The clause keeping out of a listing the works held back, about the
    /// works named `work`, to bind with [`HeldBack::bound`].
    ///
    /// Nothing at all when nothing is held back, and then nothing to bind.
    pub(crate) fn work_shown(&self, work: &str) -> String {
        self.work_shown_at(work, "?")
    }

    /// The same, for a statement whose places are numbered.
    pub(crate) fn work_shown_at(&self, work: &str, place: &str) -> String {
        if !self.holds_anything() {
            return String::new();
        }
        let new = format!("{work}.added_at > {}", settled(work));
        let episode_seen = leaf_seen("held_episode", "held_episode_copy", self.until_named);
        // A film, an episode or a video stands for itself. A series or a season
        // shows as soon as one episode under it does. A folder is a place
        // rather than something to play, and is never held: what it holds is.
        format!(
            " AND NOT ({work}.library_id IN (SELECT value FROM json_each({place}))
                  AND {new}
                  AND CASE {work}.kind
                        WHEN 'series' THEN NOT EXISTS (
                            SELECT 1 FROM works held_episode
                             WHERE held_episode.kind = 'episode'
                               AND held_episode.parent_id IN
                                   (SELECT {work}.id
                                    UNION ALL
                                    SELECT held_season.id FROM works held_season
                                     WHERE held_season.parent_id = {work}.id)
                               AND {episode_seen})
                        WHEN 'season' THEN NOT EXISTS (
                            SELECT 1 FROM works held_episode
                             WHERE held_episode.kind = 'episode'
                               AND held_episode.parent_id = {work}.id
                               AND {episode_seen})
                        WHEN 'folder' THEN 0
                        ELSE NOT {}
                      END)",
            a_copy_seen(work, "held_copy", self.until_named)
        )
    }

    /// The clause keeping a copy held back from being chosen to play, about
    /// the copies named `copy`, to bind with [`HeldBack::bound`].
    pub(crate) fn copy_shown(&self, copy: &str) -> String {
        self.copy_shown_at(copy, "?")
    }

    /// The same, for a statement whose places are numbered.
    pub(crate) fn copy_shown_at(&self, copy: &str, place: &str) -> String {
        if !self.holds_anything() {
            return String::new();
        }
        format!(" AND NOT {}", copy_held(copy, self.until_named, place))
    }

    /// The one value the clauses above are bound to, when there is one.
    pub(crate) fn bound(&self) -> Option<String> {
        self.holds_anything().then(|| {
            serde_json::Value::from(
                self.libraries
                    .iter()
                    .map(|id| id.to_db_string())
                    .collect::<Vec<_>>(),
            )
            .to_string()
        })
    }
}

/// When the library of the works named `work` last settled. Nothing settled
/// yet reads as before everything, so a first import is new from end to end.
fn settled(work: &str) -> String {
    format!("(SELECT coalesce(settled_at, '') FROM libraries WHERE id = {work}.library_id)")
}

/// Whether the work named `work` has its answer from the provider: its own,
/// or for an episode its series', which is what describes it. An episode
/// under no series is never looked up, and is taken as answered.
fn named(work: &str) -> String {
    format!(
        "(CASE WHEN {work}.kind = 'episode'
               THEN coalesce((SELECT held_series.identification
                                FROM works held_parent
                                JOIN works held_series
                                  ON held_series.kind = 'series'
                                 AND held_series.id = CASE held_parent.kind
                                                          WHEN 'season' THEN held_parent.parent_id
                                                          ELSE held_parent.id END
                               WHERE held_parent.id = {work}.parent_id), 'answered')
               ELSE {work}.identification END) <> 'pending'"
    )
}

/// Whether the copy named `copy` of the work named `work` is ready to play.
///
/// Read, or found unreadable; read for where a jump can land, unless it is a
/// photo or could not be read at all, since neither is ever read for that;
/// and named, when naming is waited for.
fn ready(copy: &str, work: &str, until_named: bool) -> String {
    let named = match until_named {
        true => format!(" AND {}", named(work)),
        false => String::new(),
    };
    format!(
        "(({copy}.analysed_at IS NOT NULL OR {copy}.analysis_failure IS NOT NULL)
          AND ({copy}.analysed_at IS NULL OR {work}.kind = 'photo'
               OR EXISTS (SELECT 1 FROM media_source_key_frames held_frames
                           WHERE held_frames.source_id = {copy}.id)){named})"
    )
}

/// Whether the copy named `copy` of the work named `work` may be seen: it was
/// there before the library last settled, or it is ready.
fn seen(copy: &str, work: &str, until_named: bool) -> String {
    format!(
        "({copy}.added_at <= {} OR {})",
        settled(work),
        ready(copy, work, until_named)
    )
}

/// Whether the work named `work` has a copy on the disk that may be seen,
/// asked through copies named `copy`.
fn a_copy_seen(work: &str, copy: &str, until_named: bool) -> String {
    format!(
        "EXISTS (SELECT 1 FROM media_sources {copy}
                  WHERE {copy}.work_id = {work}.id AND {copy}.missing_since IS NULL
                    AND {})",
        seen(copy, work, until_named)
    )
}

/// Whether the work named `work`, a film, an episode or a video, may be seen:
/// it was there before the library last settled, or a copy of it may be.
fn leaf_seen(work: &str, copy: &str, until_named: bool) -> String {
    format!(
        "({work}.added_at <= {} OR {})",
        settled(work),
        a_copy_seen(work, copy, until_named)
    )
}

/// Whether the copy named `copy` is held back, binding the libraries holding
/// anything back to one place.
fn copy_held(copy: &str, until_named: bool, place: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM works held_work
                  WHERE held_work.id = {copy}.work_id
                    AND held_work.library_id IN (SELECT value FROM json_each({place}))
                    AND NOT {})",
        seen(copy, "held_work", until_named)
    )
}

impl Database {
    /// Whether this copy is held back from being played.
    pub async fn is_held_back(&self, source_id: MediaSourceId, held: &HeldBack) -> Result<bool> {
        let Some(bound) = held.bound() else {
            return Ok(false);
        };
        let row: Option<(bool,)> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {} FROM media_sources s WHERE s.id = ?",
            copy_held("s", held.until_named, "?")
        )))
        .bind(bound)
        .bind(source_id.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        Ok(row.is_some_and(|(held,)| held))
    }

    /// Whether anything arrived in this library since it last settled: a
    /// work, or a copy of one already there.
    pub async fn arrived_since_settled(&self, library_id: LibraryId) -> Result<bool> {
        let arrived: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM works w
                             WHERE w.library_id = ?1
                               AND w.added_at > (SELECT coalesce(settled_at, '')
                                                   FROM libraries WHERE id = ?1))
                 OR EXISTS (SELECT 1 FROM media_sources s
                              JOIN library_roots r ON r.id = s.root_id
                             WHERE r.library_id = ?1
                               AND s.added_at > (SELECT coalesce(settled_at, '')
                                                   FROM libraries WHERE id = ?1))",
        )
        .bind(library_id.to_db_string())
        .fetch_one(self.reader())
        .await?;
        Ok(arrived)
    }

    /// Writes down that this library had nothing left to take in at this
    /// instant: everything that arrived before it has been shown.
    pub async fn settle_library(&self, library_id: LibraryId, at: Timestamp) -> Result<()> {
        sqlx::query("UPDATE libraries SET settled_at = ? WHERE id = ?")
            .bind(timestamp_to_text(at))
            .bind(library_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::browse::BrowseRequest;
    use melyxar_core::id::{LibraryRootId, UserId, WorkId};
    use melyxar_core::library::LibraryKind;
    use melyxar_core::time::{now, Millis};
    use melyxar_core::work::WorkKind;
    use std::path::{Path, PathBuf};

    /// A library of one kind, settled before anything in it arrived.
    async fn a_library(kind: LibraryKind) -> (Database, LibraryId, LibraryRootId, UserId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Films",
                kind,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let viewer = database
            .create_user("Viewer", None, &melyxar_core::user::Permissions::viewer())
            .await
            .expect("account created")
            .id;
        (database, library.id, library.roots[0].id, viewer)
    }

    async fn a_work(
        database: &Database,
        library: LibraryId,
        kind: WorkKind,
        title: &str,
        parent: Option<WorkId>,
    ) -> WorkId {
        let work = database
            .create_work(library, kind, title, &title.to_lowercase(), Some(2020))
            .await
            .expect("work created");
        sqlx::query("UPDATE works SET parent_id = ? WHERE id = ?")
            .bind(parent.map(|parent| parent.to_db_string()))
            .bind(work.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("placed");
        work.id
    }

    async fn a_copy(database: &Database, root: LibraryRootId, work: WorkId, name: &str) -> MediaSourceId {
        database
            .insert_source(work, root, Path::new(name), 1_000, now())
            .await
            .expect("copy recorded")
    }

    async fn analysed(database: &Database, copy: MediaSourceId) {
        sqlx::query("UPDATE media_sources SET analysed_at = ? WHERE id = ?")
            .bind(timestamp_to_text(now()))
            .bind(copy.to_db_string())
            .execute(database.writer())
            .await
            .expect("analysed");
    }

    async fn named(database: &Database, work: WorkId, identification: &str) {
        sqlx::query("UPDATE works SET identification = ? WHERE id = ?")
            .bind(identification)
            .bind(work.to_db_string())
            .execute(database.writer())
            .await
            .expect("named");
    }

    /// Analysed, read for where a jump can land, and named.
    async fn made_ready(database: &Database, work: WorkId, copy: MediaSourceId) {
        analysed(database, copy).await;
        database
            .store_key_frames(copy, &[Millis::ZERO])
            .await
            .expect("key frames kept");
        named(database, work, "identified").await;
    }

    async fn titles(database: &Database, held: &HeldBack, search: Option<&str>) -> Vec<String> {
        database
            .browse_works(&BrowseRequest {
                held_back: held.clone(),
                search: search.map(str::to_string),
                ..Default::default()
            })
            .await
            .expect("grid read")
            .cards
            .into_iter()
            .map(|card| card.title)
            .collect()
    }

    #[tokio::test]
    async fn a_new_film_is_kept_out_of_every_listing_until_it_is_ready() {
        let (database, library, root, viewer) = a_library(LibraryKind::Movies).await;
        let old = a_work(&database, library, WorkKind::Movie, "Quiet Harbour", None).await;
        let old_copy = a_copy(&database, root, old, "quiet.mkv").await;
        made_ready(&database, old, old_copy).await;
        database.settle_library(library, now()).await.expect("settled");
        let new = a_work(&database, library, WorkKind::Movie, "Quiet Shore", None).await;
        let copy = a_copy(&database, root, new, "shore.mkv").await;

        let held = HeldBack::by(vec![library], true);
        let nothing = HeldBack::nothing();
        assert_eq!(titles(&database, &held, None).await, ["Quiet Harbour"]);
        assert_eq!(titles(&database, &held, Some("shore")).await, Vec::<String>::new());
        assert_eq!(database.count_browsable(Some(library), &held).await.expect("counted"), 1);
        assert_eq!(database.count_browsable(None, &held).await.expect("counted"), 1);
        assert_eq!(
            database.initials_in_use(Some(library), &held).await.expect("counted"),
            [("q".to_string(), 1)]
        );
        assert_eq!(
            database.decades_in_use(Some(library), &held).await.expect("counted"),
            database.decades_in_use(None, &held).await.expect("counted")
        );
        assert!(database.card_of(viewer, new, &held).await.expect("read").is_none());
        assert!(database.is_held_back(copy, &held).await.expect("asked"));
        assert_eq!(
            titles(&database, &nothing, None).await,
            ["Quiet Harbour", "Quiet Shore"],
            "a library not taking in anything holds nothing back"
        );
        assert_eq!(
            titles(&database, &HeldBack::by(vec![LibraryId::new()], true), None).await.len(),
            2,
            "only the libraries holding back keep anything out"
        );

        analysed(&database, copy).await;
        assert_eq!(titles(&database, &held, None).await.len(), 1, "read, not yet for its key frames");
        database.store_key_frames(copy, &[]).await.expect("none found, which is an answer");
        assert_eq!(titles(&database, &held, None).await.len(), 1, "still waiting for its name");
        assert_eq!(
            titles(&database, &HeldBack::by(vec![library], false), None).await.len(),
            2,
            "a name is only waited for where one can be asked"
        );
        named(&database, new, "unidentified").await;
        assert_eq!(titles(&database, &held, None).await.len(), 2, "a provider that found nothing has answered");
        assert!(database.card_of(viewer, new, &held).await.expect("read").is_some());
        assert!(!database.is_held_back(copy, &held).await.expect("asked"));
    }

    #[tokio::test]
    async fn a_file_nothing_can_read_waits_for_nothing_but_its_name() {
        let (database, library, root, _) = a_library(LibraryKind::Movies).await;
        let film = a_work(&database, library, WorkKind::Movie, "Broken Reel", None).await;
        let copy = a_copy(&database, root, film, "broken.mkv").await;
        let held = HeldBack::by(vec![library], false);
        assert!(titles(&database, &held, None).await.is_empty());

        database
            .record_analysis_failure(copy, "unreadable")
            .await
            .expect("failure kept");
        assert_eq!(
            titles(&database, &held, None).await,
            ["Broken Reel"],
            "no key frame is ever read out of a file nothing could read"
        );
    }

    #[tokio::test]
    async fn a_new_copy_of_a_film_already_there_does_not_hide_it() {
        let (database, library, root, viewer) = a_library(LibraryKind::Movies).await;
        let film = a_work(&database, library, WorkKind::Movie, "Quiet Harbour", None).await;
        let old = a_copy(&database, root, film, "quiet.mkv").await;
        made_ready(&database, film, old).await;
        database.settle_library(library, now()).await.expect("settled");
        let bigger = a_copy(&database, root, film, "quiet.2160p.mkv").await;
        sqlx::query("UPDATE media_sources SET size_bytes = 9000 WHERE id = ?")
            .bind(bigger.to_db_string())
            .execute(database.writer())
            .await
            .expect("bigger");

        let held = HeldBack::by(vec![library], true);
        assert_eq!(titles(&database, &held, None).await, ["Quiet Harbour"]);
        assert!(database.is_held_back(bigger, &held).await.expect("asked"));
        assert!(!database.is_held_back(old, &held).await.expect("asked"));
        let card = database.card_of(viewer, film, &held).await.expect("read").expect("shown");
        assert_eq!(
            card.state.and_then(|state| state.source_id),
            Some(old),
            "the play button starts the copy that is ready"
        );
        let card = database
            .card_of(viewer, film, &HeldBack::nothing())
            .await
            .expect("read")
            .expect("shown");
        assert_eq!(
            card.state.and_then(|state| state.source_id),
            Some(bigger),
            "and the biggest once nothing is held back"
        );
    }

    #[tokio::test]
    async fn a_series_shows_as_soon_as_one_of_its_episodes_is_ready() {
        let (database, library, root, viewer) = a_library(LibraryKind::Series).await;
        let series = a_work(&database, library, WorkKind::Series, "Night Shift", None).await;
        let season = a_work(&database, library, WorkKind::Season, "Season 1", Some(series)).await;
        let first = a_work(&database, library, WorkKind::Episode, "Pilot", Some(season)).await;
        let second = a_work(&database, library, WorkKind::Episode, "Second", Some(season)).await;
        for (work, ordinal) in [(season, 1), (first, 1), (second, 2)] {
            sqlx::query("UPDATE works SET ordinal = ? WHERE id = ?")
                .bind(ordinal)
                .bind(work.to_db_string())
                .execute(database.writer())
                .await
                .expect("numbered");
        }
        let first_copy = a_copy(&database, root, first, "s01e01.mkv").await;
        let second_copy = a_copy(&database, root, second, "s01e02.mkv").await;
        for copy in [first_copy, second_copy] {
            analysed(&database, copy).await;
            database.store_key_frames(copy, &[Millis::ZERO]).await.expect("kept");
        }

        let held = HeldBack::by(vec![library], true);
        let shown_under = |parent| {
            let database = database.clone();
            let held = held.clone();
            async move {
                database
                    .children_of(viewer, parent, "fr", &held)
                    .await
                    .expect("read")
                    .into_iter()
                    .map(|child| child.card.id)
                    .collect::<Vec<_>>()
            }
        };
        assert!(titles(&database, &held, None).await.is_empty(), "its episodes wait for the series to be named");
        assert!(shown_under(series).await.is_empty());

        named(&database, series, "identified").await;
        assert_eq!(titles(&database, &held, None).await, ["Night Shift"]);
        assert_eq!(shown_under(series).await, [season]);
        assert_eq!(shown_under(season).await, [first, second], "an episode is named through its series");

        sqlx::query("DELETE FROM media_source_key_frames WHERE source_id = ?")
            .bind(second_copy.to_db_string())
            .execute(database.writer())
            .await
            .expect("forgotten");
        assert_eq!(shown_under(season).await, [first], "one episode is enough for the series");
        assert_eq!(titles(&database, &held, None).await, ["Night Shift"]);
        assert_eq!(
            database
                .next_episode_after(viewer, first, &held)
                .await
                .expect("read"),
            None,
            "nothing plays on into an episode held back"
        );
        assert_eq!(
            database
                .previous_episode_before(second, &held)
                .await
                .expect("read")
                .map(|episode| episode.id),
            Some(first)
        );

        database.mark_watched(viewer, first, true).await.expect("marked");
        assert_eq!(database.where_to_resume(viewer, series, &held).await.expect("read"), None);
        assert!(database.up_next(viewer, None, &held, 10).await.expect("read").is_empty());
        let waiting = database
            .up_next(viewer, None, &HeldBack::nothing(), 10)
            .await
            .expect("read");
        assert_eq!(waiting.iter().map(|up| up.card.id).collect::<Vec<_>>(), [second]);
    }

    #[tokio::test]
    async fn a_photo_waits_for_nothing_but_being_read() {
        let (database, library, root, _) = a_library(LibraryKind::HomeMedia).await;
        let folder = database
            .own_folder(library, None, "Summer", "summer")
            .await
            .expect("folder");
        let mut photos = Vec::new();
        for name in ["a", "b"] {
            let photo = database
                .create_own_work(library, Some(folder.id), WorkKind::Photo, name, name)
                .await
                .expect("photo");
            let copy = a_copy(&database, root, photo.id, name).await;
            photos.push((photo, copy));
        }
        let held = HeldBack::by(vec![library], true);
        let [(first, first_copy), (second, second_copy)] = &photos[..] else {
            unreachable!("two photos were made");
        };
        analysed(&database, *first_copy).await;
        assert_eq!(
            database.neighbouring_photos(first, &held).await.expect("read"),
            (None, None),
            "the one next to it is not read yet"
        );
        analysed(&database, *second_copy).await;
        assert_eq!(
            database.neighbouring_photos(first, &held).await.expect("read"),
            (None, Some(second.id)),
            "a photo has no key frame to wait for"
        );
        assert_eq!(
            titles(&database, &held, None).await,
            ["Summer"],
            "a folder is a place, never held back itself"
        );
    }

    /// What the provider says of a film: a genre, a saga, one actor in a part
    /// and a rating.
    fn described(title: &str) -> crate::metadata::IdentifiedWork {
        crate::metadata::IdentifiedWork {
            provider: "tmdb".to_string(),
            external_id: format!("film-{title}"),
            imdb_id: None,
            language: "fr".to_string(),
            title: title.to_string(),
            sort_title: title.to_lowercase(),
            tagline: None,
            overview: None,
            release_year: Some(2020),
            release_date: None,
            end_date: None,
            runtime: None,
            community_rating: Some(7.5),
            age_rating_label: None,
            genres: vec!["Drama".to_string()],
            studios: Vec::new(),
            credits: vec![crate::metadata::CreditRecord {
                external_id: "person-ada".to_string(),
                name: "Ada Vane".to_string(),
                sort_name: "ada vane".to_string(),
                role: "actor".to_string(),
                character: Some("The keeper".to_string()),
                ordinal: 0,
                photo_path: None,
            }],
            collection: Some(crate::metadata::CollectionRecord {
                external_id: "saga-quiet".to_string(),
                name: "The Quiet Saga".to_string(),
                sort_name: "quiet saga".to_string(),
            }),
            trailers: Vec::new(),
        }
    }

    #[tokio::test]
    async fn every_row_a_viewer_meets_leaves_out_what_is_held_back() {
        let (database, library, root, viewer) = a_library(LibraryKind::Movies).await;
        let mut films = Vec::new();
        let mut person = None;
        for title in ["Quiet Harbour", "Quiet Shore"] {
            let film = a_work(&database, library, WorkKind::Movie, title, None).await;
            let copy = a_copy(&database, root, film, title).await;
            let credited = database
                .apply_identification(film, &described(title), false)
                .await
                .expect("described");
            person = credited.first().map(|credited| credited.person_id);
            films.push((film, copy));
            if films.len() == 1 {
                made_ready(&database, film, copy).await;
                database.settle_library(library, now()).await.expect("settled");
            }
        }
        let person = person.expect("an actor");
        let [(old, _), (new, _)] = films[..] else {
            unreachable!("two films were made");
        };
        let held = HeldBack::by(vec![library], true);
        let nothing = HeldBack::nothing();
        let ids = |cards: Vec<crate::browse::WorkCard>| cards.into_iter().map(|card| card.id).collect::<Vec<_>>();

        for film in [old, new] {
            database.pin_work(film, 5).await.expect("pinned");
        }
        assert_eq!(ids(database.pinned_works(viewer, None, &held, 5).await.expect("read")), [old]);
        assert_eq!(ids(database.suggestions(viewer, None, &held, 5).await.expect("read")), [old]);
        assert_eq!(database.suggestions(viewer, None, &nothing, 5).await.expect("read").len(), 2);
        assert!(database.works_at_random(viewer, None, &held, 5).await.expect("read").is_empty());
        assert!(database.alike_by_genre(viewer, old, None, &held, 5).await.expect("read").is_none());
        assert!(database.alike_by_genre(viewer, old, None, &nothing, 5).await.expect("read").is_some());
        let saga = database.saga_of(viewer, old, None, &held).await.expect("read").expect("a saga");
        assert_eq!(ids(saga.cards), [old]);
        assert!(database.roles_elsewhere(&[person], &[old], None, &held).await.expect("read").is_empty());
        assert_eq!(database.roles_elsewhere(&[person], &[old], None, &nothing).await.expect("read").len(), 1);
        assert_eq!(ids(database.cards_in_order(viewer, &[new, old], &held).await.expect("read")), [old]);
        assert_eq!(ids(database.works_crediting(viewer, person, None, &held).await.expect("read")), [old]);
        assert_eq!(
            database.genres_in_use(Some(library), &held).await.expect("counted"),
            [("Drama".to_string(), 1)]
        );

        database
            .record_playback_progress(viewer, new, Millis::new(60_000), melyxar_core::work::PlaybackState::InProgress, now())
            .await
            .expect("kept");
        assert!(database.works_to_carry_on(viewer, None, &held, 5).await.expect("read").is_empty());
        assert_eq!(database.works_to_carry_on(viewer, None, &nothing, 5).await.expect("read").len(), 1);

        assert!(
            database.collections(viewer, None, &held, false).await.expect("read").is_empty(),
            "a saga of one film here is no saga"
        );
        assert_eq!(database.collections(viewer, None, &nothing, false).await.expect("read").len(), 1);
        let by_hand = database.create_collection("Evenings", "evenings").await.expect("made");
        database.add_to_collection(by_hand, &[old, new]).await.expect("filled");
        let collection = database.collection(viewer, by_hand, None, &held).await.expect("read").expect("there");
        assert_eq!(ids(collection.cards), [old]);

        let playlist = database.create_playlist(viewer, "Tonight").await.expect("made");
        database.add_to_playlist(viewer, playlist, &[new, old]).await.expect("filled");
        let listed = database.playlists(viewer, None, &held).await.expect("read");
        assert_eq!((listed[0].held, listed[0].cover.as_ref().map(|card| card.id)), (1, Some(old)));
        let kept = database.playlist(viewer, playlist, None, &held).await.expect("read").expect("there");
        assert_eq!(ids(kept.cards), [old]);

        let asked = ["film-Quiet Shore".to_string()];
        assert!(database.held_titles("films", &asked, None, &held).await.expect("read").is_empty());
        assert_eq!(database.held_titles("films", &asked, None, &nothing).await.expect("read").len(), 1);
    }

    #[tokio::test]
    async fn what_arrived_since_a_library_settled_is_what_settling_again_lets_go() {
        let (database, library, root, _) = a_library(LibraryKind::Movies).await;
        assert!(!database.arrived_since_settled(library).await.expect("asked"));
        let film = a_work(&database, library, WorkKind::Movie, "Quiet Harbour", None).await;
        assert!(
            database.arrived_since_settled(library).await.expect("asked"),
            "a library that never settled has everything still to settle"
        );
        database.settle_library(library, now()).await.expect("settled");
        assert!(!database.arrived_since_settled(library).await.expect("asked"));

        let copy = a_copy(&database, root, film, "quiet.mkv").await;
        assert!(
            database.arrived_since_settled(library).await.expect("asked"),
            "a copy of a film already there is an arrival too"
        );
        let held = HeldBack::by(vec![library], true);
        assert!(database.is_held_back(copy, &held).await.expect("asked"));
        database.settle_library(library, now()).await.expect("settled");
        assert!(!database.is_held_back(copy, &held).await.expect("asked"), "never held back again");
    }
}
