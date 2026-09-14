//! Writing down what a provider said about a work.
//!
//! Everything lands in one transaction, so a reader never sees a film that has
//! a new title and the old cast. Two things are never touched: a field someone
//! edited by hand, and anything a person made themselves. A refresh that wipes
//! what its owner arranged is a refresh nobody dares run twice.

use std::path::{Path, PathBuf};

use melyxar_core::id::{CollectionId, CreditId, ExtraVideoId, LibraryId, NameId, PersonId, WorkId};
use melyxar_core::time::{now, Millis};
use melyxar_core::work::{IdentificationNote, IdentificationState};
use sqlx::{Row, Sqlite, Transaction};

use crate::convert::{int_to_bool, timestamp_to_text};
use crate::{Database, DatabaseError, Result};

/// A work that still carries the name of the file it was found in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkNamedAfterItsFile {
    pub id: WorkId,
    pub title: String,
    pub release_year: Option<i32>,
    /// The file the title was read from, relative to its root.
    pub relative_path: PathBuf,
}

/// A work nobody has been able to name, and the name on disk behind it.
#[derive(Debug, Clone, PartialEq)]
pub struct NamelessWork {
    pub work: melyxar_core::work::Work,
    /// The name of one of its files, without the folders leading to it.
    ///
    /// Absent only for a work that has no file recorded at all, which is not
    /// something a scan produces.
    pub file_name: Option<String>,
}

/// A file nothing has managed to describe, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndescribedFile {
    /// The name of the file, without the folders leading to it.
    pub file_name: String,
    /// What the analyser said. Absent for a file nothing has tried yet, which
    /// is itself the answer.
    pub reason: Option<String>,
}

/// A film the library holds more than one copy of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkWithCopies {
    pub title: String,
    pub release_year: Option<i32>,
    /// The name of each copy, without the folders leading to it.
    pub file_names: Vec<String>,
}

/// One person's part in a work, as a provider described it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditRecord {
    pub external_id: String,
    pub name: String,
    pub sort_name: String,
    pub role: String,
    pub character: Option<String>,
    pub ordinal: i32,
    /// Where the photo lives at the provider. Fetching it is a separate step,
    /// so an identification never waits on a dozen pictures.
    pub photo_path: Option<String>,
}

/// A series of works a provider groups together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CollectionRecord {
    pub external_id: String,
    pub name: String,
    pub sort_name: String,
}

/// One line of the credits of a work, as a page shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkCredit {
    pub person_id: PersonId,
    pub name: String,
    pub role: String,
    pub character: Option<String>,
}

/// A person a work credits, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditedPerson {
    pub person_id: PersonId,
    pub name: String,
    /// The part they played in the work, which decides whether a page shows
    /// their face at all.
    pub role: String,
    /// Where the photo lives at the provider, when it named one.
    pub photo_path: Option<String>,
    /// Billing order, so only the faces a page actually shows are fetched.
    pub ordinal: i32,
}

/// A named film, what to ask the provider about, and what is worth writing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteNamedWork {
    pub id: WorkId,
    /// What the provider calls it, which is how it is asked about again.
    pub external_id: String,
    pub wants_pictures: bool,
    pub wants_a_synopsis: bool,
}

/// A film that has a name and is still missing something a page shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncompleteWork {
    pub title: String,
    pub release_year: Option<i32>,
    /// What a page would leave a hole for, named as the interface names it.
    pub missing: Vec<&'static str>,
}

/// A trailer hosted elsewhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemoteTrailerRecord {
    pub name: String,
    pub url: String,
}

/// Everything a provider said, ready to be written.
#[derive(Debug, Clone, PartialEq)]
pub struct IdentifiedWork {
    /// Name of the provider, kept with every field it supplied so a later
    /// refresh knows what it owns.
    pub provider: String,
    pub external_id: String,
    pub imdb_id: Option<String>,
    /// Language the texts below are written in.
    pub language: String,
    pub title: String,
    pub sort_title: String,
    pub tagline: Option<String>,
    pub overview: Option<String>,
    pub release_year: Option<i32>,
    pub runtime: Option<Millis>,
    pub community_rating: Option<f64>,
    pub age_rating_label: Option<String>,
    pub genres: Vec<String>,
    pub studios: Vec<String>,
    pub credits: Vec<CreditRecord>,
    pub collection: Option<CollectionRecord>,
    pub trailers: Vec<RemoteTrailerRecord>,
}

impl Database {
    /// Puts every work of a library back in the queue to be looked up again,
    /// and says how many that was.
    ///
    /// What a change of language needs: the films already described are
    /// described in the language that is no longer wanted, so they are asked
    /// about again rather than left as the one part of the library that never
    /// changed.
    ///
    /// A match somebody picked by hand is never touched. That is the whole
    /// meaning of picking by hand, and a person who corrected a film the
    /// provider got wrong must not find their correction undone by a setting.
    /// What was fetched stays where it is: texts are filed by work and by
    /// language, so nothing is lost while the new language arrives, and a film
    /// the provider cannot describe in it keeps what it had.
    pub async fn ask_again_about_every_work(
        &self,
        library_id: melyxar_core::id::LibraryId,
    ) -> Result<u64> {
        let result = sqlx::query(
            "UPDATE works SET identification = ?, updated_at = ?
             WHERE library_id = ? AND identification <> ?",
        )
        .bind(IdentificationState::Pending.as_str())
        .bind(timestamp_to_text(now()))
        .bind(library_id.to_db_string())
        .bind(IdentificationState::Manual.as_str())
        .execute(self.writer())
        .await?;
        Ok(result.rows_affected())
    }

    /// Works of a library that are still waiting to be looked up.
    /// Every one of them, in the order they arrived.
    ///
    /// Read in one go rather than a handful at a time. Reading in handfuls
    /// means saying where to start again, and what is left to do changes while
    /// the run goes on: a film the provider could not be reached about stays
    /// in the list, so the same handful comes back and everything behind it is
    /// never reached. A library is bounded by the disks it sits on, and these
    /// rows are small.
    pub async fn works_awaiting_identification(
        &self,
        library_id: melyxar_core::id::LibraryId,
    ) -> Result<Vec<melyxar_core::work::Work>> {
        let rows = sqlx::query(
            "SELECT id, library_id, parent_id, kind, title, sort_title, release_year, runtime_ms,
                    community_rating, age_rating_label, identification, identification_note,
                    dominant_color, added_at, updated_at
             FROM works
             WHERE library_id = ? AND identification IN ('pending', 'unidentified')
             ORDER BY added_at",
        )
        .bind(library_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter().map(crate::catalogue::work_from_row).collect()
    }

    /// Every work still without a name, whatever library it is in.
    ///
    /// For the diagnostic, which has to say which films are waiting and why
    /// rather than only how many: a count sends whoever reads it to a
    /// terminal, which is the one thing the report exists to avoid.
    ///
    /// The name on disk comes with it. A title read off a file name is only
    /// ever as good as the name it was read from, so a title that looks wrong
    /// leaves one question, and the answer to it has to be in the same report
    /// rather than a query away.
    pub async fn works_still_nameless(&self, limit: i64) -> Result<Vec<NamelessWork>> {
        let rows = sqlx::query(
            "SELECT w.id, w.library_id, w.parent_id, w.kind, w.title, w.sort_title,
                    w.release_year, w.runtime_ms, w.community_rating, w.age_rating_label,
                    w.identification, w.identification_note, w.dominant_color,
                    w.added_at, w.updated_at,
                    min(s.relative_path) AS relative_path
             FROM works w
             LEFT JOIN media_sources s ON s.work_id = w.id
             WHERE w.identification IN ('pending', 'unidentified')
             GROUP BY w.id
             ORDER BY w.sort_title
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                Ok(NamelessWork {
                    work: crate::catalogue::work_from_row(row)?,
                    file_name: row.try_get::<Option<String>, _>("relative_path")?.and_then(
                        |path| {
                            Path::new(&path)
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                        },
                    ),
                })
            })
            .collect()
    }

