//! Writing an invented library, for measuring against.
//!
//! The collection this server is designed for is a hundred thousand works, and
//! the collection it is developed against is fifty films. Everything that only
//! goes wrong at scale therefore goes unnoticed until the day somebody else
//! installs it: a page that asks one question per card, an ordering that walks
//! the table, a menu that counts the whole library to draw itself. A library
//! that is invented rather than owned is how those are met here instead.
//!
//! Invented, and never pretending otherwise. Nothing here writes a file, opens
//! one or looks at a disk: the works have no pictures and their files exist as
//! rows and nothing more. What it does write is everything a page *reads*, in
//! the shapes and the numbers a real collection has, because a measurement
//! taken against half a library is a measurement of nothing.
//!
//! It also never touches a real library. What it writes hangs off a library of
//! its own, which is also the only thing it takes away again.

use melyxar_core::id::{
    CreditId, ImageId, LibraryId, LibraryRootId, MediaSourceId, NameId, PersonId, TrackId, UserId,
    WorkId,
};
use melyxar_core::library::{Library, LibraryKind};
use melyxar_core::time::{now, Timestamp};
use melyxar_core::work::WorkKind;
use sqlx::sqlite::SqliteArguments;
use sqlx::{AssertSqlSafe, Sqlite, Transaction};
use time::Duration;

use crate::convert::timestamp_to_text;
use crate::{Database, Result};

/// The name the invented library goes under.
///
/// Fixed rather than chosen, because it is what tells the library apart from
/// somebody's own. Everything that removes anything here is bounded by it.
pub const BENCH_LIBRARY: &str = "Melyxar bench";

/// Where the invented library says its files are.
///
/// A folder that does not exist, on purpose. A root nothing can read is a root
/// every scan steps over, which is what stops a scan started by hand from
/// walking through an invented library and marking a hundred thousand files
/// gone.
pub const BENCH_ROOT: &str = "/nonexistent/melyxar-bench";

/// The most values one statement carries.
///
/// The engine takes far more than this. Kept low all the same, because the
/// gain from a wider statement is gone long before the limit and what is left
/// is a number nobody can reason about.
const VALUES_PER_STATEMENT: usize = 4_000;

/// One invented work and everything a page reads about it.
#[derive(Debug, Clone)]
pub struct InventedWork {
    pub id: WorkId,
    pub parent_id: Option<WorkId>,
    pub ordinal: Option<i32>,
    pub kind: WorkKind,
    pub title: String,
    pub sort_title: String,
    pub release_year: Option<i32>,
    pub runtime_ms: Option<i64>,
    pub community_rating: Option<f64>,
    pub age_rating_label: Option<String>,
    pub dominant_color: String,
    pub child_count: i64,
    /// How long ago this was added, in seconds. Spread out rather than all at
    /// one instant: a collection where every work arrived at the same moment
    /// is a collection an ordering by date cannot order, and the page that
    /// leads with the newest then sorts a hundred thousand ties.
    pub added_seconds_ago: i64,
    pub tagline: String,
    pub overview: String,
    pub external_id: String,
    /// Genres and studios, by the identifier the shared names were written
    /// under.
    pub genres: Vec<NameId>,
    pub studios: Vec<NameId>,
    pub credits: Vec<InventedCredit>,
    pub pictures: Vec<InventedPicture>,
    pub sources: Vec<InventedSource>,
    /// Where somebody left this work, when somebody did.
    pub watched: Option<InventedProgress>,
    /// How many of what hangs under this work are still unwatched. Set for a
    /// series and for a season, and for nothing else.
    pub unwatched: Option<i64>,
}

/// One name on the cast or crew of an invented work.
#[derive(Debug, Clone)]
pub struct InventedCredit {
    pub person_id: PersonId,
    pub role: String,
    pub character_name: Option<String>,
    pub ordinal: i32,
}

/// One size of one picture of an invented work.
///
/// A row and never a file. What a file costs to hand over does not change with
/// the size of the collection, so a hundred thousand of them would measure
/// exactly what one of them measures; what the row costs to join and to carry
/// does change, and that is what is being measured.
#[derive(Debug, Clone, Copy)]
pub struct InventedPicture {
    /// poster, backdrop or logo.
    pub kind: &'static str,
    pub width: i32,
    pub height: i32,
}

/// One invented file behind a work.
#[derive(Debug, Clone)]
pub struct InventedSource {
    pub id: MediaSourceId,
    pub relative_path: String,
    pub container: String,
    pub duration_ms: i64,
    pub overall_bitrate: i64,
    pub size_bytes: i64,
    pub tracks: Vec<InventedTrack>,
}

