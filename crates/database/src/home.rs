//! The rows a home page is made of that belong to nobody in particular.
//!
//! What somebody left halfway and what their series are waiting on live next
//! to the progress they are read from. What is here is the other half: what
//! the server was told to put in front of everybody, and what it picks when
//! nobody told it anything.

use melyxar_core::id::{LibraryId, PersonId, UserId, WorkId};
use melyxar_core::saga::Role;
use melyxar_core::time::now;
use sqlx::{AssertSqlSafe, Row};

use std::collections::HashMap;

use crate::browse::{kept_inside, met_on_its_own, WorkCard, WHAT_A_CARD_IS};
use crate::convert::{parse_id, timestamp_to_text};
use crate::images::StoredImage;
use crate::{Database, Result};

/// How many genres of somebody's own count as what they watch.
///
/// Five rather than all of them: a person who has watched forty films has
/// touched nearly every genre there is, and weighing them all equally is the
/// same as weighing none of them.
const GENRES_THAT_COUNT: i64 = 5;

/// What a work needs to be shown large rather than as a card.
///
/// A card is a poster, a title and a year, which is everything a grid needs
/// and nothing the one place that shows a work full width does. This is the
/// rest: the wide picture behind it, the title as the film's own designers
/// drew it, and enough words to say what it is.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Dressed {
    pub backdrop: Vec<StoredImage>,
    pub logo: Vec<StoredImage>,
    pub tagline: Option<String>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    /// How wide and how tall the picture of the best copy is, which is what
    /// a badge saying 4K is really saying. Both, because a film wider than a
    /// screen is stored with its black bands cut off and its height alone
    /// undersells it.
    pub width: Option<i64>,
    pub height: Option<i64>,
    /// hdr10, hlg or dolby_vision, when the picture carries one.
    pub hdr: Option<String>,
    /// What the fullest soundtrack is, as the file states it: the profile
    /// when there is one, since that is where Atmos is written, and the
    /// codec otherwise.
    pub sound: Option<String>,
}

impl Database {
    /// Dresses a handful of works for the one place that shows them large.
    ///
    /// Two questions for the lot rather than a detail page each: a detail page
    /// reads versions, tracks, credits and collections, which is a great deal
    /// of work to print a sentence and hang one picture.
    ///
    /// The words are taken in the language asked for and fall back to whatever
    /// the work has, which is the same rule the detail page follows: a page
    /// half empty is worse than a page with a paragraph somebody can read.
    pub async fn dressed_large(
        &self,
        works: &[WorkId],
        language: &str,
    ) -> Result<HashMap<WorkId, Dressed>> {
        let mut dressed: HashMap<WorkId, Dressed> = HashMap::new();
        if works.is_empty() {
            return Ok(dressed);
        }
        let owners: Vec<String> = works.iter().map(|id| id.to_db_string()).collect();
        // A row of question marks, one per identifier the caller just read
        // back out of this crate's own tables.
        let places = vec!["?"; owners.len()].join(", ");

        let mut pictures = sqlx::query(AssertSqlSafe(format!(
            "SELECT {} FROM images
              WHERE owner_kind = 'work'
                AND image_kind IN ('backdrop', 'logo')
                AND owner_id IN ({places})
              ORDER BY width DESC",
            crate::images::WHAT_A_PICTURE_IS
        )));
        for owner in &owners {
            pictures = pictures.bind(owner);
        }
        for row in pictures.fetch_all(self.reader()).await? {
            let owner: WorkId = parse_id(&row.try_get::<String, _>("owner_id")?)?;
            let kind: String = row.try_get("image_kind")?;
            let image = crate::images::image_from_row(&row)?;
            let entry = dressed.entry(owner).or_default();
            match kind.as_str() {
                "logo" => entry.logo.push(image),
                _ => entry.backdrop.push(image),
            }
        }

        // The language asked for first, then anything the work has: ordered
        // here so the first row read for a work is the one that wins.
        let mut words = sqlx::query(AssertSqlSafe(format!(
            "SELECT work_id, tagline, overview
               FROM work_translations
              WHERE work_id IN ({places})
              ORDER BY (language = ?) DESC"
        )));
        for owner in &owners {
            words = words.bind(owner);
        }
        for row in words.bind(language).fetch_all(self.reader()).await? {
            let owner: WorkId = parse_id(&row.try_get::<String, _>("work_id")?)?;
            let entry = dressed.entry(owner).or_default();
            if entry.overview.is_none() {
                entry.overview = row.try_get("overview")?;
            }
            if entry.tagline.is_none() {
                entry.tagline = row.try_get("tagline")?;
            }
        }

        let mut genres = sqlx::query(AssertSqlSafe(format!(
            "SELECT wg.work_id, g.name
               FROM work_genres wg
               JOIN genres g ON g.id = wg.genre_id
              WHERE wg.work_id IN ({places})"
        )));
        for owner in &owners {
            genres = genres.bind(owner);
        }
        for row in genres.fetch_all(self.reader()).await? {
            let owner: WorkId = parse_id(&row.try_get::<String, _>("work_id")?)?;
            dressed
                .entry(owner)
                .or_default()
                .genres
                .push(row.try_get("name")?);
        }

        // What the file itself says about its picture and its sound, which is
        // what the badges beside a title are: read off the best copy on disk
        // rather than promised by the catalogue.
        let mut facts = sqlx::query(AssertSqlSafe(format!(
            "SELECT w.id AS work_id,
                    (SELECT t.width FROM tracks t
                       JOIN media_sources s ON s.id = t.source_id
                      WHERE s.work_id = w.id AND s.missing_since IS NULL
                        AND t.kind = 'video' AND t.height IS NOT NULL
                      ORDER BY t.width * t.height DESC, t.id LIMIT 1) AS width,
                    (SELECT t.height FROM tracks t
                       JOIN media_sources s ON s.id = t.source_id
                      WHERE s.work_id = w.id AND s.missing_since IS NULL
                        AND t.kind = 'video' AND t.height IS NOT NULL
                      ORDER BY t.width * t.height DESC, t.id LIMIT 1) AS height,
                    (SELECT t.hdr_format FROM tracks t
                       JOIN media_sources s ON s.id = t.source_id
                      WHERE s.work_id = w.id AND s.missing_since IS NULL
                        AND t.kind = 'video' AND t.hdr_format IS NOT NULL
                      LIMIT 1) AS hdr,
                    (SELECT coalesce(t.profile, t.codec) FROM tracks t
                       JOIN media_sources s ON s.id = t.source_id
                      WHERE s.work_id = w.id AND s.missing_since IS NULL
                        AND t.kind = 'audio'
                      ORDER BY t.channels DESC LIMIT 1) AS sound
               FROM works w
              WHERE w.id IN ({places})"
        )));
        for owner in &owners {
            facts = facts.bind(owner);
        }
        for row in facts.fetch_all(self.reader()).await? {
            let owner: WorkId = parse_id(&row.try_get::<String, _>("work_id")?)?;
            let entry = dressed.entry(owner).or_default();
            entry.width = row.try_get("width")?;
            entry.height = row.try_get("height")?;
            entry.hdr = row.try_get("hdr")?;
            entry.sound = row.try_get("sound")?;
        }
        Ok(dressed)
    }