    /// The files nothing has managed to describe.
    ///
    /// A scan tries every file it has no analysis for, so once one has run to
    /// its end these are the ones it could not read. Named rather than
    /// counted: such a file is in the library, has a card, and fails the
    /// moment somebody presses play, and the count alone sends whoever reads
    /// it to a terminal.
    pub async fn files_nothing_could_describe(&self, limit: i64) -> Result<Vec<UndescribedFile>> {
        let rows: Vec<(String, Option<String>)> = sqlx::query_as(
            "SELECT relative_path, analysis_failure FROM media_sources
             WHERE analysed_at IS NULL AND missing_since IS NULL
             ORDER BY relative_path
             LIMIT ?",
        )
        .bind(limit)
        .fetch_all(self.reader())
        .await?;

        Ok(rows
            .into_iter()
            .map(|(path, reason)| UndescribedFile {
                file_name: Path::new(&path)
                    .file_name()
                    .map_or(path.clone(), |name| name.to_string_lossy().into_owned()),
                reason,
            })
            .collect())
    }

    /// Films held in more than one copy, with the name of each copy.
    ///
    /// Several copies of one film are ordinary and wanted: that is what puts a
    /// version chooser on a page rather than the same title twice in a grid.
    /// But copies are also what a wrong grouping leaves behind, and a grouping
    /// nobody can check is a grouping nobody should trust. Named here so that
    /// reading the names side by side settles it.
    pub async fn works_held_in_several_copies(&self) -> Result<Vec<WorkWithCopies>> {
        let rows = sqlx::query(
            "SELECT w.id, w.title, w.release_year, w.sort_title, s.relative_path
             FROM works w
             JOIN media_sources s ON s.work_id = w.id
             WHERE w.id IN (SELECT work_id FROM media_sources
                            GROUP BY work_id HAVING count(*) > 1)
             ORDER BY w.sort_title, w.id, s.relative_path",
        )
        .fetch_all(self.reader())
        .await?;

        let mut films: Vec<WorkWithCopies> = Vec::new();
        let mut current: Option<String> = None;
        for row in &rows {
            let id: String = row.try_get("id")?;
            let path: String = row.try_get("relative_path")?;
            let file_name = Path::new(&path)
                .file_name()
                .map_or_else(|| path.clone(), |name| name.to_string_lossy().into_owned());

            match (&current, films.last_mut()) {
                (Some(seen), Some(film)) if *seen == id => film.file_names.push(file_name),
                _ => {
                    current = Some(id);
                    films.push(WorkWithCopies {
                        title: row.try_get("title")?,
                        release_year: row.try_get("release_year")?,
                        file_names: vec![file_name],
                    });
                }
            }
        }
        // In the order they are shown in, which is by name: the rows come back
        // ordered by the whole path, and a folder would shuffle the list a
        // reader is comparing names down.
        for film in &mut films {
            film.file_names.sort();
        }
        Ok(films)
    }

    /// Films that were named and are still missing something.
    ///
    /// A title nobody could find says so plainly; a film with no poster says
    /// nothing at all, and the hole is only ever seen by whoever scrolls past
    /// it. Counted here so the report can name them, which is the only way to
    /// tell a film the provider has no picture of from one whose picture never
    /// arrived.
    pub async fn works_missing_something(&self) -> Result<Vec<IncompleteWork>> {
        let rows = sqlx::query(
            "SELECT w.title, w.release_year,
                    (SELECT count(*) FROM images i
                      WHERE i.owner_kind = 'work' AND i.owner_id = w.id
                        AND i.image_kind = 'poster') AS posters,
                    (SELECT count(*) FROM images i
                      WHERE i.owner_kind = 'work' AND i.owner_id = w.id
                        AND i.image_kind = 'backdrop') AS backdrops,
                    (SELECT count(*) FROM work_translations t
                      WHERE t.work_id = w.id AND t.overview IS NOT NULL
                        AND t.overview <> '') AS overviews,
                    (SELECT count(*) FROM credits c WHERE c.work_id = w.id) AS credits
             FROM works w
             WHERE w.identification IN ('identified', 'manual')
             ORDER BY w.sort_title",
        )
        .fetch_all(self.reader())
        .await?;

        let mut incomplete = Vec::new();
        for row in &rows {
            let mut missing = Vec::new();
            for (count, what) in [
                (row.try_get::<i64, _>("posters")?, "poster"),
                (row.try_get::<i64, _>("backdrops")?, "backdrop"),
                (row.try_get::<i64, _>("overviews")?, "overview"),
                (row.try_get::<i64, _>("credits")?, "cast"),
            ] {
                if count == 0 {
                    missing.push(what);
                }
            }
            if missing.is_empty() {
                continue;
            }
            incomplete.push(IncompleteWork {
                title: row.try_get("title")?,
                release_year: row.try_get("release_year")?,
                missing,
            });
        }
        Ok(incomplete)
    }