/// One stream inside an invented file.
#[derive(Debug, Clone)]
pub struct InventedTrack {
    pub stream_index: i32,
    /// video, audio or subtitle.
    pub kind: &'static str,
    pub language: Option<String>,
    pub title: Option<String>,
    pub is_default: bool,
    pub codec: String,
    pub profile: Option<String>,
    pub bitrate: Option<i64>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub frame_rate: Option<f64>,
    pub pixel_format: Option<String>,
    pub bit_depth: Option<i32>,
    pub channels: Option<i32>,
    pub channel_layout: Option<String>,
    pub sample_rate: Option<i32>,
    /// text or bitmap, for a subtitle.
    pub subtitle_layout: Option<String>,
}

/// Where somebody left an invented work.
#[derive(Debug, Clone, Copy)]
pub struct InventedProgress {
    pub position_ms: i64,
    /// in_progress or watched.
    pub state: &'static str,
    /// How long ago it was last played, in seconds. Spread out for the same
    /// reason: the row of what to carry on with is ordered by this, and a
    /// thousand works sharing one instant make it read every one of them
    /// before it can show twenty.
    pub seconds_since_played: i64,
}

/// Names shared by many works, once they have been written down.
#[derive(Debug, Clone, Default)]
pub struct SharedNames {
    pub genres: Vec<NameId>,
    pub studios: Vec<NameId>,
    pub people: Vec<PersonId>,
}

/// What an invented library holds, once it is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Invented {
    pub works: i64,
    pub files: i64,
}

impl Database {
    /// The invented library, if one has been made.
    pub async fn bench_library(&self) -> Result<Option<Library>> {
        self.library_by_name(BENCH_LIBRARY).await
    }

    /// Makes the invented library, with the root its files pretend to be under.
    ///
    /// Refuses to make a second one: everything that removes anything is
    /// bounded by the name, and two libraries wearing it would leave one of
    /// them unreachable.
    pub async fn start_an_invented_library(&self) -> Result<(LibraryId, LibraryRootId)> {
        let library = self
            .create_library(
                BENCH_LIBRARY,
                LibraryKind::Movies,
                "fr",
                &[("bench".to_string(), std::path::PathBuf::from(BENCH_ROOT))],
            )
            .await?;
        let root = library
            .roots
            .first()
            .map(|root| root.id)
            .ok_or_else(|| crate::DatabaseError::Corrupt("the bench root".to_string()))?;
        Ok((library.id, root))
    }

    /// Writes down the names many invented works share.
    ///
    /// Genres, studios and people exist once and are pointed at, exactly as
    /// they are for a real collection: a hundred thousand films credit a few
    /// thousand actors between them, and writing a person per film would
    /// measure a shape no library has.
    ///
    /// Exist once for the whole server, not once for the invented library: a
    /// collection that has been identified already holds 'Drame', and the
    /// unique index on the name is what says a second one cannot be written.
    /// So whatever is already there is borrowed, which is what a scan does
    /// when a second film turns out to be a drama too. Nothing of anybody's is
    /// disturbed by the borrowing, since taking the invented library away
    /// sweeps up the names nothing points at any more and leaves the names
    /// their own films still point at.
    pub async fn invent_the_shared_names(
        &self,
        genres: &[String],
        studios: &[String],
        people: &[String],
    ) -> Result<SharedNames> {
        let moment = timestamp_to_text(now());
        let mut transaction = self.begin().await?;

        let genre_ids = kept_or_written(&mut transaction, &GENRE_NAMES, genres, &moment).await?;
        let studio_ids = kept_or_written(&mut transaction, &STUDIO_NAMES, studios, &moment).await?;
        let person_ids = kept_or_written(&mut transaction, &PERSON_NAMES, people, &moment).await?;

        transaction.commit().await?;
        Ok(SharedNames {
            genres: genre_ids,
            studios: studio_ids,
            people: person_ids,
        })
    }