    /// What an administrator put in front of everybody, in the order they
    /// chose.
    pub async fn pinned_works(
        &self,
        viewer: UserId,
        within: Option<&[LibraryId]>,
        limit: i64,
    ) -> Result<Vec<WorkCard>> {
        let Some(inside) = kept_inside(within, "w.library_id") else {
            return Ok(Vec::new());
        };
        // No question here of what is met on its own. That rule keeps seasons
        // and episodes out of the grids nobody asked to see them in; this row
        // is the one place where somebody did ask, by hand, for this exact
        // work. Asking it again dropped a pinned episode on the floor without
        // a word, which from the outside is a menu entry that does nothing.
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_CARD_IS}
               FROM pinned_works p
               JOIN works w ON w.id = p.work_id
              WHERE 1 = 1{inside}
              ORDER BY p.rank, p.pinned_at
              LIMIT ?"
        )));
        for granted in within.iter().copied().flatten() {
            query = query.bind(granted.to_db_string());
        }
        let rows = query.bind(limit).fetch_all(self.reader()).await?;

        let mut cards = rows
            .iter()
            .map(crate::browse::card_from_row)
            .collect::<Result<Vec<_>>>()?;
        self.attach_posters(&mut cards).await?;
        self.attach_viewer_state(viewer, &mut cards).await?;
        Ok(cards)
    }

    /// Puts a work in front of everybody, at the end of what is already there.
    ///
    /// Pinning what is already pinned leaves it where it is rather than moving
    /// it to the end: an administrator who presses twice meant to pin it once,
    /// and the order of this row is arranged by hand.
    pub async fn pin_work(&self, work_id: WorkId) -> Result<()> {
        sqlx::query(
            "INSERT INTO pinned_works (work_id, rank, pinned_at)
             VALUES (?, coalesce((SELECT max(rank) FROM pinned_works), 0) + 1, ?)
             ON CONFLICT (work_id) DO NOTHING",
        )
        .bind(work_id.to_db_string())
        .bind(timestamp_to_text(now()))
        .execute(self.writer())
        .await?;
        Ok(())
    }

    /// Takes a work back off the front page. Answers whether one was there.
    pub async fn unpin_work(&self, work_id: WorkId) -> Result<bool> {
        let gone = sqlx::query("DELETE FROM pinned_works WHERE work_id = ?")
            .bind(work_id.to_db_string())
            .execute(self.writer())
            .await?
            .rows_affected();
        Ok(gone > 0)
    }

    /// Whether this work is one of the ones put in front of everybody.
    pub async fn is_pinned(&self, work_id: WorkId) -> Result<bool> {
        let row = sqlx::query("SELECT 1 FROM pinned_works WHERE work_id = ?")
            .bind(work_id.to_db_string())
            .fetch_optional(self.reader())
            .await?;
        Ok(row.is_some())
    }

    /// What this account might want to watch, said as honestly as the server
    /// can say it today.
    ///
    /// Works nobody here has started, well thought of elsewhere, kept to the
    /// genres this account watches most. There is no recommendation engine
    /// behind this and the name promises none: what has been watched is the
    /// only matter there is, and its genres are enough not to offer a horror
    /// film to somebody who only watches comedies.
    ///
    /// An account that has watched nothing yet gets the best rated things it
    /// has not started, which is the only honest answer to a question nobody
    /// has given any material for.
    ///
    /// Ordered by rating alone rather than by rating inside each genre, which
    /// is what lets it walk the index of well rated works and stop at the
    /// dozen it was asked for instead of reading and sorting every work that
    /// carries a rating.
    pub async fn suggestions(
        &self,
        viewer: UserId,
        within: Option<&[LibraryId]>,
        limit: i64,
    ) -> Result<Vec<WorkCard>> {
        // An account granted nothing has nothing to be suggested.
        if within.is_some_and(<[LibraryId]>::is_empty) {
            return Ok(Vec::new());
        }
        // Numbered throughout, and the libraries numbered on from four: the
        // viewer is named three times over, and one plain question mark among
        // them would shift every place after it.
        let granted = within.unwrap_or_default();
        let inside = match granted.is_empty() {
            true => String::new(),
            false => format!(
                " AND w.library_id IN ({})",
                (4..=granted.len() + 3)
                    .map(|place| format!("?{place}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };

        let mut query = sqlx::query(AssertSqlSafe(format!(
            "WITH watched_genres AS (
                 SELECT wg.genre_id
                   FROM playback_progress p
                   JOIN works e ON e.id = p.work_id
                   JOIN work_genres wg ON wg.work_id = e.id
                  WHERE p.user_id = ?1 AND p.state = 'watched'
                  GROUP BY wg.genre_id
                  ORDER BY count(*) DESC
                  LIMIT ?2
             )
             SELECT {WHAT_A_CARD_IS}
               FROM works w
               LEFT JOIN playback_progress p ON p.work_id = w.id AND p.user_id = ?1
              WHERE {}
                AND w.community_rating IS NOT NULL
                AND coalesce(p.state, 'not_started') = 'not_started'
                -- A series is not started by playing the series: it is started
                -- by playing an episode of it, so the row above says nothing
                -- about one and its episodes have to be asked.
                AND (w.kind <> 'series'
                     OR NOT EXISTS (
                         SELECT 1 FROM playback_progress q
                           JOIN works e ON e.id = q.work_id AND e.kind = 'episode'
                          WHERE q.user_id = ?1
                            AND q.state <> 'not_started'
                            AND (e.parent_id = w.id
                                 OR e.parent_id IN (SELECT id FROM works
                                                     WHERE parent_id = w.id))))
                -- Nothing watched yet leaves every genre open, which is the
                -- only honest answer before there is anything to go on.
                AND (NOT EXISTS (SELECT 1 FROM watched_genres)
                     OR EXISTS (SELECT 1 FROM work_genres wg
                                  JOIN watched_genres ON watched_genres.genre_id = wg.genre_id
                                 WHERE wg.work_id = w.id)){inside}
              ORDER BY w.community_rating DESC
              LIMIT ?3",
            met_on_its_own("w.")
        )))
        .bind(viewer.to_db_string())
        .bind(GENRES_THAT_COUNT)
        .bind(limit);
        for library in granted {
            query = query.bind(library.to_db_string());
        }
        let rows = query.fetch_all(self.reader()).await?;

        let mut cards = rows
            .iter()
            .map(crate::browse::card_from_row)
            .collect::<Result<Vec<_>>>()?;
        self.attach_posters(&mut cards).await?;
        self.attach_viewer_state(viewer, &mut cards).await?;
        Ok(cards)
    }

    /// A handful of works drawn at random, for a banner that is meant to be
    /// different every time the page is opened.
    ///
    /// Only what carries a wide picture: a banner is that picture, and one
    /// without it is a slab of colour with a title on it. That is also what
    /// keeps the draw from wasting its five places on things nobody put a
    /// picture to.
    ///
    /// Works met on their own, like everywhere else a card is offered: a
    /// season and an episode belong inside the thing they hang under.
    pub async fn works_at_random(
        &self,
        viewer: UserId,
        within: Option<&[LibraryId]>,
        limit: i64,
    ) -> Result<Vec<WorkCard>> {
        // An account granted nothing has nothing to draw from.
        if within.is_some_and(<[LibraryId]>::is_empty) {
            return Ok(Vec::new());
        }
        let granted = within.unwrap_or_default();
        let inside = match granted.is_empty() {
            true => String::new(),
            false => format!(
                " AND w.library_id IN ({})",
                (2..=granted.len() + 1)
                    .map(|place| format!("?{place}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };

        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_CARD_IS}
               FROM works w
              WHERE {}
                AND EXISTS (SELECT 1 FROM images i
                             WHERE i.owner_kind = 'work'
                               AND i.owner_id = w.id
                               AND i.image_kind = 'backdrop'){inside}
              ORDER BY random()
              LIMIT ?1",
            met_on_its_own("w.")
        )))
        .bind(limit);
        for library in granted {
            query = query.bind(library.to_db_string());
        }
        let rows = query.fetch_all(self.reader()).await?;

        let mut cards = rows
            .iter()
            .map(crate::browse::card_from_row)
            .collect::<Result<Vec<_>>>()?;
        self.attach_posters(&mut cards).await?;
        self.attach_viewer_state(viewer, &mut cards).await?;
        Ok(cards)
    }
}

/// How many other works a genre needs before a row of it is worth drawing: a
/// row this long fills the width of most screens.
const A_ROW_OF_ALIKE: i64 = 6;

/// The saga a work belongs to, and every work of it an account can reach.
#[derive(Debug, Clone, PartialEq)]
pub struct Saga {
    pub name: String,
    pub cards: Vec<WorkCard>,
}

/// A row of works like one other, and the genre they were found by.
#[derive(Debug, Clone, PartialEq)]
pub struct Alike {
    pub genre: String,
    pub cards: Vec<WorkCard>,
}

impl Database {
    /// Works of the same kind as this one that carry one of its genres, for
    /// the row its page ends with.
    ///
    /// The genre is drawn at random among those of its genres that fill a
    /// row, so two visits to one page can offer two rows; failing any, the
    /// one with the most works behind it. Inside the row, whatever shares the
    /// most genres with this work comes first, then the best rated: a film
    /// that is action and adventure is closer to another that is both than to
    /// one that is only action.
    ///
    /// Nothing at all when no genre of it is carried by anything else.
    pub async fn alike_by_genre(
        &self,
        viewer: UserId,
        work_id: WorkId,
        within: Option<&[LibraryId]>,
        limit: i64,
    ) -> Result<Option<Alike>> {
        let (Some(beside), Some(inside)) = (
            kept_inside(within, "o.library_id"),
            kept_inside(within, "w.library_id"),
        ) else {
            return Ok(None);
        };
        let granted = within.unwrap_or_default();

        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT id, name FROM (
                 SELECT g.id, g.name,
                        (SELECT count(*) FROM work_genres t
                           JOIN works o ON o.id = t.work_id
                          WHERE t.genre_id = g.id AND o.id <> me.id
                            AND o.kind = me.kind{beside}) AS others
                   FROM work_genres mine
                   JOIN genres g ON g.id = mine.genre_id
                   JOIN works me ON me.id = mine.work_id
                  WHERE mine.work_id = ?)
              WHERE others > 0
              ORDER BY others >= ? DESC,
                       CASE WHEN others >= ? THEN random() ELSE -others END
              LIMIT 1"
        )));
        for library in granted {
            query = query.bind(library.to_db_string());
        }
        let Some(chosen) = query
            .bind(work_id.to_db_string())
            .bind(A_ROW_OF_ALIKE)
            .bind(A_ROW_OF_ALIKE)
            .fetch_optional(self.reader())
            .await?
        else {
            return Ok(None);
        };
        let genre_id: String = chosen.try_get("id")?;
        let genre: String = chosen.try_get("name")?;

        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_CARD_IS}
               FROM works w
               JOIN work_genres wg ON wg.work_id = w.id AND wg.genre_id = ?
              WHERE w.id <> ?
                AND w.kind = (SELECT kind FROM works WHERE id = ?){inside}
              ORDER BY (SELECT count(*) FROM work_genres a
                          JOIN work_genres b ON b.genre_id = a.genre_id
                         WHERE a.work_id = w.id AND b.work_id = ?) DESC,
                       w.community_rating IS NULL, w.community_rating DESC, w.sort_title
              LIMIT ?"
        )))
        .bind(&genre_id)
        .bind(work_id.to_db_string())
        .bind(work_id.to_db_string());
        for library in granted {
            query = query.bind(library.to_db_string());
        }
        let rows = query
            .bind(work_id.to_db_string())
            .bind(limit)
            .fetch_all(self.reader())
            .await?;

        let mut cards = rows
            .iter()
            .map(crate::browse::card_from_row)
            .collect::<Result<Vec<_>>>()?;
        self.attach_posters(&mut cards).await?;
        self.attach_viewer_state(viewer, &mut cards).await?;
        Ok(Some(Alike { genre, cards }))
    }

    /// The saga a work belongs to, with every work of it this account can
    /// reach, itself included, in the order they came out.
    ///
    /// Nothing when it belongs to none.
    pub async fn saga_of(
        &self,
        viewer: UserId,
        work_id: WorkId,
        within: Option<&[LibraryId]>,
    ) -> Result<Option<Saga>> {
        let Some(inside) = kept_inside(within, "w.library_id") else {
            return Ok(None);
        };
        let saga: Option<(String, String)> = sqlx::query_as(
            "SELECT c.id, c.name FROM collections c
               JOIN collection_items ci ON ci.collection_id = c.id
              WHERE ci.work_id = ?
              ORDER BY c.origin = 'provider' DESC, c.sort_name
              LIMIT 1",
        )
        .bind(work_id.to_db_string())
        .fetch_optional(self.reader())
        .await?;
        let Some((saga_id, name)) = saga else {
            return Ok(None);
        };

        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_CARD_IS}
               FROM works w
               JOIN collection_items ci ON ci.work_id = w.id AND ci.collection_id = ?{inside}
              ORDER BY w.release_year IS NULL, w.release_year, w.sort_title"
        )))
        .bind(&saga_id);
        for library in within.unwrap_or_default() {
            query = query.bind(library.to_db_string());
        }
        let rows = query.fetch_all(self.reader()).await?;

        let mut cards = rows
            .iter()
            .map(crate::browse::card_from_row)
            .collect::<Result<Vec<_>>>()?;
        self.attach_posters(&mut cards).await?;
        self.attach_viewer_state(viewer, &mut cards).await?;
        Ok(Some(Saga { name, cards }))
    }

    /// The parts played in these works with a character to them, billed
    /// higher than `billed_under`.
    pub async fn roles_in(&self, works: &[WorkId], billed_under: i32) -> Result<Vec<Role>> {
        if works.is_empty() {
            return Ok(Vec::new());
        }
        let places = vec!["?"; works.len()].join(", ");
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT work_id, person_id, character_name, ordinal FROM credits
              WHERE work_id IN ({places}) AND role = 'actor'
                AND ordinal < ? AND character_name IS NOT NULL"
        )));
        for work in works {
            query = query.bind(work.to_db_string());
        }
        let rows = query.bind(billed_under).fetch_all(self.reader()).await?;
        rows.iter().map(role_from_row).collect()
    }

    /// The parts these people play in the films this account can reach, apart
    /// from the ones left out, in the order the films came out.
    pub async fn roles_elsewhere(
        &self,
        people: &[PersonId],
        leaving_out: &[WorkId],
        within: Option<&[LibraryId]>,
    ) -> Result<Vec<Role>> {
        let Some(inside) = kept_inside(within, "w.library_id") else {
            return Ok(Vec::new());
        };
        if people.is_empty() {
            return Ok(Vec::new());
        }
        let who = vec!["?"; people.len()].join(", ");
        let left = vec!["?"; leaving_out.len()].join(", ");
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT c.work_id, c.person_id, c.character_name, c.ordinal
               FROM credits c
               JOIN works w ON w.id = c.work_id
              WHERE c.person_id IN ({who}) AND c.role = 'actor'
                AND c.character_name IS NOT NULL
                AND w.kind = 'movie' AND w.id NOT IN ({left}){inside}
              ORDER BY w.release_year IS NULL, w.release_year, w.sort_title, c.ordinal"
        )));
        for person in people {
            query = query.bind(person.to_db_string());
        }
        for work in leaving_out {
            query = query.bind(work.to_db_string());
        }
        for library in within.unwrap_or_default() {
            query = query.bind(library.to_db_string());
        }
        let rows = query.fetch_all(self.reader()).await?;
        rows.iter().map(role_from_row).collect()
    }

    /// The cards of these works, in the order they are given.
    pub async fn cards_in_order(&self, viewer: UserId, works: &[WorkId]) -> Result<Vec<WorkCard>> {
        if works.is_empty() {
            return Ok(Vec::new());
        }
        let places = vec!["?"; works.len()].join(", ");
        let mut query = sqlx::query(AssertSqlSafe(format!(
            "SELECT {WHAT_A_CARD_IS} FROM works w WHERE w.id IN ({places})"
        )));
        for work in works {
            query = query.bind(work.to_db_string());
        }
        let mut cards = query
            .fetch_all(self.reader())
            .await?
            .iter()
            .map(crate::browse::card_from_row)
            .collect::<Result<Vec<_>>>()?;
        cards.sort_by_key(|card| works.iter().position(|work| *work == card.id));
        self.attach_posters(&mut cards).await?;
        self.attach_viewer_state(viewer, &mut cards).await?;
        Ok(cards)
    }
}