    /// A film that was named and is still missing something a provider has.
    ///
    /// Carries what to ask about and what is worth writing when the answer
    /// comes back, so that one question serves both.
    pub async fn works_missing_their_metadata(
        &self,
        library_id: LibraryId,
        provider: &str,
        language: &str,
    ) -> Result<Vec<IncompleteNamedWork>> {
        let rows = sqlx::query(
            "SELECT w.id, e.external_id,
                    NOT EXISTS (
                        SELECT 1 FROM images i
                         WHERE i.owner_kind = 'work' AND i.owner_id = w.id
                           AND i.image_kind = 'poster') AS wants_pictures,
                    NOT EXISTS (
                        SELECT 1 FROM work_translations t
                         WHERE t.work_id = w.id AND t.language = ?
                           AND t.overview IS NOT NULL AND t.overview <> '') AS wants_a_synopsis
             FROM works w
             JOIN work_external_ids e ON e.work_id = w.id AND e.provider = ?
             WHERE w.library_id = ?
               AND w.identification IN ('identified', 'manual')
               AND NOT EXISTS (
                   SELECT 1 FROM work_locked_fields l
                    WHERE l.work_id = w.id AND l.field = 'overview')
             ORDER BY w.added_at",
        )
        .bind(language)
        .bind(provider)
        .bind(library_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        let mut waiting = Vec::new();
        for row in &rows {
            let wants_pictures: bool = int_to_bool(row.try_get("wants_pictures")?);
            let wants_a_synopsis: bool = int_to_bool(row.try_get("wants_a_synopsis")?);
            if !wants_pictures && !wants_a_synopsis {
                continue;
            }
            waiting.push(IncompleteNamedWork {
                id: row
                    .try_get::<String, _>("id")?
                    .parse()
                    .map_err(|_| DatabaseError::Corrupt("work identifier".to_string()))?,
                external_id: row.try_get("external_id")?,
                wants_pictures,
                wants_a_synopsis,
            });
        }
        Ok(waiting)
    }

    /// Gives a work the synopsis it was missing, and nothing else.
    ///
    /// Narrow on purpose: this runs on films a provider already named, so the
    /// title they carry is the right one and must not be written again from an
    /// answer given in another language.
    pub async fn set_work_synopsis(
        &self,
        work_id: WorkId,
        language: &str,
        tagline: Option<&str>,
        overview: &str,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO work_translations (work_id, language, tagline, overview)
             VALUES (?, ?, ?, ?)
             ON CONFLICT (work_id, language) DO UPDATE SET
                tagline = coalesce(work_translations.tagline, excluded.tagline),
                overview = excluded.overview",
        )
        .bind(work_id.to_db_string())
        .bind(language)
        .bind(tagline)
        .bind(overview)
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Records why the last look up did not name a work.
    ///
    /// Kept apart from the state on purpose: a work the provider could not be
    /// reached about is still waiting, and must carry its reason without being
    /// marked as one nobody recognised.
    pub async fn set_identification_note(
        &self,
        work_id: WorkId,
        note: IdentificationNote,
    ) -> Result<()> {
        sqlx::query("UPDATE works SET identification_note = ? WHERE id = ?")
            .bind(note.as_str())
            .bind(work_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Works nobody has named yet, with the file they were named after.
    ///
    /// The file name is all such a work has ever been given, so when the rules
    /// that read file names improve, this is the list that has to be read
    /// again. A work someone chose by hand, or a provider named, is left out:
    /// its title no longer comes from a file name.
    pub async fn works_named_after_their_file(
        &self,
        library_id: LibraryId,
    ) -> Result<Vec<WorkNamedAfterItsFile>> {
        let rows = sqlx::query(
            "SELECT w.id, w.title, w.release_year, min(s.relative_path) AS relative_path
             FROM works w
             JOIN media_sources s ON s.work_id = w.id
             WHERE w.library_id = ? AND w.identification IN ('pending', 'unidentified')
             GROUP BY w.id
             ORDER BY w.added_at",
        )
        .bind(library_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                Ok(WorkNamedAfterItsFile {
                    id: row
                        .try_get::<String, _>("id")?
                        .parse()
                        .map_err(|_| DatabaseError::Corrupt("work identifier".to_string()))?,
                    title: row.try_get("title")?,
                    release_year: row.try_get("release_year")?,
                    relative_path: PathBuf::from(row.try_get::<String, _>("relative_path")?),
                })
            })
            .collect()
    }

    /// Gives a work the title its file name now reads as.
    ///
    /// Only ever called for a work still named after its file: a title a
    /// provider gave, or a person chose, is never overwritten by a file name.
    pub async fn rename_work(
        &self,
        work_id: WorkId,
        title: &str,
        sort_title: &str,
        release_year: Option<i32>,
    ) -> Result<()> {
        sqlx::query(
            "UPDATE works SET title = ?, sort_title = ?, release_year = ?,
                -- The old reason spoke of the old title.
                identification_note = NULL,
                updated_at = ?
             WHERE id = ?",
        )
        .bind(title)
        .bind(sort_title)
        .bind(release_year)
        .bind(timestamp_to_text(now()))
        .bind(work_id.to_db_string())
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Records that a work was looked up and not recognised.
    ///
    /// It stays in the library with a marker rather than being set aside,
    /// because a file nobody can see is a file nobody remembers to fix.
    pub async fn mark_work_unidentified(&self, work_id: WorkId) -> Result<()> {
        sqlx::query("UPDATE works SET identification = ?, updated_at = ? WHERE id = ?")
            .bind(IdentificationState::Unidentified.as_str())
            .bind(timestamp_to_text(now()))
            .bind(work_id.to_db_string())
            .execute(self.writer())
            .await?;
        Ok(())
    }

    /// Writes down everything a provider said about a work.
    ///
    /// `chosen_by_hand` marks a match a person picked, which a later refresh
    /// must never undo.
    /// Answers with the people it credited and where their photos live, so the
    /// caller can fetch those without asking the provider a second time.
    pub async fn apply_identification(
        &self,
        work_id: WorkId,
        found: &IdentifiedWork,
        chosen_by_hand: bool,
    ) -> Result<Vec<CreditedPerson>> {
        let locked = self.locked_fields(work_id).await?;
        let mut transaction = self.begin().await?;
        let moment = timestamp_to_text(now());

        let state = match chosen_by_hand {
            true => IdentificationState::Manual,
            false => IdentificationState::Identified,
        };

        // A field edited by hand keeps what it was given, whatever the
        // provider now says.
        let keeps = |field: &str| locked.iter().any(|held| held == field);

        sqlx::query(
            "UPDATE works SET
                title = CASE WHEN ?1 THEN title ELSE ?2 END,
                sort_title = CASE WHEN ?1 THEN sort_title ELSE ?3 END,
                release_year = CASE WHEN ?4 THEN release_year ELSE ?5 END,
                runtime_ms = CASE WHEN ?6 THEN runtime_ms ELSE ?7 END,
                community_rating = ?8,
                age_rating_label = ?9,
                identification = ?10,
                -- Named at last, so whatever the last failure was stops being
                -- shown next to a film that now has a title.
                identification_note = NULL,
                updated_at = ?11
             WHERE id = ?12",
        )
        .bind(keeps("title"))
        .bind(&found.title)
        .bind(&found.sort_title)
        .bind(keeps("release_year"))
        .bind(found.release_year)
        .bind(keeps("runtime"))
        .bind(found.runtime.map(Millis::get))
        .bind(found.community_rating)
        .bind(found.age_rating_label.as_deref())
        .bind(state.as_str())
        .bind(&moment)
        .bind(work_id.to_db_string())
        .execute(&mut *transaction)
        .await?;

        set_external_id(
            &mut transaction,
            work_id,
            &found.provider,
            &found.external_id,
        )
        .await?;
        if let Some(imdb_id) = &found.imdb_id {
            set_external_id(&mut transaction, work_id, "imdb", imdb_id).await?;
        }

        if !keeps("overview") {
            sqlx::query(
                "INSERT INTO work_translations (work_id, language, title, tagline, overview)
                 VALUES (?, ?, ?, ?, ?)
                 ON CONFLICT (work_id, language) DO UPDATE SET
                    title = excluded.title,
                    tagline = excluded.tagline,
                    overview = excluded.overview",
            )
            .bind(work_id.to_db_string())
            .bind(&found.language)
            .bind(&found.title)
            .bind(found.tagline.as_deref())
            .bind(found.overview.as_deref())
            .execute(&mut *transaction)
            .await?;
        }

        replace_links(&mut transaction, work_id, &GENRES, &found.genres).await?;
        replace_links(&mut transaction, work_id, &STUDIOS, &found.studios).await?;

        let people =
            replace_credits(&mut transaction, work_id, &found.provider, &found.credits).await?;

        if let Some(collection) = &found.collection {
            attach_to_collection(&mut transaction, work_id, &found.provider, collection).await?;
        }
        replace_remote_trailers(&mut transaction, work_id, &found.trailers, &moment).await?;

        for field in [
            "title",
            "overview",
            "release_year",
            "runtime",
            "community_rating",
            "age_rating",
            "genres",
            "studios",
            "credits",
        ] {
            if keeps(field) {
                continue;
            }
            sqlx::query(
                "INSERT INTO work_field_provenance (work_id, field, provider, fetched_at)
                 VALUES (?, ?, ?, ?)
                 ON CONFLICT (work_id, field) DO UPDATE SET
                    provider = excluded.provider, fetched_at = excluded.fetched_at",
            )
            .bind(work_id.to_db_string())
            .bind(field)
            .bind(&found.provider)
            .bind(&moment)
            .execute(&mut *transaction)
            .await?;
        }

        transaction.commit().await?;
        Ok(people)
    }

    /// Fields someone edited by hand, which a refresh leaves alone.
    pub async fn locked_fields(&self, work_id: WorkId) -> Result<Vec<String>> {
        let rows = sqlx::query("SELECT field FROM work_locked_fields WHERE work_id = ?")
            .bind(work_id.to_db_string())
            .fetch_all(self.reader())
            .await?;
        rows.iter().map(|row| Ok(row.try_get("field")?)).collect()
    }

    /// Marks a field as edited by hand.
    pub async fn lock_field(&self, work_id: WorkId, field: &str) -> Result<()> {
        sqlx::query(
            "INSERT INTO work_locked_fields (work_id, field, locked_at) VALUES (?, ?, ?)
             ON CONFLICT (work_id, field) DO NOTHING",
        )
        .bind(work_id.to_db_string())
        .bind(field)
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Which provider supplied which field, and when.
    pub async fn field_provenance(&self, work_id: WorkId) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query(
            "SELECT field, provider FROM work_field_provenance WHERE work_id = ? ORDER BY field",
        )
        .bind(work_id.to_db_string())
        .fetch_all(self.reader())
        .await?;
        rows.iter()
            .map(|row| Ok((row.try_get("field")?, row.try_get("provider")?)))
            .collect()
    }

    /// Texts of a work in one language.
    pub async fn work_translation(
        &self,
        work_id: WorkId,
        language: &str,
    ) -> Result<Option<(Option<String>, Option<String>, Option<String>)>> {
        let row = sqlx::query(
            "SELECT title, tagline, overview FROM work_translations
             WHERE work_id = ? AND language = ?",
        )
        .bind(work_id.to_db_string())
        .bind(language)
        .fetch_optional(self.reader())
        .await?;

        row.map(|row| {
            Ok((
                row.try_get("title")?,
                row.try_get("tagline")?,
                row.try_get("overview")?,
            ))
        })
        .transpose()
    }

    /// Genres of a work, in order.
    pub async fn work_genres(&self, work_id: WorkId) -> Result<Vec<String>> {
        self.linked_names(work_id, &GENRES).await
    }

    /// Studios of a work, in order.
    pub async fn work_studios(&self, work_id: WorkId) -> Result<Vec<String>> {
        self.linked_names(work_id, &STUDIOS).await
    }

    async fn linked_names(&self, work_id: WorkId, link: &LinkStatements) -> Result<Vec<String>> {
        let rows = sqlx::query(link.list)
            .bind(work_id.to_db_string())
            .fetch_all(self.reader())
            .await?;
        rows.iter().map(|row| Ok(row.try_get("name")?)).collect()
    }

    /// Who is credited on a work, leads first.
    pub async fn work_credits(&self, work_id: WorkId) -> Result<Vec<WorkCredit>> {
        let rows = sqlx::query(
            "SELECT credits.person_id, people.name, credits.role, credits.character_name
             FROM credits JOIN people ON people.id = credits.person_id
             WHERE credits.work_id = ?
             ORDER BY CASE credits.role WHEN 'actor' THEN 0 ELSE 1 END, credits.ordinal,
                      people.sort_name",
        )
        .bind(work_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| {
                let person_id: String = row.try_get("person_id")?;
                Ok(WorkCredit {
                    person_id: person_id.parse().map_err(|_| {
                        crate::DatabaseError::Corrupt("person identifier".to_string())
                    })?,
                    name: row.try_get("name")?,
                    role: row.try_get("role")?,
                    character: row.try_get("character_name")?,
                })
            })
            .collect()
    }

    /// Trailers hosted elsewhere, which playing means leaving this server.
    pub async fn remote_extra_videos_of_work(
        &self,
        work_id: WorkId,
    ) -> Result<Vec<(String, String)>> {
        let rows = sqlx::query(
            "SELECT COALESCE(name, kind) AS name, remote_url FROM extra_videos
             WHERE work_id = ? AND remote_url IS NOT NULL ORDER BY created_at",
        )
        .bind(work_id.to_db_string())
        .fetch_all(self.reader())
        .await?;

        rows.iter()
            .map(|row| Ok((row.try_get("name")?, row.try_get("remote_url")?)))
            .collect()
    }

    /// The collection a work belongs to, if any.
    pub async fn work_collection(&self, work_id: WorkId) -> Result<Option<String>> {
        let row = sqlx::query(
            "SELECT collections.name FROM collections
             JOIN collection_items ON collection_items.collection_id = collections.id
             WHERE collection_items.work_id = ? LIMIT 1",
        )
        .bind(work_id.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        row.map(|row| Ok(row.try_get("name")?)).transpose()
    }
}

async fn set_external_id(
    transaction: &mut Transaction<'_, Sqlite>,
    work_id: WorkId,
    provider: &str,
    external_id: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO work_external_ids (work_id, provider, external_id) VALUES (?, ?, ?)
         ON CONFLICT (work_id, provider) DO UPDATE SET external_id = excluded.external_id",
    )
    .bind(work_id.to_db_string())
    .bind(provider)
    .bind(external_id)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// The statements behind one of the shared name tables.
///
/// Written out rather than built, so no statement here is ever assembled from
/// anything that came from outside.
struct LinkStatements {
    select: &'static str,
    insert: &'static str,
    clear: &'static str,
    link: &'static str,
    list: &'static str,
}

const GENRES: LinkStatements = LinkStatements {
    select: "SELECT id FROM genres WHERE name = ? COLLATE NOCASE",
    insert: "INSERT INTO genres (id, name) VALUES (?, ?)",
    clear: "DELETE FROM work_genres WHERE work_id = ?",
    link: "INSERT INTO work_genres (work_id, genre_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
    list: "SELECT genres.name FROM genres
           JOIN work_genres ON work_genres.genre_id = genres.id
           WHERE work_genres.work_id = ? ORDER BY genres.name",
};

const STUDIOS: LinkStatements = LinkStatements {
    select: "SELECT id FROM studios WHERE name = ? COLLATE NOCASE",
    insert: "INSERT INTO studios (id, name) VALUES (?, ?)",
    clear: "DELETE FROM work_studios WHERE work_id = ?",
    link: "INSERT INTO work_studios (work_id, studio_id) VALUES (?, ?) ON CONFLICT DO NOTHING",
    list: "SELECT studios.name FROM studios
           JOIN work_studios ON work_studios.studio_id = studios.id
           WHERE work_studios.work_id = ? ORDER BY studios.name",
};

/// Replaces the genres or the studios of a work.
///
/// The names themselves are shared, so a genre is created once and reused; the
/// links are rebuilt, which is how a film that lost a genre loses it here too.
async fn replace_links(
    transaction: &mut Transaction<'_, Sqlite>,
    work_id: WorkId,
    statements: &LinkStatements,
    names: &[String],
) -> Result<()> {
    sqlx::query(statements.clear)
        .bind(work_id.to_db_string())
        .execute(&mut **transaction)
        .await?;

    for name in names {
        let existing: Option<(String,)> = sqlx::query_as(statements.select)
            .bind(name)
            .fetch_optional(&mut **transaction)
            .await?;
        let id = match existing {
            Some((id,)) => id,
            None => {
                let id = NameId::new().to_db_string();
                sqlx::query(statements.insert)
                    .bind(&id)
                    .bind(name)
                    .execute(&mut **transaction)
                    .await?;
                id
            }
        };
        sqlx::query(statements.link)
            .bind(work_id.to_db_string())
            .bind(&id)
            .execute(&mut **transaction)
            .await?;
    }
    Ok(())
}

/// Replaces who is credited on a work.
///
/// People are shared and kept: an actor who left this film is still in others,
/// and their photo was fetched once.
async fn replace_credits(
    transaction: &mut Transaction<'_, Sqlite>,
    work_id: WorkId,
    provider: &str,
    credits: &[CreditRecord],
) -> Result<Vec<CreditedPerson>> {
    sqlx::query("DELETE FROM credits WHERE work_id = ?")
        .bind(work_id.to_db_string())
        .execute(&mut **transaction)
        .await?;

    let mut people = Vec::new();
    for credit in credits {
        let existing: Option<(String,)> = sqlx::query_as(
            "SELECT person_id FROM person_external_ids WHERE provider = ? AND external_id = ?",
        )
        .bind(provider)
        .bind(&credit.external_id)
        .fetch_optional(&mut **transaction)
        .await?;

        let person_id = match existing {
            Some((id,)) => id,
            None => {
                let id = PersonId::new().to_db_string();
                sqlx::query(
                    "INSERT INTO people (id, name, sort_name, created_at) VALUES (?, ?, ?, ?)",
                )
                .bind(&id)
                .bind(&credit.name)
                .bind(&credit.sort_name)
                .bind(timestamp_to_text(now()))
                .execute(&mut **transaction)
                .await?;
                sqlx::query(
                    "INSERT INTO person_external_ids (person_id, provider, external_id)
                     VALUES (?, ?, ?)",
                )
                .bind(&id)
                .bind(provider)
                .bind(&credit.external_id)
                .execute(&mut **transaction)
                .await?;
                id
            }
        };

        sqlx::query(
            "INSERT INTO credits (id, work_id, person_id, role, character_name, ordinal)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(CreditId::new().to_db_string())
        .bind(work_id.to_db_string())
        .bind(&person_id)
        .bind(&credit.role)
        .bind(credit.character.as_deref())
        .bind(credit.ordinal)
        .execute(&mut **transaction)
        .await?;

        people.push(CreditedPerson {
            person_id: person_id
                .parse()
                .map_err(|_| crate::DatabaseError::Corrupt("person identifier".to_string()))?,
            name: credit.name.clone(),
            role: credit.role.clone(),
            photo_path: credit.photo_path.clone(),
            ordinal: credit.ordinal,
        });
    }
    Ok(people)
}

/// Puts a work in the collection a provider says it belongs to.
///
/// Only collections the provider made are touched. One a person put together
/// is theirs, and a refresh has no business rearranging it.
async fn attach_to_collection(
    transaction: &mut Transaction<'_, Sqlite>,
    work_id: WorkId,
    provider: &str,
    collection: &CollectionRecord,
) -> Result<()> {
    let existing: Option<(String,)> = sqlx::query_as(
        "SELECT collection_id FROM collection_external_ids WHERE provider = ? AND external_id = ?",
    )
    .bind(provider)
    .bind(&collection.external_id)
    .fetch_optional(&mut **transaction)
    .await?;

    let collection_id = match existing {
        Some((id,)) => id,
        None => {
            let id = CollectionId::new().to_db_string();
            sqlx::query(
                "INSERT INTO collections (id, name, sort_name, origin, created_at)
                 VALUES (?, ?, ?, 'provider', ?)",
            )
            .bind(&id)
            .bind(&collection.name)
            .bind(&collection.sort_name)
            .bind(timestamp_to_text(now()))
            .execute(&mut **transaction)
            .await?;
            sqlx::query(
                "INSERT INTO collection_external_ids (collection_id, provider, external_id)
                 VALUES (?, ?, ?)",
            )
            .bind(&id)
            .bind(provider)
            .bind(&collection.external_id)
            .execute(&mut **transaction)
            .await?;
            id
        }
    };

    sqlx::query(
        "INSERT INTO collection_items (collection_id, work_id) VALUES (?, ?)
         ON CONFLICT DO NOTHING",
    )
    .bind(&collection_id)
    .bind(work_id.to_db_string())
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

/// Replaces the trailers hosted elsewhere.
///
/// Only those: a trailer sitting on the disk was found by the scan and has
/// nothing to do with what a provider knows.
async fn replace_remote_trailers(
    transaction: &mut Transaction<'_, Sqlite>,
    work_id: WorkId,
    trailers: &[RemoteTrailerRecord],
    moment: &str,
) -> Result<()> {
    sqlx::query("DELETE FROM extra_videos WHERE work_id = ? AND remote_url IS NOT NULL")
        .bind(work_id.to_db_string())
        .execute(&mut **transaction)
        .await?;

    for trailer in trailers {
        sqlx::query(
            "INSERT INTO extra_videos (id, work_id, kind, name, remote_url, created_at)
             VALUES (?, ?, 'trailer', ?, ?, ?)",
        )
        .bind(ExtraVideoId::new().to_db_string())
        .bind(work_id.to_db_string())
        .bind(&trailer.name)
        .bind(&trailer.url)
        .bind(moment)
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::work::WorkKind;
    use std::path::PathBuf;

    async fn work_in_library() -> (Database, melyxar_core::work::Work) {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let work = database
            .create_work(
                library.id,
                WorkKind::Movie,
                "Quiet Harbour 2019 MULTi",
                "quiet harbour 2019 multi",
                None,
            )
            .await
            .expect("work created");
        (database, work)
    }

    #[tokio::test]
    async fn changing_the_language_asks_about_every_film_again_but_never_a_manual_choice() {
        let (database, work) = work_in_library().await;
        let library_id = work.library_id;

        // One the provider named, and one a person picked by hand after the
        // provider got it wrong.
        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("the provider named it");
        let chosen = database
            .create_work(
                library_id,
                WorkKind::Movie,
                "Amber Field",
                "amber field",
                Some(2020),
            )
            .await
            .expect("work created");
        database
            .apply_identification(chosen.id, &found(), true)
            .await
            .expect("picked by hand");

        assert_eq!(
            database
                .works_awaiting_identification(library_id)
                .await
                .expect("read")
                .len(),
            0,
            "both are described, so nothing is waiting"
        );

        assert_eq!(
            database
                .ask_again_about_every_work(library_id)
                .await
                .expect("queued again"),
            1,
            "the one the provider named goes back in the queue, the other does not"
        );

        let waiting = database
            .works_awaiting_identification(library_id)
            .await
            .expect("read");
        assert_eq!(waiting.len(), 1);
        assert_eq!(
            waiting[0].id, work.id,
            "a correction somebody made by hand is never undone by a setting"
        );
        assert_eq!(
            database
                .work(chosen.id)
                .await
                .expect("read")
                .expect("present")
                .identification,
            IdentificationState::Manual
        );
    }

    fn found() -> IdentifiedWork {
        IdentifiedWork {
            provider: "tmdb".to_string(),
            external_id: "111".to_string(),
            imdb_id: Some("tt7654321".to_string()),
            language: "fr".to_string(),
            title: "Quiet Harbour".to_string(),
            sort_title: "quiet harbour".to_string(),
            tagline: Some("La mer ne rend rien.".to_string()),
            overview: Some("Un port, une nuit.".to_string()),
            release_year: Some(2019),
            runtime: Some(Millis::new(118 * 60_000)),
            community_rating: Some(7.4),
            age_rating_label: Some("12".to_string()),
            genres: vec!["Drame".to_string(), "Thriller".to_string()],
            studios: vec!["Invented Pictures".to_string()],
            credits: vec![
                CreditRecord {
                    external_id: "1".to_string(),
                    name: "Alix Moreau".to_string(),
                    sort_name: "alix moreau".to_string(),
                    role: "actor".to_string(),
                    character: Some("Camille".to_string()),
                    ordinal: 0,
                    photo_path: Some("/alix.jpg".to_string()),
                },
                CreditRecord {
                    external_id: "3".to_string(),
                    name: "Sacha Nord".to_string(),
                    sort_name: "sacha nord".to_string(),
                    role: "director".to_string(),
                    character: None,
                    ordinal: 0,
                    photo_path: None,
                },
            ],
            collection: Some(CollectionRecord {
                external_id: "77".to_string(),
                name: "Harbour Trilogy".to_string(),
                sort_name: "harbour trilogy".to_string(),
            }),
            trailers: vec![RemoteTrailerRecord {
                name: "Bande annonce".to_string(),
                url: "https://www.youtube.com/watch?v=abc".to_string(),
            }],
        }
    }

    #[tokio::test]
    async fn an_identified_work_carries_what_the_page_shows() {
        let (database, work) = work_in_library().await;
        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("identification applied");

        let stored = database
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.title, "Quiet Harbour");
        assert_eq!(stored.release_year, Some(2019));
        assert_eq!(stored.identification, IdentificationState::Identified);
        assert!(!stored.identification.may_be_looked_up_again());

        assert_eq!(
            database.work_genres(work.id).await.expect("read"),
            vec!["Drame", "Thriller"]
        );
        assert_eq!(
            database.work_studios(work.id).await.expect("read"),
            vec!["Invented Pictures"]
        );
        assert_eq!(
            database
                .work_collection(work.id)
                .await
                .expect("read")
                .as_deref(),
            Some("Harbour Trilogy")
        );
        assert_eq!(
            database.work_external_ids(work.id).await.expect("read"),
            vec![
                ("imdb".to_string(), "tt7654321".to_string()),
                ("tmdb".to_string(), "111".to_string()),
            ]
        );
    }

    #[tokio::test]
    async fn the_texts_are_stored_in_the_language_they_were_asked_for() {
        let (database, work) = work_in_library().await;
        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("identification applied");

        let (title, tagline, overview) = database
            .work_translation(work.id, "fr")
            .await
            .expect("read")
            .expect("present");
        assert_eq!(title.as_deref(), Some("Quiet Harbour"));
        assert_eq!(tagline.as_deref(), Some("La mer ne rend rien."));
        assert_eq!(overview.as_deref(), Some("Un port, une nuit."));

        assert!(
            database
                .work_translation(work.id, "en")
                .await
                .expect("read")
                .is_none(),
            "a language nobody fetched holds nothing"
        );
    }

    #[tokio::test]
    async fn the_cast_comes_back_with_the_leads_first() {
        let (database, work) = work_in_library().await;
        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("identification applied");

        let credits = database.work_credits(work.id).await.expect("read");
        assert_eq!(credits.len(), 2);
        assert_eq!(credits[0].name, "Alix Moreau");
        assert_eq!(credits[0].role, "actor");
        assert_eq!(credits[0].character.as_deref(), Some("Camille"));
        assert_eq!(credits[1].role, "director");
    }

    #[tokio::test]
    async fn an_identification_says_whose_face_is_worth_fetching() {
        let (database, work) = work_in_library().await;
        let people = database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("identification applied");

        assert_eq!(people.len(), 2);
        let actor = people
            .iter()
            .find(|person| person.role == "actor")
            .expect("an actor was credited");
        assert_eq!(actor.name, "Alix Moreau");
        assert_eq!(actor.photo_path.as_deref(), Some("/alix.jpg"));
        assert_eq!(
            actor.person_id,
            database.work_credits(work.id).await.expect("read")[0].person_id,
            "the people answered for are the people the page credits"
        );
    }

    #[tokio::test]
    async fn a_person_credited_on_two_films_is_the_same_person() {
        let (database, first) = work_in_library().await;
        let second = database
            .create_work(
                first.library_id,
                WorkKind::Movie,
                "Amber Field",
                "amber field",
                None,
            )
            .await
            .expect("work created");

        let here = database
            .apply_identification(first.id, &found(), false)
            .await
            .expect("applied");
        let there = database
            .apply_identification(
                second.id,
                &IdentifiedWork {
                    external_id: "222".to_string(),
                    title: "Amber Field".to_string(),
                    sort_title: "amber field".to_string(),
                    ..found()
                },
                false,
            )
            .await
            .expect("applied");

        assert_eq!(
            here[0].person_id, there[0].person_id,
            "an actor is shared, and their photo is fetched once rather than per film"
        );
    }

    #[tokio::test]
    async fn looking_a_work_up_twice_replaces_what_it_holds_rather_than_doubling_it() {
        let (database, work) = work_in_library().await;
        for _ in 0..3 {
            database
                .apply_identification(work.id, &found(), false)
                .await
                .expect("identification applied");
        }

        assert_eq!(database.work_genres(work.id).await.expect("read").len(), 2);
        assert_eq!(database.work_credits(work.id).await.expect("read").len(), 2);
        assert_eq!(
            database
                .extra_videos_of_work(work.id)
                .await
                .expect("read")
                .len(),
            0,
            "a trailer hosted elsewhere is not a file on the disk"
        );
    }

    #[tokio::test]
    async fn a_genre_is_shared_rather_than_created_again_for_every_film() {
        let (database, first) = work_in_library().await;
        let second = database
            .create_work(
                first.library_id,
                WorkKind::Movie,
                "Amber Field",
                "amber field",
                None,
            )
            .await
            .expect("work created");

        database
            .apply_identification(first.id, &found(), false)
            .await
            .expect("applied");
        database
            .apply_identification(
                second.id,
                &IdentifiedWork {
                    external_id: "222".to_string(),
                    title: "Amber Field".to_string(),
                    sort_title: "amber field".to_string(),
                    ..found()
                },
                false,
            )
            .await
            .expect("applied");

        let count: (i64,) = sqlx::query_as("SELECT count(*) FROM genres")
            .fetch_one(database.reader())
            .await
            .expect("read");
        assert_eq!(count.0, 2, "two films sharing two genres make two genres");
    }

    #[tokio::test]
    async fn a_field_edited_by_hand_survives_a_later_lookup() {
        let (database, work) = work_in_library().await;
        database
            .lock_field(work.id, "title")
            .await
            .expect("field locked");

        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("identification applied");

        let stored = database
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            stored.title, "Quiet Harbour 2019 MULTi",
            "a refresh that undoes what someone typed is a refresh nobody dares run"
        );
        assert_eq!(
            stored.release_year,
            Some(2019),
            "the fields nobody touched still take what the provider says"
        );
        assert!(!database
            .field_provenance(work.id)
            .await
            .expect("read")
            .iter()
            .any(|(field, _)| field == "title"));
    }

    #[tokio::test]
    async fn the_trailers_that_leave_this_server_are_told_apart_from_the_rest() {
        let (database, work) = work_in_library().await;
        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("applied");

        let remote = database
            .remote_extra_videos_of_work(work.id)
            .await
            .expect("read");
        assert_eq!(remote.len(), 1);
        assert_eq!(remote[0].0, "Bande annonce");
        assert!(remote[0].1.starts_with("https://"));

        assert_eq!(
            database
                .work(work.id)
                .await
                .expect("read")
                .expect("present")
                .age_rating_label,
            Some("12".to_string())
        );
    }

    #[tokio::test]
    async fn a_match_picked_by_hand_says_so() {
        let (database, work) = work_in_library().await;
        database
            .apply_identification(work.id, &found(), true)
            .await
            .expect("identification applied");

        let stored = database
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.identification, IdentificationState::Manual);
        assert!(
            !stored.identification.may_be_looked_up_again(),
            "a choice someone made is never undone by a background refresh"
        );
    }

    #[tokio::test]
    async fn a_work_nobody_recognised_stays_in_the_library_with_a_marker() {
        let (database, work) = work_in_library().await;
        database
            .mark_work_unidentified(work.id)
            .await
            .expect("marked");

        let stored = database
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.identification, IdentificationState::Unidentified);
        assert!(
            stored.identification.may_be_looked_up_again(),
            "a film nobody recognised today may be recognised tomorrow"
        );
        assert_eq!(
            database
                .works_awaiting_identification(work.library_id)
                .await
                .expect("read")
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn the_reason_a_look_up_failed_is_read_back_beside_the_work() {
        let (database, work) = work_in_library().await;
        assert_eq!(
            database
                .work(work.id)
                .await
                .expect("read")
                .expect("present")
                .identification_note,
            None,
            "a film nobody has looked up owes no explanation"
        );

        database
            .set_identification_note(work.id, IdentificationNote::NoMatch)
            .await
            .expect("note written");

        let stored = database
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(
            stored.identification_note,
            Some(IdentificationNote::NoMatch)
        );
        assert_eq!(
            stored.identification,
            IdentificationState::Pending,
            "saying why nothing happened is not deciding that nothing will"
        );
    }

    #[tokio::test]
    async fn a_film_that_finally_got_a_name_stops_carrying_why_it_had_none() {
        let (database, work) = work_in_library().await;
        database
            .set_identification_note(work.id, IdentificationNote::ProviderUnreachable)
            .await
            .expect("note written");

        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("identification applied");

        assert_eq!(
            database
                .work(work.id)
                .await
                .expect("read")
                .expect("present")
                .identification_note,
            None,
            "a reason that outlives its cause is a lie on a screen"
        );
    }

    #[tokio::test]
    async fn a_film_with_a_name_and_no_poster_is_counted_as_missing_one() {
        let (database, work) = work_in_library().await;
        // Everything a provider gave, and no picture: a picture is not stored
        // by an identification, it arrives afterwards.
        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("identification applied");

        let incomplete = database.works_missing_something().await.expect("read");
        assert_eq!(incomplete.len(), 1);
        assert_eq!(incomplete[0].title, "Quiet Harbour");
        assert_eq!(incomplete[0].release_year, Some(2019));
        assert!(incomplete[0].missing.contains(&"poster"), "{incomplete:?}");
        assert!(
            incomplete[0].missing.contains(&"backdrop"),
            "{incomplete:?}"
        );
        assert!(
            !incomplete[0].missing.contains(&"overview"),
            "the provider gave one: {incomplete:?}"
        );
        assert!(
            !incomplete[0].missing.contains(&"cast"),
            "the provider gave one: {incomplete:?}"
        );

        database
            .replace_images(
                "work",
                &work.id.to_db_string(),
                "poster",
                &[crate::images::StoredImage {
                    owner_kind: "work".to_string(),
                    owner_id: work.id.to_db_string(),
                    image_kind: "poster".to_string(),
                    relative_path: "works/x/poster-200.webp".to_string(),
                    width: Some(200),
                    height: Some(300),
                    fingerprint: "abc".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("picture stored");

        let after = database.works_missing_something().await.expect("read");
        assert!(
            !after[0].missing.contains(&"poster"),
            "a film that got its picture stops being counted for it: {after:?}"
        );
    }

    #[tokio::test]
    async fn a_film_whose_picture_never_arrived_is_offered_for_another_try() {
        let (database, work) = work_in_library().await;
        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("identification applied");

        let waiting = database
            .works_missing_their_metadata(work.library_id, "tmdb", "fr")
            .await
            .expect("read");
        assert_eq!(waiting.len(), 1);
        assert_eq!(waiting[0].id, work.id);
        assert_eq!(
            waiting[0].external_id, "111",
            "the identifier is what lets the provider be asked again"
        );
        assert!(waiting[0].wants_pictures);
        assert!(
            !waiting[0].wants_a_synopsis,
            "the provider gave one: {waiting:?}"
        );

        database
            .replace_images(
                "work",
                &work.id.to_db_string(),
                "poster",
                &[crate::images::StoredImage {
                    owner_kind: "work".to_string(),
                    owner_id: work.id.to_db_string(),
                    image_kind: "poster".to_string(),
                    relative_path: "works/x/poster-200.webp".to_string(),
                    width: Some(200),
                    height: Some(300),
                    fingerprint: "abc".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("picture stored");

        assert!(
            database
                .works_missing_their_metadata(work.library_id, "tmdb", "fr")
                .await
                .expect("read")
                .is_empty(),
            "a film with nothing missing is never asked about again"
        );
    }

    #[tokio::test]
    async fn a_film_with_no_synopsis_is_offered_one_and_keeps_its_title() {
        let (database, work) = work_in_library().await;
        let mut wordless = found();
        wordless.overview = None;
        database
            .apply_identification(work.id, &wordless, false)
            .await
            .expect("identification applied");

        let waiting = database
            .works_missing_their_metadata(work.library_id, "tmdb", "fr")
            .await
            .expect("read");
        assert_eq!(waiting.len(), 1);
        assert!(waiting[0].wants_a_synopsis);

        database
            .set_work_synopsis(work.id, "fr", None, "Un port, une nuit.")
            .await
            .expect("synopsis written");

        let (title, _, overview) = database
            .work_translation(work.id, "fr")
            .await
            .expect("read")
            .expect("present");
        assert_eq!(overview.as_deref(), Some("Un port, une nuit."));
        assert_eq!(
            title.as_deref(),
            Some("Quiet Harbour"),
            "the title a provider already gave is never written again from another answer"
        );
        assert!(
            database
                .works_missing_their_metadata(work.library_id, "tmdb", "fr")
                .await
                .expect("read")
                .iter()
                .all(|waiting| !waiting.wants_a_synopsis),
            "a film that has its synopsis stops being asked about for it"
        );
    }

    #[tokio::test]
    async fn a_synopsis_somebody_wrote_themselves_is_never_replaced() {
        let (database, work) = work_in_library().await;
        let mut wordless = found();
        wordless.overview = None;
        database
            .apply_identification(work.id, &wordless, false)
            .await
            .expect("identification applied");
        database
            .lock_field(work.id, "overview")
            .await
            .expect("field locked");

        assert!(
            database
                .works_missing_their_metadata(work.library_id, "tmdb", "fr")
                .await
                .expect("read")
                .is_empty(),
            "a field somebody edited is theirs, empty or not"
        );
    }

    #[tokio::test]
    async fn a_film_nobody_could_name_is_never_asked_about_for_its_pictures() {
        // There is nothing to ask with: no provider ever named it.
        let (database, work) = work_in_library().await;
        assert!(database
            .works_missing_their_metadata(work.library_id, "tmdb", "fr")
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn a_film_nobody_could_name_is_left_out_of_what_is_missing() {
        // It is already named in the section above, with the reason it has no
        // title at all. Counting its holes a second time says nothing new.
        let (database, _) = work_in_library().await;
        assert!(database
            .works_missing_something()
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn two_people_who_happen_to_share_a_name_are_two_people() {
        // Namesakes are ordinary, and a provider lists them as the different
        // people they are. Refusing the second one used to stop the whole run,
        // which meant one pair of namesakes left a library nameless.
        let (database, work) = work_in_library().await;
        let mut crowded = found();
        crowded.credits.push(CreditRecord {
            external_id: "9".to_string(),
            name: "Alix Moreau".to_string(),
            sort_name: "alix moreau".to_string(),
            role: "actor".to_string(),
            character: Some("Le voisin".to_string()),
            ordinal: 1,
            photo_path: None,
        });

        let people = database
            .apply_identification(work.id, &crowded, false)
            .await
            .expect("a namesake is not a reason to give up on a film");

        let namesakes: Vec<_> = people
            .iter()
            .filter(|person| person.name == "Alix Moreau")
            .collect();
        assert_eq!(namesakes.len(), 2);
        assert_ne!(
            namesakes[0].person_id, namesakes[1].person_id,
            "the same name is not the same person, and their filmographies must not merge"
        );
    }

    #[tokio::test]
    async fn a_work_still_named_after_its_file_is_listed_with_that_file() {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let work = database
            .create_work(
                library.id,
                WorkKind::Movie,
                "Quiet Harbour 2160p",
                "quiet harbour 2160p",
                None,
            )
            .await
            .expect("work created");
        database
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new("Quiet Harbour 2160p.mkv"),
                1_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");

        let listed = database
            .works_named_after_their_file(library.id)
            .await
            .expect("read");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, work.id);
        assert_eq!(listed[0].title, "Quiet Harbour 2160p");
        assert_eq!(
            listed[0].relative_path,
            PathBuf::from("Quiet Harbour 2160p.mkv")
        );

        database
            .rename_work(work.id, "Quiet Harbour", "quiet harbour", Some(2019))
            .await
            .expect("renamed");
        let stored = database
            .work(work.id)
            .await
            .expect("read")
            .expect("present");
        assert_eq!(stored.title, "Quiet Harbour");
        assert_eq!(stored.sort_title, "quiet harbour");
        assert_eq!(stored.release_year, Some(2019));

        // A film a provider named is no longer described by its file name, so
        // it must never be renamed from one again.
        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("identification applied");
        assert!(database
            .works_named_after_their_file(library.id)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn a_film_nobody_could_name_is_listed_with_the_name_on_disk_behind_it() {
        // A title read off a file name is only ever as good as that name, so
        // the report has to carry both: the title alone leaves whoever reads
        // it with a question and a terminal to go and answer it in.
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let work = database
            .create_work(
                library.id,
                WorkKind::Movie,
                "Quiet Harbour BD Rip",
                "quiet harbour bd rip",
                None,
            )
            .await
            .expect("work created");
        database
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new("Anciens/Quiet Harbour BD Rip.avi"),
                1_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");
        database
            .set_identification_note(work.id, IdentificationNote::NoMatch)
            .await
            .expect("note written");

        let listed = database.works_still_nameless(25).await.expect("read");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].work.id, work.id);
        assert_eq!(
            listed[0].work.identification_note,
            Some(IdentificationNote::NoMatch)
        );
        assert_eq!(
            listed[0].file_name.as_deref(),
            Some("Quiet Harbour BD Rip.avi"),
            "the name of the file, and never the folders leading to it"
        );

        // A film a provider named is no longer waiting for anything.
        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("identification applied");
        assert!(database
            .works_still_nameless(25)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn the_files_nothing_could_describe_are_the_ones_with_no_analysis() {
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let work = database
            .create_work(
                library.id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");

        let described = database
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new("Quiet Harbour 1080p.mkv"),
                1_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");
        database
            .insert_source(
                work.id,
                library.roots[0].id,
                std::path::Path::new("Anciens/Quiet Harbour BD Rip.avi"),
                2_000,
                melyxar_core::time::now(),
            )
            .await
            .expect("source recorded");

        database
            .store_analysis(
                described,
                &crate::catalogue::SourceAnalysis {
                    container: Some("matroska".to_string()),
                    duration: Some(melyxar_core::time::Millis::new(7_200_000)),
                    overall_bitrate: Some(8_000_000),
                },
                &[],
                &[],
            )
            .await
            .expect("analysis stored");

        let listed = database
            .files_nothing_could_describe(15)
            .await
            .expect("read");
        assert_eq!(
            listed,
            vec![UndescribedFile {
                file_name: "Quiet Harbour BD Rip.avi".to_string(),
                reason: None,
            }],
            "the name of the file, and never the folders leading to it"
        );

        // What the analyser said is the one thing anybody can act on.
        let refused = database
            .sources_of_work(work.id)
            .await
            .expect("read")
            .into_iter()
            .find(|source| source.relative_path.ends_with("Quiet Harbour BD Rip.avi"))
            .expect("the copy nothing could describe");
        database
            .record_analysis_failure(refused.id, "Picture size 0x0 is invalid")
            .await
            .expect("written down");
        assert_eq!(
            database
                .files_nothing_could_describe(15)
                .await
                .expect("read")[0]
                .reason
                .as_deref(),
            Some("Picture size 0x0 is invalid")
        );

        // And it goes the moment the file is described, rather than outliving
        // what caused it and sending the next reader the wrong way.
        database
            .store_analysis(
                refused.id,
                &crate::catalogue::SourceAnalysis {
                    container: Some("avi".to_string()),
                    duration: Some(melyxar_core::time::Millis::new(5_400_000)),
                    overall_bitrate: Some(2_000_000),
                },
                &[],
                &[],
            )
            .await
            .expect("analysis stored");
        assert!(database
            .files_nothing_could_describe(15)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn a_film_held_in_several_copies_is_named_with_each_of_them() {
        // What a grouping leaves behind, and what makes it checkable: the
        // names side by side say at a glance whether the two really are one
        // film.
        let database = Database::open_in_memory().await.expect("database opens");
        let library = database
            .create_library(
                "Films",
                LibraryKind::Movies,
                "fr",
                &[("disk-one".to_string(), PathBuf::from("/mnt/one/Films"))],
            )
            .await
            .expect("library created");
        let held_twice = database
            .create_work(
                library.id,
                WorkKind::Movie,
                "Quiet Harbour",
                "quiet harbour",
                Some(2019),
            )
            .await
            .expect("work created");
        let held_once = database
            .create_work(
                library.id,
                WorkKind::Movie,
                "Amber Field",
                "amber field",
                Some(2020),
            )
            .await
            .expect("work created");

        for (work_id, path) in [
            (held_twice.id, "Quiet Harbour 1080p.mkv"),
            (held_twice.id, "Anciens/zz12Quiet Harbour 1080p.mkv"),
            (held_once.id, "Amber Field 1080p.mkv"),
        ] {
            database
                .insert_source(
                    work_id,
                    library.roots[0].id,
                    std::path::Path::new(path),
                    1_000,
                    melyxar_core::time::now(),
                )
                .await
                .expect("source recorded");
        }

        let listed = database.works_held_in_several_copies().await.expect("read");
        assert_eq!(listed.len(), 1, "a film held once is held once");
        assert_eq!(listed[0].title, "Quiet Harbour");
        assert_eq!(listed[0].release_year, Some(2019));
        assert_eq!(
            listed[0].file_names,
            vec![
                "Quiet Harbour 1080p.mkv".to_string(),
                "zz12Quiet Harbour 1080p.mkv".to_string(),
            ],
            "the name of each copy, and never the folders leading to it"
        );
    }

    #[tokio::test]
    async fn a_film_waiting_with_no_file_recorded_is_still_listed() {
        // Nothing a scan produces, and a list that dropped it would hide the
        // very film whose state is hardest to explain.
        let (database, work) = work_in_library().await;
        let listed = database.works_still_nameless(25).await.expect("read");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].work.id, work.id);
        assert_eq!(listed[0].file_name, None);
    }

    #[tokio::test]
    async fn a_work_with_no_file_at_all_is_not_listed_as_named_after_one() {
        let (database, work) = work_in_library().await;
        assert!(
            database
                .works_named_after_their_file(work.library_id)
                .await
                .expect("read")
                .is_empty(),
            "there is no file name to read again"
        );
    }

    #[tokio::test]
    async fn renaming_a_work_drops_the_reason_that_spoke_of_its_old_name() {
        let (database, work) = work_in_library().await;
        database
            .set_identification_note(work.id, IdentificationNote::NoMatch)
            .await
            .expect("note written");
        database
            .rename_work(work.id, "Quiet Harbour", "quiet harbour", Some(2019))
            .await
            .expect("renamed");

        assert_eq!(
            database
                .work(work.id)
                .await
                .expect("read")
                .expect("present")
                .identification_note,
            None
        );
    }

    #[tokio::test]
    async fn works_already_identified_are_not_asked_about_again() {
        let (database, work) = work_in_library().await;
        assert_eq!(
            database
                .works_awaiting_identification(work.library_id)
                .await
                .expect("read")
                .len(),
            1
        );

        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("applied");
        assert!(database
            .works_awaiting_identification(work.library_id)
            .await
            .expect("read")
            .is_empty());
    }

    #[tokio::test]
    async fn the_provider_of_every_field_it_supplied_is_written_down() {
        let (database, work) = work_in_library().await;
        database
            .apply_identification(work.id, &found(), false)
            .await
            .expect("applied");

        let provenance = database.field_provenance(work.id).await.expect("read");
        assert!(provenance.iter().all(|(_, provider)| provider == "tmdb"));
        assert!(provenance.iter().any(|(field, _)| field == "overview"));
    }
}