    /// Writes one batch of invented works, in a single transaction.
    ///
    /// A batch rather than a work, because a hundred thousand transactions is
    /// a hundred thousand waits on the journal and turns minutes into an hour.
    /// A transaction rather than the lot, because a run somebody stops halfway
    /// must leave a library that is smaller than asked for rather than one
    /// that is half written.
    ///
    /// A work has to come after whatever holds it: a series before its seasons
    /// and a season before its episodes. The batch is written in the order it
    /// is given and in statements of a few hundred rows, so an episode handed
    /// over before its season is an episode the database refuses.
    pub async fn invent_works(
        &self,
        library_id: LibraryId,
        root_id: LibraryRootId,
        viewer: UserId,
        batch: &[InventedWork],
    ) -> Result<()> {
        if batch.is_empty() {
            return Ok(());
        }
        let at = now();
        let moment = timestamp_to_text(at);
        let library = library_id.to_db_string();
        let mut transaction = self.begin().await?;

        write_rows(
            &mut transaction,
            "works",
            &[
                "id",
                "library_id",
                "parent_id",
                "ordinal",
                "kind",
                "title",
                "sort_title",
                "release_year",
                "runtime_ms",
                "community_rating",
                "age_rating_label",
                "identification",
                "dominant_color",
                "child_count",
                "added_at",
                "updated_at",
            ],
            batch,
            |statement, work| {
                statement
                    .bind(work.id.to_db_string())
                    .bind(library.clone())
                    .bind(work.parent_id.map(|parent| parent.to_db_string()))
                    .bind(work.ordinal)
                    .bind(work.kind.as_str())
                    .bind(work.title.clone())
                    .bind(work.sort_title.clone())
                    .bind(work.release_year)
                    .bind(work.runtime_ms)
                    .bind(work.community_rating)
                    .bind(work.age_rating_label.clone())
                    .bind("identified")
                    .bind(work.dominant_color.clone())
                    .bind(work.child_count)
                    .bind(a_moment_ago(at, work.added_seconds_ago))
                    .bind(moment.clone())
            },
        )
        .await?;

        write_rows(
            &mut transaction,
            "work_translations",
            &["work_id", "language", "title", "tagline", "overview"],
            batch,
            |statement, work| {
                statement
                    .bind(work.id.to_db_string())
                    .bind("fr")
                    .bind(work.title.clone())
                    .bind(work.tagline.clone())
                    .bind(work.overview.clone())
            },
        )
        .await?;

        write_rows(
            &mut transaction,
            "work_external_ids",
            &["work_id", "provider", "external_id"],
            batch,
            |statement, work| {
                statement
                    .bind(work.id.to_db_string())
                    .bind("bench")
                    .bind(work.external_id.clone())
            },
        )
        .await?;

        let links: Vec<(WorkId, NameId)> = pairs(batch, |work| &work.genres);
        write_rows(
            &mut transaction,
            "work_genres",
            &["work_id", "genre_id"],
            &links,
            |statement, (work, genre)| {
                statement.bind(work.to_db_string()).bind(genre.to_db_string())
            },
        )
        .await?;

        let links: Vec<(WorkId, NameId)> = pairs(batch, |work| &work.studios);
        write_rows(
            &mut transaction,
            "work_studios",
            &["work_id", "studio_id"],
            &links,
            |statement, (work, studio)| {
                statement
                    .bind(work.to_db_string())
                    .bind(studio.to_db_string())
            },
        )
        .await?;

        let credits: Vec<(WorkId, InventedCredit)> = pairs(batch, |work| &work.credits);
        write_rows(
            &mut transaction,
            "credits",
            &[
                "id",
                "work_id",
                "person_id",
                "role",
                "character_name",
                "ordinal",
            ],
            &credits,
            |statement, (work, credit)| {
                statement
                    .bind(CreditId::new().to_db_string())
                    .bind(work.to_db_string())
                    .bind(credit.person_id.to_db_string())
                    .bind(credit.role.clone())
                    .bind(credit.character_name.clone())
                    .bind(credit.ordinal)
            },
        )
        .await?;

        let pictures: Vec<(WorkId, String, InventedPicture)> = batch
            .iter()
            .flat_map(|work| {
                work.pictures
                    .iter()
                    .map(|picture| (work.id, work.dominant_color.clone(), *picture))
            })
            .collect();
        write_rows(
            &mut transaction,
            "images",
            &[
                "id",
                "owner_kind",
                "owner_id",
                "image_kind",
                "relative_path",
                "width",
                "height",
                "fingerprint",
                "dominant_color",
                "created_at",
            ],
            &pictures,
            |statement, (work, colour, picture)| {
                let owner = work.to_db_string();
                statement
                    .bind(ImageId::new().to_db_string())
                    .bind("work")
                    .bind(owner.clone())
                    .bind(picture.kind)
                    .bind(picture_path(&owner, picture))
                    .bind(picture.width)
                    .bind(picture.height)
                    .bind(picture_fingerprint(&owner, picture.kind))
                    .bind(colour.clone())
                    .bind(moment.clone())
            },
        )
        .await?;

        let sources: Vec<(WorkId, InventedSource)> = pairs(batch, |work| &work.sources);
        let root = root_id.to_db_string();
        write_rows(
            &mut transaction,
            "media_sources",
            &[
                "id",
                "work_id",
                "root_id",
                "relative_path",
                "container",
                "duration_ms",
                "overall_bitrate",
                "size_bytes",
                "modified_at",
                "added_at",
                "analysed_at",
            ],
            &sources,
            |statement, (work, source)| {
                statement
                    .bind(source.id.to_db_string())
                    .bind(work.to_db_string())
                    .bind(root.clone())
                    .bind(source.relative_path.clone())
                    .bind(source.container.clone())
                    .bind(source.duration_ms)
                    .bind(source.overall_bitrate)
                    .bind(source.size_bytes)
                    .bind(moment.clone())
                    .bind(moment.clone())
                    .bind(moment.clone())
            },
        )
        .await?;

        let tracks: Vec<(MediaSourceId, InventedTrack)> = batch
            .iter()
            .flat_map(|work| &work.sources)
            .flat_map(|source| {
                source
                    .tracks
                    .iter()
                    .map(|track| (source.id, track.clone()))
            })
            .collect();
        write_rows(
            &mut transaction,
            "tracks",
            &[
                "id",
                "source_id",
                "stream_index",
                "kind",
                "language",
                "title",
                "is_default",
                "codec",
                "profile",
                "bitrate",
                "width",
                "height",
                "frame_rate",
                "pixel_format",
                "bit_depth",
                "channels",
                "channel_layout",
                "sample_rate",
                "subtitle_layout",
            ],
            &tracks,
            |statement, (source, track)| {
                statement
                    .bind(TrackId::new().to_db_string())
                    .bind(source.to_db_string())
                    .bind(track.stream_index)
                    .bind(track.kind)
                    .bind(track.language.clone())
                    .bind(track.title.clone())
                    .bind(i64::from(track.is_default))
                    .bind(track.codec.clone())
                    .bind(track.profile.clone())
                    .bind(track.bitrate)
                    .bind(track.width)
                    .bind(track.height)
                    .bind(track.frame_rate)
                    .bind(track.pixel_format.clone())
                    .bind(track.bit_depth)
                    .bind(track.channels)
                    .bind(track.channel_layout.clone())
                    .bind(track.sample_rate)
                    .bind(track.subtitle_layout.clone())
            },
        )
        .await?;

        let watched: Vec<&InventedWork> = batch
            .iter()
            .filter(|work| work.watched.is_some())
            .collect();
        let viewer_id = viewer.to_db_string();
        write_rows(
            &mut transaction,
            "playback_progress",
            &[
                "user_id",
                "work_id",
                "position_ms",
                "state",
                "play_count",
                "reported_at",
                "last_played_at",
            ],
            &watched,
            |statement, work| {
                // Every work in this list carries one; the filter above is what
                // put it there.
                let progress = work.watched.unwrap_or(InventedProgress {
                    position_ms: 0,
                    state: "not_started",
                    seconds_since_played: 0,
                });
                let played = a_moment_ago(at, progress.seconds_since_played);
                statement
                    .bind(viewer_id.clone())
                    .bind(work.id.to_db_string())
                    .bind(progress.position_ms)
                    .bind(progress.state)
                    .bind(1_i64)
                    .bind(played.clone())
                    .bind(played)
            },
        )
        .await?;

        let counted: Vec<&InventedWork> = batch
            .iter()
            .filter(|work| work.unwatched.is_some())
            .collect();
        write_rows(
            &mut transaction,
            "progress_counters",
            &[
                "user_id",
                "work_id",
                "child_count",
                "unwatched_count",
                "latest_child_added_at",
            ],
            &counted,
            |statement, work| {
                statement
                    .bind(viewer_id.clone())
                    .bind(work.id.to_db_string())
                    .bind(work.child_count)
                    .bind(work.unwatched.unwrap_or_default())
                    .bind(moment.clone())
            },
        )
        .await?;

        transaction.commit().await?;
        Ok(())
    }