/// One part read back from the credits.
fn role_from_row(row: &sqlx::sqlite::SqliteRow) -> Result<Role> {
    Ok(Role {
        work_id: parse_id(&row.try_get::<String, _>("work_id")?)?,
        person_id: parse_id(&row.try_get::<String, _>("person_id")?)?,
        character: row.try_get("character_name")?,
        billed: row.try_get("ordinal")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use melyxar_core::library::LibraryKind;
    use melyxar_core::time::Millis;
    use melyxar_core::user::Permissions;
    use melyxar_core::work::{PlaybackState, WorkKind};
    use std::path::PathBuf;

    /// A library of films, each with a rating and a genre, and two accounts.
    async fn a_shelf(films: &[(&str, f64, &str)]) -> (Database, LibraryId, UserId, Vec<WorkId>) {
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

        let mut written = Vec::new();
        for (title, rating, genre) in films {
            let work = database
                .create_work(
                    library.id,
                    WorkKind::Movie,
                    title,
                    &title.to_lowercase(),
                    Some(2019),
                )
                .await
                .expect("work created");
            sqlx::query(
                "UPDATE works SET community_rating = ?, identification = 'identified'
                 WHERE id = ?",
            )
            .bind(rating)
            .bind(work.id.to_db_string())
            .execute(database.writer())
            .await
            .expect("film completed");

            let genre_id = format!("genre-{genre}");
            sqlx::query("INSERT INTO genres (id, name) VALUES (?, ?) ON CONFLICT DO NOTHING")
                .bind(&genre_id)
                .bind(*genre)
                .execute(database.writer())
                .await
                .expect("genre written");
            sqlx::query("INSERT INTO work_genres (work_id, genre_id) VALUES (?, ?)")
                .bind(work.id.to_db_string())
                .bind(&genre_id)
                .execute(database.writer())
                .await
                .expect("genre attached");
            written.push(work.id);
        }

        let who = database
            .create_user("vera", None, &Permissions::viewer())
            .await
            .expect("account created")
            .id;
        (database, library.id, who, written)
    }

    /// Gives a work the wide picture a banner is made of.
    async fn with_a_backdrop(database: &Database, work: WorkId) {
        database
            .replace_images(
                "work",
                &work.to_db_string(),
                "backdrop",
                &[StoredImage {
                    owner_kind: "work".to_string(),
                    owner_id: work.to_db_string(),
                    image_kind: "backdrop".to_string(),
                    relative_path: format!("works/{}/backdrop-abc-1280.webp", work.to_db_string()),
                    width: Some(1280),
                    height: Some(720),
                    fingerprint: "abc".to_string(),
                    dominant_color: None,
                }],
            )
            .await
            .expect("picture written");
    }

    #[tokio::test]
    async fn a_banner_drawn_at_random_only_offers_what_has_a_wide_picture() {
        let (database, _, who, films) = a_shelf(&[
            ("Sel", 7.0, "drame"),
            ("Fougère", 7.5, "drame"),
            ("Vertige", 8.0, "drame"),
        ])
        .await;
        with_a_backdrop(&database, films[0]).await;
        with_a_backdrop(&database, films[2]).await;

        let drawn = database
            .works_at_random(who, None, 5)
            .await
            .expect("a draw comes back");

        // Two of the three carry one, and the third is never offered: a
        // banner without its picture is a slab of colour with a title on it.
        assert_eq!(drawn.len(), 2);
        let mut offered: Vec<WorkId> = drawn.iter().map(|card| card.id).collect();
        offered.sort();
        let mut expected = vec![films[0], films[2]];
        expected.sort();
        assert_eq!(offered, expected);
    }

    #[tokio::test]
    async fn a_banner_drawn_at_random_never_reaches_outside_what_was_granted() {
        let (database, library, who, films) =
            a_shelf(&[("Sel", 7.0, "drame"), ("Fougère", 7.5, "drame")]).await;
        for film in &films {
            with_a_backdrop(&database, *film).await;
        }

        assert_eq!(
            database
                .works_at_random(who, Some(&[library]), 5)
                .await
                .expect("a draw comes back")
                .len(),
            2,
            "the library it was granted is drawn from"
        );
        assert!(
            database
                .works_at_random(who, Some(&[]), 5)
                .await
                .expect("a draw comes back")
                .is_empty(),
            "an account granted nothing has nothing to draw from"
        );
    }

    #[tokio::test]
    async fn a_draw_never_brings_back_more_than_it_was_asked_for() {
        let (database, _, who, films) = a_shelf(&[
            ("Sel", 7.0, "drame"),
            ("Fougère", 7.5, "drame"),
            ("Vertige", 8.0, "drame"),
        ])
        .await;
        for film in &films {
            with_a_backdrop(&database, *film).await;
        }

        assert_eq!(
            database
                .works_at_random(who, None, 2)
                .await
                .expect("a draw comes back")
                .len(),
            2
        );
    }

    #[tokio::test]
    async fn what_is_pinned_comes_back_in_the_order_it_was_pinned_in() {
        let (database, _, who, films) =
            a_shelf(&[("One", 7.0, "Drame"), ("Two", 8.0, "Drame")]).await;

        database.pin_work(films[1]).await.expect("pinned");
        database.pin_work(films[0]).await.expect("pinned");
        // Pressing twice on the same one leaves it where it is.
        database.pin_work(films[1]).await.expect("pinned again");

        let front = database
            .pinned_works(who, None, 10)
            .await
            .expect("pinned read");
        assert_eq!(
            front.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![films[1], films[0]]
        );

        assert!(database.is_pinned(films[0]).await.expect("read"));
        assert!(database.unpin_work(films[0]).await.expect("unpinned"));
        assert!(!database.is_pinned(films[0]).await.expect("read"));
        assert!(
            !database.unpin_work(films[0]).await.expect("unpinned"),
            "taking off what is not there says so"
        );

        assert!(
            database
                .pinned_works(who, Some(&[]), 10)
                .await
                .expect("pinned read")
                .is_empty(),
            "an account granted nothing sees nothing, front page included"
        );
    }

    #[tokio::test]
    async fn an_episode_put_in_front_of_everybody_comes_back_like_anything_else() {
        let (database, library_id, who, _) = a_shelf(&[("One", 7.0, "Drame")]).await;

        let mut hung = Vec::new();
        for (kind, title) in [
            (WorkKind::Series, "Distant Signal"),
            (WorkKind::Season, "Distant Signal, first year"),
            (WorkKind::Episode, "Distant Signal, first night"),
        ] {
            hung.push(
                database
                    .create_work(library_id, kind, title, &title.to_lowercase(), Some(2019))
                    .await
                    .expect("work created")
                    .id,
            );
        }
        for (child, parent) in [(1, 0), (2, 1)] {
            sqlx::query("UPDATE works SET parent_id = ? WHERE id = ?")
                .bind(hung[parent].to_db_string())
                .bind(hung[child].to_db_string())
                .execute(database.writer())
                .await
                .expect("hung under its parent");
        }

        database.pin_work(hung[2]).await.expect("pinned");

        assert_eq!(
            database
                .pinned_works(who, None, 10)
                .await
                .expect("pinned read")
                .iter()
                .map(|card| card.id)
                .collect::<Vec<_>>(),
            vec![hung[2]],
            "an episode nobody would meet on its own was asked for by hand"
        );
    }

    /// Gives a work one more genre.
    async fn also_of(database: &Database, work: WorkId, genre: &str) {
        let genre_id = format!("genre-{genre}");
        sqlx::query("INSERT INTO genres (id, name) VALUES (?, ?) ON CONFLICT DO NOTHING")
            .bind(&genre_id)
            .bind(genre)
            .execute(database.writer())
            .await
            .expect("genre written");
        sqlx::query("INSERT INTO work_genres (work_id, genre_id) VALUES (?, ?)")
            .bind(work.to_db_string())
            .bind(&genre_id)
            .execute(database.writer())
            .await
            .expect("genre attached");
    }

    #[tokio::test]
    async fn alike_works_share_a_genre_the_closest_first_and_never_another_kind() {
        let (database, library, who, films) = a_shelf(&[
            ("Iron Tide", 7.0, "Action"),
            ("Copper Wind", 6.0, "Action"),
            ("Silver Gale", 8.0, "Action"),
            ("Quiet Pond", 9.0, "Drame"),
        ])
        .await;
        also_of(&database, films[0], "Aventure").await;
        also_of(&database, films[1], "Aventure").await;
        let series = database
            .create_work(library, WorkKind::Series, "Storm Line", "storm line", Some(2020))
            .await
            .expect("series created");
        also_of(&database, series.id, "Action").await;

        let alike = database
            .alike_by_genre(who, films[0], None, 10)
            .await
            .expect("read")
            .expect("a genre is shared");
        assert_eq!(alike.genre, "Action", "the genre with the most behind it");
        assert_eq!(
            alike.cards.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![films[1], films[2]],
            "the one sharing both genres first, never itself, the drama or the series"
        );

        assert_eq!(
            database
                .alike_by_genre(who, films[0], Some(&[]), 10)
                .await
                .expect("read"),
            None,
            "an account granted nothing is offered nothing"
        );
        assert_eq!(
            database
                .alike_by_genre(who, films[3], None, 10)
                .await
                .expect("read"),
            None,
            "a genre nothing else carries makes no row"
        );
    }

    /// Puts works in a saga, as the provider would have.
    async fn in_a_saga(database: &Database, name: &str, works: &[WorkId]) {
        let saga = melyxar_core::id::CollectionId::new().to_db_string();
        sqlx::query(
            "INSERT INTO collections (id, name, sort_name, origin, created_at)
             VALUES (?, ?, ?, 'provider', '2026-09-26T00:00:00Z')",
        )
        .bind(&saga)
        .bind(name)
        .bind(name.to_lowercase())
        .execute(database.writer())
        .await
        .expect("saga written");
        for work in works {
            sqlx::query("INSERT INTO collection_items (collection_id, work_id) VALUES (?, ?)")
                .bind(&saga)
                .bind(work.to_db_string())
                .execute(database.writer())
                .await
                .expect("work put in the saga");
        }
    }

    #[tokio::test]
    async fn a_saga_holds_every_work_of_it_here_in_the_order_they_came_out() {
        let (database, _, who, films) = a_shelf(&[
            ("Iron Tide", 7.0, "Action"),
            ("Copper Wind", 6.0, "Action"),
            ("Silver Gale", 8.0, "Action"),
            ("Quiet Pond", 9.0, "Drame"),
        ])
        .await;
        for (film, year) in [(films[0], 2012), (films[1], 2008), (films[2], 2019)] {
            sqlx::query("UPDATE works SET release_year = ? WHERE id = ?")
                .bind(year)
                .bind(film.to_db_string())
                .execute(database.writer())
                .await
                .expect("year set");
        }
        in_a_saga(&database, "Tide Saga", &[films[0], films[1], films[2]]).await;
        in_a_saga(&database, "Pond Saga", &[films[3]]).await;

        let saga = database
            .saga_of(who, films[0], None)
            .await
            .expect("read")
            .expect("a saga");
        assert_eq!(saga.name, "Tide Saga");
        assert_eq!(
            saga.cards.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![films[1], films[0], films[2]],
            "every one of it, itself included, the first to come out first"
        );

        assert_eq!(
            database
                .saga_of(who, films[3], None)
                .await
                .expect("read")
                .expect("a saga")
                .cards
                .len(),
            1,
            "a saga of one here is still its saga: the films sharing its characters may follow"
        );
        assert_eq!(
            database
                .saga_of(who, WorkId::new(), None)
                .await
                .expect("read"),
            None,
            "a work in no saga has none"
        );
        assert_eq!(
            database.saga_of(who, films[0], Some(&[])).await.expect("read"),
            None,
            "an account granted nothing is offered nothing"
        );
    }

    /// Credits somebody with a part in a work.
    async fn playing(
        database: &Database,
        person: PersonId,
        work: WorkId,
        character: &str,
        billed: i32,
    ) {
        sqlx::query(
            "INSERT INTO people (id, name, sort_name, created_at)
             VALUES (?1, ?1, ?1, '2026-09-26T00:00:00Z') ON CONFLICT DO NOTHING",
        )
        .bind(person.to_db_string())
        .execute(database.writer())
        .await
        .expect("person written");
        sqlx::query(
            "INSERT INTO credits (id, work_id, person_id, role, character_name, ordinal)
             VALUES (?, ?, ?, 'actor', ?, ?)",
        )
        .bind(melyxar_core::id::CreditId::new().to_db_string())
        .bind(work.to_db_string())
        .bind(person.to_db_string())
        .bind(character)
        .bind(billed)
        .execute(database.writer())
        .await
        .expect("credit written");
    }

    #[tokio::test]
    async fn the_parts_a_saga_s_leads_play_elsewhere_are_read_back_in_the_order_films_came_out() {
        let (database, library, who, films) = a_shelf(&[
            ("Storm Rising", 7.0, "Action"),
            ("Storm Falling", 7.0, "Action"),
            ("Heroes Gather", 8.0, "Action"),
            ("Heroes Part", 8.0, "Action"),
        ])
        .await;
        for (film, year) in [(films[2], 2019), (films[3], 2012)] {
            sqlx::query("UPDATE works SET release_year = ? WHERE id = ?")
                .bind(year)
                .bind(film.to_db_string())
                .execute(database.writer())
                .await
                .expect("year set");
        }
        let [hero, extra] = std::array::from_fn(|_| PersonId::new());
        playing(&database, hero, films[0], "Kael Vorn", 0).await;
        playing(&database, extra, films[0], "Harbour Guard", 9).await;
        playing(&database, hero, films[1], "Kael Vorn", 0).await;
        playing(&database, hero, films[2], "Kael", 2).await;
        playing(&database, hero, films[3], "Kael Vorn", 1).await;

        let saga = [films[0], films[1]];
        let leads = database.roles_in(&saga, 5).await.expect("read");
        assert_eq!(leads.len(), 2, "the guard is billed too low to lead anything");
        assert!(leads.iter().all(|role| role.person_id == hero && role.billed == 0));

        let elsewhere = database
            .roles_elsewhere(&[hero], &saga, None)
            .await
            .expect("read");
        assert_eq!(
            elsewhere
                .iter()
                .map(|role| (role.work_id, role.character.as_str(), role.billed))
                .collect::<Vec<_>>(),
            vec![(films[3], "Kael Vorn", 1), (films[2], "Kael", 2)],
            "outside the saga, the earliest film first"
        );
        assert!(
            database
                .roles_elsewhere(&[hero], &saga, Some(&[]))
                .await
                .expect("read")
                .is_empty(),
            "an account granted nothing is offered nothing"
        );
        assert_eq!(
            database
                .roles_elsewhere(&[hero], &saga, Some(&[library]))
                .await
                .expect("read")
                .len(),
            2
        );

        let cards = database
            .cards_in_order(who, &[films[2], films[0], films[3]])
            .await
            .expect("read");
        assert_eq!(
            cards.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![films[2], films[0], films[3]],
            "in the order they were asked for"
        );
    }

    #[tokio::test]
    async fn suggestions_lean_on_the_genres_this_account_watches() {
        let (database, _, who, films) = a_shelf(&[
            ("Watched Comedy", 6.0, "Comedie"),
            ("Great Horror", 9.5, "Horreur"),
            ("Good Comedy", 8.0, "Comedie"),
        ])
        .await;

        // Nothing watched yet: the best rated thing comes first, whatever it
        // is, because there is nothing to go on.
        let blind = database
            .suggestions(who, None, 10)
            .await
            .expect("suggestions read");
        assert_eq!(blind[0].id, films[1], "the best rated, with nothing to go on");

        database
            .mark_watched(who, films[0], true)
            .await
            .expect("marked");

        let leaning = database
            .suggestions(who, None, 10)
            .await
            .expect("suggestions read");
        assert_eq!(
            leaning.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![films[2]],
            "a comedy watcher is offered the comedy, and never what they watched"
        );
    }

    #[tokio::test]
    async fn a_work_shown_large_carries_its_genres_and_what_its_file_holds() {
        use melyxar_core::id::{MediaSourceId, TrackId};
        use melyxar_core::media::{
            AudioDetails, ColorInfo, HdrFormat, Loudness, Track, TrackKind, VideoDetails,
        };
        use melyxar_core::time::now;
        use std::path::Path;

        let (database, library, _, films) = a_shelf(&[("One", 7.0, "Drame")]).await;
        let root = database
            .library_roots(library)
            .await
            .expect("roots read")[0]
            .id;

        let video = |source_id: MediaSourceId, height: i32, hdr: Option<HdrFormat>| Track {
            id: TrackId::new(),
            source_id,
            stream_index: 0,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Video(VideoDetails {
                codec: "hevc".to_string(),
                profile: Some("Main 10".to_string()),
                level: None,
                width: height * 16 / 9,
                height,
                margins: None,
                aspect_ratio: None,
                is_interlaced: false,
                frame_rate: None,
                bitrate: None,
                pixel_format: None,
                reference_frames: None,
                color: ColorInfo::default(),
                hdr,
            }),
        };
        let audio = |source_id: MediaSourceId, channels: i32, profile: Option<&str>| Track {
            id: TrackId::new(),
            source_id,
            stream_index: 1,
            language: None,
            title: None,
            is_default: true,
            is_forced: false,
            kind: TrackKind::Audio(AudioDetails {
                codec: "eac3".to_string(),
                profile: profile.map(str::to_string),
                channels,
                channel_layout: None,
                sample_rate: None,
                bit_depth: None,
                bitrate: None,
                loudness: Loudness::default(),
            }),
        };

        // Two copies of the same film, one of them gone from the disk. What
        // the badges say has to come from the copy that can really be played.
        let good = database
            .insert_source(films[0], root, Path::new("one-4k.mkv"), 1_000, now())
            .await
            .expect("copy recorded");
        database
            .store_analysis(
                good,
                &Default::default(),
                &[
                    video(good, 2160, Some(HdrFormat::Hdr10)),
                    audio(good, 8, Some("Dolby Atmos")),
                    audio(good, 2, None),
                ],
                &[],
            )
            .await
            .expect("copy analysed");

        let gone = database
            .insert_source(films[0], root, Path::new("one-8k.mkv"), 1_000, now())
            .await
            .expect("copy recorded");
        database
            .store_analysis(gone, &Default::default(), &[video(gone, 4320, None)], &[])
            .await
            .expect("copy analysed");
        sqlx::query("UPDATE media_sources SET missing_since = ? WHERE id = ?")
            .bind(timestamp_to_text(now()))
            .bind(gone.to_db_string())
            .execute(database.writer())
            .await
            .expect("copy lost");

        let dressed = database
            .dressed_large(&films, "fr")
            .await
            .expect("dressed read");
        let one = dressed.get(&films[0]).expect("the film is dressed");
        assert_eq!(one.genres, vec!["Drame".to_string()]);
        assert_eq!(
            (one.width, one.height),
            (Some(3840), Some(2160)),
            "the copy still on disk decides, both sides of the same picture"
        );
        assert_eq!(one.hdr.as_deref(), Some("hdr10"));
        assert_eq!(
            one.sound.as_deref(),
            Some("Dolby Atmos"),
            "the fullest soundtrack, named as the file names it"
        );
    }

    #[tokio::test]
    async fn a_work_already_started_is_never_suggested() {
        let (database, _, who, films) =
            a_shelf(&[("One", 9.0, "Drame"), ("Two", 8.0, "Drame")]).await;

        database
            .record_playback_progress(
                who,
                films[0],
                Millis::new(600_000),
                PlaybackState::InProgress,
                melyxar_core::time::now(),
            )
            .await
            .expect("position recorded");

        let offered = database
            .suggestions(who, None, 10)
            .await
            .expect("suggestions read");
        assert_eq!(
            offered.iter().map(|card| card.id).collect::<Vec<_>>(),
            vec![films[1]],
            "what somebody is in the middle of belongs to carrying on"
        );
    }
}