    /// Brings the library's own count of works up to date, once.
    ///
    /// Counted rather than added up as it goes, for the same reason the rest of
    /// this crate counts: a run somebody stopped halfway leaves a number that
    /// is still true of what is there.
    pub async fn settle_the_invented_library(&self, library_id: LibraryId) -> Result<Invented> {
        sqlx::query(
            "UPDATE libraries
                SET work_count = (SELECT count(*) FROM works WHERE works.library_id = libraries.id),
                    version = version + 1,
                    updated_at = ?
              WHERE id = ?",
        )
        .bind(timestamp_to_text(now()))
        .bind(library_id.to_db_string())
        .execute(self.writer())
        .await?;

        self.what_an_invented_library_holds(library_id).await
    }

    /// What an invented library holds right now.
    pub async fn what_an_invented_library_holds(&self, library_id: LibraryId) -> Result<Invented> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT (SELECT count(*) FROM works WHERE library_id = ?1),
                    (SELECT count(*) FROM media_sources s
                      JOIN works w ON w.id = s.work_id
                      WHERE w.library_id = ?1)",
        )
        .bind(library_id.to_db_string())
        .fetch_one(self.reader())
        .await?;

        Ok(Invented {
            works: row.0,
            files: row.1,
        })
    }
}

/// An instant that many seconds before another, written the way one is stored.
fn a_moment_ago(moment: Timestamp, seconds_ago: i64) -> String {
    timestamp_to_text(moment - Duration::seconds(seconds_ago.max(0)))
}

/// Where a picture of an invented work would live, if it had one.
fn picture_path(owner: &str, picture: &InventedPicture) -> String {
    format!("bench/{owner}/{}-{}.webp", picture.kind, picture.width)
}

/// The name a picture's content would have earned.
fn picture_fingerprint(owner: &str, kind: &str) -> String {
    melyxar_core::fingerprint::of_text(&format!("{owner}/{kind}"))
}

/// Everything hanging off a batch of works, each with the work it hangs off.
fn pairs<T: Clone>(
    batch: &[InventedWork],
    of: impl Fn(&InventedWork) -> &Vec<T>,
) -> Vec<(WorkId, T)> {
    batch
        .iter()
        .flat_map(|work| of(work).iter().map(|one| (work.id, one.clone())))
        .collect()
}

/// A statement that carries what it is given rather than borrowing it.
type Statement = sqlx::query::Query<'static, Sqlite, SqliteArguments>;

/// How many rows of a given width go into one statement.
///
/// At least one, whatever the width: a table wider than the whole allowance
/// still has to be written, one row at a time, rather than never.
fn rows_per_statement(columns: usize) -> usize {
    (VALUES_PER_STATEMENT / columns.max(1)).max(1)
}

/// The `VALUES (?, ?), (?, ?)` of a statement writing several rows at once.
fn questions(columns: usize, rows: usize) -> String {
    let one = format!("({})", vec!["?"; columns].join(", "));
    vec![one; rows].join(", ")
}

/// Writes many rows of one shape, in statements the engine will take.
///
/// One statement per few thousand values rather than one per row: the cost of
/// writing a hundred thousand works is almost entirely the number of times the
/// engine is spoken to, and this is what turns an hour into a minute.
/// One pool of names that works point at rather than carry.
///
/// Written out rather than built, so no statement here is ever assembled from
/// anything that came from outside.
struct NamePool {
    select: &'static str,
    insert: &'static str,
    /// What the insert takes, in the order it takes it.
    binds: fn(id: &str, name: &str, moment: &str) -> Vec<String>,
}

fn a_name(id: &str, name: &str, _moment: &str) -> Vec<String> {
    vec![id.to_string(), name.to_string()]
}

fn a_person(id: &str, name: &str, moment: &str) -> Vec<String> {
    vec![
        id.to_string(),
        name.to_string(),
        name.to_lowercase(),
        moment.to_string(),
    ]
}

const GENRE_NAMES: NamePool = NamePool {
    select: "SELECT id FROM genres WHERE name = ? COLLATE NOCASE",
    insert: "INSERT INTO genres (id, name) VALUES (?, ?)",
    binds: a_name,
};

const STUDIO_NAMES: NamePool = NamePool {
    select: "SELECT id FROM studios WHERE name = ? COLLATE NOCASE",
    insert: "INSERT INTO studios (id, name) VALUES (?, ?)",
    binds: a_name,
};

const PERSON_NAMES: NamePool = NamePool {
    select: "SELECT id FROM people WHERE name = ? COLLATE NOCASE",
    insert: "INSERT INTO people (id, name, sort_name, created_at) VALUES (?, ?, ?, ?)",
    binds: a_person,
};

/// The identifier each name already has, or the one it is given here.
///
/// One statement per name rather than one for the lot, which is the same way
/// a scan writes them: these are a few hundred rows written once, against the
/// hundred thousand works that follow.
async fn kept_or_written<Id>(
    transaction: &mut Transaction<'_, Sqlite>,
    pool: &NamePool,
    names: &[String],
    moment: &str,
) -> Result<Vec<Id>>
where
    Id: Default + std::fmt::Display + std::str::FromStr,
{
    let mut ids = Vec::with_capacity(names.len());
    for name in names {
        let kept: Option<(String,)> = sqlx::query_as(pool.select)
            .bind(name)
            .fetch_optional(&mut **transaction)
            .await?;

        let id = match kept {
            Some((id,)) => id
                .parse()
                .map_err(|_| crate::DatabaseError::Corrupt("a shared name".to_string()))?,
            None => {
                let id = Id::default();
                let mut statement = sqlx::query(pool.insert);
                for value in (pool.binds)(&id.to_string(), name, moment) {
                    statement = statement.bind(value);
                }
                statement.execute(&mut **transaction).await?;
                id
            }
        };
        ids.push(id);
    }
    Ok(ids)
}

async fn write_rows<T>(
    transaction: &mut Transaction<'_, Sqlite>,
    into: &str,
    columns: &[&str],
    rows: &[T],
    bind: impl Fn(Statement, &T) -> Statement,
) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }
    let per_statement = rows_per_statement(columns.len());

    for chunk in rows.chunks(per_statement) {
        // Every piece of this statement comes from the call above: a table
        // name and a list of column names written out in this file, and a row
        // of question marks. Nothing anybody sent is ever part of the text.
        let sql = format!(
            "INSERT INTO {into} ({}) VALUES {}",
            columns.join(", "),
            questions(columns.len(), chunk.len())
        );
        let mut statement = sqlx::query(AssertSqlSafe(sql));
        for row in chunk {
            statement = bind(statement, row);
        }
        statement.execute(&mut **transaction).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::user::Permissions;

    /// An invented library of two films and one series, written the way the
    /// generator writes one.
    async fn a_small_invented_library() -> (Database, LibraryId, UserId) {
        let database = Database::open_in_memory().await.expect("database opens");
        let viewer = database
            .create_user("watcher", None, &Permissions::viewer())
            .await
            .expect("account created")
            .id;
        let (library_id, root_id) = database
            .start_an_invented_library()
            .await
            .expect("library started");
        let names = database
            .invent_the_shared_names(
                &["Drame".to_string(), "Policier".to_string()],
                &["Atelier Nord".to_string()],
                &["Adele Alvarez".to_string(), "Bruno Baumann".to_string()],
            )
            .await
            .expect("shared names written");

        let series_id = WorkId::new();
        let season_id = WorkId::new();
        // A work comes after whatever holds it, which is what the writer
        // requires: the series, then its season, then that season's episode.
        let batch = vec![
            a_film("Quiet Harbour", "quiet harbour", 2019, &names),
            a_film("Silver Orchard", "silver orchard", 1994, &names),
            InventedWork {
                id: series_id,
                kind: WorkKind::Series,
                sources: Vec::new(),
                unwatched: Some(1),
                child_count: 1,
                ..a_film("Amber Tide", "amber tide", 2021, &names)
            },
            InventedWork {
                id: season_id,
                parent_id: Some(series_id),
                ordinal: Some(1),
                kind: WorkKind::Season,
                sources: Vec::new(),
                unwatched: Some(1),
                child_count: 1,
                ..a_film("Season 1", "amber tide 01", 2021, &names)
            },
            InventedWork {
                id: WorkId::new(),
                parent_id: Some(season_id),
                ordinal: Some(1),
                kind: WorkKind::Episode,
                title: "Open Window".to_string(),
                sort_title: "amber tide 01 01".to_string(),
                ..a_film("Open Window", "amber tide 01 01", 2021, &names)
            },
        ];
        database
            .invent_works(library_id, root_id, viewer, &batch)
            .await
            .expect("works written");
        database
            .settle_the_invented_library(library_id)
            .await
            .expect("library settled");

        (database, library_id, viewer)
    }

    #[tokio::test]
    async fn a_name_a_real_collection_already_has_is_borrowed_rather_than_rewritten() {
        // What this is: an installation whose own films have been identified
        // holds 'Drame' already, and only one row may ever carry that name.
        // Writing a second one is what stopped the bench dead on the only
        // machine it was meant for.
        let database = Database::open_in_memory().await.expect("database opens");
        let genre_already_there = NameId::new();
        sqlx::query("INSERT INTO genres (id, name) VALUES (?, ?)")
            .bind(genre_already_there.to_db_string())
            .bind("Drame")
            .execute(database.writer())
            .await
            .expect("genre written");
        let person_already_there = PersonId::new();
        sqlx::query("INSERT INTO people (id, name, sort_name, created_at) VALUES (?, ?, ?, ?)")
            .bind(person_already_there.to_db_string())
            .bind("Adele Alvarez")
            .bind("alvarez, adele")
            .bind(timestamp_to_text(now()))
            .execute(database.writer())
            .await
            .expect("person written");

        let names = database
            .invent_the_shared_names(
                // Asked for in another case, since the name is what a row is
                // known by whoever wrote it.
                &["drame".to_string(), "Policier".to_string()],
                &["Atelier Nord".to_string()],
                &["Adele Alvarez".to_string(), "Bruno Baumann".to_string()],
            )
            .await
            .expect("shared names written");

        assert_eq!(
            names.genres[0], genre_already_there,
            "the genre that was already there is the one pointed at"
        );
        assert_eq!(
            names.people[0], person_already_there,
            "and so is the person, whatever their own films sort them under"
        );
        assert_ne!(names.genres[1], genre_already_there);

        let genres: i64 = sqlx::query_scalar("SELECT count(*) FROM genres")
            .fetch_one(database.reader())
            .await
            .expect("counted");
        let people: i64 = sqlx::query_scalar("SELECT count(*) FROM people")
            .fetch_one(database.reader())
            .await
            .expect("counted");
        assert_eq!(genres, 2, "one borrowed and one written, never a second row");
        assert_eq!(people, 2);

        let sorted: String = sqlx::query_scalar("SELECT sort_name FROM people WHERE id = ?")
            .bind(person_already_there.to_db_string())
            .fetch_one(database.reader())
            .await
            .expect("read");
        assert_eq!(
            sorted, "alvarez, adele",
            "borrowing a row must never rewrite what it says"
        );
    }

    fn a_film(title: &str, sort_title: &str, year: i32, names: &SharedNames) -> InventedWork {
        InventedWork {
            id: WorkId::new(),
            parent_id: None,
            ordinal: None,
            kind: WorkKind::Movie,
            title: title.to_string(),
            sort_title: sort_title.to_string(),
            release_year: Some(year),
            runtime_ms: Some(6_000_000),
            community_rating: Some(7.4),
            age_rating_label: Some("-12".to_string()),
            dominant_color: "#3a5f7d".to_string(),
            child_count: 0,
            added_seconds_ago: 3_600,
            tagline: "Une nuit suffit.".to_string(),
            overview: "Ce qui a été laissé derrière finit par revenir.".to_string(),
            external_id: format!("bench-{sort_title}"),
            genres: names.genres.clone(),
            studios: names.studios.clone(),
            credits: names
                .people
                .iter()
                .enumerate()
                .map(|(ordinal, person_id)| InventedCredit {
                    person_id: *person_id,
                    role: "actor".to_string(),
                    character_name: Some("The Captain".to_string()),
                    ordinal: ordinal as i32,
                })
                .collect(),
            pictures: vec![
                InventedPicture {
                    kind: "poster",
                    width: 200,
                    height: 300,
                },
                InventedPicture {
                    kind: "poster",
                    width: 400,
                    height: 600,
                },
            ],
            sources: vec![InventedSource {
                id: MediaSourceId::new(),
                relative_path: format!("{title} ({year}).mkv"),
                container: "matroska".to_string(),
                duration_ms: 6_000_000,
                overall_bitrate: 8_000_000,
                size_bytes: 6_000_000_000,
                tracks: vec![InventedTrack {
                    stream_index: 0,
                    kind: "video",
                    language: None,
                    title: None,
                    is_default: true,
                    codec: "hevc".to_string(),
                    profile: Some("main".to_string()),
                    bitrate: Some(8_000_000),
                    width: Some(1920),
                    height: Some(1080),
                    frame_rate: Some(24.0),
                    pixel_format: Some("yuv420p".to_string()),
                    bit_depth: Some(8),
                    channels: None,
                    channel_layout: None,
                    sample_rate: None,
                    subtitle_layout: None,
                }],
            }],
            watched: Some(InventedProgress {
                position_ms: 1_200_000,
                state: "in_progress",
                seconds_since_played: 7_200,
            }),
            unwatched: None,
        }
    }

    #[tokio::test]
    async fn an_invented_library_reads_back_the_way_a_real_one_does() {
        let (database, library_id, viewer) = a_small_invented_library().await;

        let page = database
            .browse_works(&crate::browse::BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("the grid reads");
        // Two films and one series are met on their own; the season and the
        // episode are opened from what holds them.
        assert_eq!(page.cards.len(), 3);
        assert!(
            page.cards.iter().all(|card| card.poster.len() == 2),
            "every card carries every size of its poster"
        );
        assert!(page.cards.iter().all(|card| card.dominant_color.is_some()));

        // The card of a film rather than the first of the page: the ordering
        // is alphabetical, and the series sorts before both films.
        let first = page
            .cards
            .iter()
            .find(|card| card.kind == WorkKind::Movie)
            .expect("a film is met on its own")
            .id;
        let translation = database
            .work_translation(first, "fr")
            .await
            .expect("the synopsis reads");
        assert!(translation.is_some(), "a page has something to show");
        assert_eq!(
            database.work_genres(first).await.expect("genres read").len(),
            2
        );
        assert_eq!(
            database
                .work_studios(first)
                .await
                .expect("studios read")
                .len(),
            1
        );
        assert_eq!(
            database
                .work_credits(first)
                .await
                .expect("credits read")
                .len(),
            2
        );
        assert_eq!(
            database
                .work_external_ids(first)
                .await
                .expect("identifiers read")
                .len(),
            1
        );

        let sources = database
            .sources_of_work(first)
            .await
            .expect("the files read");
        assert_eq!(sources.len(), 1);
        assert_eq!(
            database
                .tracks_of_source(sources[0].id)
                .await
                .expect("the tracks read")
                .len(),
            1
        );

        let left_at: Option<i64> = sqlx::query_scalar(
            "SELECT position_ms FROM playback_progress WHERE user_id = ? AND work_id = ?",
        )
        .bind(viewer.to_db_string())
        .bind(first.to_db_string())
        .fetch_optional(database.reader())
        .await
        .expect("the resume point reads")
        .flatten();
        assert_eq!(
            left_at,
            Some(1_200_000),
            "a home page is made of what is half watched"
        );
    }

    #[tokio::test]
    async fn the_menus_that_narrow_a_grid_are_drawn_from_it() {
        let (database, library_id, _) = a_small_invented_library().await;

        let genres = database
            .genres_in_use(Some(library_id))
            .await
            .expect("the genre menu reads");
        assert_eq!(genres.len(), 2, "{genres:?}");
        // The season and the episode are not met on their own, so they are not
        // counted in a menu either.
        assert!(genres.iter().all(|(_, count)| *count == 3), "{genres:?}");

        let decades = database
            .decades_in_use(Some(library_id))
            .await
            .expect("the decade menu reads");
        assert_eq!(decades.len(), 3, "{decades:?}");

        let initials = database
            .initials_in_use(Some(library_id))
            .await
            .expect("the letters read");
        assert!(!initials.is_empty());
    }

    #[tokio::test]
    async fn a_series_holds_its_seasons_and_a_season_its_episodes() {
        let (database, library_id, viewer) = a_small_invented_library().await;

        let series = database
            .browse_works(&crate::browse::BrowseRequest {
                library_id: Some(library_id),
                ..Default::default()
            })
            .await
            .expect("the grid reads")
            .cards
            .into_iter()
            .find(|card| card.kind == WorkKind::Series)
            .expect("the series is met on its own");

        let seasons = database
            .children_of(viewer, series.id, "fr")
            .await
            .expect("the seasons read");
        assert_eq!(seasons.len(), 1);
        assert_eq!(
            seasons[0].card.state.as_ref().map(|state| state.unwatched),
            Some(1),
            "a season says how much of it is left"
        );

        let episodes = database
            .children_of(viewer, seasons[0].card.id, "fr")
            .await
            .expect("the episodes read");
        assert_eq!(episodes.len(), 1);
    }

    #[tokio::test]
    async fn what_an_invented_library_holds_is_counted_rather_than_remembered() {
        let (database, library_id, _) = a_small_invented_library().await;
        let held = database
            .what_an_invented_library_holds(library_id)
            .await
            .expect("the counts read");
        assert_eq!(held.works, 5, "two films, a series, a season, an episode");
        assert_eq!(held.files, 3, "the two films and the one episode");

        let counted: i64 = sqlx::query_scalar("SELECT work_count FROM libraries WHERE id = ?")
            .bind(library_id.to_db_string())
            .fetch_one(database.reader())
            .await
            .expect("the library reads");
        assert_eq!(counted, held.works);
    }

    #[tokio::test]
    async fn taking_the_invented_library_away_leaves_nothing_behind() {
        let (database, library_id, _) = a_small_invented_library().await;
        let removed = database
            .delete_library(library_id)
            .await
            .expect("the library goes");
        assert_eq!(removed.works, 5);

        for table in [
            "works",
            "work_translations",
            "work_external_ids",
            "work_genres",
            "work_studios",
            "credits",
            "images",
            "media_sources",
            "tracks",
            "playback_progress",
            "progress_counters",
        ] {
            let left: i64 =
                sqlx::query_scalar(AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
                    .fetch_one(database.reader())
                    .await
                    .expect("the table reads");
            assert_eq!(left, 0, "{table} still holds rows of an invented library");
        }
        assert!(database
            .bench_library()
            .await
            .expect("the library reads")
            .is_none());
    }

    #[tokio::test]
    async fn a_second_invented_library_is_never_started_beside_the_first() {
        let (database, _, _) = a_small_invented_library().await;
        assert!(
            database.bench_library().await.expect("reads").is_some(),
            "the first one answers to its name, which is what bounds every removal"
        );
    }

    #[test]
    fn a_statement_stays_within_what_the_engine_takes() {
        for columns in [1, 2, 5, 16, 25, 40] {
            let rows = rows_per_statement(columns);
            assert!(rows >= 1, "{columns} columns must still write a row");
            assert!(
                rows * columns <= VALUES_PER_STATEMENT,
                "{columns} columns times {rows} rows is more than one statement carries"
            );
        }
    }

    #[test]
    fn a_table_wider_than_the_whole_allowance_is_still_written() {
        assert_eq!(rows_per_statement(VALUES_PER_STATEMENT * 2), 1);
        assert_eq!(rows_per_statement(0), VALUES_PER_STATEMENT);
    }

    #[test]
    fn the_questions_match_the_columns_and_the_rows() {
        assert_eq!(questions(2, 1), "(?, ?)");
        assert_eq!(questions(2, 3), "(?, ?), (?, ?), (?, ?)");
        assert_eq!(questions(1, 2), "(?), (?)");
    }

    #[test]
    fn two_sizes_of_one_picture_never_share_a_path() {
        let owner = "0192f0c0-0000-7000-8000-000000000001";
        let poster = InventedPicture {
            kind: "poster",
            width: 200,
            height: 300,
        };
        let larger = InventedPicture {
            width: 400,
            ..poster
        };
        assert_ne!(picture_path(owner, &poster), picture_path(owner, &larger));
        assert_eq!(
            picture_fingerprint(owner, "poster"),
            picture_fingerprint(owner, "poster"),
            "every size of one picture shares the name its content earned"
        );
        assert_ne!(
            picture_fingerprint(owner, "poster"),
            picture_fingerprint(owner, "backdrop")
        );
    }
}
